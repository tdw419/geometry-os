use geometry_os::riscv::{RiscvVm, cpu, csr, mmu};

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    // Get VM after setup, run exactly 178200 steps (just before the crash)
    let (mut vm, _br) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        178_200,
        "console=ttyS0 loglevel=8",
    ).unwrap();

    let satp = vm.cpu.csr.satp;
    let pg_dir_phys = ((satp & 0x3FFFFF) as u64) * 4096;
    
    // Check if early_pg_dir PTEs are correct (the kernel's own page table)
    println!("Kernel page table at PA 0x{:08X}:", pg_dir_phys);
    println!("\nL1 entries for kernel linear mapping (L1[768..776]):");
    for i in 768..776 {
        let pte_addr = pg_dir_phys + (i as u64) * 4;
        let pte = vm.bus.read_word(pte_addr).unwrap_or(0);
        let v = pte & 1;
        let rwx = (pte >> 1) & 7;
        let ppn = pte >> 10;
        
        // Expected: megapage mapping VA (0xC0000000 + (i-768)*4MB) to PA ((i-768)*4MB)
        let expected_pa = ((i - 768) as u64) * 0x400000;
        
        if v != 0 && rwx != 0 {
            // Megapage
            let ppn_hi = (ppn >> 10) & 0xFFF;
            let pa_base = (ppn_hi as u64) << 22;
            let status = if pa_base == expected_pa { "OK" } else { "WRONG" };
            println!("  L1[{}] = 0x{:08X} ppn_hi={} PA=0x{:08X} expected=0x{:08X} {}", 
                i, pte, ppn_hi, pa_base, expected_pa, status);
        } else if v != 0 {
            // Non-leaf: follow to L2
            let l2_base = (ppn as u64) << 12;
            let mut l2_ok = true;
            for j in 0..4 {
                let l2_pte = vm.bus.read_word(l2_base + (j as u64) * 4).unwrap_or(0);
                let l2_ppn = l2_pte >> 10;
                let l2_v = l2_pte & 1;
                if l2_v == 0 {
                    l2_ok = false;
                }
            }
            let status = if l2_ok { "OK (L2)" } else { "CHECK L2" };
            println!("  L1[{}] = 0x{:08X} -> L2 at PA 0x{:08X} {}", 
                i, pte, l2_base, status);
            
            // Show first few L2 entries
            for j in 0..4 {
                let l2_pte = vm.bus.read_word(l2_base + (j as u64) * 4).unwrap_or(0);
                let l2_ppn = l2_pte >> 10;
                let l2_v = l2_pte & 1;
                let l2_pa = (l2_ppn as u64) << 12;
                let exp_pa = expected_pa + (j as u64) * 0x1000;
                let s = if l2_pa == exp_pa { "OK" } else { "WRONG" };
                println!("    L2[{}] = 0x{:08X} PA=0x{:08X} expected=0x{:08X} {}", 
                    j, l2_pte, l2_pa, exp_pa, s);
            }
        } else {
            println!("  L1[{}] = 0x{:08X} NOT MAPPED", i, pte);
        }
    }
    
    // Check identity mappings for low addresses
    println!("\nL1 entries for low addresses (L1[0..8]):");
    for i in 0..8 {
        let pte_addr = pg_dir_phys + (i as u64) * 4;
        let pte = vm.bus.read_word(pte_addr).unwrap_or(0);
        let v = pte & 1;
        let rwx = (pte >> 1) & 7;
        let ppn = pte >> 10;
        if v != 0 && rwx != 0 {
            let ppn_hi = (ppn >> 10) & 0xFFF;
            let pa_base = (ppn_hi as u64) << 22;
            let expected_pa = (i as u64) * 0x400000;
            let status = if pa_base == expected_pa { "OK" } else { "WRONG" };
            println!("  L1[{}] = 0x{:08X} ppn_hi={} PA=0x{:08X} expected=0x{:08X} {}", 
                i, pte, ppn_hi, pa_base, expected_pa, status);
        } else if v != 0 {
            println!("  L1[{}] = 0x{:08X} -> L2", i, pte);
        } else {
            println!("  L1[{}] = NOT MAPPED", i);
        }
    }
    
    // Verify a specific translation: the crash site PC
    println!("\nTranslation verification:");
    let test_vas = [0xC003F9CA, 0xC0210F14, 0xC0400000, 0xC0800000, 0xC0C00000, 0xC1400000, 0xC1482000];
    let sum = (vm.cpu.csr.mstatus >> csr::MSTATUS_SUM) & 1 != 0;
    let mxr = (vm.cpu.csr.mstatus >> csr::MSTATUS_MXR) & 1 != 0;
    for &va in &test_vas {
        match mmu::translate(va, mmu::AccessType::Fetch, cpu::Privilege::Supervisor, sum, mxr, satp, &mut vm.bus, &mut vm.cpu.tlb) {
            mmu::TranslateResult::Ok(pa) => {
                let expected_pa = if va >= 0xC0000000 { (va - 0xC0000000) as u64 } else { va as u64 };
                let status = if pa == expected_pa { "OK" } else { "WRONG" };
                println!("  VA 0x{:08X} -> PA 0x{:08X} (expected 0x{:08X}) {}", va, pa, expected_pa, status);
            }
            mmu::TranslateResult::FetchFault => println!("  VA 0x{:08X} -> FETCH FAULT", va),
            _ => println!("  VA 0x{:08X} -> FAULT", va),
        }
    }
}
