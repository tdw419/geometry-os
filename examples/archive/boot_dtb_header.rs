use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi loglevel=8",
    )
    .unwrap();

    // Read all header fields (big-endian)
    let mut read_be32 = |off: u64| -> u32 {
        let w = vm.bus.read_word(dtb_addr + off).unwrap() as u32;
        u32::from_be(w)
    };

    eprintln!("DTB Header:");
    eprintln!("  magic:           0x{:08X}", read_be32(0x00));
    eprintln!(
        "  totalsize:       0x{:08X} ({})",
        read_be32(0x04),
        read_be32(0x04)
    );
    eprintln!("  off_dt_struct:   0x{:08X}", read_be32(0x08));
    eprintln!("  off_dt_strings:  0x{:08X}", read_be32(0x0C));
    eprintln!("  off_mem_rsvmap:  0x{:08X}", read_be32(0x10));
    eprintln!(
        "  version:         0x{:08X} ({})",
        read_be32(0x14),
        read_be32(0x14)
    );
    eprintln!(
        "  last_comp_ver:   0x{:08X} ({})",
        read_be32(0x18),
        read_be32(0x18)
    );
    eprintln!("  boot_cpuid:      0x{:08X}", read_be32(0x1C));
    eprintln!(
        "  sz_dt_strings:   0x{:08X} ({})",
        read_be32(0x20),
        read_be32(0x20)
    );
    eprintln!(
        "  sz_dt_struct:    0x{:08X} ({})",
        read_be32(0x24),
        read_be32(0x24)
    );

    // Validate
    eprintln!("\nValidation:");
    let magic = read_be32(0x00);
    eprintln!("  magic == FDT_MAGIC (0xD00DFEED): {}", magic == 0xD00DFEED);
    let version = read_be32(0x14);
    let last_comp = read_be32(0x18);
    eprintln!(
        "  version ({}) >= FDT_FIRST_SUPPORTED (2): {}",
        version,
        version >= 2
    );
    eprintln!(
        "  last_comp ({}) <= FDT_LAST_SUPPORTED (17): {}",
        last_comp,
        last_comp <= 17
    );

    let totalsize = read_be32(0x04);
    let hdrsize = if version >= 17 {
        40
    } else if version >= 16 {
        36
    } else if version >= 3 {
        28
    } else {
        20
    };
    eprintln!("  hdrsize: {}", hdrsize);
    eprintln!(
        "  totalsize ({}) >= hdrsize ({}): {}",
        totalsize,
        hdrsize,
        totalsize >= hdrsize
    );

    // Check struct block
    let struct_off = read_be32(0x08) as u64;
    let struct_size = read_be32(0x24) as u64;
    eprintln!(
        "  struct_off + struct_size = 0x{:X} <= totalsize = 0x{:X}: {}",
        struct_off + struct_size,
        totalsize,
        struct_off + struct_size <= totalsize as u64
    );
}
