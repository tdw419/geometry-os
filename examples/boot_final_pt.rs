// Diagnostic: check the FINAL page table (after 3rd SATP change)
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, _result) = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        512,
        20_000_000u64,
        bootargs,
    ).unwrap();
    
    // Final SATP is 0x80001485 -> PPN = 0x01485
    let satp = vm.cpu.csr.satp;
    let pg_dir_phys = ((satp & 0x3FFFFF) as u64) * 4096;
    println!("Final SATP: 0x{:08X}, page dir PA: 0x{:08X}", satp, pg_dir_phys);
    
    // Check kernel_map at this point
    let km_phys: u64 = 0x00C79E90;
    let km_pa = vm.bus.read_word(km_phys + 12).unwrap_or(0);
    let km_vapo = vm.bus.read_word(km_phys + 20).unwrap_or(0);
    let km_vkpo = vm.bus.read_word(km_phys + 24).unwrap_or(0);
    println!("kernel_map: pa=0x{:08X} vapo=0x{:08X} vkpo=0x{:08X}", km_pa, km_vapo, km_vkpo);
    
    // Check L1 entries 768-864 of the FINAL page table
    println!("\nFinal page table L1[768-864]:");
    let mut bad_count = 0;
    for i in 768..864 {
        let addr = pg_dir_phys + (i as u64) * 4;
        let entry = vm.bus.read_word(addr).unwrap_or(0);
        if entry != 0 {
            let ppn = (entry >> 10) & 0x3FFFFF;
            let is_leaf = (entry & 0xE) != 0;
            let v = (entry & 1) != 0;
            
            if is_leaf {
                // Megapage: ppn_hi = PPN[19:10]
                let ppn_hi = (ppn >> 10) & 0xFFF;
                let expected_pa = ((i - 768) as u64) * 0x200000;
                let actual_pa = (ppn_hi as u64) << 22;
                
                if actual_pa != expected_pa {
                    println!("  L1[{}] = 0x{:08X} ppn_hi={} PA=0x{:08X} EXPECTED=0x{:08X} *** WRONG ***",
                        i, entry, ppn_hi, actual_pa, expected_pa);
                    bad_count += 1;
                    if bad_count > 10 {
                        println!("  ... (stopping after 10 wrong entries)");
                        break;
                    }
                }
            } else {
                // Non-leaf: points to L2 table
                let l2_base = (ppn as u64) * 4096;
                // Check first L2 entry
                let l2_0 = vm.bus.read_word(l2_base).unwrap_or(0);
                let l2_ppn = (l2_0 >> 10) & 0x3FFFFF;
                let expected_pa = ((i - 768) as u64) * 0x200000;
                let actual_pa = (l2_ppn as u64) * 4096;
                
                if actual_pa != expected_pa && (l2_0 & 1) != 0 {
                    let l2_ppn_hi = (l2_ppn >> 10) & 0xFFF;
                    // For 4KB pages, the first L2 entry should map to
                    // PA = (i-768) * 2MB
                    let exp_l2_ppn = expected_pa / 4096;
                    if (l2_ppn as u64) != exp_l2_ppn {
                        println!("  L1[{}] -> L2 at PA 0x{:08X}, L2[0]=0x{:08X} ppn=0x{:06X} exp_ppn=0x{:06X} *** WRONG ***",
                            i, l2_base, l2_0, l2_ppn, exp_l2_ppn);
                        bad_count += 1;
                        if bad_count > 10 {
                            println!("  ... (stopping after 10 wrong entries)");
                            break;
                        }
                    }
                }
            }
        }
    }
    if bad_count == 0 {
        println!("  All entries correct!");
    }
    
    // Also check: are there any L2 tables created by the kernel that have wrong PPNs?
    // Check the trampoline_pg_dir at PA 0x01484000
    let tramp_pa = 0x01484000u64;
    println!("\nTrampoline pg_dir at PA 0x{:08X} (created by kernel):", tramp_pa);
    for i in 768..800 {
        let entry = vm.bus.read_word(tramp_pa + (i as u64) * 4).unwrap_or(0);
        if entry != 0 {
            let ppn = (entry >> 10) & 0x3FFFFF;
            let is_leaf = (entry & 0xE) != 0;
            if !is_leaf && ppn >= 0xC0000 {
                println!("  L1[{}] = 0x{:08X} NON-LEAF PPN=0x{:06X} *** VIRTUAL PPN ***",
                    i, entry, ppn);
            }
        }
    }
}
