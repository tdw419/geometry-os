use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (vm, _br) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        20_000_000,
        "console=ttyS0 earlycon=sbi loglevel=8",
    ).unwrap();
    
    println!("UART write_count: {}", vm.bus.uart.write_count);
    println!("UART tx_buf: {} bytes", vm.bus.uart.tx_buf.len());
    println!("SBI console: {} bytes", vm.bus.sbi.console_output.len());
    
    if vm.bus.uart.tx_buf.len() > 0 {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        println!("UART output: {}", &s[..s.len().min(500)]);
    }
}
