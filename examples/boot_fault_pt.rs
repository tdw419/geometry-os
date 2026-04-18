use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;

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

    vm.bus.auto_pte_fixup = false;

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;

    while count < 300_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

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
            } else if cause_code != 11 {
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
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Track SATP changes
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!(
                "[diag] SATP changed: 0x{:08X} -> 0x{:08X} at count={}",
                last_satp, cur_satp, count
            );
            last_satp = cur_satp;
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let step_result = vm.step();

        // When we see the first S-mode fault, dump the page table for the handler VA
        if vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor {
            let scause = vm.cpu.csr.scause;
            let cause_code = scause & !(1u32 << 31);
            if cause_code >= 12 && cause_code <= 15 && count > 180000 {
                eprintln!(
                    "[diag] S-mode fault at count={}: PC=0x{:08X} scause=0x{:X} stval=0x{:08X}",
                    count, vm.cpu.pc, scause, vm.cpu.csr.stval
                );

                let satp = vm.cpu.csr.satp;
                let pg_dir_ppn = (satp & 0x3FFFFF) as u64;
                let pg_dir_phys = pg_dir_ppn * 4096;
                eprintln!(
                    "[diag] SATP=0x{:08X} pg_dir_phys=0x{:08X}",
                    satp, pg_dir_phys
                );

                // Dump L1[768..780] - kernel linear mapping
                for i in 768..780u32 {
                    let l1_addr = pg_dir_phys + (i as u64) * 4;
                    let entry = vm.bus.read_word(l1_addr).unwrap_or(0);
                    let v = (entry >> 0) & 1;
                    let r = (entry >> 1) & 1;
                    let w = (entry >> 2) & 1;
                    let x = (entry >> 3) & 1;
                    let u = (entry >> 4) & 1;
                    let ppn = (entry >> 10) & 0x3FFFFF;
                    let is_leaf = r | w | x;
                    eprintln!(
                        "[diag] L1[{}] = 0x{:08X} V={} R={} W={} X={} U={} PPN=0x{:06X} {}",
                        i,
                        entry,
                        v,
                        r,
                        w,
                        x,
                        u,
                        ppn,
                        if is_leaf != 0 {
                            "LEAF (megapage)"
                        } else {
                            "NON-LEAF (L2 ptr)"
                        }
                    );

                    // If non-leaf, dump L2 entries
                    if v == 1 && is_leaf == 0 && ppn > 0 {
                        let l2_base = (ppn as u64) * 4096;
                        // Check the L2 entry for VA 0xC0210F14
                        // VPN0 = (0xC0210F14 >> 12) & 0x3FF = 0x210 = 528
                        let vpn0 = (0xC0210F14u32 >> 12) & 0x3FF;
                        if i == 768 {
                            eprintln!(
                                "[diag] L2 base at PA 0x{:08X}, checking entry [{}]",
                                l2_base, vpn0
                            );
                            let l2_addr = l2_base + (vpn0 as u64) * 4;
                            let l2_entry = vm.bus.read_word(l2_addr).unwrap_or(0);
                            let l2_ppn = (l2_entry >> 10) & 0x3FFFFF;
                            let l2_v = l2_entry & 1;
                            let l2_leaf = ((l2_entry >> 1) & 1)
                                | ((l2_entry >> 2) & 1)
                                | ((l2_entry >> 3) & 1);
                            eprintln!(
                                "[diag] L2[{}] = 0x{:08X} V={} LEAF={} PPN=0x{:06X} PA=0x{:08X}",
                                vpn0,
                                l2_entry,
                                l2_v,
                                l2_leaf,
                                l2_ppn,
                                (l2_ppn as u64) * 4096
                            );
                        }
                    }
                }

                // Also check: what instruction is actually at PA 0x00210F14?
                let inst_pa = 0x00210F14u64;
                let inst = vm.bus.read_word(inst_pa).unwrap_or(0);
                eprintln!("[diag] Instruction at PA 0x{:08X}: 0x{:08X}", inst_pa, inst);

                // What's the VA the kernel uses for the handler?
                let handler_va = 0xC0210F14u32;
                eprintln!(
                    "[diag] Handler VA 0x{:08X}, expected PA 0x{:08X}",
                    handler_va,
                    handler_va - 0xC0000000
                );

                break;
            }
        }

        match step_result {
            StepResult::Ok => {}
            StepResult::Ebreak => break,
            _ => {}
        }
        count += 1;
    }
}
