/// Diagnostic: Catch the memblock_alloc panic and dump the format args
/// to understand what allocation fails and why.
use geometry_os::riscv::RiscvVm;

fn read_string(vm: &mut RiscvVm, va: u32, max_len: usize) -> String {
    let mut chars = Vec::new();
    let pa = if va >= 0xC0000000 { (va - 0xC0000000) as u64 } else { va as u64 };
    for j in 0..max_len {
        if let Ok(b) = vm.bus.read_byte(pa + j as u64) {
            if b == 0 { break; }
            if b >= 0x20 && b < 0x7f { chars.push(b as char); } else { chars.push('.'); }
        } else { break; }
    }
    chars.iter().collect()
}

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8 earlycon=sbi",
    ).unwrap();

    eprintln!("[DIAG] DTB at PA 0x{:08X}", dtb_addr);
    eprintln!("[DIAG] fw_addr at PA 0x{:08X}", fw_addr);

    let max_instructions = 2_000_000u64;
    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut panic_hit = false;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested { break; }

        // SATP change handling (same as boot.rs)
        {
            let cur_satp = vm.cpu.csr.satp;
            if cur_satp != last_satp {
                eprintln!("\n[DIAG] SATP change: 0x{:08X} -> 0x{:08X} at count={}", last_satp, cur_satp, count);
                let mode = (cur_satp >> 31) & 1;
                if mode == 1 {
                    let ppn = cur_satp & 0x3FFFFF;
                    let pg_dir_phys = (ppn as u64) * 4096;
                    let device_l1_entries: &[u32] = &[0, 1, 2, 3, 4, 5, 8, 48, 64];
                    let identity_pte: u32 = 0x0000_00CF;
                    for &l1_idx in device_l1_entries {
                        let addr = pg_dir_phys + (l1_idx as u64) * 4;
                        let existing = vm.bus.read_word(addr).unwrap_or(0);
                        if (existing & 1) == 0 {
                            vm.bus.write_word(addr, identity_pte | (l1_idx << 20)).ok();
                        }
                    }
                    let mega_flags: u32 = 0x0000_00CF;
                    for l1_scan in 768..780u32 {
                        let scan_addr = pg_dir_phys + (l1_scan as u64) * 4;
                        let entry = vm.bus.read_word(scan_addr).unwrap_or(0);
                        let is_valid = (entry & 1) != 0;
                        let is_non_leaf = is_valid && (entry & 0xE) == 0;
                        let ppn_val = (entry >> 10) & 0x3FFFFF;
                        let needs_fix = !is_valid || (is_non_leaf && ppn_val == 0);
                        if needs_fix {
                            let pa_offset = l1_scan - 768;
                            let fixup_pte = mega_flags | (pa_offset << 20);
                            vm.bus.write_word(scan_addr, fixup_pte).ok();
                        }
                    }
                    // kernel_map fixup
                    let km_phys: u64 = 0x00C79E90;
                    vm.bus.write_word(km_phys + 12, 0x00000000).ok();
                    vm.bus.write_word(km_phys + 20, 0xC0000000).ok();
                    vm.bus.write_word(km_phys + 24, 0x00000000).ok();
                    vm.cpu.tlb.flush_all();
                }
                last_satp = cur_satp;
            }
        }

        // Trap handling
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                // ECALL_M = SBI call
                eprintln!("[DIAG] ECALL_M at count={}: a7={:#x} a6={:#x} a0={:#x}", count, vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10]);
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            } else if cause_code == 9 {
                // ECALL_S = SBI call (delegated)
                eprintln!("[DIAG] ECALL_S at count={}: a7={:#x} a6={:#x} a0={:#x}", count, vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10]);
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            } else {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                        vm.cpu.csr.mstatus &= !(1 << 1);
                        if cause_code == 7 { vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000; }
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Panic detection at 0xC000252E (panic function entry)
        if vm.cpu.pc == 0xC000252E && !panic_hit {
            panic_hit = true;
            eprintln!("\n[DIAG] *** PANIC at count={} ***", count);
            eprintln!("[DIAG] PC=0x{:08X} RA=0x{:08X} SP=0x{:08X}", vm.cpu.pc, vm.cpu.x[1], vm.cpu.x[2]);

            // Save registers before calling read_string (borrow issue)
            let a0 = vm.cpu.x[10];
            let a1 = vm.cpu.x[11];
            let a2 = vm.cpu.x[12];

            // a0 = format string pointer
            let fmt_str = read_string(&mut vm, a0, 200);
            eprintln!("[DIAG] a0 (fmt) = 0x{:08X}: \"{}\"", a0, fmt_str);

            // a1 = first vararg (function name for %s)
            let fn_name = read_string(&mut vm, a1, 100);
            eprintln!("[DIAG] a1 (fn_name) = 0x{:08X}: \"{}\"", a1, fn_name);

            // a2 = second vararg (pointer to phys_addr_t for %pap)
            let size_ptr = a2;
            let alloc_size = if size_ptr >= 0xC0000000 {
                vm.bus.read_word((size_ptr - 0xC0000000) as u64).unwrap_or(0)
            } else {
                vm.bus.read_word(size_ptr as u64).unwrap_or(0)
            };
            eprintln!("[DIAG] a2 (size_ptr) = 0x{:08X} -> *ptr = 0x{:08X} = {} bytes", a2, alloc_size, alloc_size);
            // Also check a3, s0, s1 for additional args
            eprintln!("[DIAG] a3 = 0x{:08X}, s0 = 0x{:08X}, s1 = 0x{:08X}", vm.cpu.x[13], vm.cpu.x[8], vm.cpu.x[9]);

            // Check kernel_map values
            let km_phys: u64 = 0x00C79E90;
            let km_pa = vm.bus.read_word(km_phys + 12).unwrap_or(0);
            let km_vapo = vm.bus.read_word(km_phys + 20).unwrap_or(0);
            let km_vkpo = vm.bus.read_word(km_phys + 24).unwrap_or(0);
            let km_po = vm.bus.read_word(km_phys).unwrap_or(0);
            eprintln!("[DIAG] kernel_map: page_offset=0x{:X} phys_addr=0x{:X} va_pa_offset=0x{:X} va_kernel_pa_offset=0x{:X}",
                km_po, km_pa, km_vapo, km_vkpo);

            // Check medeleg
            eprintln!("[DIAG] medeleg=0x{:08X} stvec=0x{:08X} satp=0x{:08X}",
                vm.cpu.csr.medeleg, vm.cpu.csr.stvec, vm.cpu.csr.satp);

            // Check SBI console output
            let sbi_str: String = vm.bus.sbi.console_output.iter().map(|&b| b as char).collect();
            if !sbi_str.is_empty() {
                eprintln!("[DIAG] SBI output ({} bytes): {:?}", sbi_str.len(), sbi_str.chars().take(500).collect::<String>());
            } else {
                eprintln!("[DIAG] SBI output: EMPTY (0 bytes)");
            }

            break;
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
        count += 1;
    }

    if !panic_hit {
        eprintln!("[DIAG] No panic in {} instructions. PC=0x{:08X}", count, vm.cpu.pc);
    }

    eprintln!("[DIAG] SBI calls: {} bytes", vm.bus.sbi.console_output.len());
    let sbi_str: String = vm.bus.sbi.console_output.iter().map(|&b| b as char).collect();
    if !sbi_str.is_empty() {
        eprintln!("[DIAG] SBI console: {}", sbi_str.chars().take(1000).collect::<String>());
    }
}
