fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::Privilege;
    use geometry_os::riscv::csr;

    // Boot to just before the corruption (~16.9M instructions)
    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let max_instr = 16_979_000u64;
    let mut count: u64 = 0;

    while count < max_instr {
        if vm.bus.sbi.shutdown_requested { break; }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
            if cause_code == csr::CAUSE_ECALL_S || cause_code == csr::CAUSE_ECALL_M {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0; vm.cpu.x[11] = a1;
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
        }
        let _ = vm.step();
        count += 1;
    }

    // Now check what virtual address 0xC1CCA520 translates to
    // And check L1[775] (the VPN[1] for the destination)
    let satp = vm.cpu.csr.read(csr::SATP);
    let ppn = satp & 0x3FFFFF;
    let pt_base = (ppn as u64) << 12;
    
    // Check several L1 entries around the corrupted one
    for idx in [769, 770, 771, 772, 775, 776] {
        let addr = pt_base + (idx as u64) * 4;
        let pte = vm.bus.read_word(addr).unwrap_or(0);
        let v = pte & 1;
        let rwx = (pte >> 1) & 7;
        let pte_ppn = pte >> 10;
        let is_leaf = rwx != 0;
        println!("L1[{}] = 0x{:08X} V={} RWX={} PPN=0x{:X} leaf={}", 
            idx, pte, v, rwx, pte_ppn, is_leaf);
    }
    
    // Translate 0xC1CCA520 manually
    let va = 0xC1CCA520u32;
    let vpn1 = ((va >> 22) & 0x3FF) as u64;
    let vpn0 = ((va >> 12) & 0x3FF) as u64;
    let offset = (va & 0xFFF) as u64;
    println!("\nTranslating VA 0x{:08X}: VPN1={} VPN0={} offset=0x{:X}", va, vpn1, vpn0, offset);
    
    // Read L1 entry
    let l1_addr = pt_base + vpn1 * 4;
    let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
    println!("L1[{}] at phys 0x{:X} = 0x{:08X}", vpn1, l1_addr, l1_pte);
    
    if (l1_pte & 1) != 0 && (l1_pte & 0xE) != 0 {
        // Megapage
        let ppn_hi = ((l1_pte >> 20) & 0xFFF) as u64;
        let pa = (ppn_hi << 22) | (vpn0 << 12) | offset;
        println!("  Megapage -> PA = 0x{:X}", pa);
    } else if (l1_pte & 1) != 0 {
        // Non-leaf, follow to L2
        let l2_ppn = ((l1_pte >> 10) & 0x3FFFFF) as u64;
        let l2_base = l2_ppn << 12;
        let l2_addr = l2_base + vpn0 * 4;
        let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
        println!("  L2 at phys 0x{:X} = 0x{:08X}", l2_addr, l2_pte);
        if (l2_pte & 1) != 0 {
            let l2_ppn_val = ((l2_pte >> 10) & 0x3FFFFF) as u64;
            let pa = (l2_ppn_val << 12) | offset;
            println!("  Leaf -> PA = 0x{:X}", pa);
        }
    }
    
    // Also translate the source 0xC1585D50
    let src = 0xC1585D50u32;
    let src_vpn1 = ((src >> 22) & 0x3FF) as u64;
    let src_vpn0 = ((src >> 12) & 0x3FF) as u64;
    let src_offset = (src & 0xFFF) as u64;
    println!("\nTranslating VA 0x{:08X}: VPN1={} VPN0={} offset=0x{:X}", src, src_vpn1, src_vpn0, src_offset);
    
    let src_l1_addr = pt_base + src_vpn1 * 4;
    let src_l1_pte = vm.bus.read_word(src_l1_addr).unwrap_or(0);
    println!("L1[{}] at phys 0x{:X} = 0x{:08X}", src_vpn1, src_l1_addr, src_l1_pte);
    
    if (src_l1_pte & 1) != 0 && (src_l1_pte & 0xE) != 0 {
        let ppn_hi = ((src_l1_pte >> 20) & 0xFFF) as u64;
        let pa = (ppn_hi << 22) | (src_vpn0 << 12) | src_offset;
        println!("  Megapage -> PA = 0x{:X}", pa);
    } else if (src_l1_pte & 1) != 0 {
        let l2_ppn = ((src_l1_pte >> 10) & 0x3FFFFF) as u64;
        let l2_base = l2_ppn << 12;
        let l2_addr = l2_base + src_vpn0 * 4;
        let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
        println!("  L2 at phys 0x{:X} = 0x{:08X}", l2_addr, l2_pte);
        if (l2_pte & 1) != 0 {
            let l2_ppn_val = ((l2_pte >> 10) & 0x3FFFFF) as u64;
            let pa = (l2_ppn_val << 12) | src_offset;
            println!("  Leaf -> PA = 0x{:X}", pa);
        }
    }
    
    // Check if the destination PA range overlaps with the PT at 0x1002C08
    println!("\nPage table physical range: 0x{:X} - 0x{:X}", pt_base, pt_base + 4096);
    println!("L1[770] physical addr: 0x{:X}", pt_base + 770 * 4);
    println!("\nPC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
}
