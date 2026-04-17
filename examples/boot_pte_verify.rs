use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _br) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        200_000,
        "console=ttyS0 loglevel=8",
    ).unwrap();

    let satp = vm.cpu.csr.satp;
    let pg_dir_phys = ((satp & 0x3FFFFF) as u64) * 4096;
    
    println!("SATP=0x{:08X} pg_dir at PA 0x{:08X}", satp, pg_dir_phys);
    
    // For each L1 entry 768-773, check if the megapage mapping is correct
    // Correct: VA 0xC0000000 + N*2MB should map to PA 0 + N*2MB
    println!("\nChecking linear mapping PTEs:");
    for i in 768..774 {
        let pte_addr = pg_dir_phys + (i as u64) * 4;
        let pte = vm.bus.read_word(pte_addr).unwrap_or(0);
        
        // For megapage: PA = PTE[29:20] << 20 | VA[21:0]
        let pte_ppn = (pte >> 20) & 0x3FF;  // bits [29:20]
        let expected_pa_base = (i as u32 - 768) << 21;  // (i-768)*2MB
        
        let va_start = 0xC0000000u32 + ((i as u32 - 768) << 21);
        let actual_pa_base = pte_ppn << 20;
        
        let status = if actual_pa_base == expected_pa_base { "OK" } else { "WRONG" };
        
        println!("  L1[{}] PTE=0x{:08X} pte_ppn=0x{:03X} pa_base=0x{:08X} expected=0x{:08X} {}",
            i, pte, pte_ppn, actual_pa_base, expected_pa_base, status);
        
        // Verify by reading: the word at VA should match the word at PA
        if actual_pa_base != 0 && (pte & 1) != 0 {
            let pa_word = vm.bus.read_word(actual_pa_base as u64).unwrap_or(0);
            // We can't easily read through the MMU from here, so skip cross-check
            let _ = pa_word;
        }
    }
    
    // Also check the trampoline_pg_dir (0x80001484)
    let tramp_phys = 0x01484000u64;
    println!("\nTrampoline pg_dir at PA 0x{:08X}:", tramp_phys);
    for i in 768..774 {
        let pte_addr = tramp_phys + (i as u64) * 4;
        let pte = vm.bus.read_word(pte_addr).unwrap_or(0);
        let pte_ppn = (pte >> 20) & 0x3FF;
        let expected_pa_base = (i as u32 - 768) << 21;
        let actual_pa_base = pte_ppn << 20;
        let status = if actual_pa_base == expected_pa_base { "OK" } else { "WRONG" };
        println!("  L1[{}] PTE=0x{:08X} pte_ppn=0x{:03X} pa_base=0x{:08X} expected=0x{:08X} {}",
            i, pte, pte_ppn, actual_pa_base, expected_pa_base, status);
    }
    
    // Check the boot page table (0x8000157A)
    let boot_phys = 0x0157A000u64;
    println!("\nBoot pg_dir at PA 0x{:08X}:", boot_phys);
    for i in 768..774 {
        let pte_addr = boot_phys + (i as u64) * 4;
        let pte = vm.bus.read_word(pte_addr).unwrap_or(0);
        let pte_ppn = (pte >> 20) & 0x3FF;
        let expected_pa_base = (i as u32 - 768) << 21;
        let actual_pa_base = pte_ppn << 20;
        let status = if actual_pa_base == expected_pa_base { "OK" } else { "WRONG" };
        println!("  L1[{}] PTE=0x{:08X} pte_ppn=0x{:03X} pa_base=0x{:08X} expected=0x{:08X} {}",
            i, pte, pte_ppn, actual_pa_base, expected_pa_base, status);
    }
}
