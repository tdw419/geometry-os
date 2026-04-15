fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 panic=1";

    use geometry_os::riscv::RiscvVm;

    let (mut vm, _) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        20_000_000,
        bootargs,
    )
    .unwrap();

    // Show all SATP writes
    println!("SATP writes:");
    for event in vm.bus.mmu_log.iter() {
        if let geometry_os::riscv::mmu::MmuEvent::SatpWrite { old, new } = event {
            println!("  SATP: 0x{:08X} -> 0x{:08X}", old, new);
            let mode = (new >> 31) & 1;
            let ppn = (new >> 22) & 0x1FF;
            let asid = new & 0x3FFFFF;
            println!("    MODE={} PPN={} (root=0x{:08X}) ASID={}", mode, ppn, ppn << 12, asid);
        }
    }

    // Check first few page table walks
    println!("\nFirst 10 page table walks:");
    let mut count = 0;
    for event in vm.bus.mmu_log.iter() {
        if let geometry_os::riscv::mmu::MmuEvent::PageTableWalk { va, pa, ptes } = event {
            println!("  VA=0x{:08X} -> PA=0x{:08X} PTEs={:?}", va, pa, ptes);
            count += 1;
            if count >= 10 { break; }
        }
    }

    // Check what the kernel put at the page table root
    // Read first few entries of the page table
    let satp = vm.cpu.csr.satp;
    let ppn = (satp >> 22) & 0x1FF;
    let pt_root = (ppn as u64) << 12;
    println!("\nPage table root at PA=0x{:08X}", pt_root);
    for i in 0..16 {
        let pte_addr = pt_root + (i as u64) * 4;
        let pte = vm.bus.read_word(pte_addr).unwrap_or(0);
        if pte != 0 {
            println!("  L1[{}] = 0x{:08X} (V={} RWX={:03b} PPN=0x{:08X})",
                i, pte, pte & 1, (pte >> 1) & 7, (pte >> 10) << 12);
        }
    }

    // Check L1 entry for the faulting VA (VPN2=770)
    let l1_idx = 770;
    let l1_pte_addr = pt_root + (l1_idx as u64) * 4;
    let l1_pte = vm.bus.read_word(l1_pte_addr).unwrap_or(0);
    println!("\nL1[770] at PA=0x{:08X} = 0x{:08X}", l1_pte_addr, l1_pte);
    let l1_ppn = (l1_pte >> 10) << 2; // bits [31:10] shifted right by 10, gives physical page number
    let l2_base = (l1_pte as u64 >> 10) << 12;
    println!("  L1 PPN bits: 0x{:08X}, L2 base PA: 0x{:08X}", l1_ppn, l2_base);
    
    // Check L2 entry for the faulting VA (VPN1=189)
    let l2_idx = 189;
    let l2_pte_addr = l2_base + (l2_idx as u64) * 4;
    let l2_pte = vm.bus.read_word(l2_pte_addr).unwrap_or(0);
    println!("  L2[{}] at PA=0x{:08X} = 0x{:08X}", l2_idx, l2_pte_addr, l2_pte);
}
