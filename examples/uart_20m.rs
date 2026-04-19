use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    // Test with earlycon=sbi at 20M instructions
    let (vm, _br) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        20_000_000,
        "console=ttyS0 earlycon=sbi loglevel=8",
    )
    .unwrap();

    println!("PC=0x{:08X}", vm.cpu.pc);
    println!("SBI console: {} bytes", vm.bus.sbi.console_output.len());
    println!("UART tx_buf: {} bytes", vm.bus.uart.tx_buf.len());

    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        println!("SBI output (first 500 chars):");
        println!("{}", &s[..s.len().min(500)]);
    }
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        println!("UART output (first 500 chars):");
        println!("{}", &s[..s.len().min(500)]);
    }

    // Check SBI ecall log
    println!("\nSBI ecall log (last 20):");
    for (i, (a7, a6, a0)) in vm.bus.sbi.ecall_log.iter().rev().take(20).enumerate() {
        println!("  [{}] a7=0x{:08X} a6=0x{:08X} a0=0x{:08X}", i, a7, a6, a0);
    }
    println!("Total ecalls: {}", vm.bus.sbi.ecall_log.len());
}
