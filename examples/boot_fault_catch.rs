fn main() {
    // Write a custom boot loop that stops at the first fault and dumps diagnostics
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::cpu::{Privilege, StepResult};
    use geometry_os::riscv::csr;
    use geometry_os::riscv::mmu;
    use geometry_os::riscv::RiscvVm;

    // Boot the normal way to get the VM state
    let (mut vm, _) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        16_900_000,
        bootargs,
    )
    .unwrap();

    println!(
        "At 16.9M: PC=0x{:08X} priv={:?}",
        vm.cpu.pc, vm.cpu.privilege
    );

    // Continue stepping until we hit the fault or reach 17.5M
    let l1_idx: u32 = 770;
    let mut last_satp = vm.cpu.csr.read(csr::SATP);
    let ppn = last_satp & 0x3FFFFF;
    let pt_base = (ppn as u64) << 12;
    let l1_addr = pt_base + (l1_idx as u64) * 4;
    let mut last_l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);

    let mut sfence_count: u64 = 0;
    let mut fault_found = false;

    for count in 16_900_000..17_500_000 {
        // Check SATP
        let satp = vm.cpu.csr.read(csr::SATP);
        if satp != last_satp {
            println!(
                "[{}] SATP changed: 0x{:08X} -> 0x{:08X}",
                count, last_satp, satp
            );
            last_satp = satp;
        }

        // Check L1[770] every 10000 steps
        if count % 10000 == 0 {
            let pte = vm.bus.read_word(l1_addr).unwrap_or(0);
            if pte != last_l1_pte {
                println!(
                    "[{}] L1[770] changed: 0x{:08X} -> 0x{:08X} (PC=0x{:08X})",
                    count, last_l1_pte, pte, vm.cpu.pc
                );
                last_l1_pte = pte;
            }
        }

        let step_result = vm.step();

        // Count SFENCE.VMA
        if let Some(ref last) = vm.cpu.last_step {
            if let geometry_os::riscv::decode::Operation::SfenceVma { .. } = last.op {
                sfence_count += 1;
                if sfence_count <= 5 {
                    println!("[{}] SFENCE.VMA at PC=0x{:08X}", count, last.pc);
                }
            }
        }

        match step_result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if !fault_found {
                    fault_found = true;
                    println!("\n=== FIRST FAULT at count={} ===", count);
                    println!("PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
                    println!(
                        "scause=0x{:08X} sepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X}",
                        vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.stvec
                    );
                    println!("satp=0x{:08X}", vm.cpu.csr.read(csr::SATP));

                    // Dump L1[770] NOW
                    let pte = vm.bus.read_word(l1_addr).unwrap_or(0);
                    println!(
                        "L1[770] = 0x{:08X} V={} R={} W={} X={}",
                        pte,
                        pte & 1,
                        (pte >> 1) & 1,
                        (pte >> 2) & 1,
                        (pte >> 3) & 1
                    );

                    // Check what instruction was at the PC that caused the fault
                    let sepc = vm.cpu.csr.sepc;
                    let ppn_check = last_satp & 0x3FFFFF;
                    let pt_base_check = (ppn_check as u64) << 12;
                    let vpn1_sepc = (sepc >> 22) & 0x3FF;
                    let vpn0_sepc = (sepc >> 12) & 0x3FF;
                    let l1_addr_sepc = pt_base_check + (vpn1_sepc as u64) * 4;
                    let l1_pte_sepc = vm.bus.read_word(l1_addr_sepc).unwrap_or(0);
                    println!(
                        "\nsepc=0x{:08X} VPN[1]={} VPN[0]={}",
                        sepc, vpn1_sepc, vpn0_sepc
                    );
                    println!(
                        "L1[{}] = 0x{:08X} V={}",
                        vpn1_sepc,
                        l1_pte_sepc,
                        l1_pte_sepc & 1
                    );

                    if (l1_pte_sepc & 1) != 0 && ((l1_pte_sepc & 0xE) != 0) {
                        // Megapage - check if X is set
                        println!("  Megapage, X={}", (l1_pte_sepc >> 3) & 1);
                    } else if (l1_pte_sepc & 1) != 0 {
                        // Non-leaf - follow to L2
                        let l2_ppn = (l1_pte_sepc >> 10) & 0x3FFFFF;
                        let l2_base = (l2_ppn as u64) << 12;
                        let l2_addr = l2_base + (vpn0_sepc as u64) * 4;
                        let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
                        println!(
                            "L2[{}] at PA=0x{:08X} = 0x{:08X} V={} R={} W={} X={}",
                            vpn0_sepc,
                            l2_addr,
                            l2_pte,
                            l2_pte & 1,
                            (l2_pte >> 1) & 1,
                            (l2_pte >> 2) & 1,
                            (l2_pte >> 3) & 1
                        );
                    }

                    // Run a few more steps to see what happens
                    println!("\nNext 5 steps after first fault:");
                    for i in 0..5 {
                        let sr = vm.step();
                        println!(
                            "  [{}] PC=0x{:08X} priv={:?} scause=0x{:08X} stvec=0x{:08X} => {:?}",
                            count + i + 1,
                            vm.cpu.pc,
                            vm.cpu.privilege,
                            vm.cpu.csr.scause,
                            vm.cpu.csr.stvec,
                            sr
                        );
                    }
                    break;
                }
            }
            StepResult::Ebreak => {
                println!("EBREAK at count={}", count);
                break;
            }
            _ => {}
        }
    }

    println!("\nTotal SFENCE.VMA: {}", sfence_count);
    println!("Final PC=0x{:08X}", vm.cpu.pc);
}
