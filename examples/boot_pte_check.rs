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

    // Use the same PPN extraction as the MMU code
    let satp = vm.cpu.csr.satp;
    let ppn = satp & 0x003F_FFFF; // satp_ppn() implementation
    let asid = ((satp >> 22) & 0x1FF) as u16; // satp_asid() implementation
    let pt_root = (ppn as u64) << 12;
    println!("SATP=0x{:08X} PPN=0x{:06X} ASID={} root=0x{:08X}", satp, ppn, asid, pt_root);

    // Check L1 entry for the faulting VA (0xC08BDFFCu32, VPN2=770)
    let l1_idx = 770;
    let l1_pte_addr = pt_root + (l1_idx as u64) * 4;
    let l1_pte = vm.bus.read_word(l1_pte_addr).unwrap_or(0);
    println!("L1[770] at PA=0x{:08X} = 0x{:08X}", l1_pte_addr, l1_pte);
    let l1_v = l1_pte & 1;
    let l1_rwx = (l1_pte >> 1) & 7;
    println!("  V={} RWX={:03b}", l1_v, l1_rwx);
    
    if l1_v != 0 && l1_rwx == 0 {
        // Non-leaf: points to L2 table
        let l2_base = ((l1_pte as u64) >> 10) << 12;
        println!("  L2 base PA: 0x{:08X}", l2_base);
        let l2_idx = (0xC08BDFFCu32 >> 12) & 0x3FF; // VPN1 = 189
        let l2_pte_addr = l2_base + (l2_idx as u64) * 4;
        let l2_pte = vm.bus.read_word(l2_pte_addr).unwrap_or(0);
        println!("  L2[{}] at PA=0x{:08X} = 0x{:08X}", l2_idx, l2_pte_addr, l2_pte);
        let l2_v = l2_pte & 1;
        let l2_rwx = (l2_pte >> 1) & 7;
        println!("  V={} RWX={:03b}", l2_v, l2_rwx);
        let pa = ((l2_pte as u64 >> 10) << 12) | (0xC08BDFFCu32 & 0xFFF) as u64;
        println!("  Translated PA: 0x{:08X}", pa);
    } else if l1_v != 0 {
        // Leaf (megapage)
        let ppn_hi = (l1_pte >> 20) & 0xFFF;
        let vpn0 = (0xC08BDFFCu32 >> 12) & 0x3FF;
        let pa = ((ppn_hi as u64) << 22) | ((vpn0 as u64) << 12) | (0xC08BDFFCu32 & 0xFFF) as u64;
        println!("  Megapage PA: 0x{:08X}", pa);
    }

    // Now check: what does the kernel's page table look like for the kernel code region?
    // The kernel code is at 0xC0000000-0xC08F012E (LOAD[0])
    // VPN2 for 0xC0000000 = 768 (0x300)
    println!("\nL1 entries for kernel code region (VPN2 768-771):");
    for i in 768..772 {
        let pte_addr = pt_root + (i as u64) * 4;
        let pte = vm.bus.read_word(pte_addr).unwrap_or(0);
        let v = pte & 1;
        let rwx = (pte >> 1) & 7;
        if v != 0 {
            let is_leaf = rwx != 0;
            if is_leaf {
                let ppn_hi = (pte >> 20) & 0xFFF;
                println!("  L1[{}] = 0x{:08X} V=1 LEAF megapage PPN_hi=0x{:03X} RWX={:03b}", 
                    i, pte, ppn_hi, rwx);
            } else {
                let l2_base = ((pte as u64) >> 10) << 12;
                println!("  L1[{}] = 0x{:08X} V=1 NON-LEAF L2 at 0x{:08X}", i, pte, l2_base);
            }
        } else {
            println!("  L1[{}] = 0x{:08X} V=0 (not present)", i, pte);
        }
    }

    // Check: is the crash caused by the memmove overwriting the page table?
    // Read L1[770] BEFORE the crash (at 16M instructions)
    println!("\n--- Checking at 16M instructions (before crash) ---");
    let (mut vm2, _) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        16_000_000,  // before the crash at ~17M
        bootargs,
    )
    .unwrap();
    let satp2 = vm2.cpu.csr.satp;
    let ppn2 = satp2 & 0x003F_FFFF;
    let pt_root2 = (ppn2 as u64) << 12;
    let l1_770_before = vm2.bus.read_word(pt_root2 + 770 * 4).unwrap_or(0);
    let l1_769_before = vm2.bus.read_word(pt_root2 + 769 * 4).unwrap_or(0);
    let l1_768_before = vm2.bus.read_word(pt_root2 + 768 * 4).unwrap_or(0);
    println!("L1[768] before crash = 0x{:08X}", l1_768_before);
    println!("L1[769] before crash = 0x{:08X}", l1_769_before);
    println!("L1[770] before crash = 0x{:08X}", l1_770_before);

    // Now check AFTER the crash
    println!("\n--- After crash (20M instructions) ---");
    let l1_770_after = vm.bus.read_word(pt_root + 770 * 4).unwrap_or(0);
    let l1_769_after = vm.bus.read_word(pt_root + 769 * 4).unwrap_or(0);
    let l1_768_after = vm.bus.read_word(pt_root + 768 * 4).unwrap_or(0);
    println!("L1[768] after crash = 0x{:08X}", l1_768_after);
    println!("L1[769] after crash = 0x{:08X}", l1_769_after);
    println!("L1[770] after crash = 0x{:08X}", l1_770_after);

    if l1_770_before != l1_770_after {
        println!("\n*** L1[770] CHANGED! Corruption detected ***");
        println!("  Before: 0x{:08X}", l1_770_before);
        println!("  After:  0x{:08X}", l1_770_after);
    }
}
