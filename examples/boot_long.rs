fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;

    let (mut vm, result) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        200_000_000,
        bootargs,
    )
    .unwrap();

    println!(
        "{} instructions, PC=0x{:08X} priv={:?}",
        result.instructions, vm.cpu.pc, vm.cpu.privilege
    );
    println!("UART output: {} chars", vm.bus.uart.tx_buf.len());
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        let preview: String = s.chars().take(2000).collect();
        println!("UART:\n{}", preview);
    }
}
