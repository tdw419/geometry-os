// Standalone test: boot Linux RV32 kernel in the RISC-V interpreter.
// Run with: cargo run --example boot_linux_test

use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    println!("=== Geometry OS Linux Boot Test ===\n");

    // Load kernel image.
    let kernel_image = match fs::read(kernel_path) {
        Ok(data) => {
            println!("Kernel: {} bytes ({:.1} MB)", data.len(), data.len() as f64 / 1_048_576.0);
            data
        }
        Err(e) => {
            eprintln!("Error loading kernel: {}", e);
            std::process::exit(1);
        }
    };

    // Load initramfs.
    let initramfs = match fs::read(initramfs_path) {
        Ok(data) => {
            println!("Initramfs: {} bytes ({:.1} KB)", data.len(), data.len() as f64 / 1024.0);
            Some(data)
        }
        Err(_) => {
            println!("Warning: no initramfs found, booting without");
            None
        }
    };

    // Create UART bridge for capturing output.
    let mut bridge = geometry_os::riscv::bridge::UartBridge::new();
    let canvas_cols = 80;
    let canvas_rows = 128;
    let mut canvas = vec![0u32; canvas_rows * canvas_cols];

    // Boot the kernel.
    println!("--- Starting boot (100,000 instructions) ---");
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";
    let start = std::time::Instant::now();

    let result = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        100_000,
        bootargs,
    );

    match result {
        Ok((mut vm, r)) => {
            let elapsed = start.elapsed();
            println!("Boot result: {} instructions in {:?}", r.instructions, elapsed);
            println!("Entry: 0x{:08X}, DTB: 0x{:08X}", r.entry, r.dtb_addr);
            println!("PC after boot: 0x{:08X}", vm.cpu.pc);
            println!("Privilege: {:?}", vm.cpu.privilege);

            // Drain UART output.
            bridge.drain_uart_to_canvas(&mut vm.bus, &mut canvas);

            // Print canvas content.
            println!("\n--- UART Output ---");
            for row in 0..canvas_rows {
                let mut line = String::new();
                let mut has_content = false;
                for col in 0..canvas_cols {
                    let ch = canvas[row * canvas_cols + col];
                    if ch != 0 {
                        has_content = true;
                        if ch >= 32 && ch < 127 {
                            line.push(char::from_u32(ch).unwrap_or('.'));
                        } else {
                            line.push('.');
                        }
                    } else {
                        line.push(' ');
                    }
                }
                if has_content {
                    println!("  {:3}: {}", row, line.trim_end());
                }
            }
        }
        Err(e) => {
            eprintln!("Boot failed: {}", e);
            std::process::exit(1);
        }
    }
}
