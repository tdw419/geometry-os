use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;

/// Boot test: identity mappings on SATP change, NO megapage forcing.
/// Tests if identity maps alone (without kernel-range fixup) are sufficient.
fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    )
    .unwrap();

    vm.bus.auto_pte_fixup = true;

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut sbi_count: u64 = 0;
    let mut forward_count: u64 = 0;
    let mut smode_fault_count: u64 = 0;
    let mut last_log_count: u64 = 0;
    let max_count: u64 = 5_000_000;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            eprintln!("[test] Shutdown at count={}", count);
            break;
        }

        // Handle M-mode traps
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 9 || cause_code == 11 {
                // SBI call (ECALL_S or ECALL_M)
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
            } else {
                let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 8)) | (spp << 8);
                        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                        vm.cpu.csr.mstatus &= !(1 << 1);
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
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // On SATP change, inject ONLY identity mappings for device regions.
        // Do NOT touch the kernel range (L1[768+]) - the kernel manages those.
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!(
                "[test] SATP: 0x{:08X} -> 0x{:08X} at count={}",
                last_satp, cur_satp, count
            );
            let mode = (cur_satp >> 31) & 1;
            if mode == 1 {
                let ppn = (cur_satp & 0x3FFFFF) as u64;
                let pg_dir_phys = ppn * 4096;
                let identity_pte: u32 = 0x0000_00CF; // V+R+W+X+A+D, U=0

                // Low RAM + device regions for identity mapping
                let device_l1: &[u32] = &[0, 1, 2, 3, 4, 5, 8, 48, 64];
                for &l1_idx in device_l1 {
                    let addr = pg_dir_phys + (l1_idx as u64) * 4;
                    let existing = vm.bus.read_word(addr).unwrap_or(0);
                    if (existing & 1) == 0 {
                        let pte = identity_pte | (l1_idx << 20);
                        vm.bus.write_word(addr, pte).ok();
                    }
                }
                // Also inject identity for L1[6] (0x01800000-0x01BFFFFF) - covers more of initramfs/DTB area
                for l1_idx in [6u32, 7] {
                    let addr = pg_dir_phys + (l1_idx as u64) * 4;
                    let existing = vm.bus.read_word(addr).unwrap_or(0);
                    if (existing & 1) == 0 {
                        let pte = identity_pte | (l1_idx << 20);
                        vm.bus.write_word(addr, pte).ok();
                    }
                }
                vm.cpu.tlb.flush_all();
                eprintln!(
                    "[test] Injected identity maps into pg_dir PA 0x{:08X}",
                    pg_dir_phys
                );
            }
            last_satp = cur_satp;
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        let step_result = vm.step();

        if vm.cpu.privilege == Privilege::Supervisor {
            let scause = vm.cpu.csr.scause;
            let cause_code = scause & !(1u32 << 31);
            if cause_code >= 12 && cause_code <= 15 {
                smode_fault_count += 1;
            }
        }

        match step_result {
            StepResult::Ok => {}
            StepResult::Ebreak => {
                eprintln!("[test] EBREAK at count={}, PC=0x{:08X}", count, vm.cpu.pc);
                break;
            }
            StepResult::Ecall => {}
            _ => {}
        }

        count += 1;

        if count - last_log_count >= 500_000 {
            let new_faults = smode_fault_count;
            eprintln!(
                "[test] {}K: PC=0x{:08X} priv={:?} SBI={} fwd={} faults={}",
                count / 1000,
                vm.cpu.pc,
                vm.cpu.privilege,
                sbi_count,
                forward_count,
                new_faults
            );
            last_log_count = count;
            if new_faults > 0 && new_faults < 100 {
                // Check UART
                let uart = vm.bus.uart.drain_tx();
                if !uart.is_empty() {
                    eprintln!("[test] UART: {}", String::from_utf8_lossy(&uart));
                }
            }
        }
    }

    eprintln!(
        "[test] Done: count={} SBI={} fwd={} faults={}",
        count, sbi_count, forward_count, smode_fault_count
    );
    let uart = vm.bus.uart.drain_tx();
    if !uart.is_empty() {
        eprintln!(
            "[test] UART ({} bytes): {}",
            uart.len(),
            String::from_utf8_lossy(&uart)
        );
    } else {
        eprintln!("[test] No UART output");
    }
}
