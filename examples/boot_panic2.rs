// Diagnostic: check what function the kernel is spinning in, with UART earlycon
use std::fs;
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    // Try UART direct earlycon instead of SBI
    let bootargs = "console=ttyS0 earlycon=uart8250,mmio32,0x10000000 panic=5";

    let start = Instant::now();
    let (mut vm, result) = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        512,
        20_000_000u64,
        bootargs,
    )
    .unwrap();

    let elapsed = start.elapsed();
    println!(
        "Boot: {} instructions in {:?}",
        result.instructions, elapsed
    );
    println!("PC: 0x{:08X}, Privilege: {:?}", vm.cpu.pc, vm.cpu.privilege);
    println!("SATP: 0x{:08X}", vm.cpu.csr.satp);

    // SBI console output
    println!(
        "\nSBI console output: {} bytes",
        vm.bus.sbi.console_output.len()
    );
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        println!("{}", s);
    }

    // UART output
    let tx = vm.bus.uart.drain_tx();
    println!("UART TX: {} bytes", tx.len());
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        println!("{}", s);
    }

    // Check where RA points
    let ra = vm.cpu.x[1];
    println!("\nRA: 0x{:08X}", ra);

    // Read a few words at RA to see what function it's in
    for i in -2..=2i32 {
        let addr = (ra as i64 + (i * 4) as i64) as u64;
        let word = vm.bus.read_word(addr).unwrap_or(0);
        println!("  PA[RA{:+}] = 0x{:08X}", i * 4, word);
    }

    // Check the stack for the panic message pointer
    let sp = vm.cpu.x[2];
    println!("\nSP: 0x{:08X}", sp);
    println!("Stack contents (first 20 words):");
    for i in 0..20 {
        let addr = sp as u64 + (i * 4) as u64;
        let word = vm.bus.read_word(addr).unwrap_or(0);
        // Try to interpret as string if it looks like a pointer
        let mut extra = String::new();
        if word > 0xC0000000 && word < 0xC2000000 {
            // Might be a kernel VA pointing to a string
            let mut chars = Vec::new();
            for j in 0..64 {
                let b_addr = word as u64 + j;
                let b = vm.bus.read_byte(b_addr).unwrap_or(0);
                if b == 0 {
                    break;
                }
                if b >= 0x20 && b < 0x7f {
                    chars.push(b as char);
                } else {
                    chars.push('.');
                }
            }
            if !chars.is_empty() && chars.len() > 3 {
                extra = format!(" -> \"{}\"", chars.iter().collect::<String>());
            }
        }
        println!("  SP[{:2}] = 0x{:08X}{}", i * 4, word, extra);
    }
}
