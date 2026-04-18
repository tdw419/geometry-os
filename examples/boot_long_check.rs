use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel = fs::read(".geometry_os/build/linux-6.14/vmlinux").unwrap();
    let initramfs = fs::read(".geometry_os/fs/linux/rv32/initramfs.cpio.gz").ok();

    // Run with 200M instructions
    let (mut vm, result) = RiscvVm::boot_linux(
        &kernel,
        initramfs.as_deref(),
        256,
        200_000_000,
        "console=ttyS0 earlycon",
    )
    .unwrap();

    eprintln!("Instructions: {}", result.instructions);
    eprintln!("PC: 0x{:08X}  Priv: {:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("SATP: 0x{:08X}", vm.cpu.csr.satp);

    let uart = vm.bus.uart.drain_tx();
    eprintln!("UART TX: {} bytes", uart.len());
    if !uart.is_empty() {
        let s = String::from_utf8_lossy(&uart);
        for line in s.lines().take(30) {
            eprintln!("  {}", line);
        }
    }
}
