
use std::fs;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();
    
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";
    let (mut vm, _r) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        100_000_000,
        bootargs,
    ).unwrap();
    
    let sbi = &vm.bus.sbi;
    println!("SBI console: {} bytes", &sbi.console_output.len());
    if !&sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&sbi.console_output);
        println!("{}", s);
    }
    
    let uart_out: Vec<u8> = vm.bus.uart.drain_tx();
    println!("UART TX: {} bytes", uart_out.len());
    if !uart_out.is_empty() {
        let s = String::from_utf8_lossy(&uart_out);
        println!("{}", s);
    }
    
    println!("PC: 0x{:08X}", vm.cpu.pc);
    println!("Privilege: {:?}", vm.cpu.privilege);
    println!("mtime: {}", vm.bus.clint.mtime);
}
