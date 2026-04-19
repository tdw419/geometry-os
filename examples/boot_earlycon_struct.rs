use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi loglevel=8",
    )
    .unwrap();

    // Run to after earlycon setup (should be around 390K instructions)
    for _ in 0..500_000 {
        if vm.bus.sbi.shutdown_requested { break; }
        let _ = vm.step();
    }

    // Read earlycon_console struct at PA 0x00804368
    let earlycon_pa: u64 = 0x00804368;
    eprintln!("earlycon_console at PA 0x{:08X}:", earlycon_pa);
    let mut offset = 0u64;
    while offset < 0x80 {
        let val = vm.bus.read_word(earlycon_pa + offset).unwrap_or(0);
        eprintln!("  +{}: 0x{:08X}", offset, val);
        offset += 4;
    }

    // Check what sbi_dbcn_console_write was registered as
    // The write function pointer should be at offset 16 of the console struct (struct console.write)
    // Let me also dump the UART output and SBI console output
    eprintln!("\nSBI console output: {} bytes", vm.bus.sbi.console_output.len());
    let out = String::from_utf8_lossy(&vm.bus.sbi.console_output);
    eprintln!("  \"{}\"", out.replace(|c: char| !c.is_ascii_graphic() && c != ' ' && c != '\n', "?"));
}
