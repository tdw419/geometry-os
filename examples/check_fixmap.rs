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
        "loglevel=0 quiet",
    ).unwrap();

    // swapper_pg_dir at PA 0x802000 - check L1[0xFF]
    let swapper_pa: u64 = 0x802000;
    let l1_ff = vm.bus.read_word(swapper_pa + 0xFF * 4).unwrap_or(0xDEAD);
    eprintln!("swapper_pg_dir L1[0xFF] = 0x{:08X}", l1_ff);
    
    // Check all non-zero L1 entries in swapper_pg_dir
    eprintln!("
All non-zero entries in swapper_pg_dir (PA 0x802000):");
    for i in 0..1024u32 {
        let pte = vm.bus.read_word(swapper_pa + i as u64 * 4).unwrap_or(0);
        if pte != 0 {
            let ppn = (pte & 0xFFFFFC00) >> 10;
            let pa = (ppn as u64) << 12;
            let is_leaf = pte & 0xE != 0;
            eprintln!("  L1[0x{:03X}] = 0x{:08X} ppn=0x{:05X} PA=0x{:08X} {}", 
                i, pte, ppn, pa, if is_leaf { "leaf" } else { "ptr" });
        }
    }

    // Check early_pg_dir -- where is it?
    // From the symbol table: early_pg_dir is not exported but it's at PA 0x802000
    // Actually, swapper_pg_dir IS early_pg_dir initially (setup_vm uses it)
    // setup_vm_final creates a new swapper_pg_dir... but for RV32, 
    // the #if !CONFIG_64BIT copies the FDT entry:
    // set_pgd(&swapper_pg_dir[idx], early_pg_dir[idx]);
    // Then creates fixmap mapping in swapper_pg_dir.
    
    // Check fixmap_pte table
    let fixmap_pte_pa: u64 = 0x1483000;
    eprintln!("
fixmap_pte (PA 0x{:08X}) first 32 entries:", fixmap_pte_pa);
    for i in 0..32u32 {
        let pte = vm.bus.read_word(fixmap_pte_pa + i as u64 * 4).unwrap_or(0);
        if pte != 0 {
            eprintln!("  [{}] = 0x{:08X}", i, pte);
        }
    }

    // Also check: what SATP value is currently set?
    let satp = vm.cpu.csr.satp;
    let ppn = satp & 0x3FFFFF;
    eprintln!("
Current SATP=0x{:08X} ppn=0x{:X} pgdir_PA=0x{:08X}", satp, ppn, ppn * 4096);
}
