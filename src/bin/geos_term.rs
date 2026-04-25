// geos-term -- standalone single-window Geometry OS terminal.
//
// Boots straight into one program (default programs/host_term.asm) inside a
// minifb window. No infinite desktop, no map mode, no building icons -- just
// the VM, the program, and a window. Intended as the "open a shell" entry
// point so GeOS can replace gnome-terminal etc.
//
// Usage:
//   geos-term                        # runs programs/host_term.asm
//   geos-term programs/snake.asm     # runs any GeOS program
//   geos-term -- --scale 4           # 4x scale (1024x1024 window)

use minifb::{Key, KeyRepeat, Window, WindowOptions};

use geometry_os::assembler::assemble;
use geometry_os::keys::{key_to_ascii, key_to_ascii_shifted};
use geometry_os::preprocessor::Preprocessor;
use geometry_os::vm::Vm;

const VM_W: usize = 256;
const VM_H: usize = 256;

fn main() {
    let mut asm_path = String::from("programs/host_term.asm");
    let mut scale: usize = 3;
    let mut dump_frames: Option<usize> = None;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--scale" if i + 1 < args.len() => {
                scale = args[i + 1].parse().unwrap_or(3).clamp(1, 8);
                i += 2;
            }
            "--dump" if i + 1 < args.len() => {
                dump_frames = Some(args[i + 1].parse().unwrap_or(300));
                i += 2;
            }
            "-h" | "--help" => {
                eprintln!("Usage: geos-term [PROGRAM.asm] [--scale N] [--dump N]");
                eprintln!("  PROGRAM.asm  GeOS program to run (default: programs/host_term.asm)");
                eprintln!("  --scale N    Pixel scale factor (1-8, default 3 -> 768x768 window)");
                eprintln!("  --dump N     Run N frames headless then dump screen as ASCII (no window)");
                return;
            }
            other if !other.starts_with('-') => {
                asm_path = other.to_string();
                i += 1;
            }
            other => {
                eprintln!("unknown arg: {}", other);
                std::process::exit(2);
            }
        }
    }

    let source = std::fs::read_to_string(&asm_path).unwrap_or_else(|e| {
        eprintln!("read {}: {}", asm_path, e);
        std::process::exit(1);
    });
    let mut pp = Preprocessor::new();
    let preprocessed = pp.preprocess(&source);
    let asm = assemble(&preprocessed, 0).unwrap_or_else(|e| {
        eprintln!("assemble {}: line {}: {}", asm_path, e.line, e.message);
        std::process::exit(1);
    });

    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    // Headless dump mode: run N frames, dump screen as ASCII, exit
    if let Some(nframes) = dump_frames {
        eprintln!("[geos-term] headless mode: {} frames", nframes);
        for frame in 0..nframes {
            if vm.halted {
                eprintln!("[geos-term] VM halted at frame {}", frame);
                break;
            }
            vm.frame_ready = false;
            for _ in 0..1_000_000 {
                if !vm.step() {
                    break;
                }
                // No child processes in standalone mode -- skip scheduler
                if vm.frame_ready {
                    break;
                }
            }
            // Give PTY reader thread time to deliver data
            if frame % 100 == 99 {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        // Dump diagnostics
        let pty_handle = vm.ram[0x4E03];
        eprintln!("[geos-term] PTY slots: {} active, handle={}", 
            vm.pty_slots.iter().filter(|s| s.is_some()).count(), pty_handle);
        // Drain PTY channel directly to see what's left
        if pty_handle < vm.pty_slots.len() as u32 {
            if let Some(ref slot) = vm.pty_slots[pty_handle as usize] {
                let leftover = slot.drain_remaining();
                eprintln!("[geos-term] PTY channel leftover: {} bytes", leftover.len());
                if !leftover.is_empty() {
                    let hex: Vec<String> = leftover[..leftover.len().min(60)].iter()
                        .map(|b| format!("{:02x}", b)).collect();
                    eprintln!("[geos-term] hex: {}", hex.join(" "));
                    let txt: String = leftover.iter().map(|&b| {
                        if b >= 32 && b < 127 { b as char } else { '.' }
                    }).collect();
                    eprintln!("[geos-term] txt: '{}'", &txt[..txt.len().min(200)]);
                }
                eprintln!("[geos-term] PTY alive? {}", slot.is_alive());
            }
        }
        let cur_col = vm.ram[0x4E00];
        let cur_row = vm.ram[0x4E01];
        let ansi_state = vm.ram[0x4E04];
        eprintln!("[geos-term] cursor: col={}, row={}, ansi_state={}, pc={}, r28={}", 
            cur_col, cur_row, ansi_state, vm.pc, vm.regs[28]);
        // Sample text buffer rows
        for row in 0..3 {
            let mut sample = String::new();
            for col in 0..20 {
                let ch = vm.ram[0x4000 + row * 85 + col] & 0xFF;
                if ch >= 32 && ch < 127 { sample.push(ch as u8 as char); } else { sample.push('.'); }
            }
            eprintln!("[geos-term] buf row {}: '{}'", row, sample);
        }
        // Dump 256x256 screen as ASCII
        // Map pixel brightness to characters
        let chars = " .:-=+*#%@";
        for y in 0..VM_H {
            let mut line = String::with_capacity(VM_W);
            for x in 0..VM_W {
                let px = vm.screen[y * VM_W + x];
                let r = (px >> 16) & 0xFF;
                let g = (px >> 8) & 0xFF;
                let b = px & 0xFF;
                let bright = ((r as usize + g as usize + b as usize) * chars.len()) / (3 * 256 + 1);
                let idx = bright.min(chars.len() - 1);
                line.push(chars.chars().nth(idx).unwrap());
            }
            // Trim trailing spaces for readability
            let trimmed = line.trim_end();
            if !trimmed.is_empty() {
                println!("{}", trimmed);
            }
        }
        return;
    }

    let win_w = VM_W * scale;
    let win_h = VM_H * scale;
    let title = format!("geos-term -- {}", asm_path);
    let mut window = Window::new(
        &title,
        win_w,
        win_h,
        WindowOptions {
            resize: false,
            ..Default::default()
        },
    )
    .expect("Failed to open window. Ensure a display is available.");
    window.set_target_fps(60);

    let mut framebuffer = vec![0u32; win_w * win_h];

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Pump keys
        let shift = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
        for key in window.get_keys_pressed(KeyRepeat::Yes) {
            if let Some(ch) = key_to_ascii_shifted(key, shift) {
                vm.push_key(ch as u32);
            } else if let Some(ch) = key_to_ascii(key) {
                vm.push_key(ch as u32);
            }
        }

        // Step until FRAME or halt (cap to keep the window responsive)
        if !vm.halted {
            vm.frame_ready = false;
            for _ in 0..1_000_000 {
                if !vm.step() {
                    break;
                }
                vm.step_all_processes();
                if vm.frame_ready {
                    break;
                }
            }
        }

        // Blit vm.screen to framebuffer at integer scale
        for y in 0..VM_H {
            for x in 0..VM_W {
                let pixel = vm.screen[y * VM_W + x];
                let dst_y0 = y * scale;
                let dst_x0 = x * scale;
                for dy in 0..scale {
                    let row = (dst_y0 + dy) * win_w;
                    for dx in 0..scale {
                        framebuffer[row + dst_x0 + dx] = pixel;
                    }
                }
            }
        }

        if let Err(e) = window.update_with_buffer(&framebuffer, win_w, win_h) {
            eprintln!("present: {}", e);
            break;
        }
    }
}
