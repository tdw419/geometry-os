// examples/sh_run.rs -- Interactive RISC-V shell runner for Geometry OS.
//
// Boots a bare-metal ELF (sh.elf), pipes host terminal stdin to UART RX,
// drains UART TX to host stdout. Very lightweight: 1MB RAM, no Linux kernel.
//
// Live rendering: when the guest calls fb_present (writes to 0x6040_0000),
// the framebuffer is dumped to framebuf_live_NNNN.png in real-time.
//
// Usage: cargo run --release --example sh_run
//     or: cargo run --release --example sh_run -- /path/to/custom.elf

use geometry_os::riscv::{loader, RiscvVm};
use std::cell::RefCell;
use std::fs;
use std::io::{self, Read, Write};
use std::rc::Rc;

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

    // Frame counter for live PNG dumps
    let frame_counter = Rc::new(RefCell::new(0u32));

    // Create the present callback -- dumps a PNG each time guest calls fb_present
    let fc = frame_counter.clone();
    let present_cb: geometry_os::riscv::framebuf::PresentCallback = Rc::new(RefCell::new(
        move |pixels: &[u32]| {
            use geometry_os::riscv::framebuf::{FB_WIDTH, FB_HEIGHT};
            let frame = {
                let mut n = fc.borrow_mut();
                *n += 1;
                *n
            };
            let any = pixels.iter().any(|&p| p != 0);
            if !any {
                return; // Skip empty frames
            }
            let out_path = format!("framebuf_live_{:04}.png", frame);
            let file = match std::fs::File::create(&out_path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("frame {}: failed to create {}: {}", frame, out_path, e);
                    return;
                }
            };
            let mut encoder = png::Encoder::new(file, FB_WIDTH as u32, FB_HEIGHT as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = match encoder.write_header() {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("frame {}: png header error: {}", frame, e);
                    return;
                }
            };
            let mut rgba = vec![0u8; FB_WIDTH * FB_HEIGHT * 4];
            for (i, &pixel) in pixels.iter().enumerate() {
                let bytes = pixel.to_be_bytes();
                rgba[i * 4 + 0] = bytes[0]; // R
                rgba[i * 4 + 1] = bytes[1]; // G
                rgba[i * 4 + 2] = bytes[2]; // B
                rgba[i * 4 + 3] = bytes[3]; // A
            }
            match writer.write_image_data(&rgba) {
                Ok(()) => eprintln!("frame {}: saved {}", frame, out_path),
                Err(e) => eprintln!("frame {}: png write error: {}", frame, e),
            }
        },
    ));

    // Create VM with 1MB RAM at 0x8000_0000
    let mut vm = RiscvVm::new(1024 * 1024);

    // Wire up the live present callback
    vm.bus.framebuf.on_present = Some(present_cb);

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
                    eprintln!(
                        "\n[Guest requested shutdown after {} instructions]",
                        total_instructions
                    );
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

            if vm.bus.sbi.shutdown_requested {
                eprintln!(
                    "\n[Guest requested shutdown after {} instructions]",
                    total_instructions
                );
                shutdown = true;
                break;
            }
        }

        if shutdown {
            let sbi_out: Vec<u8> = vm.bus.sbi.console_output.drain(..).collect();
            if !sbi_out.is_empty() {
                let _ = stdout_handle.write_all(&sbi_out);
                let _ = stdout_handle.flush();
            }
            let _ = vm.bus.uart.drain_tx();
            break;
        }

        // 3. Drain console output
        let sbi_out: Vec<u8> = vm.bus.sbi.console_output.drain(..).collect();
        if !sbi_out.is_empty() {
            stdout_handle.write_all(&sbi_out).unwrap();
            stdout_handle.flush().unwrap();
        }
        let tx_bytes = vm.bus.uart.drain_tx();
        drop(tx_bytes);
    }

    // Restore terminal on exit
    #[cfg(unix)]
    {
        // Terminal will be restored by the OS when the process exits.
    }

    eprintln!("\nTotal instructions: {}", total_instructions);
    eprintln!("Final PC: 0x{:08X}", vm.cpu.pc);

    // Final framebuffer dump (always, even if no present was called)
    use geometry_os::riscv::framebuf::{FB_HEIGHT, FB_WIDTH};
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
            let bytes = pixel.to_be_bytes();
            rgba[i * 4 + 0] = bytes[0];
            rgba[i * 4 + 1] = bytes[1];
            rgba[i * 4 + 2] = bytes[2];
            rgba[i * 4 + 3] = bytes[3];
        }
        writer.write_image_data(&rgba).expect("write png");
        eprintln!(
            "MMIO framebuffer saved to {} ({}x{})",
            out_path, FB_WIDTH, FB_HEIGHT
        );
    }
}
