fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    use geometry_os::riscv::RiscvVm;

    // Try different bootargs configurations
    for bootargs in &[
        "console=ttyS0 earlycon=sbi panic=1",
        "console=ttyS0 panic=1 earlycon=uart8250,mmio32,0x10000000",
        "console=ttyS0,115200n8 panic=1",
        "panic=1 quiet",
    ] {
        let (vm, boot_result) = RiscvVm::boot_linux(
            &kernel_image,
            initramfs.as_deref(),
            256,
            20_000_000,
            bootargs,
        )
        .unwrap();

        println!("[{}] PC=0x{:08X} instrs={} SBI={} UART={}",
            bootargs,
            vm.cpu.pc,
            boot_result.instructions,
            vm.bus.sbi.console_output.len(),
            vm.bus.uart.tx_buf.len(),
        );

        if !vm.bus.sbi.console_output.is_empty() {
            let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
            let preview: String = s.chars().take(200).collect();
            println!("  SBI: {}", preview);
        }
        if !vm.bus.uart.tx_buf.is_empty() {
            let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
            let preview: String = s.chars().take(200).collect();
            println!("  UART: {}", preview);
        }
    }
}
