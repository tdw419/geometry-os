use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;

/// Diagnostic: check page table entries after each SATP change,
/// especially the kernel range (L1[768..]).
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
    let satp_change_count = std::cell::Cell::new(0u32);

    while count < 2_000_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 9 || cause_code == 11 {
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

        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            let n = satp_change_count.get();
            satp_change_count.set(n + 1);
            eprintln!(
                "[pt] SATP #{}: 0x{:08X} -> 0x{:08X} at count={}",
                n + 1,
                last_satp,
                cur_satp,
                count
            );
            let mode = (cur_satp >> 31) & 1;
            if mode == 1 {
                let ppn = (cur_satp & 0x3FFFFF) as u64;
                let pg_dir_phys = ppn * 4096;

                // Dump kernel range L1 entries BEFORE injection
                eprintln!(
                    "[pt] pg_dir at PA 0x{:08X}, kernel range L1[768..779]:",
                    pg_dir_phys
                );
                for i in 768..779u32 {
                    let addr = pg_dir_phys + (i as u64) * 4;
                    let entry = vm.bus.read_word(addr).unwrap_or(0);
                    let v = entry & 1;
                    let rwx = (entry >> 1) & 7;
                    let ppn = (entry >> 10) & 0x3FFFFF;
                    let kind = if rwx != 0 {
                        "mega"
                    } else if v != 0 {
                        "L2"
                    } else {
                        "---"
                    };
                    eprintln!(
                        "[pt]   L1[{}] = 0x{:08X} V={} RWX={} PPN=0x{:06X} ({})",
                        i, entry, v, rwx, ppn, kind
                    );

                    // If non-leaf (L2 pointer), dump a few L2 entries
                    if v != 0 && rwx == 0 && ppn > 0 {
                        let l2_base = (ppn as u64) * 4096;
                        for j in 0..4u32 {
                            let l2_addr = l2_base + (j as u64) * 4;
                            let l2e = vm.bus.read_word(l2_addr).unwrap_or(0);
                            if l2e != 0 {
                                let l2_ppn = (l2e >> 10) & 0x3FFFFF;
                                let l2_rwx = (l2e >> 1) & 7;
                                eprintln!(
                                    "[pt]     L2[{}] = 0x{:08X} PPN=0x{:06X} PA=0x{:08X}",
                                    j,
                                    l2e,
                                    l2_ppn,
                                    (l2_ppn as u64) << 12
                                );
                            }
                        }
                    }
                }

                // Inject identity maps for device/low regions
                let identity_pte: u32 = 0x0000_00CF;
                let device_l1: &[u32] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 48, 64];
                for &l1_idx in device_l1 {
                    let addr = pg_dir_phys + (l1_idx as u64) * 4;
                    let existing = vm.bus.read_word(addr).unwrap_or(0);
                    if (existing & 1) == 0 {
                        let pte = identity_pte | (l1_idx << 20);
                        vm.bus.write_word(addr, pte).ok();
                    }
                }
                vm.cpu.tlb.flush_all();
                eprintln!("[pt] Injected identity maps");
            }
            last_satp = cur_satp;
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let step_result = vm.step();

        // Log what happens right after the 3rd SATP change
        let n = satp_change_count.get();
        if n >= 3 && count < 800_000 && forward_count <= 10 {
            eprintln!(
                "[pt] count={} PC=0x{:08X} priv={:?} step={:?}",
                count, vm.cpu.pc, vm.cpu.privilege, step_result
            );
        }

        match step_result {
            StepResult::Ok => {}
            StepResult::Ebreak => break,
            StepResult::Ecall => {}
            _ => {}
        }

        count += 1;

        if count == 800_000 {
            eprintln!(
                "[pt] At 800K: PC=0x{:08X} priv={:?} fwd={}",
                vm.cpu.pc, vm.cpu.privilege, forward_count
            );
            // Check what instruction is at the current PC
            let inst = vm.bus.read_word(vm.cpu.pc as u64).unwrap_or(0);
            eprintln!("[pt] Instruction at PA 0x{:08X}: 0x{:08X}", vm.cpu.pc, inst);
        }
    }

    eprintln!(
        "[pt] Done: count={} SBI={} fwd={}",
        count, sbi_count, forward_count
    );
    let uart = vm.bus.uart.drain_tx();
    if !uart.is_empty() {
        eprintln!("[pt] UART: {}", String::from_utf8_lossy(&uart));
    }
}
