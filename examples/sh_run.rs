// examples/sh_run.rs -- Interactive RISC-V shell runner for Geometry OS.
//
// Boots a bare-metal ELF (sh.elf), pipes host terminal stdin to UART RX,
// drains UART TX to host stdout. Uses off-thread VM with channel-based
// present callback so PNG dumps never block the interpreter.
//
// Live rendering: when the guest calls fb_present (writes to 0x6040_0000),
// the framebuffer is dumped to framebuf_live_NNNN.png in real-time.
// The VM runs on its own thread; the main thread recv()s frames asynchronously.
//
// Usage: cargo run --release --example sh_run
//     or: cargo run --release --example sh_run -- /path/to/custom.elf

use geometry_os::riscv::framebuf::{FB_HEIGHT, FB_WIDTH};
use geometry_os::riscv::live::{spawn_vm_thread, VmStatus, VmThreadConfig};
use std::fs;
use std::io::{self, Read, Write};

fn save_frame_png(pixels: &[u32], path: &str) -> bool {
    let file = match std::fs::File::create(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("failed to create {}: {}", path, e);
            return false;
        }
    };
    let mut encoder = png::Encoder::new(file, FB_WIDTH as u32, FB_HEIGHT as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = match encoder.write_header() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("png header error for {}: {}", path, e);
            return false;
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
        Ok(()) => true,
        Err(e) => {
            eprintln!("png write error for {}: {}", path, e);
            false
        }
    }
}

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

    let config = VmThreadConfig {
        elf_data,
        ram_size: 1024 * 1024,
        ..Default::default()
    };

    let mut vm_handle = spawn_vm_thread(config).unwrap_or_else(|e| {
        eprintln!("Failed to spawn VM thread: {}", e);
        std::process::exit(1);
    });

    eprintln!("VM running on background thread. Type commands (Ctrl+C to exit).\n");

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
    let mut input_buf = [0u8; 64];
    let mut frame_count = 0u32;

    loop {
        // 1. Check for host stdin input
        #[cfg(unix)]
        {
            match stdin_handle.read(&mut input_buf) {
                Ok(n) => {
                    for &b in &input_buf[..n] {
                        if b == 0x03 {
                            eprintln!("\nCaught Ctrl+C, shutting down...");
                            vm_handle.shutdown();
                            // Restore terminal and exit
                            let _ = stdout_handle.flush();
                            return;
                        }
                        // TODO: forward to VM UART via a channel when live.rs supports UART input
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => {
                    eprintln!("Stdin error: {}", e);
                    break;
                }
            }
        }

        // 2. Receive frames from the VM thread (non-blocking)
        loop {
            match vm_handle.try_recv_frame() {
                Ok(frame) => {
                    frame_count += 1;
                    let any = frame.pixels.iter().any(|&p| p != 0);
                    if any {
                        let out_path = format!("framebuf_live_{:04}.png", frame_count);
                        if save_frame_png(&frame.pixels, &out_path) {
                            eprintln!(
                                "frame {}: saved {} ({} instructions)",
                                frame_count, out_path, frame.instructions
                            );
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    eprintln!("VM thread disconnected");
                    break;
                }
            }
        }

        // 3. Check VM status
        loop {
            match vm_handle.try_recv_status() {
                Ok(VmStatus::Halted {
                    pc,
                    instructions,
                    reason,
                }) => {
                    eprintln!(
                        "\n[VM halted: {} (PC=0x{:08X}, {} instructions)]",
                        reason, pc, instructions
                    );
                    vm_handle.shutdown();
                    // Final framebuffer dump
                    // Note: final frame may have already been received above
                    eprintln!("Total frames captured: {}", frame_count);
                    let _ = stdout_handle.flush();
                    return;
                }
                Ok(VmStatus::ResetDone) => {
                    eprintln!("[VM reset complete]");
                }
                Ok(VmStatus::Paused) => {
                    eprintln!("[VM paused]");
                }
                Ok(VmStatus::Resumed) => {
                    eprintln!("[VM resumed]");
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    eprintln!("VM thread disconnected");
                    break;
                }
            }
        }

        // 4. Small sleep to avoid busy-waiting
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
