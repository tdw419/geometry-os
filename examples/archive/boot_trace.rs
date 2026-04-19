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
    let max_count: u64 = 500_000;
    let mut count: u64 = 0;
    let mut sbi_count: u64 = 0;
    let mut ecall_m_count: u64 = 0;
    let mut forward_count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut satp_changes: u32 = 0;
    let mut dtb_early_va_expected: u32 = (dtb_addr.wrapping_add(0xC0000000)) as u32;
    let mut last_report_pc: u32 = 0;
    let mut unique_pcs: u32 = 0;

    // PC tracking - log unique PCs to trace execution path
    let mut pc_log: Vec<(u64, u32, &'static str)> = Vec::new();
    let mut seen_pcs: std::collections::HashSet<u32> = std::collections::HashSet::new();

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
                }
            }
        }

        // SATP change handling (same as boot_panic_diag)
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
                    let mega_flags: u32 = 0x0000_00CF;
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
                        let pa_offset = l1_scan - 768;
                        let fixup_pte = mega_flags | (pa_offset << 20);
                        vm.bus.write_word(scan_addr, fixup_pte).ok();
                    }
                    vm.cpu.tlb.flush_all();
                    // Re-protect kernel_map
                    vm.bus.write_word(0x00C7A098 + 12, 0x00000000).ok();
                    vm.bus.write_word(0x00C7A098 + 20, 0xC0000000).ok();
                    vm.bus.write_word(0x00C7A098 + 24, 0x00000000).ok();
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
            let is_interrupt = (mcause >> 31) & 1 == 1;

            if is_interrupt {
                // Timer interrupt
                vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000;
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else {
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
                        if let Some((a0, a1)) = result {
                            vm.cpu.x[10] = a0;
                            vm.cpu.x[11] = a1;
                        }
                    }
                    11 => {
                        // ECALL_M
                        ecall_m_count += 1;
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
                        // Forward traps from S/U to S-mode
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
                                forward_count += 1;
                                count += 1;
                                continue;
                            }
                        }
                    }
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _step_result = vm.step();

        // Track unique PCs to understand execution flow
        let pc = vm.cpu.pc;
        if seen_pcs.insert(pc) {
            unique_pcs += 1;
            let priv_str = match vm.cpu.privilege {
                Privilege::Machine => "M",
                Privilege::Supervisor => "S",
                Privilege::User => "U",
            };
            // Only log after setup_vm (count > 180K) to reduce noise
            if count > 177_000 && unique_pcs <= 500 {
                pc_log.push((count, pc, priv_str));
            }
        }

        // Report every 50K instructions
        if count > 0 && count % 50_000 == 0 {
            let priv_str = match vm.cpu.privilege {
                Privilege::Machine => "M",
                Privilege::Supervisor => "S",
                Privilege::User => "U",
            };
            eprintln!(
                "[{}K] PC=0x{:08X} SBI={} ecall_m={} fwd={} priv={}",
                count / 1000,
                pc,
                sbi_count,
                ecall_m_count,
                forward_count,
                priv_str
            );
        }

        count += 1;
    }

    // Print execution trace after setup_vm
    eprintln!("\n=== Execution trace after setup_vm (first 200 unique PCs) ===");
    for (c, p, pr) in pc_log.iter().take(200) {
        eprintln!("  count={}: PC=0x{:08X} priv={}", c, p, pr);
    }

    eprintln!(
        "\nTotal: {} instructions, {} unique PCs, {} SBI calls, {} ECALL_M, {} forwards",
        count, unique_pcs, sbi_count, ecall_m_count, forward_count
    );

    // Check what the final PC is
    eprintln!("Final: PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);

    // UART output
    let tx = vm.bus.uart.drain_tx();
    eprintln!("UART: {} bytes", tx.len());
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        eprintln!("{}", &s[..s.len().min(5000)]);
    }
}
