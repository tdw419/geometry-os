use geometry_os::riscv::RiscvVm;
fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    eprintln!("[uart_earlycon] Booting with earlycon=uart8250...");
    let result = RiscvVm::boot_linux(
        &kernel_data,
        None,
        512,
        50_000_000,
        "console=ttyS0 earlycon=uart8250,mmio,0x10000000 loglevel=8 nosmp",
    );
    match result {
        Ok((mut vm, stats)) => {
            let mut uart_output = Vec::new();
            loop {
                match vm.bus.uart.read_byte(0) {
                    0 => break,
                    b => uart_output.push(b),
                }
            }
            let s = String::from_utf8_lossy(&uart_output);
            eprintln!("UART output ({} bytes):", uart_output.len());
            if uart_output.len() > 0 {
                // Show first 500 and last 500 bytes
                let output_str = s.to_string();
                if output_str.len() > 1000 {
                    eprintln!("{}", &output_str[..500]);
                    eprintln!("... ({} bytes total) ...", output_str.len());
                    eprintln!("{}", &output_str[output_str.len()-500..]);
                } else {
                    eprintln!("{}", output_str);
                }
            } else {
                eprintln!("No UART output!");
            }
            eprintln!("PC: 0x{:08X}, Privilege: {:?}", vm.cpu.pc, vm.cpu.privilege);
            eprintln!("SBI output: {} bytes", vm.bus.sbi.console_output.len());
            eprintln!("Instructions: {}", stats.instructions);
        }
        Err(e) => eprintln!("Error: {:?}", e),
    }
}
