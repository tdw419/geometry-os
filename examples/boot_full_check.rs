use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::Privilege;
use std::io::Write;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    // Use boot_linux() which has full handling
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";
    let (mut vm, boot_result) = RiscvVm::boot_linux(
        &kernel_image, initramfs.as_deref(), 256, 20_000_000, bootargs).unwrap();

    eprintln!("Boot: {} instr, PC=0x{:08X}", boot_result.instructions, vm.cpu.pc);
    eprintln!("UART tx_buf: {} chars", vm.bus.uart.tx_buf.len());
    eprintln!("SBI console: {} chars", vm.bus.sbi.console_output.len());
    eprintln!("CPU ecall_count: {}", vm.cpu.ecall_count);

    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        eprintln!("SBI output:\n{}", &s[..s.len().min(3000)]);
    }
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        eprintln!("UART output:\n{}", &s[..s.len().min(3000)]);
    }

    // Print boot result stats
    eprintln!("\nBoot result: instructions={}, entry=0x{:08X}, dtb_addr=0x{:X}",
        boot_result.instructions, boot_result.entry, boot_result.dtb_addr);
}
