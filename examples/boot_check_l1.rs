use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel = std::fs::read(".geometry_os/build/linux-6.14/vmlinux").unwrap();
    let initramfs = std::fs::read(".geometry_os/fs/linux/rv32/initramfs.cpio.gz").unwrap();
    
    let (mut vm, _result) = RiscvVm::boot_linux(
        &kernel,
        Some(&initramfs),
        64,
        200_000,
        "console=ttyS0 earlycon=sbi",
    ).unwrap();
    
    let satp = vm.cpu.csr.satp;
    let mode = (satp >> 31) & 1;
    let ppn = satp & 0x3FFFFF;
    let pg_dir_phys = (ppn as u64) * 4096;
    
    println!("SATP=0x{:08X} mode={} pg_dir_phys=0x{:08X}", satp, mode, pg_dir_phys);
    
    println!("\nL1 entries (kernel linear mapping 768-780):");
    for i in 768..=780 {
        let addr = pg_dir_phys + (i as u64) * 4;
        let entry = vm.bus.read_word(addr).unwrap_or(0);
        let v = (entry >> 0) & 1;
        let ppn0 = (entry >> 20) & 0xFFF;
        let ppn1 = (entry >> 10) & 0x3FF;
        let is_leaf = (entry & 0xE) != 0;
        let expected_pa = if i >= 768 { (i - 768) * 0x400000 } else { i * 0x400000 };
        println!("  L1[{}] = 0x{:08X} V={} ppn1={} ppn0={} leaf={} (expected PA=0x{:08X})",
            i, entry, v, ppn1, ppn0, is_leaf, expected_pa);
        
        if v == 1 && !is_leaf && ppn0 > 0 {
            let l2_base = (ppn0 as u64) * 4096;
            let mut non_zero = 0u32;
            for j in 0..512u64 {
                let l2_addr = l2_base + j * 4;
                let l2_entry = vm.bus.read_word(l2_addr).unwrap_or(0);
                if l2_entry != 0 {
                    non_zero += 1;
                    if non_zero <= 3 {
                        println!("    L2[{}] = 0x{:08X}", j, l2_entry);
                    }
                }
            }
            println!("    ({} non-zero L2 entries)", non_zero);
        }
    }
    
    // Read fixmap_pte directly
    let fixmap_pa = 0x01483000u64;
    let val = vm.bus.read_word(fixmap_pa).unwrap_or(0);
    println!("\nfixmap_pte at PA 0x{:08X}: 0x{:08X}", fixmap_pa, val);
}
