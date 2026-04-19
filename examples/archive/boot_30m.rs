// Run for 20M instructions, capture panic message and UART output
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=1";
    let (vm, result) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        30_000_000,
        bootargs,
    )
    .unwrap();

    eprintln!(
        "Done: {} instructions, PC=0x{:08X} priv={:?}",
        result.instructions, vm.cpu.pc, vm.cpu.privilege
    );
    eprintln!("UART output: {} chars", vm.bus.uart.tx_buf.len());
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        let preview: String = s.chars().take(5000).collect();
        eprintln!("UART:\n{}", preview);
    } else {
        eprintln!("No UART output!");
    }
}
