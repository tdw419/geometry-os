use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let configs = [
        "console=ttyS0 earlycon=uart8250,mmio,0x10000000 loglevel=8",
        "console=ttyS0 earlycon=uart,mmio32,0x10000000 loglevel=8",
        "earlycon=uart8250,mmio,0x10000000 loglevel=8",
        "console=ttyS0 loglevel=8",
    ];

    for bootargs in &configs {
        let (vm, _br) = RiscvVm::boot_linux(
            &kernel_image,
            initramfs.as_deref(),
            256,
            5_000_000,
            bootargs,
        )
        .unwrap();

        let sbi_out = vm.bus.sbi.console_output.len();
        let uart_out = vm.bus.uart.tx_buf.len();
        let sbi_ecalls = vm.bus.sbi.ecall_log.len();
        println!(
            "[{}] PC=0x{:08X} SBI={} UART={} ecalls={}",
            bootargs, vm.cpu.pc, sbi_out, uart_out, sbi_ecalls
        );
    }
}
