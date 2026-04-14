// Diagnostic: boot Linux with progressive output capture
use std::fs;

use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    
    let bootargs = "console=ttyS0 earlycon=sbi panic=5";
    let max_instructions = 50_000_000u64;
    let dump_interval = 1_000_000u64;
    
    let start = Instant::now();
    let (mut vm, result) = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        512,
        max_instructions,
        bootargs,
    ).unwrap();
    
    let elapsed = start.elapsed();
    let mips = result.instructions as f64 / elapsed.as_secs_f64() / 1_000_000.0;
    println!("Boot: {} instructions in {:?} = {:.2} MIPS", result.instructions, elapsed, mips);
    println!("Entry: 0x{:08X}, DTB: 0x{:08X}", result.entry, result.dtb_addr);
    println!("PC: 0x{:08X}, Privilege: {:?}", vm.cpu.pc, vm.cpu.privilege);
    println!("RAM base: 0x{:08X}", vm.bus.mem.ram_base);
    
    // Drain UART
    let tx = vm.bus.uart.drain_tx();
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        println!("\n=== UART OUTPUT ({} bytes) ===\n{}", tx.len(), s);
    } else {
        println!("\nNo UART output");
    }
    
    // CPU state
    println!("\nmcause: 0x{:08X}, mepc: 0x{:08X}", vm.cpu.csr.mcause, vm.cpu.csr.mepc);
    println!("scause: 0x{:08X}, sepc: 0x{:08X}", vm.cpu.csr.scause, vm.cpu.csr.sepc);
    println!("satp: 0x{:08X}", vm.cpu.csr.satp);
    println!("mstatus: 0x{:08X}", vm.cpu.csr.mstatus);
    println!("mie: 0x{:08X}, mip: 0x{:08X}", vm.cpu.csr.mie, vm.cpu.csr.mip);
    // sstatus is a restricted view of mstatus
    let sstatus = vm.cpu.csr.mstatus & ((1 << 1) | (1 << 5) | (1 << 8) | (1 << 18) | (1 << 19));
    println!("sstatus (derived): 0x{:08X}", sstatus);
    println!("stvec: 0x{:08X}", vm.cpu.csr.stvec);
    println!("shutdown_requested: {}", vm.bus.sbi.shutdown_requested);
    
    // Last few registers
    println!("\nSP (x2): 0x{:08X}", vm.cpu.x[2]);
    println!("ra (x1): 0x{:08X}", vm.cpu.x[1]);
    println!("a0 (x10): 0x{:08X}", vm.cpu.x[10]);
    println!("a1 (x11): 0x{:08X}", vm.cpu.x[11]);
}
