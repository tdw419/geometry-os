//! Quick boot test without initramfs, 100M instructions.
use geometry_os::riscv::RiscvVm;
fn main() {
    let kernel_data = std::fs::read(".geometry_os/build/linux-6.14/vmlinux").expect("kernel");
    eprintln!("[100m] Booting 100M instructions, no initramfs...");
    let result = RiscvVm::boot_linux(
        &kernel_data,
        None, // no initramfs
        128,  // 128MB RAM (smaller = faster init)
        100_000_000,
        "console=ttyS0 earlycon=sbi loglevel=8 nosmp",
    );
    match result {
        Ok((mut vm, stats)) => {
            let mut uart = Vec::new();
            loop { match vm.bus.uart.read_byte(0) { 0 => break, b => uart.push(b) } }
            let sbi = vm.bus.sbi.console_output.len();
            eprintln!("UART: {} bytes, SBI: {} bytes", uart.len(), sbi);
            if !uart.is_empty() { 
                let s = String::from_utf8_lossy(&uart);
                eprintln!("UART: {}", &s[..s.len().min(500)]);
            }
            if sbi > 0 {
                let s: String = vm.bus.sbi.console_output.iter().map(|&b| b as char).collect();
                eprintln!("SBI: {}", &s[..s.len().min(500)]);
            }
            eprintln!("Instructions: {}, PC: 0x{:08X}", stats.instructions, vm.cpu.pc);
        }
        Err(e) => eprintln!("Error: {:?}", e),
    }
}
