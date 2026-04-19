// Use the internal boot_linux() function which has full fault handling
use geometry_os::riscv::RiscvVm;
use std::fs;
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let start = Instant::now();
    
    let (mut vm, result) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        128,
        10_000_000,
        bootargs,
    ).unwrap();

    let elapsed = start.elapsed();
    eprintln!("\n=== boot_linux() Results ===");
    eprintln!("Instructions: {} ({:.1}s)", result.instructions, elapsed.as_secs_f64());
    
    // SBI calls
    eprintln!("SBI ecall_log: {} entries", vm.bus.sbi.ecall_log.len());
    let mut sbi_types: std::collections::HashMap<(u32,u32), u32> = std::collections::HashMap::new();
    for &(a7, a6, _a0) in &vm.bus.sbi.ecall_log {
        *sbi_types.entry((a7, a6)).or_insert(0) += 1;
    }
    for ((a7, a6), cnt) in &sbi_types {
        eprintln!("  a7=0x{:08X} a6={}: {} calls", a7, a6, cnt);
    }

    // UART output
    let tx = vm.bus.uart.drain_tx();
    eprintln!("\nUART: {} bytes", tx.len());
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        eprintln!("{}", &s[..s.len().min(5000)]);
    }

    // CPU state
    eprintln!("\nCPU: PC=0x{:08X} SP=0x{:08X} SATP=0x{:08X}", 
        vm.cpu.pc, vm.cpu.x[2], vm.cpu.csr.satp);
}
