use std::fs;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();
    
    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image, initramfs.as_deref(), 256,
        "console=ttyS0 earlycon=sbi panic=1 quiet",
    ).unwrap();
    
    // Verify patches
    let val_10da = vm.bus.read_word(0x10DA).unwrap_or(0);
    let val_10de = vm.bus.read_word(0x10DE).unwrap_or(0);
    println!("PA 0x10DA: 0x{:08X} (expect 0x00210697 auipc a3,0x210)", val_10da);
    println!("PA 0x10DE: 0x{:08X} (expect 0xE3A68693 addi a3,a3,-454)", val_10de);
    
    let max = 5_000_000;
    let report_interval = 500_000;
    let mut count = 0u64;
    let mut last_pc = 0u32;
    let mut spin_count = 0u32;
    let mut ecall_count = 0u64;
    
    while count < max {
        let pc = vm.cpu.pc;
        let _ = vm.step();
        count += 1;
        
        if vm.cpu.pc == last_pc {
            spin_count += 1;
            if spin_count > 100000 {
                println!("\nSPIN at PC=0x{:08X} after {} instrs", vm.cpu.pc, count);
                break;
            }
        } else {
            spin_count = 0;
        }
        last_pc = vm.cpu.pc;
        
        if count % report_interval == 0 {
            let uart_bytes = vm.bus.uart.drain_tx();
            let uart_str: String = uart_bytes.iter().map(|&b| b as char).collect();
            println!("[{}] PC=0x{:08X} SP=0x{:08X} stvec=0x{:08X} ecalls={} uart_len={}",
                count, vm.cpu.pc, vm.cpu.x[2], vm.cpu.csr.read(0x005), ecall_count, uart_bytes.len());
            if !uart_str.is_empty() {
                println!("  UART: {}", &uart_str[..uart_str.len().min(200)]);
            }
        }
    }
    
    let uart_bytes = vm.bus.uart.drain_tx();
    let uart_str: String = uart_bytes.iter().map(|&b| b as char).collect();
    
    println!("\n=== FINAL ===");
    println!("Instructions: {}", count);
    println!("PC: 0x{:08X}", vm.cpu.pc);
    println!("STVEC: 0x{:08X}", vm.cpu.csr.read(0x005));
    println!("UART ({} bytes):", uart_bytes.len());
    if !uart_str.is_empty() {
        println!("{}", &uart_str[..uart_str.len().min(2000)]);
    }
}
