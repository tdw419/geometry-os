fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::cpu::{Privilege, StepResult};
    use geometry_os::riscv::RiscvVm;

    let (mut vm, fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut illegal_count = 0u32;
    let max_instr = 100_000_000u64;

    while count < max_instr {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // DTB watchdog (same as boot_linux)
        if count % 100 == 0 {
            let prb = vm.bus.read_word(0x00C79EACu64).unwrap_or(0);
            if prb == 0 {
                let dtb_early_va_expected = (dtb_addr.wrapping_add(0xC0000000)) as u32;
                let cur_va = vm.bus.read_word(0x00801008).unwrap_or(0);
                if cur_va != dtb_early_va_expected {
                    vm.bus.write_word(0x00801008, dtb_early_va_expected).ok();
                    vm.bus.write_word(0x0080100C, dtb_addr as u32).ok();
                }
            }
        }

        // SATP change handling (same as boot_linux)
        {
            let cur_satp = vm.cpu.csr.satp;
            if cur_satp != last_satp {
                let mode = (cur_satp >> 31) & 1;
                if mode == 1 {
                    let ppn = cur_satp & 0x3FFFFF;
                    let pg_dir_phys = (ppn as u64) * 4096;
                    let device_l1: &[u32] = &[0, 1, 2, 3, 4, 5, 8, 48, 64];
                    let identity_pte: u32 = 0x0000_00CF;
                    for &l1_idx in device_l1 {
                        let addr = pg_dir_phys + (l1_idx as u64) * 4;
                        let existing = vm.bus.read_word(addr).unwrap_or(0);
                        if (existing & 1) == 0 {
                            let pte = identity_pte | (l1_idx << 20);
                            vm.bus.write_word(addr, pte).ok();
                        }
                    }
                    // Fix kernel PT entries
                    let mega_flags: u32 = 0x0000_00CF;
                    for l1_scan in 768..780u32 {
                        let scan_addr = pg_dir_phys + (l1_scan as u64) * 4;
                        let entry = vm.bus.read_word(scan_addr).unwrap_or(0);
                        let is_valid = (entry & 1) != 0;
                        let is_non_leaf = is_valid && (entry & 0xE) == 0;
                        let ppn_val = (entry >> 10) & 0x3FFFFF;
                        let needs_fix = !is_valid || (is_non_leaf && ppn_val == 0);
                        if !needs_fix {
                            continue;
                        }
                        let pa_offset = l1_scan - 768;
                        let fixup_pte = mega_flags | (pa_offset << 20);
                        vm.bus.write_word(scan_addr, fixup_pte).ok();
                    }
                    vm.cpu.tlb.flush_all();
                    // Re-verify kernel_map
                    let km_phys: u64 = 0x00C79E90;
                    let km_pa = vm.bus.read_word(km_phys + 12).unwrap_or(0);
                    let km_vapo = vm.bus.read_word(km_phys + 20).unwrap_or(0);
                    let km_vkpo = vm.bus.read_word(km_phys + 24).unwrap_or(0);
                    if km_pa != 0 || km_vapo != 0xC0000000 || km_vkpo != 0 {
                        vm.bus.write_word(km_phys + 12, 0).ok();
                        vm.bus.write_word(km_phys + 20, 0xC0000000).ok();
                        vm.bus.write_word(km_phys + 24, 0).ok();
                    }
                    // Re-set DTB pointers
                    vm.bus
                        .write_word(0x00801008, (dtb_addr.wrapping_add(0xC0000000)) as u32)
                        .ok();
                    vm.bus.write_word(0x0080100C, dtb_addr as u32).ok();
                    eprintln!(
                        "[diag] SATP changed to 0x{:08X} at count={}",
                        cur_satp, count
                    );
                }
                last_satp = cur_satp;
            }
        }

        // Trap handling (same as boot_linux)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code != 11 {
                // Not ECALL_M
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;

                if cause_code == 9 {
                    // ECALL_S = SBI call
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
                } else if mpp != 3 && cause_code == 2 {
                    // Illegal instruction from S/U mode
                    illegal_count += 1;
                    if illegal_count <= 5 {
                        let mepc = vm.cpu.csr.mepc;
                        eprintln!(
                            "[diag] Illegal instr #{} at count={}: mepc=0x{:08X} mpp={}",
                            illegal_count, count, mepc, mpp
                        );

                        // Manually translate mepc through MMU to get the actual PA
                        let vpn1 = ((mepc >> 22) & 0x3FF) as u64;
                        let vpn0 = ((mepc >> 12) & 0x3FF) as u64;
                        let satp = vm.cpu.csr.satp;
                        let pg_ppn = (satp & 0x3FFFFF) as u64;
                        let pg_dir_phys = pg_ppn * 4096;
                        let l1_entry = vm.bus.read_word(pg_dir_phys + vpn1 * 4).unwrap_or(0);
                        let l1_ppn = ((l1_entry >> 10) & 0x3FFFFF) as u64;
                        let l1_is_leaf = (l1_entry & 0xE) != 0;

                        if l1_is_leaf {
                            // Megapage
                            let pa = (l1_ppn << 12) | ((mepc as u64) & 0x3FFFFF);
                            let inst = vm.bus.read_word(pa).unwrap_or(0);
                            eprintln!(
                                "[diag]   L1[{}] = 0x{:08X} (megapage), PA=0x{:08X}, inst=0x{:08X}",
                                vpn1, l1_entry, pa, inst
                            );
                        } else if (l1_entry & 1) != 0 {
                            // Non-leaf -> L2 lookup
                            let l2_entry = vm.bus.read_word(l1_ppn * 4096 + vpn0 * 4).unwrap_or(0);
                            let l2_ppn = ((l2_entry >> 10) & 0x3FFFFF) as u64;
                            let pa = (l2_ppn << 12) | ((mepc as u64) & 0xFFF);
                            let inst = vm.bus.read_word(pa).unwrap_or(0);
                            eprintln!("[diag]   L1[{}] = 0x{:08X} (non-leaf), L2[{}] = 0x{:08X}, PA=0x{:08X}, inst=0x{:08X}", 
                                vpn1, l1_entry, vpn0, l2_entry, pa, inst);
                        } else {
                            eprintln!("[diag]   L1[{}] = 0x{:08X} (UNMAPPED!)", vpn1, l1_entry);
                        }

                        // Check what the correct instruction should be (direct from kernel binary)
                        // For VA 0xC000xxxx, PA should be 0x0000xxxx
                        let expected_pa = if mepc >= 0xC0000000 {
                            (mepc - 0xC0000000) as u64
                        } else {
                            mepc as u64
                        };
                        let correct_inst = vm.bus.read_word(expected_pa).unwrap_or(0);
                        eprintln!(
                            "[diag]   Expected PA=0x{:08X}, correct inst=0x{:08X}",
                            expected_pa, correct_inst
                        );
                    }

                    // Forward to S-mode
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
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                } else if mpp != 3 {
                    // Other faults from S/U mode - forward to S-mode
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

            // Skip faulting instruction
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // On-demand identity mapping for low address page faults
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        match vm.step() {
            StepResult::Ok => {}
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if vm.cpu.privilege == Privilege::Supervisor {
                    let fault_addr = vm.cpu.csr.stval;
                    if fault_addr < 0x0200_0000 {
                        let satp = vm.cpu.csr.satp;
                        let pg_dir_ppn = (satp & 0x3FFFFF) as u64;
                        if pg_dir_ppn > 0 {
                            let pg_dir_phys = pg_dir_ppn * 4096;
                            let vpn1 = ((fault_addr >> 22) & 0x3FF) as u64;
                            let l1_addr = pg_dir_phys + vpn1 * 4;
                            let existing = vm.bus.read_word(l1_addr).unwrap_or(0);
                            if (existing & 1) == 0 {
                                let pte: u32 = 0x0000_00CF | ((vpn1 as u32) << 20);
                                vm.bus.write_word(l1_addr, pte).ok();
                                vm.cpu.tlb.flush_all();
                            }
                        }
                    }
                }
            }
            StepResult::Ebreak => break,
            StepResult::Ecall => {}
        }

        if count.is_multiple_of(10_000_000) && count > 0 {
            eprintln!(
                "[diag] {}M: PC=0x{:08X} priv={:?}",
                count / 1_000_000,
                vm.cpu.pc,
                vm.cpu.privilege
            );
        }
        count += 1;
    }

    eprintln!(
        "[diag] Done: {} instr, {} illegal, PC=0x{:08X}",
        count, illegal_count, vm.cpu.pc
    );
    eprintln!(
        "[diag] UART: {} chars, SBI console: {} chars",
        vm.bus.uart.tx_buf.len(),
        vm.bus.sbi.console_output.len()
    );
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        let preview: String = s.chars().take(2000).collect();
        eprintln!("[diag] SBI output:\n{}", preview);
    }
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        let preview: String = s.chars().take(2000).collect();
        eprintln!("[diag] UART output:\n{}", preview);
    }
}
