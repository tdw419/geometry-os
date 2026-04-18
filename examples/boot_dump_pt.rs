/// Dump the trampoline page table to see what mappings the kernel created.
fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    use geometry_os::riscv::cpu::{Privilege, StepResult};
    use geometry_os::riscv::csr;
    use geometry_os::riscv::RiscvVm;

    let (mut vm, fw_addr, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    // Run until the first SATP change (kernel switches to trampoline)
    let max_count = 500_000u64;
    let mut count: u64 = 0;
    let mut last_satp = vm.cpu.csr.satp;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Handle trap forwarding (same as boot_linux)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code != csr::CAUSE_ECALL_M {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if cause_code == csr::CAUSE_ECALL_S {
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
                    if let Some((a0_val, a1_val)) = result {
                        vm.cpu.x[10] = a0_val;
                        vm.cpu.x[11] = a1_val;
                    }
                } else if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP))
                            | (spp << csr::MSTATUS_SPP);
                        let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE))
                            | (sie << csr::MSTATUS_SPIE);
                        vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else {
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
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
        }

        let step_result = vm.step();

        // Check for SATP change
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!(
                "[{}] SATP changed: 0x{:08X} -> 0x{:08X}",
                count, last_satp, cur_satp
            );
            last_satp = cur_satp;

            // Dump the new page table (trampoline_pg_dir at PA 0x01484000)
            let ppn = cur_satp & 0x3FFFFF;
            let pt_phys = (ppn as u64) << 12;
            eprintln!("[boot] Page table at PA 0x{:08X}", pt_phys);
            eprintln!("[boot] Dumping first 20 L1 entries:");
            for i in 0..20 {
                let pte = vm.bus.read_word(pt_phys + (i as u64) * 4).unwrap_or(0);
                if pte != 0 {
                    eprintln!(
                        "  L1[{}] = 0x{:08X} (V={} R={} W={} X={} PPN=0x{:05X})",
                        i,
                        pte,
                        (pte >> 0) & 1,
                        (pte >> 1) & 1,
                        (pte >> 2) & 1,
                        (pte >> 3) & 1,
                        (pte >> 10) & 0x3FFFFF
                    );
                }
            }
            // Also dump L1[768] (kernel VA 0xC0000000)
            for i in 768..776 {
                let pte = vm.bus.read_word(pt_phys + (i as u64) * 4).unwrap_or(0);
                if pte != 0 {
                    eprintln!(
                        "  L1[{}] = 0x{:08X} (V={} R={} W={} X={} PPN=0x{:05X})",
                        i,
                        pte,
                        (pte >> 0) & 1,
                        (pte >> 1) & 1,
                        (pte >> 2) & 1,
                        (pte >> 3) & 1,
                        (pte >> 10) & 0x3FFFFF
                    );
                } else {
                    eprintln!("  L1[{}] = 0x{:08X} (EMPTY)", i, pte);
                }
            }

            // Try translating VA 0xC0001048
            eprintln!(
                "[boot] Trying to translate VA 0xC0001048 with SATP=0x{:08X}",
                cur_satp
            );
            let vpn1 = ((0xC0001048u32 >> 22) & 0x3FF) as u64; // = 768
            let l1_addr = pt_phys + (vpn1 << 2);
            let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
            eprintln!(
                "[boot]   VPN1=768, L1 addr=0x{:08X}, L1 PTE=0x{:08X}",
                l1_addr, l1_pte
            );

            if l1_pte & 1 == 0 {
                eprintln!("[boot]   L1 PTE not valid! Page fault.");
            } else {
                let l1_ppn = ((l1_pte >> 10) & 0x3FFFFF) as u64;
                let l2_addr = (l1_ppn << 12) + (((0xC0001048u32 >> 12) & 0x3FF) as u64) * 4;
                let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
                eprintln!(
                    "[boot]   L1 PPN=0x{:05X}, L2 addr=0x{:08X}, L2 PTE=0x{:08X}",
                    l1_ppn, l2_addr, l2_pte
                );
            }

            // Only dump first SATP change, then stop
            break;
        }

        match step_result {
            StepResult::Ebreak => break,
            _ => {}
        }
        count += 1;
    }

    eprintln!(
        "\n[boot] Stopped at count={}, PC=0x{:08X}, priv={:?}",
        count, vm.cpu.pc, vm.cpu.privilege
    );
}
