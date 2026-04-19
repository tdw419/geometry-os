fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    use geometry_os::riscv::RiscvVm;

    // Try with no console at all
    let configs = [
        "loglevel=0 quiet",
        "console=ttyS0 loglevel=0",
        "earlycon=sbi loglevel=8",
    ];

    for bootargs in &configs {
        let (vm, br) = RiscvVm::boot_linux(
            &kernel_image,
            initramfs.as_deref(),
            256,
            1_000_000,
            bootargs,
        )
        .unwrap();
        println!(
            "[{}] PC=0x{:08X} SBI={} UART={}",
            bootargs,
            vm.cpu.pc,
            vm.bus.sbi.console_output.len(),
            vm.bus.uart.tx_buf.len()
        );
    }

    // Try with 5M to see if it gets past the memmove
    let (vm, br) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        5_000_000,
        "loglevel=0 quiet",
    )
    .unwrap();
    println!(
        "\n[loglevel=0 quiet, 5M] PC=0x{:08X} SBI={} UART={}",
        vm.cpu.pc,
        vm.bus.sbi.console_output.len(),
        vm.bus.uart.tx_buf.len()
    );
}
