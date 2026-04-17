use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _, _, _) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    ).unwrap();
    
    // Check memory BEFORE running any instructions
    eprintln!("=== Memory at key VAs before execution ===");
    // The kernel entry is at 0xC0000000, let's check some key regions
    for pa in [0u64, 0x10B2, 0x10B8, 0x10000, 0x100000, 0x200000, 0x400000] {
        let w = vm.bus.read_word(pa).unwrap_or(0xDEAD);
        eprintln!("PA 0x{:08X} (VA 0x{:08X}): 0x{:08X}", pa, pa + 0xC0000000, w);
    }
    
    // Now check the hot PCs from the trace
    // Those were at VA 0xC020B55E etc which maps to PA 0x20B55E etc
    // Let's check before running
    for pa in [0x3B354u64, 0x20B55E, 0x3038, 0x3A632] {
        let w = vm.bus.read_word(pa).unwrap_or(0xDEAD);
        eprintln!("PA 0x{:08X} (VA 0x{:08X}): 0x{:08X}", pa, pa + 0xC0000000, w);
    }
    
    // Run 5M instructions
    for _ in 0..5_000_000 {
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
    }
    
    eprintln!("\n=== Memory AFTER 5M instructions ===");
    for pa in [0u64, 0x10B2, 0x10B8, 0x10000, 0x100000, 0x200000, 0x400000, 0x3B354, 0x20B55E, 0x3038, 0x3A632] {
        let w = vm.bus.read_word(pa).unwrap_or(0xDEAD);
        eprintln!("PA 0x{:08X} (VA 0x{:08X}): 0x{:08X}", pa, pa + 0xC0000000, w);
    }
}
