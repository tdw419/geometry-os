// Standalone Linux boot diagnostic
// cargo run --example boot_diag
use std::fs;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel = match fs::read(kernel_path) {
        Ok(d) => d,
        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
    };
    let initramfs = fs::read(initramfs_path).ok();

    println!("=== Linux Boot Diagnostic ===");
    println!("Kernel: {} bytes", kernel.len());

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, result) = RiscvVm::boot_linux(
        &kernel,
        initramfs.as_deref(),
        512,
        50_000_000, // 50M instructions
        bootargs,
    ).unwrap();

    println!("Entry: 0x{:08X}, DTB: 0x{:08X}", result.entry, result.dtb_addr);
    println!("RAM base: 0x{:08X}", vm.bus.mem.ram_base);
    println!("Instructions: {}", result.instructions);
    println!("Final PC: 0x{:08X}", vm.cpu.pc);
    println!("Privilege: {:?}", vm.cpu.privilege);
    println!("mcause: 0x{:08X}, mepc: 0x{:08X}", vm.cpu.csr.mcause, vm.cpu.csr.mepc);
    println!("satp: 0x{:08X}", vm.cpu.csr.satp);
    println!("mstatus: 0x{:08X}", vm.cpu.csr.mstatus);
    println!("mtvec: 0x{:08X}", vm.cpu.csr.read(geometry_os::riscv::csr::MTVEC));

    // Check UART
    let mut out = Vec::new();
    loop {
        match vm.bus.uart.read_byte(0) {
            0 => break,
            b => out.push(b),
        }
    }
    if !out.is_empty() {
        println!("\n=== UART Output ({} bytes) ===", out.len());
        println!("{}", String::from_utf8_lossy(&out));
    } else {
        println!("\nNo UART output");
    }
}
