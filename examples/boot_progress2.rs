fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 panic=1";

    use geometry_os::riscv::RiscvVm;

    for max_instr in [1000, 5000, 10000, 50000, 100000, 200000, 300000, 400000, 500000] {
        let (mut vm, _) = RiscvVm::boot_linux(
            &kernel_image,
            initramfs.as_deref(),
            256,
            max_instr,
            bootargs,
        )
        .unwrap();
        println!("{} instr: PC=0x{:08X} SP=0x{:08X} SBI={} UART={}",
            max_instr, vm.cpu.pc, vm.cpu.x[2],
            vm.bus.sbi.console_output.len(),
            vm.bus.uart.tx_buf.len());
    }
}
