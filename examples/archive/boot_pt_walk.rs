fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::mmu;
    use geometry_os::riscv::RiscvVm;

    let (mut vm, _) =
        RiscvVm::boot_linux(&kernel_image, initramfs.as_deref(), 256, 500_000, bootargs).unwrap();

    let satp = vm.cpu.csr.read(geometry_os::riscv::csr::SATP);
    let mode = satp >> 31;
    let asid = (satp >> 22) & 0x1FF;
    let ppn = satp & 0x3FFFFF;
    println!(
        "SATP: 0x{:08X} mode={} asid={} ppn=0x{:X} (phys=0x{:08X})",
        satp,
        mode,
        asid,
        ppn,
        ppn << 12
    );
    println!("PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);

    // Try to translate the faulting address
    let fault_va: u32 = 0x003A4F9C;
    println!(
        "\nAttempting to translate faulting VA 0x{:08X}...",
        fault_va
    );
    let mut tlb = mmu::Tlb::new();
    match mmu::translate(
        fault_va,
        mmu::AccessType::Fetch,
        geometry_os::riscv::cpu::Privilege::Supervisor,
        false,
        false,
        satp,
        &mut vm.bus,
        &mut tlb,
    ) {
        mmu::TranslateResult::Ok(pa) => println!("  -> OK, PA=0x{:08X}", pa),
        mmu::TranslateResult::FetchFault => println!("  -> FetchFault"),
        other => println!("  -> {:?}", other),
    }

    // Try kernel address
    let kernel_va: u32 = 0xC0000000;
    println!("\nKernel VA 0x{:08X}...", kernel_va);
    match mmu::translate(
        kernel_va,
        mmu::AccessType::Fetch,
        geometry_os::riscv::cpu::Privilege::Supervisor,
        false,
        false,
        satp,
        &mut vm.bus,
        &mut tlb,
    ) {
        mmu::TranslateResult::Ok(pa) => println!("  -> OK, PA=0x{:08X}", pa),
        other => println!("  -> {:?}", other),
    }

    // Dump page table
    let pt_base = (ppn as u64) << 12;
    println!("\nFirst 16 L1 PTEs at PA=0x{:08X}:", pt_base);
    for i in 0..16 {
        let pte = vm.bus.read_word(pt_base + i * 4).unwrap_or(0);
        let valid = (pte & 1) != 0;
        let is_leaf = (pte & 0xE) != 0;
        let pte_ppn = (pte >> 10) & 0x3FFFFF;
        println!(
            "  L1[{:2}]: 0x{:08X} valid={} leaf={} ppn=0x{:05X} flags={:03b}",
            i,
            pte,
            valid,
            is_leaf,
            pte_ppn,
            pte & 0xE
        );
    }

    // The faulting VA is 0x003A4F9C. VPN[1] = bits [31:22] = 0
    // So L1 index is 0. Let's check L1[0] and follow it
    let l1_0 = vm.bus.read_word(pt_base).unwrap_or(0);
    let l1_0_ppn = (l1_0 >> 10) & 0x3FFFFF;
    let l2_base = (l1_0_ppn as u64) << 12;
    println!("\nL1[0] -> L2 table at PA=0x{:08X}:", l2_base);
    let vpn0 = (fault_va >> 12) & 0x3FF; // VPN[0]
    let vpn1 = (fault_va >> 22) & 0x3FF; // VPN[1]
    println!("  Fault VA VPN[1]={} VPN[0]={}", vpn1, vpn0);
    for i in 0..16 {
        let pte = vm.bus.read_word(l2_base + i * 4).unwrap_or(0);
        println!("  L2[{:3}]: 0x{:08X}", i, pte);
    }
}
