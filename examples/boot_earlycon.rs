// Diagnostic: try different earlycon options + capture the panic reason
use std::fs;
use std::time::Instant;

fn try_boot(earlycon: &str, label: &str) {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = format!("console=ttyS0 {} panic=5", earlycon);
    let start = Instant::now();
    let (mut vm, result) = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        512,
        20_000_000u64,
        &bootargs,
    ).unwrap();
    let elapsed = start.elapsed();
    
    println!("\n=== {} ===", label);
    println!("Boot: {} instructions in {:?}", result.instructions, elapsed);
    println!("PC: 0x{:08X} Priv: {:?} SATP: 0x{:08X}", vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp);
    
    // Check for SBI calls
    let tx = vm.bus.uart.drain_tx();
    let sbi_out = &vm.bus.sbi.console_output;
    
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        println!("UART ({} bytes):\n{}", tx.len(), s);
    }
    if !sbi_out.is_empty() {
        let s = String::from_utf8_lossy(sbi_out);
        println!("SBI ({} bytes):\n{}", sbi_out.len(), s);
    }
    if tx.is_empty() && sbi_out.is_empty() {
        println!("No output at all!");
        
        // Try to find panic string on stack
        let sp = vm.cpu.x[2];
        for i in 0..40 {
            let addr = sp as u64 + (i * 4) as u64;
            let word = vm.bus.read_word(addr).unwrap_or(0);
            if word > 0xC0000000 && word < 0xC2000000 {
                // Try to read as string
                let mut chars = Vec::new();
                for j in 0..128 {
                    let b = vm.bus.read_word(word as u64 + j as u64).unwrap_or(0);
                    let bytes = b.to_le_bytes();
                    for &byte in &bytes {
                        if byte == 0 { break; }
                        if byte >= 0x20 && byte < 0x7f {
                            chars.push(byte as char);
                        } else {
                            chars.push('.');
                        }
                    }
                }
                if chars.len() > 4 {
                    let s: String = chars.iter().collect();
                    if s.contains("panic") || s.contains("fatal") || s.contains("error") || s.contains("BUG") || s.contains("die") {
                        println!("  SP[{:2}] = 0x{:08X} -> \"{}\"", i * 4, word, s);
                    }
                }
            }
        }
    }
}

fn main() {
    try_boot("earlycon=sbi", "earlycon=sbi");
    try_boot("earlycon=uart8250,mmio32,0x10000000", "earlycon=uart MMIO");
}
