use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::RiscvVm;
use std::fs;
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    // Use 128MB to reduce page table setup time
    // Try uart8250 earlycon (direct MMIO, no SBI dependency) and max log level
    let bootargs = "console=ttyS0 earlycon=uart8250,mmio32,0x10000000 panic=5 nosmp maxcpus=0 loglevel=8";
    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 128, bootargs).unwrap();

    // Verify DTB is readable at pre-set address
    {
        let dtb_pa: u64 = _dtb_addr;
        let magic = vm.bus.read_word(dtb_pa).unwrap_or(0);
        eprintln!("[diag] DTB at PA 0x{:08X}: magic=0x{:08X} (expect 0xD00DFEED)", dtb_pa, magic);
        // Check first few DTB fields
        let totalsize = vm.bus.read_word(dtb_pa + 4).unwrap_or(0);
        eprintln!("[diag] DTB totalsize={} bytes", totalsize);
        // Scan for bootargs in DTB
        let mut bootargs_found = String::new();
        for off in 0..totalsize.min(8192) as u64 {
            if let Ok(b) = vm.bus.read_byte(dtb_pa + off) {
                if b == 0 {
                    if !bootargs_found.is_empty() && bootargs_found.contains("console") {
                        eprintln!("[diag] Found bootargs: {}", bootargs_found);
                        break;
                    }
                    bootargs_found.clear();
                } else if b.is_ascii_graphic() || b == b' ' {
                    bootargs_found.push(b as char);
                    if bootargs_found.len() > 200 {
                        bootargs_found.clear();
                    }
                } else {
                    bootargs_found.clear();
                }
            }
        }
        // Also check _dtb_early_va and _dtb_early_pa
        let dtb_early_va = vm.bus.read_word(0x00801008).unwrap_or(0);
        let dtb_early_pa = vm.bus.read_word(0x0080100C).unwrap_or(0);
        eprintln!("[diag] _dtb_early_va=0x{:08X} _dtb_early_pa=0x{:08X}", dtb_early_va, dtb_early_pa);
    }

    let fw_addr_u32 = fw_addr as u32;
    let max_count: u64 = 200_000_000; // 200M instructions
    let mut count: u64 = 0;
    let mut sbi_count: u64 = 0;
    let mut ecall_m_count: u64 = 0;
    let mut smode_trap_count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut satp_changes: u32 = 0;
    let mut last_medeleg: u32 = vm.cpu.csr.medeleg;
    let mut start = Instant::now();
    let mut next_report: u64 = 1_000_000;
    let mut last_uart_len: usize = 0;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Handle M-mode traps at fw_addr
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let is_interrupt = (mcause >> 31) & 1 == 1;
            if !is_interrupt {
                match cause_code {
                    9 => {
                        // ECALL_S = SBI call
                        sbi_count += 1;
                        let result = vm.bus.sbi.handle_ecall(
                            vm.cpu.x[17],
                            vm.cpu.x[16],
                            vm.cpu.x[10],
                            vm.cpu.x[11],
                            vm.cpu.x[12],
                            vm.cpu.x[13],
                            vm.cpu.x[14],
                            vm.cpu.x[15],
                            &mut vm.bus.uart,
                            &mut vm.bus.clint,
                        );
                        // Handle DBCN pending write
                        if let Some((phys_addr, num_bytes)) = vm.bus.sbi.dbcn_pending_write.take() {
                            for i in 0..num_bytes {
                                if let Ok(b) = vm.bus.read_byte(phys_addr + i as u64) {
                                    if b != 0 {
                                        vm.bus.uart.write_byte(0, b);
                                        vm.bus.sbi.console_output.push(b);
                                    }
                                }
                            }
                            vm.cpu.x[10] = 0; // SBI_SUCCESS
                            vm.cpu.x[11] = num_bytes as u32;
                        } else if let Some((a0, a1)) = result {
                            vm.cpu.x[10] = a0;
                            vm.cpu.x[11] = a1;
                        }
                    }
                    11 => {
                        // ECALL_M
                        ecall_m_count += 1;
                    }
                    _ => {}
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Use vm.step() which handles tick_clint + sync_mip internally
        let _step_result = vm.step();

        // Track SATP changes and apply full boot fixups.
        // After the kernel switches page tables (setup_vm, paging_init),
        // we need to:
        // 1. Inject identity mappings for device/low-address access
        // 2. Map the DTB in the new page table
        // 3. Fix broken kernel L1 entries (non-leaf with PPN=0)
        // 4. Re-patch kernel_map (phys_addr=0, va_pa_offset=0xC0000000)
        // 5. Protect critical addresses from BSS/init overwrites
        if vm.cpu.csr.satp != last_satp {
            satp_changes += 1;
            eprintln!(
                "[satp] #{} at count={}: 0x{:08X} -> 0x{:08X} PC=0x{:08X} medeleg=0x{:04X}",
                satp_changes, count, last_satp, vm.cpu.csr.satp, vm.cpu.pc, vm.cpu.csr.medeleg
            );
            let cur_satp = vm.cpu.csr.satp;
            let mode = (cur_satp >> 31) & 1;
            if mode == 1 {
                let ppn = cur_satp & 0x3FFFFF;
                let pg_dir_phys = (ppn as u64) * 4096;
                let identity_pte: u32 = 0x0000_00CF; // V+R+W+X+A+D, U=0
                let mut injected = 0u32;

                // 1. Inject identity mappings for L1[0..768]
                for l1_idx in 0..768u32 {
                    let addr = pg_dir_phys + (l1_idx as u64) * 4;
                    let existing = vm.bus.read_word(addr).unwrap_or(0);
                    if (existing & 1) == 0 {
                        let pte = identity_pte | (l1_idx << 20);
                        vm.bus.write_word(addr, pte).ok();
                        injected += 1;
                    }
                }

                // 2. Fix broken kernel L1 entries (L1[768..780])
                // Replace unmapped entries or non-leaf with PPN=0 with megapages
                let mega_flags: u32 = 0x0000_00CF;
                let mut fixup_count = 0u32;
                for l1_scan in 768..780u32 {
                    let scan_addr = pg_dir_phys + (l1_scan as u64) * 4;
                    let entry = vm.bus.read_word(scan_addr).unwrap_or(0);
                    let is_valid = (entry & 1) != 0;
                    let is_non_leaf = is_valid && (entry & 0xE) == 0;
                    let entry_ppn = (entry >> 10) & 0x3FFFFF;
                    let needs_fix = !is_valid || (is_non_leaf && entry_ppn == 0);
                    if needs_fix {
                        let pa_offset = l1_scan - 768;
                        let fixup_pte = mega_flags | (pa_offset << 20);
                        vm.bus.write_word(scan_addr, fixup_pte).ok();
                        fixup_count += 1;
                    }
                }
                if fixup_count > 0 {
                    eprintln!("[satp]   Fixed {} kernel L1 entries", fixup_count);
                }

                // 3. Re-patch kernel_map (phys_addr=0, va_pa_offset=0xC0000000)
                let km_phys: u64 = 0x00C7A098;
                vm.bus.write_word(km_phys + 12, 0x00000000).ok(); // phys_addr = 0
                vm.bus.write_word(km_phys + 20, 0xC0000000).ok(); // va_pa_offset
                vm.bus.write_word(km_phys + 24, 0x00000000).ok(); // va_kernel_pa_offset
                // Protect from future overwrites
                vm.bus.protected_addrs.push((km_phys + 12, 0x00000000));
                vm.bus.protected_addrs.push((km_phys + 20, 0xC0000000));
                vm.bus.protected_addrs.push((km_phys + 24, 0x00000000));
                // Protect DTB pointers
                vm.bus.protected_addrs.push((0x00C7A380, _dtb_addr as u32));
                vm.bus.protected_addrs.push((0x00C7A3B0, _dtb_addr as u32));

                // 4. Ensure DTB is mapped in new page table
                let dtb_va: u32 = _dtb_addr as u32; // identity-mapped
                let dtb_vpn1 = ((dtb_va >> 22) & 0x3FF) as u64;
                let dtb_vpn0 = ((dtb_va >> 12) & 0x3FF) as u64;
                let l1_addr = pg_dir_phys + dtb_vpn1 * 4;
                let l1_entry = vm.bus.read_word(l1_addr).unwrap_or(0);
                let l1_valid = (l1_entry & 1) != 0;
                let l1_leaf = l1_valid && (l1_entry & 0xE) != 0;
                if l1_valid && !l1_leaf {
                    let l2_ppn = ((l1_entry >> 10) & 0x3FFFFF) as u64;
                    let l2_base = l2_ppn * 4096;
                    let l2_addr = l2_base + dtb_vpn0 * 4;
                    let l2_entry = vm.bus.read_word(l2_addr).unwrap_or(0);
                    if (l2_entry & 1) == 0 {
                        let dtb_ppn = (_dtb_addr >> 12) as u32;
                        let dtb_pte: u32 = (dtb_ppn << 10) | 0x0000_00CF;
                        vm.bus.write_word(l2_addr, dtb_pte).ok();
                        eprintln!("[satp]   Added DTB L2 entry at VPN0={}", dtb_vpn0);
                    }
                }

                vm.cpu.tlb.flush_all();
                eprintln!(
                    "[satp]   Total: {} identity + {} kernel fixups into pg_dir at PA 0x{:08X}",
                    injected, fixup_count, pg_dir_phys
                );
            }
            last_satp = vm.cpu.csr.satp;
        }
        if vm.cpu.csr.medeleg != last_medeleg && count > 1000 {
            eprintln!(
                "[medeleg] Changed to 0x{:04X} at count={} PC=0x{:08X}",
                vm.cpu.csr.medeleg, count, vm.cpu.pc
            );
            last_medeleg = vm.cpu.csr.medeleg;
        }

        // Detect kernel panic (PC in panic function)
        if (0xC000252E..=0xC00027A0).contains(&vm.cpu.pc) && count > 1_000_000 {
            if sbi_count == 0 {
                // First time hitting panic - dump registers
                eprintln!(
                    "\n!!! KERNEL PANIC detected at count={} PC=0x{:08X} !!!",
                    count, vm.cpu.pc
                );
                eprintln!(
                    "    SP=0x{:08X} RA=0x{:08X} GP=0x{:08X} TP=0x{:08X}",
                    vm.cpu.x[2], vm.cpu.x[1], vm.cpu.x[3], vm.cpu.x[4]
                );
                eprintln!(
                    "    T0=0x{:08X} T1=0x{:08X} T2=0x{:08X} A0=0x{:08X}",
                    vm.cpu.x[5], vm.cpu.x[6], vm.cpu.x[7], vm.cpu.x[10]
                );
                eprintln!(
                    "    A1=0x{:08X} A2=0x{:08X} S0=0x{:08X} S1=0x{:08X}",
                    vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[8], vm.cpu.x[9]
                );
                eprintln!(
                    "    mcause=0x{:08X} sepc=0x{:08X} scause=0x{:08X}",
                    vm.cpu.csr.mcause, vm.cpu.csr.sepc, vm.cpu.csr.scause
                );
                // Check stack for panic message pointer (s3 register in panic)
                eprintln!(
                    "    S2=0x{:08X} S3=0x{:08X} S4=0x{:08X} S5=0x{:08X}",
                    vm.cpu.x[18], vm.cpu.x[19], vm.cpu.x[20], vm.cpu.x[21]
                );
                // Try to read panic message from the stack or registers
                // In panic(), a0 = the panic string pointer
                let panic_str_ptr = vm.cpu.x[10]; // a0 usually has the format string
                if panic_str_ptr > 0xC0000000 && panic_str_ptr < 0xC2000000 {
                    let pa = (panic_str_ptr - 0xC0000000) as u64;
                    let mut msg_bytes = Vec::new();
                    for i in 0..128u64 {
                        if let Ok(byte_val) = vm.bus.read_byte(pa + i) {
                            if byte_val == 0 {
                                break;
                            }
                            msg_bytes.push(byte_val);
                        } else {
                            break;
                        }
                    }
                    if let Ok(msg) = String::from_utf8(msg_bytes.clone()) {
                        eprintln!("    A0 string: '{}'", &msg[..msg.len().min(200)]);
                    }
                }
                // Check UART for any output before panic
                let tx = vm.bus.uart.drain_tx();
                if !tx.is_empty() {
                    let s = String::from_utf8_lossy(&tx);
                    eprintln!("    UART before panic: {}", &s[..s.len().min(2000)]);
                }
                break; // Stop execution on panic
            }
        }

        count += 1;

        if count == next_report {
            let elapsed = start.elapsed();
            let ips = count as f64 / elapsed.as_secs_f64();
            let priv_str = match vm.cpu.privilege {
                Privilege::Machine => "M",
                Privilege::Supervisor => "S",
                Privilege::User => "U",
            };
            // Drain UART to check for any output
            let uart_bytes = vm.bus.uart.drain_tx();
            let uart_preview = if uart_bytes.is_empty() {
                String::new()
            } else {
                let s = String::from_utf8_lossy(&uart_bytes);
                format!(" uart=\"{}\"", &s[..s.len().min(100)])
            };
            eprintln!("[{}M] PC=0x{:08X} SP=0x{:08X} RA=0x{:08X} SBI={} SATP=0x{:08X} priv={}{}",
                count / 1_000_000, vm.cpu.pc, vm.cpu.x[2], vm.cpu.x[1],
                sbi_count, vm.cpu.csr.satp, priv_str, uart_preview);
            if !uart_bytes.is_empty() {
                let s = String::from_utf8_lossy(&uart_bytes);
                eprintln!("[uart] {}", &s[..s.len().min(2000)]);
            }
            next_report += 1_000_000;
        }
    }

    let elapsed = start.elapsed();
    eprintln!(
        "\n=== Final State ({}M instructions, {:.1}s) ===",
        count / 1_000_000,
        elapsed.as_secs_f64()
    );
    eprintln!(
        "PC: 0x{:08X} SP: 0x{:08X} RA: 0x{:08X}",
        vm.cpu.pc, vm.cpu.x[2], vm.cpu.x[1]
    );
    eprintln!("SATP: 0x{:08X}", vm.cpu.csr.satp);
    eprintln!("medeleg: 0x{:04X}", vm.cpu.csr.medeleg);
    eprintln!("stvec: 0x{:08X}", vm.cpu.csr.stvec);
    eprintln!("SBI calls: {}", sbi_count);
    eprintln!("ECALL_M: {}", ecall_m_count);
    eprintln!("SATP changes: {}", satp_changes);
    eprintln!("CLINT mtime: {}", vm.bus.clint.mtime);
    eprintln!("MIP: 0x{:08X}", vm.cpu.csr.mip);

    let tx = vm.bus.uart.drain_tx();
    eprintln!("\nUART: {} bytes", tx.len());
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        eprintln!("{}", &s[..s.len().min(5000)]);
    }
}
