
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
    
    // After 100M steps, check progress over next 10M steps
    let pc_start = vm.cpu.pc;
    let mut min_pc = u32::MAX;
    let mut max_pc = 0u32;
    let mut unique_count = std::collections::HashSet::new();
    
    for _ in 0..10_000_000 {
        unique_count.insert(vm.cpu.pc);
        min_pc = min_pc.min(vm.cpu.pc);
        max_pc = max_pc.max(vm.cpu.pc);
        vm.step();
    }
    
    let pc_end = vm.cpu.pc;
    println!("PC range: 0x{:08X} to 0x{:08X}", min_pc, max_pc);
    println!("PC at start: 0x{:08X}", pc_start);
    println!("PC at end: 0x{:08X}", pc_end);
    println!("Unique PCs in 10M steps: {}", unique_count.len());
    
    // Check if kernel made forward progress
    if pc_end == pc_start {
        println!("STUCK: PC unchanged after 10M additional steps");
    } else if pc_end > pc_start {
        println!("Progressing: PC advanced by {} bytes", pc_end - pc_start);
    } else {
        println!("Looping: PC went backward");
    }
    
    // Check SBI output
    println!("
SBI console: {} bytes", vm.bus.sbi.console_output.len());
    let uart_out: Vec<u8> = vm.bus.uart.drain_tx();
    println!("UART TX: {} bytes", uart_out.len());
    if !uart_out.is_empty() {
        let s = String::from_utf8_lossy(&uart_out);
        let preview = if s.len() > 500 { &s[..500] } else { &s };
        println!("{}", preview);
    }
}
