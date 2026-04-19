use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    // Try different earlycon options
    let configs = [
        // DTB-based earlycon (stdout-path in chosen node points to uart@10000000)
        "earlycon loglevel=8",
        // Direct 8250 with mmio
        "earlycon=8250,mmio,0x10000000 loglevel=8",
        "earlycon=8250mmio,0x10000000 loglevel=8",
        // ns16550a
        "earlycon=ns16550a,mmio,0x10000000 loglevel=8",
    ];

    for bootargs in &configs {
        let (vm, _br) = RiscvVm::boot_linux(
            &kernel_image,
            initramfs.as_deref(),
            256,
            10_000_000,
            bootargs,
        ).unwrap();
        
        println!("[{}] UART_writes={} UART_buf={} SBI={}", 
            bootargs, vm.bus.uart.write_count, vm.bus.uart.tx_buf.len(), vm.bus.sbi.console_output.len());
    }
}
