fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::cpu::Privilege;
    use geometry_os::riscv::csr;
    use geometry_os::riscv::mmu;
    use geometry_os::riscv::RiscvVm;

    let (mut vm, fw_addr, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    // Run to 11,765,500
    for i in 0..11_765_500u64 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mc = vm.cpu.csr.mcause & !(1u32 << 31);
            if mc == csr::CAUSE_ECALL_S {
                let r = vm.bus.sbi.handle_ecall(
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
                if let Some((a0, a1)) = r {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else if mc != csr::CAUSE_ECALL_M {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = vm.cpu.csr.mcause;
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
                    } else {
                        vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                    }
                } else {
                    vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                }
            } else {
                let r = vm.bus.sbi.handle_ecall(
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
                if let Some((a0, a1)) = r {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
            continue;
        }
        let _ = vm.step();
    }

    // Now manually translate the virtual addresses and compare with what __memmove uses
    let asid: u16 = ((vm.cpu.csr.satp >> 16) & 0xFFFF) as u16;

    // Check dst address 0xC1CCA520
    let dst_va = 0xC1CCA520u32;
    let dst_vpn = dst_va >> 12;
    if let Some((ppn, flags)) = vm.cpu.tlb.lookup(dst_vpn, asid) {
        let pa = ((ppn as u64) << 12) | (dst_va as u64 & 0xFFF);
        eprintln!(
            "TLB: dst VA 0x{:08X} -> VPN {} -> PPN 0x{:X} flags 0x{:X} -> PA 0x{:X}",
            dst_va, dst_vpn, ppn, flags, pa
        );
    } else {
        eprintln!("TLB MISS for dst VA 0x{:08X}", dst_va);
    }

    // Check current x29 value (the write pointer)
    let x29 = vm.cpu.x[29];
    let x29_vpn = x29 >> 12;
    if let Some((ppn, flags)) = vm.cpu.tlb.lookup(x29_vpn, asid) {
        let pa = ((ppn as u64) << 12) | (x29 as u64 & 0xFFF);
        eprintln!(
            "TLB: x29 VA 0x{:08X} -> VPN {} -> PPN 0x{:X} flags 0x{:X} -> PA 0x{:X}",
            x29, x29_vpn, ppn, flags, pa
        );
    }

    // Check the source address 0xC0CBC3xx (a4)
    let a4 = vm.cpu.x[14];
    let a4_vpn = a4 >> 12;
    if let Some((ppn, flags)) = vm.cpu.tlb.lookup(a4_vpn, asid) {
        let pa = ((ppn as u64) << 12) | (a4 as u64 & 0xFFF);
        eprintln!(
            "TLB: a4  VA 0x{:08X} -> VPN {} -> PPN 0x{:X} flags 0x{:X} -> PA 0x{:X}",
            a4, a4_vpn, ppn, flags, pa
        );
    }

    // Manually walk the page table for x29
    let satp = vm.cpu.csr.satp;
    let pt_base = ((satp & 0x3FFFFF) as u64) << 12;
    let l1_idx = (x29 >> 22) & 0x3FF;
    let l1_pte = vm.bus.read_word(pt_base + (l1_idx as u64) * 4).unwrap_or(0);
    eprintln!("\nPage table walk for x29=0x{:08X}:", x29);
    eprintln!("  Root: 0x{:X}", pt_base);
    eprintln!(
        "  L1[{}] = 0x{:08X} (PA of PTE: 0x{:X})",
        l1_idx,
        l1_pte,
        pt_base + (l1_idx as u64) * 4
    );

    let l1_v = (l1_pte & mmu::PTE_V) != 0;
    let l1_r = (l1_pte & mmu::PTE_R) != 0;
    let l1_x = (l1_pte & mmu::PTE_X) != 0;
    let l1_w = (l1_pte & mmu::PTE_W) != 0;

    if l1_v && (l1_r || l1_x) {
        // Leaf (megapage)
        let l1_ppn = ((l1_pte) >> 10);
        let offset = x29 & 0x3FFFFF;
        let pa = ((l1_ppn as u64) << 12) | (offset as u64);
        eprintln!(
            "  L1 MEGAPAGE: PPN=0x{:X} offset=0x{:X} -> PA=0x{:X}",
            l1_ppn, offset, pa
        );
        eprintln!("  Expected identity-mapped PA: 0x{:X}", x29 as u64);
        eprintln!("  MATCH: {}", pa == x29 as u64);
    } else if l1_v {
        // Points to L2 table
        let l2_base = (((l1_pte) >> 10) as u64) << 12;
        let l2_idx = (x29 >> 12) & 0x3FF;
        let l2_pte = vm.bus.read_word(l2_base + (l2_idx as u64) * 4).unwrap_or(0);
        let l2_ppn = ((l2_pte) >> 10);
        let offset = x29 & 0xFFF;
        let pa = ((l2_ppn as u64) << 12) | (offset as u64);
        eprintln!(
            "  L2[{}] = 0x{:08X} (PA of PTE: 0x{:X})",
            l2_idx,
            l2_pte,
            l2_base + (l2_idx as u64) * 4
        );
        eprintln!(
            "  L2 PPN=0x{:X} offset=0x{:X} -> PA=0x{:X}",
            l2_ppn, offset, pa
        );
        eprintln!("  Expected identity-mapped PA: 0x{:X}", x29 as u64);
        eprintln!("  MATCH: {}", pa == x29 as u64);
    }

    // Also walk for dst
    let l1_idx_dst = (dst_va >> 22) & 0x3FF;
    let l1_pte_dst = vm
        .bus
        .read_word(pt_base + (l1_idx_dst as u64) * 4)
        .unwrap_or(0);
    eprintln!("\nPage table walk for dst=0x{:08X}:", dst_va);
    eprintln!("  L1[{}] = 0x{:08X}", l1_idx_dst, l1_pte_dst);
    if (l1_pte_dst & mmu::PTE_V) != 0
        && ((l1_pte_dst & mmu::PTE_R) != 0 || (l1_pte_dst & mmu::PTE_X) != 0)
    {
        let l1_ppn = ((l1_pte_dst) >> 10);
        let offset = dst_va & 0x3FFFFF;
        let pa = ((l1_ppn as u64) << 12) | (offset as u64);
        eprintln!("  L1 MEGAPAGE: PPN=0x{:X} -> PA=0x{:X}", l1_ppn, pa);
        eprintln!("  Expected: 0x{:X}", dst_va as u64);
        eprintln!("  MATCH: {}", pa == dst_va as u64);
    }
}
