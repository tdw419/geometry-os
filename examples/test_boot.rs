use std::fs;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();
    
    let bootargs = std::env::args().nth(1)
        .unwrap_or_else(|| "console=ttyS0 earlycon=sbi panic=1 quiet".to_string());
    
    let (mut vm, r) = RiscvVm::boot_linux(
        &kernel_image, initramfs.as_deref(), 256, 2_000_000, &bootargs,
    ).unwrap();
    
    // SBI output
    let sbi_out = &vm.bus.sbi.console_output;
    if !sbi_out.is_empty() {
        let s = String::from_utf8_lossy(sbi_out);
        println!("=== SBI Console ({} bytes) ===", sbi_out.len());
        println!("{}", s);
    }
    
    // UART output
    let uart: Vec<u8> = vm.bus.uart.drain_tx();
    if !uart.is_empty() {
        let s = String::from_utf8_lossy(&uart);
        println!("=== UART ({} bytes) ===", uart.len());
        println!("{}", s);
    }
    
    if sbi_out.is_empty() && uart.is_empty() {
        println!("No console output. PC=0x{:08X} priv={:?} scause=0x{:08X} sepc=0x{:08X} stval=0x{:08X}",
            vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval);
    }
    
    println!("\n{} instructions", r.instructions);
}
