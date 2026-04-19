use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 128, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let max_count: u64 = 5_000_000;
    let mut count: u64 = 0;
    let mut sbi_count: u64 = 0;
    let mut ecall_s_count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut satp_changes: u32 = 0;
    let mut panic_detected = false;
    let mut panic_ra: u32 = 0;
    let mut dtb_early_va_expected: u32 = (dtb_addr.wrapping_add(0xC0000000)) as u32;
    let mut dtb_watchdog_triggers: u32 = 0;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // DTB pointer watchdog
        if count % 100 == 0 {
            let prb = vm.bus.read_word(0x00C7A0B4u64).unwrap_or(0);
            if prb == 0 {
                let cur_va = vm.bus.read_word(0x00801008).unwrap_or(0);
                if cur_va != dtb_early_va_expected {
                    vm.bus.write_word(0x00801008, dtb_early_va_expected).ok();
                    vm.bus.write_word(0x0080100C, dtb_addr as u32).ok();
                    dtb_watchdog_triggers += 1;
                    if dtb_watchdog_triggers <= 3 {
                        eprintln!("[dtb-watchdog] #{} at count={}: restored _dtb_early_va from 0x{:08X} to 0x{:08X}",
                            dtb_watchdog_triggers, count, cur_va, dtb_early_va_expected);
                    }
                }
            }
        }

        // SATP change handling
        {
            let cur_satp = vm.cpu.csr.satp;
            if cur_satp != last_satp {
                satp_changes += 1;
                eprintln!(
                    "[satp] #{} at count={}: 0x{:08X} -> 0x{:08X} PC=0x{:08X}",
                    satp_changes, count, last_satp, cur_satp, vm.cpu.pc
                );
                let mode = (cur_satp >> 31) & 1;
                if mode == 1 {
                    let ppn = cur_satp & 0x3FFFFF;
                    let pg_dir_phys = (ppn as u64) * 4096;

                    // Inject device identity mappings
                    let device_l1_entries: &[u32] = &[0, 1, 2, 3, 4, 5, 8, 48, 64];
                    let identity_pte: u32 = 0x0000_00CF;
                    for &l1_idx in device_l1_entries {
                        let addr = pg_dir_phys + (l1_idx as u64) * 4;
                        let existing = vm.bus.read_word(addr).unwrap_or(0);
                        if (existing & 1) == 0 {
                            let pte = identity_pte | (l1_idx << 20);
                            vm.bus.write_word(addr, pte).ok();
                        }
                    }
                    vm.cpu.tlb.flush_all();

                    // Fix broken/missing kernel page table entries
                    let mega_flags: u32 = 0x0000_00CF;
                    let mut fixup_count = 0u32;
                    for l1_scan in 768..780u32 {
                        let scan_addr = pg_dir_phys + (l1_scan as u64) * 4;
                        let entry = vm.bus.read_word(scan_addr).unwrap_or(0);
                        let is_valid = (entry & 1) != 0;
                        let is_non_leaf = is_valid && (entry & 0xE) == 0;
                        let ppn = (entry >> 10) & 0x3FFFFF;
                        let needs_fix = !is_valid || (is_non_leaf && ppn == 0);
                        if !needs_fix {
                            continue;
                        }
                        fixup_count += 1;
                        let pa_offset = l1_scan - 768;
                        let fixup_pte = mega_flags | (pa_offset << 20);
                        vm.bus.write_word(scan_addr, fixup_pte).ok();
                        if fixup_count <= 10 {
                            eprintln!(
                                "[fixup] L1[{}] 0x{:08X} -> 0x{:08X}",
                                l1_scan, entry, fixup_pte
                            );
                        }
                    }
                    if fixup_count > 0 {
                        eprintln!("[fixup] Fixed {} kernel page table entries", fixup_count);
                        vm.cpu.tlb.flush_all();
                    }

                    // Re-protect kernel_map
                    let km_phys: u64 = 0x00C7A098;
                    vm.bus.write_word(km_phys + 12, 0x00000000).ok();
                    vm.bus.write_word(km_phys + 20, 0xC0000000).ok();
                    vm.bus.write_word(km_phys + 24, 0x00000000).ok();

                    // Re-set DTB pointers
                    vm.bus.write_word(0x00801008, dtb_early_va_expected).ok();
                    vm.bus.write_word(0x0080100C, dtb_addr as u32).ok();
                }
                last_satp = cur_satp;
            }
        }

        // Handle M-mode traps
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;

            match cause_code {
                9 => {
                    // ECALL_S = SBI call
                    ecall_s_count += 1;
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
                    if let Some((a0, a1)) = result {
                        vm.cpu.x[10] = a0;
                        vm.cpu.x[11] = a1;
                    }
                }
                11 => {
                    // ECALL_M
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
                    if let Some((a0, a1)) = result {
                        vm.cpu.x[10] = a0;
                        vm.cpu.x[11] = a1;
                    }
                }
                _ => {
                    // Forward non-SBI traps from S/U mode to S-mode handler
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
                            vm.cpu.csr.mstatus &= !(1u32 << 1);
                            if cause_code == 7 {
                                vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000;
                            }
                            vm.cpu.pc = stvec;
                            vm.cpu.privilege = Privilege::Supervisor;
                            vm.cpu.tlb.flush_all();
                            count += 1;
                            continue;
                        }
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _step_result = vm.step();

        // Detect panic
        if (0xC000252E..=0xC00027A0).contains(&vm.cpu.pc) && !panic_detected && count > 100_000 {
            panic_detected = true;
            panic_ra = vm.cpu.x[1]; // RA = caller of panic
            eprintln!(
                "\n!!! PANIC at count={} PC=0x{:08X} RA=0x{:08X} !!!",
                count, vm.cpu.pc, panic_ra
            );
            eprintln!(
                "    SP=0x{:08X} GP=0x{:08X} TP=0x{:08X}",
                vm.cpu.x[2], vm.cpu.x[3], vm.cpu.x[4]
            );
            eprintln!("    S2=0x{:08X} S3=0x{:08X}", vm.cpu.x[18], vm.cpu.x[19]);

            // Read panic message from s2 (saved a0 = format string)
            let fmt_ptr = vm.cpu.x[18];
            if fmt_ptr > 0xC0000000 && fmt_ptr < 0xC2000000 {
                let pa = (fmt_ptr - 0xC0000000) as u64;
                let mut msg = Vec::new();
                for i in 0..200u64 {
                    if let Ok(b) = vm.bus.read_byte(pa + i) {
                        if b == 0 {
                            break;
                        }
                        msg.push(b);
                    } else {
                        break;
                    }
                }
                if let Ok(s) = String::from_utf8(msg) {
                    eprintln!("    PANIC MSG (s2): '{}'", s);
                }
            }

            // Also read from the stack (s0+0 should have the fmt string)
            let s0 = vm.cpu.x[8];
            if s0 > 0xC0000000 {
                let s0_pa = (s0 - 0xC0000000) as u64;
                // panic stores: sw a1,4(s0); sw a2,8(s0); etc.
                // s0 = sp+64, so the original a0 (fmt string) is on the stack
                // Read the saved RA from stack to trace the caller
                let saved_ra = vm.bus.read_word(s0_pa + 60).unwrap_or(0); // ra at s0+60
                let saved_a0 = vm.bus.read_word(s0_pa + 0).unwrap_or(0); // a0 is NOT saved at s0
                eprintln!("    Stack: saved_ra=0x{:08X}", saved_ra);

                // Read the original a0 from the stack frame
                // panic frame: sp=original_sp-96, s0=original_sp+64-96=original_sp-32
                // Wait, s0 = sp + 64, and sp was decremented by 96. So original_sp = s0 - 64 + 96 = s0 + 32
                // But we need to find where a0 was saved. In panic, a0 is moved to s2 first.
                // Let's try reading from the caller's frame.
                // The caller's a0 should be the panic format string.
            }

            // Disassemble the caller (RA)
            if panic_ra > 0xC0000000 {
                let caller_pa = (panic_ra - 0xC0000000) as u64;
                // Read the instruction that called panic
                let mut bytes = Vec::new();
                for i in 0..16u64 {
                    if let Ok(b) = vm.bus.read_byte(caller_pa + i) {
                        bytes.push(b);
                    }
                }
                eprintln!(
                    "    Caller bytes at PA 0x{:08X}: {:02X?}",
                    caller_pa, &bytes
                );

                // Also read a few instructions before the call to find the panic string setup
                for i in (0..20).rev() {
                    let addr = caller_pa.saturating_sub(i as u64 * 2);
                    if let Ok(w) = vm.bus.read_word(addr) {
                        // Look for LUI/LI/AUIPC patterns that load a string address
                        if (w & 0x7F) == 0x37 || (w & 0x7F) == 0x17 || (w & 0x7F) == 0x13 {
                            // Could be an address load
                        }
                    }
                }
            }

            // Check UART
            let tx = vm.bus.uart.drain_tx();
            if !tx.is_empty() {
                let s = String::from_utf8_lossy(&tx);
                eprintln!("    UART ({} bytes): {}", tx.len(), &s[..s.len().min(2000)]);
            }

            // Dump memblock state
            let memblock_pa: u64 = 0x00803448;
            let mem_cnt = vm.bus.read_word(memblock_pa + 8).unwrap_or(0);
            let res_cnt = vm.bus.read_word(memblock_pa + 28).unwrap_or(0);
            eprintln!(
                "    memblock: memory.cnt={} reserved.cnt={}",
                mem_cnt, res_cnt
            );
            let prb = vm.bus.read_word(0x00C7A0B4u64).unwrap_or(0);
            eprintln!("    phys_ram_base=0x{:08X}", prb);

            break;
        }

        count += 1;
        if count % 2_000_000 == 0 {
            let priv_str = match vm.cpu.privilege {
                Privilege::Machine => "M",
                Privilege::Supervisor => "S",
                Privilege::User => "U",
            };
            eprintln!(
                "[{}M] PC=0x{:08X} SBI={} priv={}",
                count / 1_000_000,
                vm.cpu.pc,
                ecall_s_count,
                priv_str
            );
        }
    }

    if !panic_detected {
        eprintln!("No panic detected in {}M instructions", count / 1_000_000);
    }
    let tx = vm.bus.uart.drain_tx();
    eprintln!("UART: {} bytes", tx.len());
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        eprintln!("{}", &s[..s.len().min(5000)]);
    }
}
