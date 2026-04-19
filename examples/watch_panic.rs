use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    // Run from scratch, watching for panic entry at 0xC000252E
    let (mut vm, _br) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        100, // minimal -- we step manually
        "console=ttyS0 earlycon=sbi loglevel=8",
    ).unwrap();
    
    let panic_start = 0xC000252Eu32;
    let max = 25_000_000u64;
    let mut count = 0u64;
    
    // Check every N steps
    let check_interval = 10_000u64;
    
    while count < max {
        // Run check_interval steps
        for _ in 0..check_interval {
            let prev_pc = vm.cpu.pc;
            vm.step();
            count += 1;
            if vm.cpu.pc == panic_start {
                println!("PANIC at instruction {}!", count);
                println!("  Called from PC=0x{:08X}", prev_pc);
                println!("  a0 (format string) = 0x{:08X}", vm.cpu.x[10]);
                println!("  a1 = 0x{:08X}", vm.cpu.x[11]);
                println!("  a2 = 0x{:08X}", vm.cpu.x[12]);
                println!("  ra = 0x{:08X}", vm.cpu.x[1]);
                println!("  sp = 0x{:08X}", vm.cpu.x[2]);
                
                // Read the format string from guest memory
                let mut s = String::new();
                let mut addr = vm.cpu.x[10] as u64;
                for _ in 0..200 {
                    match vm.bus.read_byte(addr) {
                        Ok(b) if b != 0 => {
                            s.push(b as char);
                            addr += 1;
                        }
                        _ => break,
                    }
                }
                println!("  Panic message: \"{}\"", s);
                
                // Disassemble the call site
                println!("\n  SBI ecalls so far: {}", vm.bus.sbi.ecall_log.len());
                return;
            }
        }
        
        if count % 5_000_000 == 0 {
            eprintln!("[{}] PC=0x{:08X}", count, vm.cpu.pc);
        }
    }
    
    println!("No panic in {} instructions. Final PC=0x{:08X}", count, vm.cpu.pc);
}
