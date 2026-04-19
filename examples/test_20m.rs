use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let configs = [
        "console=ttyS0 earlycon=uart8250,mmio,0x10000000 loglevel=8",
        "console=ttyS0 earlycon=sbi loglevel=8",
    ];

    for bootargs in &configs {
        let (vm, _br) = RiscvVm::boot_linux(
            &kernel_image,
            initramfs.as_deref(),
            256,
            20_000_000,
            bootargs,
        )
        .unwrap();

        let sbi_out = vm.bus.sbi.console_output.len();
        let uart_out = vm.bus.uart.tx_buf.len();
        let sbi_ecalls = vm.bus.sbi.ecall_log.len();
        println!(
            "\n[{}] PC=0x{:08X} SBI={} UART={} ecalls={}",
            bootargs, vm.cpu.pc, sbi_out, uart_out, sbi_ecalls
        );

        if sbi_out > 0 {
            let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
            println!("SBI output: {}", &s[..s.len().min(300)]);
        }
        if uart_out > 0 {
            let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
            println!("UART output: {}", &s[..s.len().min(300)]);
        }

        // Show last 5 ecalls
        for (i, (a7, a6, a0)) in vm.bus.sbi.ecall_log.iter().rev().take(5).enumerate() {
            println!("  ecall[{}] a7=0x{:08X} a6={} a0=0x{:08X}", i, a7, a6, a0);
        }
    }
}
