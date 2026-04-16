/// Test boot with correct earlycon bootargs.
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    // Use uart8250 earlycon matching our MMIO address and reg-shift=0
    let bootargs = "console=ttyS0 earlycon=uart8250,mmio,0x10000000 panic=1";

    let (mut vm, result) = RiscvVm::boot_linux(&kernel_image, initramfs.as_deref(), 256, 5_000_000, bootargs).unwrap();

    println!("{} instructions, PC=0x{:08X} priv={:?}", result.instructions, vm.cpu.pc, vm.cpu.privilege);
    println!("UART: ier={} lcr={} lsr=0x{:02X} tx_buf={}",
        vm.bus.uart.ier, vm.bus.uart.lcr, vm.bus.uart.lsr, vm.bus.uart.tx_buf.len());
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        let preview: String = s.chars().take(3000).collect();
        println!("UART output:\n{}", preview);
    }
}
