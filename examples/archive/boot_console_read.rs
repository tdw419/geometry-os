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

    // Run to 392K (after early_sbi_setup, which sets up earlycon)
    for _ in 0..392_300 {
        let _ = vm.step();
    }

    // Now read the console output that was accumulated
    eprintln!(
        "Console output so far: {} bytes",
        vm.bus.sbi.console_output.len()
    );
    let out = String::from_utf8_lossy(&vm.bus.sbi.console_output);
    eprintln!("Content: \"{}\"", out.replace('\0', "\\0"));

    // Also read the UART tx buffer
    eprintln!("UART tx_buf: {} bytes", vm.bus.uart.tx_buf.len());
    eprintln!(
        "UART output: \"{}\"",
        String::from_utf8_lossy(&vm.bus.uart.tx_buf)
    );
}
