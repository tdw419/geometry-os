use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    
    eprintln!("Kernel: {} bytes", kernel_image.len());

    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";
    let start = std::time::Instant::now();

    let result = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        512,
        5_000_000,
        bootargs,
    );

    match result {
        Ok((mut vm, r)) => {
            let elapsed = start.elapsed();
            let mips = r.instructions as f64 / elapsed.as_secs_f64() / 1_000_000.0;
            eprintln!("Boot: {} instructions in {:?} = {:.2} MIPS", r.instructions, elapsed, mips);
            eprintln!("PC: 0x{:08X}, Privilege: {:?}", vm.cpu.pc, vm.cpu.privilege);
            eprintln!("mcause: 0x{:08X}, mepc: 0x{:08X}", vm.cpu.csr.mcause, vm.cpu.csr.mepc);
            eprintln!("scause: 0x{:08X}, sepc: 0x{:08X}", vm.cpu.csr.scause, vm.cpu.csr.sepc);
            eprintln!("satp: 0x{:08X}", vm.cpu.csr.satp);

            // Drain UART
            let mut uart_output = Vec::new();
            loop {
                match vm.bus.uart.read_byte(0) {
                    0 => break,
                    b => uart_output.push(b),
                }
            }
            if !uart_output.is_empty() {
                let s = String::from_utf8_lossy(&uart_output);
                eprintln!("UART output ({} bytes): {}", uart_output.len(), s);
            } else {
                eprintln!("No UART output");
            }
        }
        Err(e) => {
            eprintln!("Boot failed: {}", e);
            std::process::exit(1);
        }
    }
}
