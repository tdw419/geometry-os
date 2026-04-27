// examples/sh_run.rs -- Interactive RISC-V shell runner for Geometry OS.
//
// Boots a bare-metal ELF (sh.elf), pipes host terminal stdin to UART RX,
// drains UART TX to host stdout. Very lightweight: 1MB RAM, no Linux kernel.
//
// Usage: cargo run --release --example sh_run
//     or: cargo run --release --example sh_run -- /path/to/custom.elf

use geometry_os::riscv::{loader, RiscvVm};
use std::fs;
use std::io::{self, Read, Write};

fn main() {
    // Pick ELF file: first non-dash arg, or default sh.elf
    let elf_path = std::env::args()
        .skip(1)
        .find(|a| !a.starts_with('-'))
        .unwrap_or_else(|| "examples/riscv-hello/sh.elf".into());

    eprintln!("Loading {}...", elf_path);
    let elf_data = fs::read(&elf_path).unwrap_or_else(|e| {
        eprintln!("Failed to read {}: {}", elf_path, e);
        eprintln!("Build it first: cd examples/riscv-hello && ./build.sh sh.c");
        std::process::exit(1);
    });

    // Create VM with 1MB RAM at 0x8000_0000
    let mut vm = RiscvVm::new(1024 * 1024);

    // Load ELF into bus memory
    let load_info = loader::load_elf(&mut vm.bus, &elf_data).unwrap_or_else(|e| {
        eprintln!("Failed to load ELF: {:?}", e);
        std::process::exit(1);
    });
    eprintln!(
        "Entry: 0x{:08X}, loaded {} bytes (0x{:X}-0x{:X})",
        load_info.entry,
        elf_data.len(),
        load_info.entry,
        load_info.highest_addr,
    );

    // Set PC to entry point
    vm.cpu.pc = load_info.entry;

    // Set up non-blocking stdin
    let stdin = io::stdin();
    let mut stdin_handle = stdin.lock();
    // On Unix, set stdin to raw mode for character-at-a-time input
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let fd = stdin_handle.as_raw_fd();
        unsafe {
            let mut termios: libc::termios = std::mem::zeroed();
            libc::tcgetattr(fd, &mut termios);
            termios.c_lflag &= !(libc::ICANON | libc::ECHO);
            termios.c_cc[libc::VMIN] = 0;
            termios.c_cc[libc::VTIME] = 0;
            libc::tcsetattr(fd, libc::TCSANOW, &termios);
        }
    }

    let stdout = io::stdout();
    let mut stdout_handle = stdout.lock();

    eprintln!("VM running. Type commands (Ctrl+C to exit).\n");

    // Instruction batching: run N steps between checking for I/O
    const BATCH_SIZE: u32 = 5000;
    let mut total_instructions: u64 = 0;
    let mut input_buf = [0u8; 64];
    let mut shutdown = false;

    while !shutdown {
        // 1. Check for host stdin input -> push to UART RX
        #[cfg(unix)]
        {
            match stdin_handle.read(&mut input_buf) {
                Ok(n) => {
                    for &b in &input_buf[..n] {
                        // Map Ctrl+C to shutdown
                        if b == 0x03 {
                            eprintln!("\nCaught Ctrl+C, shutting down...");
                            shutdown = true;
                            break;
                        }
                        vm.bus.uart.receive_byte(b);
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => {
                    eprintln!("Stdin error: {}", e);
                    shutdown = true;
                }
            }
        }
        #[cfg(not(unix))]
        {
            // Non-Unix: just run without input (output-only mode)
        }

        if shutdown {
            break;
        }

        // 2. Run a batch of instructions
        for _ in 0..BATCH_SIZE {
            match vm.step() {
                geometry_os::riscv::cpu::StepResult::Ok => {}
                geometry_os::riscv::cpu::StepResult::Ebreak => {
                    eprintln!("\n[EBREAK at PC=0x{:08X}]", vm.cpu.pc);
                    shutdown = true;
                    break;
                }
                geometry_os::riscv::cpu::StepResult::Shutdown => {
                    eprintln!("\n[Guest requested shutdown after {} instructions]", total_instructions);
                    shutdown = true;
                    break;
                }
                geometry_os::riscv::cpu::StepResult::Ecall => {
                    // SBI handled internally
                }
                geometry_os::riscv::cpu::StepResult::FetchFault
                | geometry_os::riscv::cpu::StepResult::LoadFault
                | geometry_os::riscv::cpu::StepResult::StoreFault => {
                    eprintln!("\n[FAULT at PC=0x{:08X}]", vm.cpu.pc);
                    shutdown = true;
                    break;
                }
            }
            total_instructions += 1;

            // Check for shutdown requested via SBI
            if vm.bus.sbi.shutdown_requested {
                eprintln!("\n[Guest requested shutdown after {} instructions]", total_instructions);
                shutdown = true;
                break;
            }
        }

        if shutdown {
            // Final drain before exiting -- only use console_output
            // (uart.tx_buf is a duplicate of what SBI already pushed there)
            let sbi_out: Vec<u8> = vm.bus.sbi.console_output.drain(..).collect();
            if !sbi_out.is_empty() {
                let _ = stdout_handle.write_all(&sbi_out);
                let _ = stdout_handle.flush();
            }
            let _ = vm.bus.uart.drain_tx(); // clear duplicate
            break;
        }

        // 3. Drain console output to host stdout.
        // SBI putchar writes to BOTH uart.tx_buf and console_output,
        // so we only drain one to avoid duplicating output.
        let sbi_out: Vec<u8> = vm.bus.sbi.console_output.drain(..).collect();
        if !sbi_out.is_empty() {
            stdout_handle.write_all(&sbi_out).unwrap();
            stdout_handle.flush().unwrap();
        }
        // Also drain uart.tx_buf in case guest wrote directly to UART MMIO
        // (not via SBI). Skip bytes already captured by console_output.
        let tx_bytes = vm.bus.uart.drain_tx();
        // tx_bytes contains everything SBI wrote too, so only drain what's new
        // For now just clear it since SBI output is already captured above.
        drop(tx_bytes);
    }

    // Restore terminal on exit
    #[cfg(unix)]
    {
        // Terminal will be restored by the OS when the process exits.
        // For clean restoration, we'd save/restore termios, but _exit is simpler.
    }

    eprintln!("\nTotal instructions: {}", total_instructions);
    eprintln!("Final PC: 0x{:08X}", vm.cpu.pc);

    // Dump MMIO framebuffer (256x256 at 0x6000_0000) if guest wrote any pixels
    use geometry_os::riscv::framebuf::{FB_WIDTH, FB_HEIGHT};
    let mmio_fb = &vm.bus.framebuf.pixels;
    let any_mmio = mmio_fb.iter().any(|&p| p != 0);
    if any_mmio {
        let out_path = "framebuf_output.png";
        let file = std::fs::File::create(out_path).expect("create png");
        let mut encoder = png::Encoder::new(file, FB_WIDTH as u32, FB_HEIGHT as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("png header");
        let mut rgba = vec![0u8; FB_WIDTH * FB_HEIGHT * 4];
        for (i, &pixel) in mmio_fb.iter().enumerate() {
            // Color format: [31:24]=R, [23:16]=G, [15:8]=B, [7:0]=A
            let bytes = pixel.to_be_bytes();
            rgba[i * 4 + 0] = bytes[0]; // R
            rgba[i * 4 + 1] = bytes[1]; // G
            rgba[i * 4 + 2] = bytes[2]; // B
            rgba[i * 4 + 3] = bytes[3]; // A
        }
        writer.write_image_data(&rgba).expect("write png");
        eprintln!("MMIO framebuffer saved to {} ({}x{})", out_path, FB_WIDTH, FB_HEIGHT);
    }

    // Dump SBI pixel framebuffer (64x64) if guest used SBI pixel extension
    use geometry_os::riscv::sbi::{GEO_FB_HEIGHT, GEO_FB_WIDTH};
    let sbi_fb = &vm.bus.sbi.pixel_fb;
    let any_sbi = sbi_fb.iter().any(|&p| p != 0);
    if any_sbi {
        let out_path = "painter_output.png";
        let file = std::fs::File::create(out_path).expect("create png");
        let mut encoder = png::Encoder::new(file, GEO_FB_WIDTH as u32, GEO_FB_HEIGHT as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("png header");
        let mut rgba = vec![0u8; GEO_FB_WIDTH * GEO_FB_HEIGHT * 4];
        for (i, &pixel) in sbi_fb.iter().enumerate() {
            let bytes = pixel.to_be_bytes();
            rgba[i * 4 + 0] = bytes[0]; // R
            rgba[i * 4 + 1] = bytes[1]; // G
            rgba[i * 4 + 2] = bytes[2]; // B
            rgba[i * 4 + 3] = bytes[3]; // A
        }
        writer.write_image_data(&rgba).expect("write png");
        eprintln!("SBI pixel framebuffer saved to {} ({}x{})", out_path, GEO_FB_WIDTH, GEO_FB_HEIGHT);
    }
}
