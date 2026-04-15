fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::mmu;
    use geometry_os::riscv::cpu::Privilege;

    // Run boot for enough instructions to hit the fault
    let (mut vm, _) = RiscvVm::boot_linux(&kernel_image, initramfs.as_deref(), 256, 20_000_000, bootargs).unwrap();

    let satp = vm.cpu.csr.read(geometry_os::riscv::csr::SATP);
    println!("After boot: PC=0x{:08X} priv={:?} satp=0x{:08X}", vm.cpu.pc, vm.cpu.privilege, satp);
    println!("stvec=0x{:08X} scause=0x{:08X} sepc=0x{:08X} stval=0x{:08X}",
        vm.cpu.csr.stvec, vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval);

    // Try to translate handle_exception address
    let he_va: u32 = 0xC08EFF1C;
    println!("\n=== Translating handle_exception VA 0x{:08X} ===", he_va);
    let mut tlb = mmu::Tlb::new();
    let sum = (vm.cpu.csr.mstatus >> geometry_os::riscv::csr::MSTATUS_SUM) & 1 != 0;
    let mxr = (vm.cpu.csr.mstatus >> geometry_os::riscv::csr::MSTATUS_MXR) & 1 != 0;
    println!("SUM={} MXR={}", sum, mxr);

    match mmu::translate(he_va, mmu::AccessType::Fetch, Privilege::Supervisor, sum, mxr, satp, &mut vm.bus, &mut tlb) {
        mmu::TranslateResult::Ok(pa) => println!("  Fetch -> OK, PA=0x{:08X}", pa),
        mmu::TranslateResult::FetchFault => println!("  Fetch -> FetchFault!"),
        other => println!("  Fetch -> {:?}", other),
    }

    // Try __memmove address
    let memmove_va: u32 = 0xC08E5D6A;
    println!("\n=== Translating __memmove VA 0x{:08X} ===", memmove_va);
    match mmu::translate(memmove_va, mmu::AccessType::Fetch, Privilege::Supervisor, sum, mxr, satp, &mut vm.bus, &mut tlb) {
        mmu::TranslateResult::Ok(pa) => println!("  Fetch -> OK, PA=0x{:08X}", pa),
        other => println!("  Fetch -> {:?}", other),
    }

    // Try the faulting load address from __memmove
    let load_va: u32 = 0xC08BDFFC;
    println!("\n=== Translating __memmove load target VA 0x{:08X} ===", load_va);
    match mmu::translate(load_va, mmu::AccessType::Load, Privilege::Supervisor, sum, mxr, satp, &mut vm.bus, &mut tlb) {
        mmu::TranslateResult::Ok(pa) => println!("  Load -> OK, PA=0x{:08X}", pa),
        other => println!("  Load -> {:?}", other),
    }

    // Dump page table walk for handle_exception
    let mode = satp >> 31;
    let ppn = satp & 0x3FFFFF;
    let pt_base = (ppn as u64) << 12;

    let vpn1 = (he_va >> 22) & 0x3FF;
    let vpn0 = (he_va >> 12) & 0x3FF;
    println!("\n=== Page table walk for 0x{:08X} ===", he_va);
    println!("Root PT at PA=0x{:08X}, VPN[1]={} VPN[0]={}", pt_base, vpn1, vpn0);

    let l1_addr = pt_base + (vpn1 as u64) * 4;
    let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
    let l1_valid = (l1_pte & 1) != 0;
    let l1_leaf = (l1_pte & 0xE) != 0;
    let l1_ppn = (l1_pte >> 10) & 0x3FFFFF;
    println!("L1[{}] at PA=0x{:08X}: 0x{:08X} valid={} leaf={} ppn=0x{:05X}", vpn1, l1_addr, l1_pte, l1_valid, l1_leaf, l1_ppn);
    println!("  flags: V={} R={} W={} X={} U={} A={} D={} G={}",
        l1_pte & 1, (l1_pte>>1)&1, (l1_pte>>2)&1, (l1_pte>>3)&1,
        (l1_pte>>4)&1, (l1_pte>>6)&1, (l1_pte>>7)&1, (l1_pte>>5)&1);

    if l1_valid && !l1_leaf {
        let l2_base = (l1_ppn as u64) << 12;
        let l2_addr = l2_base + (vpn0 as u64) * 4;
        let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
        let l2_valid = (l2_pte & 1) != 0;
        let l2_leaf = (l2_pte & 0xE) != 0;
        let l2_ppn = (l2_pte >> 10) & 0x3FFFFF;
        println!("L2[{}] at PA=0x{:08X}: 0x{:08X} valid={} leaf={} ppn=0x{:05X}", vpn0, l2_addr, l2_pte, l2_valid, l2_leaf, l2_ppn);
        println!("  flags: V={} R={} W={} X={} U={} A={} D={} G={}",
            l2_pte & 1, (l2_pte>>1)&1, (l2_pte>>2)&1, (l2_pte>>3)&1,
            (l2_pte>>4)&1, (l2_pte>>6)&1, (l2_pte>>7)&1, (l2_pte>>5)&1);

        // The PTE for handle_exception: does it have X (execute) permission?
        if l2_leaf {
            if (l2_pte & (1 << 3)) == 0 {
                println!("  *** BUG: Page is mapped but NOT executable (X=0)! ***");
            }
        }
    }

    // Also check what the L2 table looks like around VPN[0] for the faulting page
    if l1_valid && !l1_leaf {
        let l2_base = (l1_ppn as u64) << 12;
        println!("\nL2 entries around VPN[0]={} (handle_exception page):", vpn0);
        for i in vpn0.saturating_sub(2)..=vpn0+2 {
            let l2_addr = l2_base + (i as u64) * 4;
            let pte = vm.bus.read_word(l2_addr).unwrap_or(0);
            let pa = if (pte >> 10) & 0x3FFFFF != 0 { ((pte >> 10) & 0x3FFFFF) << 12 } else { 0 };
            println!("  L2[{}]: 0x{:08X} -> PA=0x{:08X} V={} R={} W={} X={} U={}",
                i, pte, pa, pte&1, (pte>>1)&1, (pte>>2)&1, (pte>>3)&1, (pte>>4)&1);
        }
    }
}
