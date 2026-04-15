
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
        500_000_000,
        bootargs,
    ).unwrap();
    
    let sbi = &vm.bus.sbi;
    println!("SBI console: {} bytes", sbi.console_output.len());
    if !sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&sbi.console_output);
        let preview = if s.len() > 500 { &s[..500] } else { &s };
        println!("{}", preview);
    }
    
    let uart_out: Vec<u8> = vm.bus.uart.drain_tx();
    println!("UART TX: {} bytes", uart_out.len());
    if !uart_out.is_empty() {
        let s = String::from_utf8_lossy(&uart_out);
        let preview = if s.len() > 500 { &s[..500] } else { &s };
        println!("{}", preview);
    }
    
    println!("
PC: 0x{:08X}", vm.cpu.pc);
    println!("Privilege: {:?}", vm.cpu.privilege);
    println!("mtime: {}", vm.cpu.csr.mcause);
    println!("scause: 0x{:08X}", vm.cpu.csr.scause);
    
    // Find kernel symbol
    let nm = std::process::Command::new("riscv64-linux-gnu-nm")
        .args(["-n", ".geometry_os/build/linux-6.14/vmlinux"])
        .output().unwrap();
    let nm_out = String::from_utf8_lossy(&nm.stdout);
    let pc = vm.cpu.pc as u64;
    let mut best_sym = String::new();
    let mut best_addr = 0u64;
    for line in nm_out.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            if let Ok(addr) = u64::from_str_radix(parts[0], 16) {
                if addr <= pc && addr > best_addr {
                    best_addr = addr;
                    best_sym = parts[2].to_string();
                }
            }
        }
    }
    println!("Near symbol: {} (offset +{})", best_sym, pc - best_addr);
}
