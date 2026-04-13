// main.rs -- Geometry OS Canvas Text Surface
//
// The canvas grid IS a text editor. Type assembly, press F8 to assemble,
// press F5 to run. Each keystroke writes a colored pixel glyph.
//
// Build: cargo run
// Test:  cargo test

mod assembler;
mod font;
mod vfs;
mod vm;
mod preprocessor;

use minifb::{Key, KeyRepeat, Window, WindowOptions};
use std::collections::{HashSet, VecDeque};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

// ── Layout constants ─────────────────────────────────────────────
const WIDTH: usize = 1024;
const HEIGHT: usize = 768;

// Canvas grid
const CANVAS_SCALE: usize = 16; // 16x16 screen pixels per cell
const CANVAS_COLS: usize = 32;
const CANVAS_ROWS: usize = 32; // visible rows on screen
const CANVAS_MAX_ROWS: usize = 128; // total logical rows (scrollable)

// VM screen (256x256, positioned to the right of the canvas)
const VM_SCREEN_X: usize = 640;
const VM_SCREEN_Y: usize = 64;

// Register display
const REGS_X: usize = 640;
const REGS_Y: usize = 340;

// RAM Inspector (32x32 grid of words, 8x8 pixels per word)
const RAM_VIEW_X: usize = 0;
const RAM_VIEW_Y: usize = 512;
const RAM_VIEW_SCALE: usize = 8;

// Global Heatmap (256x256, each pixel is 1 RAM word)
const HEATMAP_X: usize = 256;
const HEATMAP_Y: usize = 512;

// ── Memory map ───────────────────────────────────────────────────
// 0x000-0x3FF   Canvas grid (source text, 1024 cells visible on 32x32 grid)
// 0x1000-0x1FFF Assembled bytecode output (F8 writes here)
// 0xFFB         Key bitmask port (bits 0-5: up/down/left/right/space/enter, read-only)
// 0xFFD         ASM result port (bytecode word count on success, 0xFFFFFFFF on error)
// 0xFFE         TICKS port (frame counter, incremented each FRAME opcode, read-only)
// 0xFFF         Keyboard port (memory-mapped I/O, cleared on IKEY read)
const CANVAS_BYTECODE_ADDR: usize = 0x1000;
const KEYS_BITMASK_PORT: usize = 0xFFB;
const NET_PORT: usize = 0xFFC;
#[allow(dead_code)]
const TICKS_PORT: usize = 0xFFE;
const KEY_PORT: usize = 0xFFF;

// ── Save file ───────────────────────────────────────────────────
const SAVE_FILE: &str = "geometry_os.sav";

// ── Colors ───────────────────────────────────────────────────────
const BG: u32 = 0x050508;
const GRID_BG: u32 = 0x0A0A14;
const GRID_LINE: u32 = 0x141420;
const CURSOR_COL: u32 = 0x00FFFF;
const STATUS_FG: u32 = 0x888899;
const SCROLLBAR_BG: u32 = 0x181828;
const SCROLLBAR_FG: u32 = 0x334466;

// ── Syntax highlighting colors ──────────────────────────────────
const SYN_OPCODE: u32 = 0x00CCFF; // cyan -- opcodes (LDI, ADD, HALT, etc.)
const SYN_REGISTER: u32 = 0x44FF88; // green -- registers (r0-r31)
const SYN_NUMBER: u32 = 0xFFAA33; // orange -- immediate values
const SYN_LABEL: u32 = 0xFFDD44; // yellow -- label definitions and refs
const SYN_COMMENT: u32 = 0x555566; // gray -- comments (; ...)
const SYN_DEFAULT: u32 = 0xAAAA88; // default text color

// ── Terminal mode ──────────────────────────────────────────────────
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Terminal,
    Editor,
}

use std::sync::OnceLock;
use std::sync::mpsc::{channel, Sender};

static BEEP_SENDER: OnceLock<Sender<Vec<u8>>> = OnceLock::new();

fn get_beep_sender() -> &'static Sender<Vec<u8>> {
    BEEP_SENDER.get_or_init(|| {
        let (tx, rx) = channel::<Vec<u8>>();
        std::thread::spawn(move || {
            while let Ok(wav) = rx.recv() {
                if let Ok(mut child) = std::process::Command::new("aplay")
                    .args(["-q", "-t", "wav", "-"])
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = std::io::Write::write_all(&mut stdin, &wav);
                    }
                    let _ = child.wait();
                }
            }
        });
        tx
    })
}

/// Play a sine-wave tone by generating a PCM WAV and sending it to a worker thread.
fn play_beep(freq: u32, dur_ms: u32) {
    use std::f32::consts::PI;

    const SAMPLE_RATE: u32 = 22050;
    let num_samples = (SAMPLE_RATE * dur_ms / 1000).max(1) as usize;

    // Build a minimal 16-bit mono WAV in memory.
    let data_bytes = (num_samples * 2) as u32;
    let mut wav: Vec<u8> = Vec::with_capacity(44 + data_bytes as usize);
    let write_u32 = |v: u32| v.to_le_bytes();
    let write_u16 = |v: u16| v.to_le_bytes();

    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&write_u32(36 + data_bytes));
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&write_u32(16));           // chunk size
    wav.extend_from_slice(&write_u16(1));            // PCM
    wav.extend_from_slice(&write_u16(1));            // mono
    wav.extend_from_slice(&write_u32(SAMPLE_RATE));
    wav.extend_from_slice(&write_u32(SAMPLE_RATE * 2)); // byte rate
    wav.extend_from_slice(&write_u16(2));            // block align
    wav.extend_from_slice(&write_u16(16));           // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&write_u32(data_bytes));

    let amplitude = i16::MAX as f32 * 0.25;
    for i in 0..num_samples {
        let t = i as f32 / SAMPLE_RATE as f32;
        let sample = (amplitude * (2.0 * PI * freq as f32 * t).sin()) as i16;
        wav.extend_from_slice(&sample.to_le_bytes());
    }

    let _ = get_beep_sender().send(wav);
}

/// Write a text string into the canvas buffer at the given row.
/// Returns the next row index after the written line(s).
fn write_line_to_canvas(canvas_buffer: &mut [u32], row: usize, text: &str) -> usize {
    let mut r = row;
    if r >= CANVAS_MAX_ROWS {
        return r;
    }
    let bytes = text.as_bytes();
    let mut col = 0usize;
    for &b in bytes {
        if b == b'\n' || col >= CANVAS_COLS {
            // Pad rest of row with zeros
            while col < CANVAS_COLS {
                canvas_buffer[r * CANVAS_COLS + col] = 0;
                col += 1;
            }
            r += 1;
            if r >= CANVAS_MAX_ROWS {
                return r;
            }
            col = 0;
            if b == b'\n' {
                continue;
            }
            // b didn't fit, write it on new line
            canvas_buffer[r * CANVAS_COLS + col] = b as u32;
            col += 1;
        } else {
            canvas_buffer[r * CANVAS_COLS + col] = b as u32;
            col += 1;
        }
    }
    // Pad rest of this row
    while col < CANVAS_COLS {
        canvas_buffer[r * CANVAS_COLS + col] = 0;
        col += 1;
    }
    r + 1
}

/// Read text from a canvas buffer row (up to first null/newline).
fn read_canvas_line(canvas_buffer: &[u32], row: usize) -> String {
    let mut s = String::new();
    for col in 0..CANVAS_COLS {
        let val = canvas_buffer[row * CANVAS_COLS + col];
        let byte = (val & 0xFF) as u8;
        if byte == 0 || byte == 0x0A {
            break;
        }
        s.push(byte as char);
    }
    s
}

/// Handle a terminal command. Returns (switch_to_editor, should_quit).
fn handle_terminal_command(
    cmd: &str,
    vm: &mut vm::Vm,
    canvas_buffer: &mut Vec<u32>,
    output_row: &mut usize,
    scroll_offset: &mut usize,
    loaded_file: &mut Option<PathBuf>,
    canvas_assembled: &mut bool,
    breakpoints: &mut HashSet<u32>,
) -> (bool, bool) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        // Write a new "geo> " prompt
        *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
        ensure_scroll(*output_row, scroll_offset);
        return (false, false);
    }

    let command = parts[0].to_lowercase();
    match command.as_str() {
        "help" | "?" => {
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "Commands:");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  list              List .asm programs");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  load <file>       Load .asm onto canvas");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  run               Assemble canvas & run");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  edit              Switch to canvas editor");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  regs              Show register dump");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  peek <addr>       Read RAM[addr]");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  poke <addr> <val> Write RAM[addr]");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  step              Step one instruction");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  bp [addr]         Toggle/list breakpoints");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  bpc               Clear all breakpoints");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  trace [n]         Execute n steps with log");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  disasm [addr] [n] Disassemble n instrs");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  reset             Reset VM state");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  save [slot]       Save state to slot");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  load [slot]       Load state from slot");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  clear             Clear terminal");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  quit              Exit Geometry OS");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (false, false)
        }
        "list" | "ls" => {
            let files = list_asm_files("programs");
            if files.is_empty() {
                *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  (no .asm files in programs/)");
            } else {
                for f in &files {
                    let name = Path::new(f)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| f.clone());
                    *output_row = write_line_to_canvas(canvas_buffer, *output_row, &format!("  {}", name));
                }
                *output_row = write_line_to_canvas(
                    canvas_buffer,
                    *output_row,
                    &format!("  {} programs", files.len()),
                );
            }
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (false, false)
        }
        "load" => {
            if parts.len() < 2 {
                *output_row = write_line_to_canvas(canvas_buffer, *output_row, "Usage: load <file.asm> or load <slot>");
                *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
                ensure_scroll(*output_row, scroll_offset);
                return (false, false);
            }
            let filename_arg = parts[1..].join(" ");
            
            // If it ends in .asm or contains a path separator, assume source file
            if filename_arg.ends_with(".asm") || filename_arg.contains('/') || filename_arg.contains('\\') {
                let filename = filename_arg.clone();
                let path = Path::new(&filename);
                let path = if path.exists() {
                    path.to_path_buf()
                } else {
                    let prefixed = Path::new("programs").join(&filename);
                    if prefixed.exists() {
                        prefixed
                    } else {
                        *output_row = write_line_to_canvas(
                            canvas_buffer,
                            *output_row,
                            &format!("File not found: {}", filename),
                        );
                        *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
                        ensure_scroll(*output_row, scroll_offset);
                        return (false, false);
                    }
                };

                match std::fs::read_to_string(&path) {
                    Ok(source) => {
                        let mut cr = 0usize;
                        let mut cc = 0usize;
                        load_source_to_canvas(canvas_buffer, &source, &mut cr, &mut cc);
                        *loaded_file = Some(path.clone());
                        let name = path.file_name().unwrap().to_string_lossy();
                        let lines = source.lines().count();
                        *output_row = write_line_to_canvas(
                            canvas_buffer,
                            *output_row,
                            &format!("Loaded {} ({} lines)", name, lines),
                        );
                    }
                    Err(e) => {
                        *output_row = write_line_to_canvas(
                            canvas_buffer,
                            *output_row,
                            &format!("Error: {}", e),
                        );
                    }
                }
            } else {
                // Assume it's a state slot
                let filename = format!("geometry_os_{}.sav", filename_arg);
                match load_state(&filename) {
                    Ok((saved_vm, saved_canvas, saved_assembled)) => {
                        *vm = saved_vm;
                        *canvas_buffer = saved_canvas;
                        *canvas_assembled = saved_assembled;
                        let msg = format!("Loaded state from {}", filename);
                        *output_row = write_line_to_canvas(canvas_buffer, *output_row, &msg);
                    }
                    Err(_) => {
                        // Fallback: try loading as .asm if slot not found
                        let mut filename = filename_arg.clone();
                        filename.push_str(".asm");
                        let path = Path::new("programs").join(&filename);
                        if path.exists() {
                            if let Ok(source) = std::fs::read_to_string(&path) {
                                let mut cr = 0usize;
                                let mut cc = 0usize;
                                load_source_to_canvas(canvas_buffer, &source, &mut cr, &mut cc);
                                *loaded_file = Some(path.clone());
                                *output_row = write_line_to_canvas(canvas_buffer, *output_row, &format!("Loaded programs/{}", filename));
                            } else {
                                *output_row = write_line_to_canvas(canvas_buffer, *output_row, &format!("Slot {} not found and could not read .asm", filename_arg));
                            }
                        } else {
                            *output_row = write_line_to_canvas(canvas_buffer, *output_row, &format!("Slot or file {} not found", filename_arg));
                        }
                    }
                }
            }
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (false, false)
        }
        "run" => {
            let buffer_size = CANVAS_MAX_ROWS * CANVAS_COLS;
            let source: String = canvas_buffer[..buffer_size]
                .iter()
                .map(|&cell| {
                    let val = cell & 0xFF;
                    if val == 0 || val == 0x0A {
                        '\n'
                    } else {
                        (val as u8) as char
                    }
                })
                .collect();
            let source = source.replace("\n\n", "\n");

            match assembler::assemble(&source, CANVAS_BYTECODE_ADDR) {
                Ok(asm_result) => {
                    let ram_len = vm.ram.len();
                    for v in vm.ram
                        [CANVAS_BYTECODE_ADDR..ram_len.min(CANVAS_BYTECODE_ADDR + 4096)]
                        .iter_mut()
                    {
                        *v = 0;
                    }
                    for (i, &pixel) in asm_result.pixels.iter().enumerate() {
                        let addr = CANVAS_BYTECODE_ADDR + i;
                        if addr < ram_len {
                            vm.ram[addr] = pixel;
                        }
                    }
                    vm.pc = CANVAS_BYTECODE_ADDR as u32;
                    vm.halted = false;
                    *output_row = write_line_to_canvas(
                        canvas_buffer,
                        *output_row,
                        &format!("Assembled {} bytes at 0x1000", asm_result.pixels.len()),
                    );
                    // Run the VM
                    for _ in 0..10_000_000 {
                        if !vm.step() {
                            break;
                        }
                    }
                    if vm.halted {
                        *output_row = write_line_to_canvas(
                            canvas_buffer,
                            *output_row,
                            &format!("Halted at PC=0x{:04X}", vm.pc),
                        );
                    } else {
                        *output_row = write_line_to_canvas(
                            canvas_buffer,
                            *output_row,
                            &format!("Running... PC=0x{:04X}", vm.pc),
                        );
                    }
                    *canvas_assembled = true;
                }
                Err(e) => {
                    *output_row = write_line_to_canvas(
                        canvas_buffer,
                        *output_row,
                        &format!("{}", e),
                    );
                }
            }
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (false, false)
        }
        "edit" => {
            (true, false)
        }
        "regs" => {
            for row_group in 0..4 {
                let mut line = String::new();
                for col in 0..8 {
                    let i = row_group * 8 + col;
                    line.push_str(&format!("r{:02}={:08X} ", i, vm.regs[i]));
                }
                *output_row = write_line_to_canvas(canvas_buffer, *output_row, &line);
            }
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                &format!("PC={:04X} SP={:04X} LR={:04X}", vm.pc, vm.regs[30], vm.regs[31]),
            );
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (false, false)
        }
        "peek" => {
            if parts.len() < 2 {
                *output_row = write_line_to_canvas(canvas_buffer, *output_row, "Usage: peek <addr>");
                *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
                ensure_scroll(*output_row, scroll_offset);
                return (false, false);
            }
            match u32::from_str_radix(parts[1].trim_start_matches("0x").trim_start_matches("0X"), 16) {
                Ok(addr) if (addr as usize) < vm.ram.len() => {
                    let val = vm.ram[addr as usize];
                    *output_row = write_line_to_canvas(
                        canvas_buffer,
                        *output_row,
                        &format!("RAM[0x{:04X}] = 0x{:08X}", addr, val),
                    );
                }
                Ok(addr) => {
                    *output_row = write_line_to_canvas(
                        canvas_buffer,
                        *output_row,
                        &format!("Address 0x{:04X} out of range", addr),
                    );
                }
                Err(_) => {
                    *output_row = write_line_to_canvas(canvas_buffer, *output_row, "Invalid address");
                }
            }
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (false, false)
        }
        "poke" => {
            if parts.len() < 3 {
                *output_row = write_line_to_canvas(canvas_buffer, *output_row, "Usage: poke <addr> <val>");
                *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
                ensure_scroll(*output_row, scroll_offset);
                return (false, false);
            }
            let addr_str = parts[1].trim_start_matches("0x").trim_start_matches("0X");
            let val_str = parts[2].trim_start_matches("0x").trim_start_matches("0X");
            match (u32::from_str_radix(addr_str, 16), u32::from_str_radix(val_str, 16)) {
                (Ok(addr), Ok(val)) if (addr as usize) < vm.ram.len() => {
                    vm.ram[addr as usize] = val;
                    *output_row = write_line_to_canvas(
                        canvas_buffer,
                        *output_row,
                        &format!("RAM[0x{:04X}] <- 0x{:08X}", addr, val),
                    );
                }
                _ => {
                    *output_row = write_line_to_canvas(canvas_buffer, *output_row, "Usage: poke <hex_addr> <hex_val>");
                }
            }
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (false, false)
        }
        "step" => {
            if vm.halted {
                *output_row = write_line_to_canvas(canvas_buffer, *output_row, "VM halted. Use reset to restart.");
            } else {
                vm.step();
                *output_row = write_line_to_canvas(
                    canvas_buffer,
                    *output_row,
                    &format!("step -> PC=0x{:04X}", vm.pc),
                );
            }
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (false, false)
        }
        "bp" => {
            if parts.len() < 2 {
                // List breakpoints
                if breakpoints.is_empty() {
                    *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  No breakpoints set");
                } else {
                    let mut sorted: Vec<u32> = breakpoints.iter().copied().collect();
                    sorted.sort();
                    for addr in sorted {
                        *output_row = write_line_to_canvas(
                            canvas_buffer,
                            *output_row,
                            &format!("  BP @ 0x{:04X}", addr),
                        );
                    }
                }
            } else {
                match u32::from_str_radix(parts[1].trim_start_matches("0x").trim_start_matches("0X"), 16) {
                    Ok(addr) => {
                        if breakpoints.contains(&addr) {
                            breakpoints.remove(&addr);
                            *output_row = write_line_to_canvas(
                                canvas_buffer,
                                *output_row,
                                &format!("Cleared BP @ 0x{:04X}", addr),
                            );
                        } else {
                            breakpoints.insert(addr);
                            *output_row = write_line_to_canvas(
                                canvas_buffer,
                                *output_row,
                                &format!("Set BP @ 0x{:04X}", addr),
                            );
                        }
                    }
                    Err(_) => {
                        *output_row = write_line_to_canvas(canvas_buffer, *output_row, "Usage: bp <hex_addr>");
                    }
                }
            }
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (false, false)
        }
        "bpc" => {
            let n = breakpoints.len();
            breakpoints.clear();
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                &format!("Cleared {} breakpoint(s)", n),
            );
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (false, false)
        }
        "reset" => {
            vm.reset();
            *canvas_assembled = false;
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "VM reset");
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (false, false)
        }
        "save" => {
            let slot = parts.get(1).map(|&s| s);
            let filename = match slot {
                Some(s) => format!("geometry_os_{}.sav", s),
                None => SAVE_FILE.to_string(),
            };
            match save_state(&filename, vm, canvas_buffer, *canvas_assembled) {
                Ok(()) => {
                    let msg = format!("Saved state to {}", filename);
                    *output_row = write_line_to_canvas(canvas_buffer, *output_row, &msg);
                }
                Err(e) => {
                    let msg = format!("Save error: {}", e);
                    *output_row = write_line_to_canvas(canvas_buffer, *output_row, &msg);
                }
            }
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (false, false)
        }

        "clear" | "cls" => {
            for cell in canvas_buffer.iter_mut() {
                *cell = 0;
            }
            *output_row = 0;
            *output_row = write_line_to_canvas(canvas_buffer, 0, "geo> ");
            *scroll_offset = 0;
            (false, false)
        }
        "quit" | "exit" => (false, true),
        _ => {
            *output_row = write_line_to_canvas(
                canvas_buffer,
                *output_row,
                &format!("Unknown: {} (try help)", command),
            );
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
            ensure_scroll(*output_row, scroll_offset);
            (false, false)
        }
    }
}

/// Ensure scroll offset keeps the output row visible.
fn ensure_scroll(output_row: usize, scroll_offset: &mut usize) {
    if output_row >= *scroll_offset + CANVAS_ROWS {
        *scroll_offset = output_row - CANVAS_ROWS + 1;
    }
}

// ── CLI mode: headless geo> prompt on stdin/stdout ────────────────
fn cli_main(extra_args: &[String]) {
    let mut vm = vm::Vm::new();
    let mut canvas_assembled = false;
    let mut loaded_file: Option<PathBuf> = None;
    let mut source_text = String::new(); // holds the currently loaded source
    let mut cli_breakpoints: Vec<u32> = Vec::new();
    let mut canvas_buffer: Vec<u32> = vec![0; 4096];

    // If extra args given, treat first as a file to load
    if !extra_args.is_empty() {
        let path = PathBuf::from(&extra_args[0]);
        match std::fs::read_to_string(&path) {
            Ok(src) => {
                source_text = src;
                loaded_file = Some(path);
            }
            Err(e) => {
                eprintln!("Error reading {}: {}", extra_args[0], e);
            }
        }
    }

    println!("Geometry OS v1.0.0 CLI");
    println!("40 opcodes | 32 regs | 256x256");
    println!("Type 'help' for commands.");
    println!();

    let stdin = io::stdin();
    loop {
        print!("geo> ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if stdin.read_line(&mut line).unwrap() == 0 {
            break; // EOF
        }
        let cmd = line.trim();
        if cmd.is_empty() {
            continue;
        }

        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let command = parts[0].to_lowercase();
        match command.as_str() {
            "help" | "?" => {
                println!("Commands:");
                println!("  list              List .asm programs");
                println!("  load <file>       Load .asm source");
                println!("  run               Assemble source & run VM");
                println!("  regs              Show register dump");
                println!("  peek <addr>       Read RAM[addr]");
                println!("  poke <addr> <val> Write RAM[addr]");
                println!("  screen <addr>     Dump 16 pixels from screen buffer");
                println!("  reset             Reset VM state");
                println!("  step              Step one instruction");
                println!("  trace [n]         Execute n instructions with log");
                println!("  bp [addr]         Toggle/list breakpoints");
                println!("  bpc               Clear all breakpoints");
                println!("  disasm [addr] [n] Disassemble n instrs");
                println!("  quit              Exit");
            }
            "list" | "ls" => {
                let files = list_asm_files("programs");
                if files.is_empty() {
                    println!("  (no .asm files in programs/)");
                } else {
                    for f in &files {
                        let name = Path::new(f)
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| f.clone());
                        println!("  {}", name);
                    }
                    println!("  {} programs", files.len());
                }
            }
            "load" => {
                if parts.len() < 2 {
                    println!("Usage: load <file.asm> or load <slot>");
                    continue;
                }
                let filename_arg = parts[1..].join(" ");
                if filename_arg.ends_with(".asm") || filename_arg.contains('/') || filename_arg.contains('\\') {
                    let filename = filename_arg.clone();
                    let path = Path::new(&filename);
                    let path = if path.exists() {
                        path.to_path_buf()
                    } else {
                        let prefixed = Path::new("programs").join(&filename);
                        if prefixed.exists() {
                            prefixed
                        } else {
                            println!("File not found: {}", filename);
                            continue;
                        }
                    };
                    match std::fs::read_to_string(&path) {
                        Ok(src) => {
                            let lines = src.lines().count();
                            source_text = src;
                            loaded_file = Some(path.clone());
                            println!(
                                "Loaded {} ({} lines)",
                                path.file_name().unwrap().to_string_lossy(),
                                lines
                            );
                        }
                        Err(e) => {
                            println!("Error: {}", e);
                        }
                    }
                } else {
                    // Assume it's a state slot
                    let filename = format!("geometry_os_{}.sav", filename_arg);
                    match load_state(&filename) {
                        Ok((saved_vm, saved_canvas, saved_assembled)) => {
                            vm = saved_vm;
                            canvas_buffer = saved_canvas;
                            canvas_assembled = saved_assembled;
                            println!("Loaded state from {}", filename);
                        }
                        Err(_) => {
                            // Fallback: try loading as .asm if slot not found
                            let mut filename = filename_arg.clone();
                            filename.push_str(".asm");
                            let path = Path::new("programs").join(&filename);
                            if path.exists() {
                                if let Ok(src) = std::fs::read_to_string(&path) {
                                    source_text = src;
                                    loaded_file = Some(path.clone());
                                    println!("Loaded programs/{}", filename);
                                } else {
                                    println!("Slot {} not found and could not read .asm", filename_arg);
                                }
                            } else {
                                println!("Slot or file {} not found", filename_arg);
                            }
                        }
                    }
                }
            }
            "run" => {
                if source_text.is_empty() {
                    println!("No source loaded. Use 'load <file>' first.");
                    continue;
                }
                // Abstraction Layer: Preprocess macros and variables
                let mut pp = preprocessor::Preprocessor::new();
                let preprocessed_source = pp.preprocess(&source_text);

                match assembler::assemble(&preprocessed_source, 0) {
                    Ok(asm_result) => {
                        // Clear bytecode region (load at 0 so labels resolve correctly)
                        let ram_len = vm.ram.len();
                        let load_addr = 0usize;
                        for v in vm.ram[load_addr..ram_len.min(load_addr + 4096)].iter_mut() {
                            *v = 0;
                        }
                        for (i, &pixel) in asm_result.pixels.iter().enumerate() {
                            let addr = load_addr + i;
                            if addr < ram_len {
                                vm.ram[addr] = pixel;
                            }
                        }
                        vm.pc = load_addr as u32;
                        vm.halted = false;

                        println!(
                            "Assembled {} bytes at 0x{:04X}",
                            asm_result.pixels.len(),
                            load_addr
                        );

                        // Run the VM
                        let mut hit_bp = false;
                        for _ in 0..10_000_000 {
                            if !vm.step() {
                                break;
                            }
                            if cli_breakpoints.contains(&vm.pc) {
                                hit_bp = true;
                                break;
                            }
                        }
                        if hit_bp {
                            println!("BREAK @ PC=0x{:04X}", vm.pc);
                        } else if vm.halted {
                            println!("Halted at PC=0x{:04X}", vm.pc);
                        } else {
                            println!("Running... PC=0x{:04X}", vm.pc);
                        }
                        canvas_assembled = true;
                    }
                    Err(e) => {
                        println!("{}", e);
                    }
                }
            }
            "regs" => {
                for row_group in 0..4 {
                    let mut line = String::new();
                    for col in 0..8 {
                        let i = row_group * 8 + col;
                        line.push_str(&format!("r{:02}={:08X} ", i, vm.regs[i]));
                    }
                    println!("{}", line);
                }
                println!(
                    "PC={:04X} SP={:04X} LR={:04X}",
                    vm.pc, vm.regs[30], vm.regs[31]
                );
            }
            "peek" => {
                if parts.len() < 2 {
                    println!("Usage: peek <addr>");
                    continue;
                }
                match u32::from_str_radix(
                    parts[1].trim_start_matches("0x").trim_start_matches("0X"),
                    16,
                ) {
                    Ok(addr) if (addr as usize) < vm.ram.len() => {
                        let val = vm.ram[addr as usize];
                        println!("RAM[0x{:04X}] = 0x{:08X}", addr, val);
                    }
                    Ok(addr) => {
                        println!("Address 0x{:04X} out of range", addr);
                    }
                    Err(_) => {
                        println!("Invalid address");
                    }
                }
            }
            "poke" => {
                if parts.len() < 3 {
                    println!("Usage: poke <addr> <val>");
                    continue;
                }
                let addr_str = parts[1].trim_start_matches("0x").trim_start_matches("0X");
                let val_str = parts[2].trim_start_matches("0x").trim_start_matches("0X");
                match (
                    u32::from_str_radix(addr_str, 16),
                    u32::from_str_radix(val_str, 16),
                ) {
                    (Ok(addr), Ok(val)) if (addr as usize) < vm.ram.len() => {
                        vm.ram[addr as usize] = val;
                        println!("RAM[0x{:04X}] <- 0x{:08X}", addr, val);
                    }
                    _ => {
                        println!("Usage: poke <hex_addr> <hex_val>");
                    }
                }
            }
            "screen" => {
                // Dump 16 pixels from the screen buffer starting at addr
                let start = if parts.len() >= 2 {
                    u32::from_str_radix(
                        parts[1].trim_start_matches("0x").trim_start_matches("0X"),
                        16,
                    )
                    .unwrap_or(0) as usize
                } else {
                    0
                };
                for row in 0..4 {
                    let mut line = String::new();
                    for col in 0..4 {
                        let idx = start + row * 4 + col;
                        if idx < vm::SCREEN_SIZE {
                            line.push_str(&format!("{:06X} ", vm.screen[idx] & 0xFFFFFF));
                        } else {
                            line.push_str("------ ");
                        }
                    }
                    println!("{}", line);
                }
            }
            "save" => {
                let slot = parts.get(1).map(|&s| s);
                let filename = match slot {
                    Some(s) => format!("geometry_os_{}.sav", s),
                    None => SAVE_FILE.to_string(),
                };
                match save_state(&filename, &vm, &canvas_buffer, canvas_assembled) {
                    Ok(()) => println!("Saved state to {}", filename),
                    Err(e) => println!("Error saving state: {}", e),
                }
            }
            "ppm" => {
                let filename = if parts.len() >= 2 {
                    parts[1].to_string()
                } else {
                    "output.ppm".to_string()
                };
                match std::fs::File::create(&filename) {
                    Ok(mut f) => {
                        // PPM P6 format
                        let header = format!("P6\n256 256\n255\n");
                        use std::io::Write;
                        f.write_all(header.as_bytes()).unwrap();
                        for pixel in &vm.screen {
                            let r = (pixel >> 16) & 0xFF;
                            let g = (pixel >> 8) & 0xFF;
                            let b = pixel & 0xFF;
                            f.write_all(&[r as u8, g as u8, b as u8]).unwrap();
                        }
                        println!("Saved screen to {}", filename);
                    }
                    Err(e) => println!("Error saving: {}", e),
                }
            }
            "step" => {
                if vm.halted {
                    println!("VM halted. Use reset to restart.");
                } else {
                    vm.step();
                    println!("step -> PC=0x{:04X}", vm.pc);
                }
            }
            "bp" => {
                if parts.len() < 2 {
                    if cli_breakpoints.is_empty() {
                        println!("  No breakpoints set");
                    } else {
                        for &addr in &cli_breakpoints {
                            println!("  BP @ 0x{:04X}", addr);
                        }
                    }
                } else {
                    match u32::from_str_radix(parts[1].trim_start_matches("0x").trim_start_matches("0X"), 16) {
                        Ok(addr) => {
                            if let Some(pos) = cli_breakpoints.iter().position(|&a| a == addr) {
                                cli_breakpoints.remove(pos);
                                println!("Cleared BP @ 0x{:04X}", addr);
                            } else {
                                cli_breakpoints.push(addr);
                                println!("Set BP @ 0x{:04X}", addr);
                            }
                        }
                        Err(_) => println!("Invalid address"),
                    }
                }
            }
            "bpc" => {
                cli_breakpoints.clear();
                println!("Breakpoints cleared");
            }
            "trace" => {
                // trace [count] — execute N instructions, logging each one
                let count = if parts.len() >= 2 {
                    parts[1].parse::<usize>().unwrap_or(20)
                } else {
                    20
                };
                if vm.halted {
                    println!("VM halted. Use reset to restart.");
                } else {
                    for i in 0..count {
                        let addr_before = vm.pc;
                        let (mnemonic, _len) = vm.disassemble_at(vm.pc);
                        if !vm.step() {
                            println!("{:04} {:04X} {:30} -> HALTED", i, addr_before, mnemonic);
                            break;
                        }
                        // Show non-zero registers (up to 4 most interesting)
                        let mut reg_info = String::new();
                        let mut shown = 0;
                        // Always show PC and any regs that were likely modified
                        for r in 0..8 {
                            if vm.regs[r] != 0 && shown < 4 {
                                reg_info.push_str(&format!(" r{}={:X}", r, vm.regs[r]));
                                shown += 1;
                            }
                        }
                        if reg_info.is_empty() {
                            reg_info = " (no regs changed)".to_string();
                        }
                        println!("{:04} {:04X} {:30} -> {:04X}{}", i, addr_before, mnemonic, vm.pc, reg_info);
                        if cli_breakpoints.contains(&vm.pc) {
                            println!("BREAK @ PC=0x{:04X}", vm.pc);
                            break;
                        }
                    }
                }
            }
            "disasm" => {
                // disasm [addr] [count] — defaults to PC, 10 lines
                let start_addr = if parts.len() >= 2 {
                    u32::from_str_radix(parts[1].trim_start_matches("0x"), 16)
                        .unwrap_or(vm.pc)
                } else {
                    vm.pc
                };
                let count = if parts.len() >= 3 {
                    parts[2].parse::<usize>().unwrap_or(10)
                } else {
                    10
                };
                let mut addr = start_addr;
                for _ in 0..count {
                    if addr as usize >= vm.ram.len() { break; }
                    let (mnemonic, len) = vm.disassemble_at(addr);
                    let marker = if addr == vm.pc { ">" } else { " " };
                    println!(" {}{:04X} {}", marker, addr, mnemonic);
                    addr += len as u32;
                }
            }
            "reset" => {
                vm.reset();
                canvas_assembled = false;
                println!("VM reset");
            }
            "hermes" => {
                if parts.len() < 2 {
                    println!("Usage: hermes <prompt>");
                    println!("  Starts an agent loop driven by a local LLM.");
                    println!("  The LLM can run geo> commands to accomplish tasks.");
                    println!("  Requires Ollama running locally (qwen3.5-tools).");
                    continue;
                }
                let user_prompt = parts[1..].join(" ");
                run_hermes_loop(&user_prompt, &mut vm, &mut source_text, &mut loaded_file, &mut canvas_assembled);
            }
            "quit" | "exit" => {
                break;
            }
            _ => {
                println!("Unknown: {} (try help)", command);
            }
        }
    }
}

// ── Hermes: local LLM agent loop ──────────────────────────────────

const HERMES_SYSTEM_PROMPT: &str = r#"You are an agent inside the Geometry OS terminal. You drive a bytecode VM by issuing geo> commands.

## Available commands
- load <file>       Load .asm source (from programs/ dir or absolute path)
- run               Assemble source & run VM
- regs              Show register dump (r0-r31, PC, SP, LR)
- peek <hex_addr>   Read RAM[addr]
- poke <hex_addr> <hex_val>  Write RAM[addr]
- screen [addr]     Dump 16 pixels from screen buffer
- save [file.ppm]   Save screen as PPM image
- png [file.png]    Save screen as PNG image
- reset             Reset VM state
- help              Show commands

## Instruction set (assembly mnemonics)
- LDI reg, imm      Load immediate (hex: 0x10)
- LOAD reg, addr_r  Load from RAM (0x11)
- STORE addr_r, reg Store to RAM (0x12)
- ADD/SUB/MUL/DIV rd, rs  Arithmetic (0x20-0x23)
- AND/OR/XOR rd, rs  Bitwise (0x24-0x26)
- SHL/SHR rd, rs    Shift (0x27-0x28)
- MOD rd, rs        Modulo (0x29)
- JMP addr          Unconditional jump (0x30)
- JZ/JNZ reg, addr  Conditional jump (0x31-0x32)
- CALL addr / RET   Subroutine (0x33-0x34)
- BLT/BGE reg, addr Branch on CMP (0x35-0x36)
- PUSH reg / POP reg Stack (0x60-0x61), SP=r30
- PSET xr,yr,cr     Set pixel (0x40)
- PSETI x,y,color   Set pixel immediate (0x41)
- FILL cr            Fill screen (0x42)
- RECTF xr,yr,wr,hr,cr  Filled rect (0x43)
- TEXT xr,yr,ar      Render text (0x44)
- CMP rd, rs         Compare, sets r0 (0x50)
- PEEK rx,ry,rd      Read screen pixel (0x4F)
- SYSCALL num        Trap into kernel mode (0x52)
- RETK               Return from kernel to user mode (0x53)
- HALT (0x00), NOP (0x01)

## Response format
Respond with one geo> command per line. No explanation, no markdown, no backticks.
Just the commands you want executed. You can also write new .asm programs by using
the write command:
  write <filename.asm>  (then subsequent lines are the file content, end with ENDWRITE on its own line)

After your commands run, you'll see the output and can issue more commands.
Think step by step but only output commands."#;

fn build_hermes_context(vm: &vm::Vm, source_text: &str, loaded_file: &Option<PathBuf>) -> String {
    let mut ctx = String::new();

    // VM state
    ctx.push_str("## Current VM State\n");
    for row_group in 0..4 {
        let mut line = String::new();
        for col in 0..8 {
            let i = row_group * 8 + col;
            line.push_str(&format!("r{:02}={:08X} ", i, vm.regs[i]));
        }
        ctx.push_str(&line);
        ctx.push('\n');
    }
    ctx.push_str(&format!(
        "PC={:04X} SP={:04X} LR={:04X}\n",
        vm.pc, vm.regs[30], vm.regs[31]
    ));
    ctx.push_str(&format!("Halted: {}\n", vm.halted));

    // Loaded file
    if let Some(ref f) = loaded_file {
        ctx.push_str(&format!("\n## Loaded file: {}\n", f.display()));
    }

    // Source text (first 100 lines)
    if !source_text.is_empty() {
        ctx.push_str("\n## Current source (first 100 lines)\n");
        for (i, line) in source_text.lines().take(100).enumerate() {
            ctx.push_str(&format!("{:3}: {}\n", i + 1, line));
        }
        let total = source_text.lines().count();
        if total > 100 {
            ctx.push_str(&format!("... ({} more lines)\n", total - 100));
        }
    }

    ctx
}

fn call_ollama(system_prompt: &str, user_message: &str) -> Option<String> {

    // Build the JSON payload
    // Escape strings for JSON
    let esc_sys = system_prompt
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t");
    let esc_user = user_message
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t");

    let payload = format!(
        r#"{{"model":"qwen3.5-tools","messages":[{{"role":"system","content":"{}"}},{{"role":"user","content":"{}"}}],"stream":false}}"#,
        esc_sys, esc_user
    );

    // Write payload to temp file to avoid shell escaping issues
    let tmp_path = "/tmp/geo_hermes_payload.json";
    match std::fs::write(tmp_path, &payload) {
        Ok(()) => {}
        Err(e) => {
            println!("[hermes] Error writing payload: {}", e);
            return None;
        }
    }

    let output = match std::process::Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            "http://localhost:11434/api/chat",
            "-d",
            &format!("@{}", tmp_path),
            "-H",
            "Content-Type: application/json",
        ])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            println!("[hermes] curl failed: {}", e);
            return None;
        }
    };

    // Parse response -- extract message.content
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Simple JSON extraction: find "content":"..."`
    // Look for the content field in the response
    if let Some(start) = stdout.find(r#""content":""#) {
        let content_start = start + r#""content":""#.len();
        // Find the closing quote (handle escaped quotes)
        let mut i = content_start;
        let mut result = String::new();
        let bytes = stdout.as_bytes();
        while i < bytes.len() {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                // Escaped character
                match bytes[i + 1] {
                    b'n' => result.push('\n'),
                    b't' => result.push('\t'),
                    b'"' => result.push('"'),
                    b'\\' => result.push('\\'),
                    _ => {
                        result.push(bytes[i] as char);
                        result.push(bytes[i + 1] as char);
                    }
                }
                i += 2;
            } else if bytes[i] == b'"' {
                break;
            } else {
                result.push(bytes[i] as char);
                i += 1;
            }
        }
        Some(result)
    } else {
        println!("[hermes] Could not parse LLM response");
        None
    }
}

fn run_hermes_loop(
    initial_prompt: &str,
    vm: &mut vm::Vm,
    source_text: &mut String,
    loaded_file: &mut Option<PathBuf>,
    canvas_assembled: &mut bool,
) {
    println!("[hermes] Starting agent loop (qwen3.5-tools via Ollama)");
    println!("[hermes] Type 'stop' to end the loop, or just press Enter to continue.");

    let mut conversation_history = initial_prompt.to_string();

    for iteration in 0..10 {
        // Build context
        let ctx = build_hermes_context(vm, source_text, loaded_file);
        let full_system = format!("{}\n\n{}", HERMES_SYSTEM_PROMPT, ctx);

        println!("[hermes] --- iteration {} ---", iteration + 1);

        // Call LLM
        let response = match call_ollama(&full_system, &conversation_history) {
            Some(r) => r,
            None => {
                println!("[hermes] LLM call failed. Stopping.");
                break;
            }
        };

        // Strip <think/> blocks (qwen3.5 includes reasoning)
        // Also handle unicode-escaped versions: \u003cthink\u003e
        let response_clean = response
            .replace("\\u003cthink\\u003e", "<think")
            .replace("\\u003c/think\\u003e", "</think");
        let mut commands = String::new();
        let mut in_think = false;
        for line in response_clean.lines() {
            if line.contains("<think") {
                in_think = true;
            }
            if !in_think {
                let trimmed = line.trim();
                if !trimmed.is_empty() && !trimmed.starts_with("//") {
                    commands.push_str(trimmed);
                    commands.push('\n');
                }
            }
            if line.contains("</think") {
                in_think = false;
            }
        }

        if commands.trim().is_empty() {
            println!("[hermes] LLM returned no commands. Stopping.");
            break;
        }

        println!("[hermes] LLM commands:\n{}", commands);

        // Track any write buffers
        let mut write_buffer: Option<(String, String)> = None;

        // Execute each command
        let mut output_capture = String::new();
        for cmd_line in commands.lines() {
            let cmd_line = cmd_line.trim();
            if cmd_line.is_empty() { continue; }

            // Handle write command for creating .asm files
            if let Some(ref mut wb) = write_buffer {
                if cmd_line == "ENDWRITE" {
                    // Write the file
                    match std::fs::write(&wb.0, &wb.1) {
                        Ok(()) => {
                            let msg = format!("Wrote {}", wb.0);
                            println!("{}", msg);
                            output_capture.push_str(&msg);
                            output_capture.push('\n');
                        }
                        Err(e) => {
                            let msg = format!("Write error: {}", e);
                            println!("{}", msg);
                            output_capture.push_str(&msg);
                            output_capture.push('\n');
                        }
                    }
                    write_buffer = None;
                } else {
                    wb.1.push_str(cmd_line);
                    wb.1.push('\n');
                }
                continue;
            }

            if cmd_line.starts_with("write ") {
                let filename = cmd_line.strip_prefix("write ").unwrap().trim();
                write_buffer = Some((filename.to_string(), String::new()));
                continue;
            }

            // Skip non-geo commands
            let cmd_parts: Vec<&str> = cmd_line.split_whitespace().collect();
            if cmd_parts.is_empty() { continue; }
            let cmd_word = cmd_parts[0].to_lowercase();

            // Only execute known geo> commands
            match cmd_word.as_str() {
                "load" | "run" | "regs" | "peek" | "poke" | "screen" | "save" | "reset" | "list" | "ls" | "png" => {
                    println!("geo> {}", cmd_line);
                    // Capture output by redirecting through a helper
                    execute_cli_command(
                        cmd_line, vm, source_text, loaded_file, canvas_assembled,
                        &mut output_capture,
                    );
                }
                _ => {
                    // Skip unknown commands silently
                }
            }
        }

        // Handle unclosed write buffer
        if let Some(wb) = write_buffer {
            match std::fs::write(&wb.0, &wb.1) {
                Ok(()) => println!("Wrote {}", wb.0),
                Err(e) => println!("Write error: {}", e),
            }
        }

        // Ask if user wants to continue
        print!("[hermes] Continue? (Enter=continue, stop=quit): ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).unwrap() == 0 {
            break;
        }
        let answer = input.trim().to_lowercase();
        if answer == "stop" || answer == "quit" || answer == "exit" || answer == "q" {
            println!("[hermes] Stopped.");
            break;
        }

        // Feed output back as context for next iteration
        conversation_history = format!(
            "Previous commands output:\n{}\n\nUser instruction: {}",
            output_capture,
            if answer.is_empty() { "continue" } else { &answer }
        );
    }

    println!("[hermes] Agent loop ended.");
}

/// Execute a single CLI command and capture output.
fn execute_cli_command(
    cmd: &str,
    vm: &mut vm::Vm,
    source_text: &mut String,
    loaded_file: &mut Option<PathBuf>,
    canvas_assembled: &mut bool,
    output: &mut String,
) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() { return; }
    let command = parts[0].to_lowercase();

    match command.as_str() {
        "list" | "ls" => {
            let files = list_asm_files("programs");
            if files.is_empty() {
                let msg = "  (no .asm files in programs/)".to_string();
                println!("{}", msg); output.push_str(&msg); output.push('\n');
            } else {
                for f in &files {
                    let name = Path::new(f).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| f.clone());
                    let msg = format!("  {}", name);
                    println!("{}", msg); output.push_str(&msg); output.push('\n');
                }
                let msg = format!("  {} programs", files.len());
                println!("{}", msg); output.push_str(&msg); output.push('\n');
            }
        }
        "load" => {
            if parts.len() < 2 {
                let msg = "Usage: load <file>".to_string();
                println!("{}", msg); output.push_str(&msg); output.push('\n');
                return;
            }
            let mut filename = parts[1..].join(" ");
            if !filename.ends_with(".asm") { filename.push_str(".asm"); }
            let path = Path::new(&filename);
            let path = if path.exists() {
                path.to_path_buf()
            } else {
                let prefixed = Path::new("programs").join(&filename);
                if prefixed.exists() { prefixed } else {
                    let msg = format!("File not found: {}", filename);
                    println!("{}", msg); output.push_str(&msg); output.push('\n');
                    return;
                }
            };
            match std::fs::read_to_string(&path) {
                Ok(src) => {
                    let lines = src.lines().count();
                    *source_text = src;
                    *loaded_file = Some(path.clone());
                    let msg = format!("Loaded {} ({} lines)", path.file_name().unwrap().to_string_lossy(), lines);
                    println!("{}", msg); output.push_str(&msg); output.push('\n');
                }
                Err(e) => {
                    let msg = format!("Error: {}", e);
                    println!("{}", msg); output.push_str(&msg); output.push('\n');
                }
            }
        }
        "run" => {
            if source_text.is_empty() {
                let msg = "No source loaded.".to_string();
                println!("{}", msg); output.push_str(&msg); output.push('\n');
                return;
            }
            // Abstraction Layer: Preprocess macros and variables
            let mut pp = preprocessor::Preprocessor::new();
            let preprocessed_source = pp.preprocess(source_text);

            match assembler::assemble(&preprocessed_source, 0) {
                Ok(asm_result) => {
                    let ram_len = vm.ram.len();
                    let load_addr = 0usize;
                    for v in vm.ram[load_addr..ram_len.min(load_addr + 4096)].iter_mut() { *v = 0; }
                    for (i, &pixel) in asm_result.pixels.iter().enumerate() {
                        let addr = load_addr + i;
                        if addr < ram_len { vm.ram[addr] = pixel; }
                    }
                    vm.pc = load_addr as u32;
                    vm.halted = false;
                    let msg = format!("Assembled {} bytes at 0x{:04X}", asm_result.pixels.len(), load_addr);
                    println!("{}", msg); output.push_str(&msg); output.push('\n');
                    for _ in 0..10_000_000 {
                        if !vm.step() { break; }
                    }
                    let msg = if vm.halted {
                        format!("Halted at PC=0x{:04X}", vm.pc)
                    } else {
                        format!("Running... PC=0x{:04X}", vm.pc)
                    };
                    println!("{}", msg); output.push_str(&msg); output.push('\n');
                    *canvas_assembled = true;
                }
                Err(e) => {
                    let msg = format!("{}", e);
                    println!("{}", msg); output.push_str(&msg); output.push('\n');
                }
            }
        }
        "regs" => {
            for row_group in 0..4 {
                let mut line = String::new();
                for col in 0..8 {
                    let i = row_group * 8 + col;
                    line.push_str(&format!("r{:02}={:08X} ", i, vm.regs[i]));
                }
                println!("{}", line); output.push_str(&line); output.push('\n');
            }
            let line = format!("PC={:04X} SP={:04X} LR={:04X}", vm.pc, vm.regs[30], vm.regs[31]);
            println!("{}", line); output.push_str(&line); output.push('\n');
        }
        "peek" => {
            if parts.len() < 2 {
                let msg = "Usage: peek <addr>".to_string();
                println!("{}", msg); output.push_str(&msg); output.push('\n');
                return;
            }
            match u32::from_str_radix(parts[1].trim_start_matches("0x").trim_start_matches("0X"), 16) {
                Ok(addr) if (addr as usize) < vm.ram.len() => {
                    let val = vm.ram[addr as usize];
                    let msg = format!("RAM[0x{:04X}] = 0x{:08X}", addr, val);
                    println!("{}", msg); output.push_str(&msg); output.push('\n');
                }
                Ok(addr) => {
                    let msg = format!("Address 0x{:04X} out of range", addr);
                    println!("{}", msg); output.push_str(&msg); output.push('\n');
                }
                Err(_) => {
                    let msg = "Invalid address".to_string();
                    println!("{}", msg); output.push_str(&msg); output.push('\n');
                }
            }
        }
        "poke" => {
            if parts.len() < 3 {
                let msg = "Usage: poke <addr> <val>".to_string();
                println!("{}", msg); output.push_str(&msg); output.push('\n');
                return;
            }
            let addr_str = parts[1].trim_start_matches("0x").trim_start_matches("0X");
            let val_str = parts[2].trim_start_matches("0x").trim_start_matches("0X");
            match (u32::from_str_radix(addr_str, 16), u32::from_str_radix(val_str, 16)) {
                (Ok(addr), Ok(val)) if (addr as usize) < vm.ram.len() => {
                    vm.ram[addr as usize] = val;
                    let msg = format!("RAM[0x{:04X}] <- 0x{:08X}", addr, val);
                    println!("{}", msg); output.push_str(&msg); output.push('\n');
                }
                _ => {
                    let msg = "Usage: poke <hex_addr> <hex_val>".to_string();
                    println!("{}", msg); output.push_str(&msg); output.push('\n');
                }
            }
        }
        "screen" => {
            let start = if parts.len() >= 2 {
                u32::from_str_radix(parts[1].trim_start_matches("0x").trim_start_matches("0X"), 16)
                    .unwrap_or(0) as usize
            } else { 0 };
            for row in 0..4 {
                let mut line = String::new();
                for col in 0..4 {
                    let idx = start + row * 4 + col;
                    if idx < vm::SCREEN_SIZE {
                        line.push_str(&format!("{:06X} ", vm.screen[idx] & 0xFFFFFF));
                    } else {
                        line.push_str("------ ");
                    }
                }
                println!("{}", line); output.push_str(&line); output.push('\n');
            }
        }
        "save" => {
            let filename = if parts.len() >= 2 { parts[1].to_string() } else { "output.ppm".to_string() };
            match std::fs::File::create(&filename) {
                Ok(mut f) => {
                    let header = format!("P6\n256 256\n255\n");
                    use std::io::Write;
                    let _ = f.write_all(header.as_bytes());
                    for pixel in &vm.screen {
                        let r = (pixel >> 16) & 0xFF;
                        let g = (pixel >> 8) & 0xFF;
                        let b = pixel & 0xFF;
                        let _ = f.write_all(&[r as u8, g as u8, b as u8]);
                    }
                    let msg = format!("Saved screen to {}", filename);
                    println!("{}", msg); output.push_str(&msg); output.push('\n');
                }
                Err(e) => {
                    let msg = format!("Error saving: {}", e);
                    println!("{}", msg); output.push_str(&msg); output.push('\n');
                }
            }
        }
        "png" => {
            let filename = if parts.len() >= 2 { parts[1].to_string() } else { "screenshot.png".to_string() };
            match save_screen_png(&filename, &vm.screen) {
                Ok(()) => {
                    let msg = format!("Saved screenshot to {}", filename);
                    println!("{}", msg); output.push_str(&msg); output.push('\n');
                }
                Err(e) => {
                    let msg = format!("Error saving PNG: {}", e);
                    println!("{}", msg); output.push_str(&msg); output.push('\n');
                }
            }
        }
        "reset" => {
            vm.reset();
            *canvas_assembled = false;
            let msg = "VM reset".to_string();
            println!("{}", msg); output.push_str(&msg); output.push('\n');
        }
        _ => {
            let msg = format!("Unknown: {} (skipped)", command);
            println!("{}", msg); output.push_str(&msg); output.push('\n');
        }
    }
}

fn main() {
    // ── CLI mode: headless geo> prompt on stdin/stdout ─────────────
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "--cli" {
        cli_main(&args[2..]);
        return;
    }

    // Networking setup
    let mut local_port = 9000;
    let mut remote_port = 9001;
    let mut boot_mode = false;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--local-port" && i + 1 < args.len() {
            local_port = args[i + 1].parse().unwrap_or(9000);
            i += 2;
        } else if args[i] == "--remote-port" && i + 1 < args.len() {
            remote_port = args[i + 1].parse().unwrap_or(9001);
            i += 2;
        } else if args[i] == "--boot" {
            boot_mode = true;
            i += 1;
        } else {
            i += 1;
        }
    }
    let socket = std::net::UdpSocket::bind(format!("127.0.0.1:{}", local_port)).ok();
    if let Some(ref s) = socket {
        s.set_nonblocking(true).unwrap();
    }

    let mut window = Window::new(
        "Geometry OS -- Canvas Text Surface",
        WIDTH,
        HEIGHT,
        WindowOptions {
            resize: false,
            ..Default::default()
        },
    )
    .unwrap();

    window.set_target_fps(60);

    let mut buffer: Vec<u32> = vec![0; WIDTH * HEIGHT];

    // ── State ────────────────────────────────────────────────────
    let mut vm = vm::Vm::new();
    let mut is_running = false;
    let mut canvas_assembled = false;
    let mut breakpoints: HashSet<u32> = HashSet::new();
    let mut hit_breakpoint = false;
    let mut recording = false;
    let mut frame_id = 0;

    // Visual Debugger state
    let mut ram_intensity = vec![0.0f32; vm::RAM_SIZE];
    let mut ram_kind = vec![vm::MemAccessKind::Read; vm::RAM_SIZE];
    let mut pc_history: VecDeque<u32> = VecDeque::with_capacity(64);
    let mut ram_view_base: usize = 0x2000;

    // Cursor position on canvas (logical coordinates, can exceed visible area)
    let mut cursor_row: usize;
    let mut cursor_col: usize;

    // Scroll offset: which logical row is at the top of the visible window
    let mut scroll_offset: usize;

    // Canvas backing buffer (separate from VM RAM to allow > 32 rows
    // without overlapping bytecode at 0x1000)
    let mut canvas_buffer: Vec<u32> = vec![0; CANVAS_MAX_ROWS * CANVAS_COLS];

    // Status bar message
    let mut status_msg = String::from("[TERM: type commands, Enter=run]");

    // Last loaded file (for Ctrl+F8 reload)
    let mut loaded_file: Option<PathBuf> = None;

    // If --boot flag, perform boot sequence: load init.asm as PID 1
    if boot_mode {
        match vm.boot() {
            Ok(pid) => {
                status_msg = format!("[BOOT] init started as PID {}", pid);
                is_running = true;
            }
            Err(e) => {
                status_msg = format!("[BOOT FAILED] {}", e);
            }
        }
    }

    // ── Mode state ──────────────────────────────────────────────
    let mut mode = Mode::Terminal;
    // In terminal mode, track which row the prompt is on
    let mut term_prompt_row: usize;
    // The "output row" for terminal -- where next line goes
    let mut term_output_row: usize;

    // Boot: write welcome banner + first prompt into canvas
    {
        term_output_row = write_line_to_canvas(&mut canvas_buffer, 0, "Geometry OS v1.0.0");
        term_output_row = write_line_to_canvas(&mut canvas_buffer, term_output_row, "40 opcodes | 32 regs | 256x256");
        term_output_row = write_line_to_canvas(&mut canvas_buffer, term_output_row, "Type 'help' for commands.");
        term_output_row = write_line_to_canvas(&mut canvas_buffer, term_output_row, "");
        term_prompt_row = term_output_row;
        let _ = write_line_to_canvas(&mut canvas_buffer, term_output_row, "geo> ");
        // Position cursor after "geo> "
        cursor_row = term_prompt_row;
        cursor_col = 5; // after "geo> "
        scroll_offset = 0;
    }

    // File input mode (Ctrl+F8 activates this)
    let mut file_input_mode = false;
    let mut file_input_buf = String::new();
    let mut file_completions: Vec<String> = Vec::new();
    let mut file_completion_idx: usize = 0;

    // Load file from command-line argument at startup
    if let Some(path_str) = std::env::args().nth(1) {
        let path = PathBuf::from(&path_str);
        if let Ok(source) = std::fs::read_to_string(&path) {
            load_source_to_canvas(
                &mut canvas_buffer,
                &source,
                &mut cursor_row,
                &mut cursor_col,
            );
            scroll_offset = 0;
            status_msg = format!("[loaded: {}]", path.display());
            loaded_file = Some(path);
        } else {
            status_msg = format!("[error: could not read {}]", path_str);
        }
    }

    // Restore saved state on startup (only if no command-line arg)
    if std::env::args().nth(1).is_none() {
        if let Ok((saved_vm, saved_canvas, saved_assembled)) = load_state(SAVE_FILE) {
            vm = saved_vm;
            canvas_buffer = saved_canvas;
            canvas_assembled = saved_assembled;
            status_msg = String::from("[state restored from geometry_os.sav]");
        }
    }

    // ── Main loop ────────────────────────────────────────────────
    let mut should_quit = false;
    while window.is_open() && !should_quit {
        // ── Handle input ─────────────────────────────────────────
        if is_running {
            let mut mask: u32 = 0;
            if window.is_key_down(Key::Up)    || window.is_key_down(Key::W) { mask |= 1 << 0; }
            if window.is_key_down(Key::Down)  || window.is_key_down(Key::S) { mask |= 1 << 1; }
            if window.is_key_down(Key::Left)  || window.is_key_down(Key::A) { mask |= 1 << 2; }
            if window.is_key_down(Key::Right) || window.is_key_down(Key::D) { mask |= 1 << 3; }
            if window.is_key_down(Key::Space) { mask |= 1 << 4; }
            if window.is_key_down(Key::Enter) { mask |= 1 << 5; }
            vm.ram[KEYS_BITMASK_PORT] = mask;

            // ── Networking ───────────────────────────────────────
            if let Some(ref s) = socket {
                let val = vm.ram[NET_PORT];
                if val != 0 {
                    // VM wrote something, send it
                    let _ = s.send_to(&val.to_le_bytes(), format!("127.0.0.1:{}", remote_port));
                    vm.ram[NET_PORT] = 0; // clear after send
                } else {
                    // Port is empty, try to receive
                    let mut buf = [0u8; 4];
                    if let Ok((amt, _src)) = s.recv_from(&mut buf) {
                        if amt == 4 {
                            vm.ram[NET_PORT] = u32::from_le_bytes(buf);
                        }
                    }
                }
            }
        }

        for key in window.get_keys_pressed(KeyRepeat::No) {
            if is_running {
                // Runtime: send keys to VM keyboard port
                if let Some(ch) = key_to_ascii(key) {
                    vm.ram[KEY_PORT] = ch as u32;
                }
                continue;
            }

            // Escape: in editor mode, switch back to terminal. In terminal, quit.
            if key == Key::Escape {
                if mode == Mode::Editor {
                    mode = Mode::Terminal;
                    status_msg = String::from("[TERM: type commands, Enter=run]");
                    // Set cursor to after the last "geo> " prompt
                    cursor_row = term_prompt_row;
                    cursor_col = 5;
                    ensure_cursor_visible(&cursor_row, &mut scroll_offset);
                    continue;
                } else {
                    should_quit = true;
                    break;
                }
            }

            // File input mode: Ctrl+F8 activates, handles typing a path
            if file_input_mode {
                match key {
                    Key::Escape => {
                        file_input_mode = false;
                        file_input_buf.clear();
                        status_msg =
                            String::from("[TEXT mode: type assembly, F8=assemble, F5=run]");
                    }
                    Key::Enter => {
                        // Attempt to load the file
                        let path = Path::new(&file_input_buf);
                        if let Ok(source) = std::fs::read_to_string(path) {
                            load_source_to_canvas(
                                &mut canvas_buffer,
                                &source,
                                &mut cursor_row,
                                &mut cursor_col,
                            );
                            scroll_offset = 0;
                            loaded_file = Some(path.to_path_buf());
                            status_msg = format!("[loaded: {}]", file_input_buf);
                        } else {
                            status_msg = format!("[error: cannot read {}]", file_input_buf);
                        }
                        file_input_mode = false;
                        file_input_buf.clear();
                    }
                    Key::Backspace => {
                        file_input_buf.pop();
                        status_msg = format!(
                            "[load file: {} | Tab=complete, Enter=load, Esc=cancel]",
                            file_input_buf
                        );
                    }
                    Key::Tab => {
                        // Cycle through completions from programs/*.asm
                        if !file_completions.is_empty() {
                            file_completion_idx =
                                (file_completion_idx + 1) % file_completions.len();
                            file_input_buf = file_completions[file_completion_idx].clone();
                            status_msg = format!(
                                "[load file: {} | Tab=complete, Enter=load, Esc=cancel]",
                                file_input_buf
                            );
                        }
                    }
                    _ => {
                        // Type characters into the path buffer
                        let shift = window.is_key_down(Key::LeftShift)
                            || window.is_key_down(Key::RightShift);
                        if let Some(ch) = key_to_ascii_shifted(key, shift) {
                            file_input_buf.push(ch as char);
                            // Reset completion index when user types manually
                            file_completion_idx = 0;
                            status_msg = format!(
                                "[load file: {} | Tab=complete, Enter=load, Esc=cancel]",
                                file_input_buf
                            );
                        }
                    }
                }
                continue;
            }

            // ── Mode-aware key handling ───────────────────────────
            if mode == Mode::Terminal {
                // Terminal mode: type into prompt line, Enter = execute
                match key {
                    Key::Enter => {
                        // Read command text from prompt row (skip "geo> " prefix)
                        let raw = read_canvas_line(&canvas_buffer, term_prompt_row);
                        let cmd = if raw.starts_with("geo> ") {
                            &raw[5..]
                        } else {
                            &raw
                        };
                        let cmd = cmd.trim();

                        // Output goes on the line after the prompt
                        term_output_row = term_prompt_row + 1;

                        let (go_edit, quit) = handle_terminal_command(
                            cmd,
                            &mut vm,
                            &mut canvas_buffer,
                            &mut term_output_row,
                            &mut scroll_offset,
                            &mut loaded_file,
                            &mut canvas_assembled,
                            &mut breakpoints,
                        );

                        if quit {
                            should_quit = true;
                            break;
                        }

                        if go_edit {
                            mode = Mode::Editor;
                            status_msg = String::from(
                                "[EDIT mode: type assembly, F8=assemble, F5=run, Esc=terminal]",
                            );
                            // Position cursor at start of canvas
                            cursor_row = 0;
                            cursor_col = 0;
                            scroll_offset = 0;
                        } else {
                            // Track the new prompt position
                            term_prompt_row = term_output_row - 1; // write_line left us after the "geo> " line
                            cursor_row = term_prompt_row;
                            cursor_col = 5; // after "geo> "
                            ensure_cursor_visible(&cursor_row, &mut scroll_offset);
                            // Update term_output_row for next command
                            // (it's already set past the "geo> " prompt)
                        }
                    }
                    Key::Backspace => {
                        if cursor_col > 5 {
                            cursor_col -= 1;
                            let idx = cursor_row * CANVAS_COLS + cursor_col;
                            canvas_buffer[idx] = 0;
                        }
                    }
                    Key::Up => {
                        if cursor_row > 0 {
                            cursor_row -= 1;
                            ensure_cursor_visible(&cursor_row, &mut scroll_offset);
                        }
                    }
                    Key::Down => {
                        if cursor_row < CANVAS_MAX_ROWS - 1 {
                            cursor_row += 1;
                            ensure_cursor_visible(&cursor_row, &mut scroll_offset);
                        }
                    }
                    _ => {
                        // Type characters into prompt line
                        let shift = window.is_key_down(Key::LeftShift)
                            || window.is_key_down(Key::RightShift);
                        if let Some(ch) = key_to_ascii_shifted(key, shift) {
                            if cursor_col < CANVAS_COLS - 1 {
                                let idx = cursor_row * CANVAS_COLS + cursor_col;
                                canvas_buffer[idx] = ch as u32;
                                cursor_col += 1;
                            }
                        }
                    }
                }
                continue;
            }

            // ── Editor mode: canvas editing (VM paused) ──────────
            match key {
                Key::Enter => {
                    let idx = cursor_row * CANVAS_COLS + cursor_col;
                    canvas_buffer[idx] = '\n' as u32;
                    cursor_col = 0;
                    cursor_row += 1;
                    if cursor_row >= CANVAS_MAX_ROWS {
                        cursor_row = CANVAS_MAX_ROWS - 1;
                    }
                    ensure_cursor_visible(&cursor_row, &mut scroll_offset);
                }
                Key::Space => {
                    let idx = cursor_row * CANVAS_COLS + cursor_col;
                    canvas_buffer[idx] = 0x20;
                    advance_cursor(
                        &mut canvas_buffer,
                        &mut cursor_row,
                        &mut cursor_col,
                        &mut scroll_offset,
                    );
                }
                Key::Backspace => {
                    if cursor_col > 0 {
                        cursor_col -= 1;
                    } else if cursor_row > 0 {
                        cursor_row -= 1;
                        cursor_col = CANVAS_COLS - 1;
                    }
                    let idx = cursor_row * CANVAS_COLS + cursor_col;
                    canvas_buffer[idx] = 0;
                    ensure_cursor_visible(&cursor_row, &mut scroll_offset);
                }
                Key::F5 => {
                    if vm.halted {
                        vm.pc = if canvas_assembled {
                            CANVAS_BYTECODE_ADDR as u32
                        } else {
                            0
                        };
                        vm.halted = false;
                    }
                    hit_breakpoint = false;
                    is_running = !is_running;
                }
                Key::F6 => {
                    // Single-step: execute one instruction when paused
                    if !is_running && !vm.halted && canvas_assembled {
                        hit_breakpoint = false;
                        vm.step();
                        if breakpoints.contains(&vm.pc) {
                            hit_breakpoint = true;
                        }
                        status_msg = format!("[step] PC=0x{:04X}", vm.pc);
                    }
                }
                Key::F8 => {
                    let ctrl =
                        window.is_key_down(Key::LeftCtrl) || window.is_key_down(Key::RightCtrl);
                    if ctrl {
                        // Ctrl+F8: enter file input mode
                        file_input_mode = true;
                        file_input_buf.clear();
                        file_completions = list_asm_files("programs");
                        file_completion_idx = 0;
                        // Pre-populate with last loaded file path if available
                        if let Some(ref path) = loaded_file {
                            file_input_buf = path.to_string_lossy().to_string();
                        }
                        status_msg = format!(
                            "[load file: {} | Tab=complete, Enter=load, Esc=cancel]",
                            file_input_buf
                        );
                    } else {
                        canvas_assemble(
                            &canvas_buffer,
                            &mut vm,
                            &mut canvas_assembled,
                            &mut status_msg,
                        );
                    }
                }
                Key::F7 => {
                    // Save state to file
                    match save_state(SAVE_FILE, &vm, &canvas_buffer, canvas_assembled) {
                        Ok(()) => {
                            let file_size = std::fs::metadata(SAVE_FILE)
                                .map(|m| m.len())
                                .unwrap_or(0);
                            status_msg = format!(
                                "[saved: {} ({:.0}KB)]",
                                SAVE_FILE,
                                file_size as f64 / 1024.0
                            );
                        }
                        Err(e) => {
                            status_msg = format!("[save error: {}]", e);
                        }
                    }
                }
                Key::F9 => {
                    // Screenshot: save screen as PNG
                    let png_path = "screenshot.png";
                    match save_screen_png(png_path, &vm.screen) {
                        Ok(()) => {
                            let file_size = std::fs::metadata(png_path)
                                .map(|m| m.len())
                                .unwrap_or(0);
                            status_msg = format!(
                                "[screenshot: {} ({:.0}KB)]",
                                png_path,
                                file_size as f64 / 1024.0
                            );
                        }
                        Err(e) => {
                            status_msg = format!("[screenshot error: {}]", e);
                        }
                    }
                }
                Key::F10 => {
                    recording = !recording;
                    if recording {
                        frame_id = 0;
                        let _ = std::fs::create_dir_all("/tmp/geo_frames");
                        status_msg = String::from("[RECORDING STARTED: /tmp/geo_frames/]");
                    } else {
                        status_msg = format!("[RECORDING STOPPED: {} frames saved. Use ffmpeg to compile GIF]", frame_id);
                    }
                }
                Key::PageUp => {
                    if mode == Mode::Terminal {
                        ram_view_base = ram_view_base.saturating_sub(1024);
                        status_msg = format!("[RAM Inspector: 0x{:04X}-0x{:04X}]", ram_view_base, ram_view_base + 1023);
                    } else {
                        if scroll_offset > 0 {
                            scroll_offset = scroll_offset.saturating_sub(CANVAS_ROWS);
                            let new_cursor = scroll_offset + CANVAS_ROWS / 2;
                            if new_cursor < cursor_row || cursor_row < scroll_offset {
                                cursor_row = new_cursor.min(CANVAS_MAX_ROWS - 1);
                            }
                        }
                    }
                }
                Key::PageDown => {
                    if mode == Mode::Terminal {
                        ram_view_base = ram_view_base.saturating_add(1024).min(0xFC00);
                        status_msg = format!("[RAM Inspector: 0x{:04X}-0x{:04X}]", ram_view_base, ram_view_base + 1023);
                    } else {
                        let max_scroll = CANVAS_MAX_ROWS.saturating_sub(CANVAS_ROWS);
                        if scroll_offset < max_scroll {
                            scroll_offset = (scroll_offset + CANVAS_ROWS).min(max_scroll);
                            let new_cursor = scroll_offset + CANVAS_ROWS / 2;
                            if new_cursor > cursor_row || cursor_row >= scroll_offset + CANVAS_ROWS {
                                cursor_row = new_cursor.min(CANVAS_MAX_ROWS - 1);
                            }
                        }
                    }
                }
                Key::Left => {
                    if cursor_col > 0 {
                        cursor_col -= 1;
                    }
                }
                Key::Right => {
                    if cursor_col < CANVAS_COLS - 1 {
                        cursor_col += 1;
                    }
                }
                Key::Up => {
                    if cursor_row > 0 {
                        cursor_row -= 1;
                    }
                    ensure_cursor_visible(&cursor_row, &mut scroll_offset);
                }
                Key::Down => {
                    if cursor_row < CANVAS_MAX_ROWS - 1 {
                        cursor_row += 1;
                    }
                    ensure_cursor_visible(&cursor_row, &mut scroll_offset);
                }
                Key::V => {
                    let ctrl =
                        window.is_key_down(Key::LeftCtrl) || window.is_key_down(Key::RightCtrl);
                    if ctrl {
                        match arboard::Clipboard::new() {
                            Ok(mut clipboard) => match clipboard.get_text() {
                                Ok(text) => {
                                    let pasted = paste_text_to_canvas(
                                        &mut canvas_buffer,
                                        &text,
                                        &mut cursor_row,
                                        &mut cursor_col,
                                    );
                                    ensure_cursor_visible(&cursor_row, &mut scroll_offset);
                                    status_msg = format!("[pasted {} chars]", pasted);
                                }
                                Err(e) => {
                                    status_msg = format!("[paste error: {}]", e);
                                }
                            },
                            Err(e) => {
                                status_msg = format!("[clipboard error: {}]", e);
                            }
                        }
                    } else {
                        let shift = window.is_key_down(Key::LeftShift)
                            || window.is_key_down(Key::RightShift);
                        if let Some(ch) = key_to_ascii_shifted(Key::V, shift) {
                            let idx = cursor_row * CANVAS_COLS + cursor_col;
                            canvas_buffer[idx] = ch as u32;
                            advance_cursor(
                                &mut canvas_buffer,
                                &mut cursor_row,
                                &mut cursor_col,
                                &mut scroll_offset,
                            );
                        }
                    }
                }
                _ => {
                    let shift =
                        window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
                    if let Some(ch) = key_to_ascii_shifted(key, shift) {
                        let idx = cursor_row * CANVAS_COLS + cursor_col;
                        canvas_buffer[idx] = ch as u32;
                        advance_cursor(
                            &mut canvas_buffer,
                            &mut cursor_row,
                            &mut cursor_col,
                            &mut scroll_offset,
                        );
                    }
                }
            }
        }

        // ── VM execution ─────────────────────────────────────────
        if is_running && !vm.halted {
            // Run until FRAME, breakpoint, halt, or 1M steps (safety cap)
            vm.frame_ready = false;
            for _ in 0..1_000_000 {
                if !vm.step() {
                    is_running = false;
                    break;
                }
                // Step all spawned child processes in lock-step with the primary
                vm.step_all_processes();
                if vm.frame_ready {
                    // FRAME opcode hit: stop here, let the host render this tick
                    break;
                }
                if breakpoints.contains(&vm.pc) {
                    is_running = false;
                    hit_breakpoint = true;
                    status_msg = format!("[BREAK] PC=0x{:04X}", vm.pc);
                    break;
                }
            }
        }

        // ── Audio dispatch ───────────────────────────────────────
        if let Some((freq, dur)) = vm.beep.take() {
            play_beep(freq, dur);
        }

        // ── Shutdown check ────────────────────────────────────────
        if vm.shutdown_requested {
            status_msg = "[SHUTDOWN] System halted cleanly.".into();
            is_running = false;
            break;
        }

        // ── Update Visual Debugger intensities ──────────────────
        // Process new accesses
        for access in &vm.access_log {
            if access.addr < ram_intensity.len() {
                let boost = if access.kind == vm::MemAccessKind::Write { 1.5 } else { 1.0 };
                ram_intensity[access.addr] = boost;
                ram_kind[access.addr] = access.kind;
            }
        }
        // Decay existing intensities (every frame)
        for val in ram_intensity.iter_mut() {
            if *val > 0.01 {
                *val *= 0.75;
            } else {
                *val = 0.0;
            }
        }
        
        // Track PC for trail
        if is_running {
            pc_history.push_back(vm.pc);
            if pc_history.len() > 64 {
                pc_history.pop_front();
            }
        } else {
            pc_history.clear();
        }

        // ── Render ───────────────────────────────────────────────
        render(
            &mut buffer,
            &vm,
            &canvas_buffer,
            cursor_row,
            cursor_col,
            scroll_offset,
            is_running,
            hit_breakpoint,
            &status_msg,
            &ram_intensity,
            &ram_kind,
            &pc_history,
            ram_view_base,
        );
        window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();

        if recording {
            let path = format!("/tmp/geo_frames/frame_{:05}.png", frame_id);
            if let Err(e) = save_full_buffer_png(&path, &buffer, WIDTH, HEIGHT) {
                status_msg = format!("[rec error: {}]", e);
                recording = false;
            } else {
                frame_id += 1;
            }
        }
    }
}

// ── Ensure cursor is visible (adjust scroll_offset if needed) ───
fn ensure_cursor_visible(cursor_row: &usize, scroll_offset: &mut usize) {
    if *cursor_row < *scroll_offset {
        *scroll_offset = *cursor_row;
    } else if *cursor_row >= *scroll_offset + CANVAS_ROWS {
        *scroll_offset = *cursor_row - CANVAS_ROWS + 1;
    }
}

// ── Load source text from a string onto the canvas grid ──────────
fn load_source_to_canvas(
    canvas_buffer: &mut Vec<u32>,
    source: &str,
    cursor_row: &mut usize,
    cursor_col: &mut usize,
) {
    // Clear canvas buffer
    for cell in canvas_buffer.iter_mut() {
        *cell = 0;
    }

    let mut row = 0usize;
    let mut col = 0usize;

    for ch in source.chars() {
        if row >= CANVAS_MAX_ROWS {
            break;
        }
        if ch == '\n' {
            row += 1;
            col = 0;
        } else if col < CANVAS_COLS {
            canvas_buffer[row * CANVAS_COLS + col] = ch as u32;
            col += 1;
        }
        // characters beyond column 32 on a single line are dropped
    }

    *cursor_row = 0;
    *cursor_col = 0;
}

// ── Paste text from clipboard onto the canvas grid at cursor position ──
fn paste_text_to_canvas(
    canvas_buffer: &mut Vec<u32>,
    text: &str,
    cursor_row: &mut usize,
    cursor_col: &mut usize,
) -> usize {
    let mut row = *cursor_row;
    let mut col = *cursor_col;
    let mut count = 0usize;

    for ch in text.chars() {
        if row >= CANVAS_MAX_ROWS {
            break;
        }
        if ch == '\n' {
            row += 1;
            col = 0;
        } else if ch == '\r' {
            // Skip carriage returns
            continue;
        } else if col < CANVAS_COLS {
            canvas_buffer[row * CANVAS_COLS + col] = ch as u32;
            col += 1;
            if col >= CANVAS_COLS {
                row += 1;
                col = 0;
            }
            count += 1;
        }
    }

    *cursor_row = row.min(CANVAS_MAX_ROWS - 1);
    *cursor_col = col.min(CANVAS_COLS - 1);
    count
}

// ── Canvas assembly: read grid as text, assemble, store bytecode ──
fn canvas_assemble(
    canvas_buffer: &[u32],
    vm: &mut vm::Vm,
    canvas_assembled: &mut bool,
    status_msg: &mut String,
) {
    let buffer_size = CANVAS_MAX_ROWS * CANVAS_COLS;
    let source: String = canvas_buffer[..buffer_size]
        .iter()
        .map(|&cell| {
            let val = cell & 0xFF;
            if val == 0 || val == 0x0A {
                '\n'
            } else {
                (val as u8) as char
            }
        })
        .collect();

    let source = source.replace("\n\n", "\n");

    // Abstraction Layer: Preprocess macros and variables
    let mut pp = preprocessor::Preprocessor::new();
    let preprocessed_source = pp.preprocess(&source);

    match assembler::assemble(&preprocessed_source, CANVAS_BYTECODE_ADDR) {
        Ok(asm_result) => {
            let ram_len = vm.ram.len();
            for v in vm.ram[CANVAS_BYTECODE_ADDR..ram_len.min(CANVAS_BYTECODE_ADDR + 4096)].iter_mut()
            {
                *v = 0;
            }
            for (i, &pixel) in asm_result.pixels.iter().enumerate() {
                let addr = CANVAS_BYTECODE_ADDR + i;
                if addr < ram_len {
                    vm.ram[addr] = pixel;
                }
            }
            *canvas_assembled = true;
            vm.pc = CANVAS_BYTECODE_ADDR as u32;
            vm.halted = false;
            *status_msg = format!(
                "[OK: {} bytes at 0x{:04X}]",
                asm_result.pixels.len(),
                CANVAS_BYTECODE_ADDR
            );
        }
        Err(e) => {
            *status_msg = format!("[ASM ERROR line {}: {}]", e.line, e.message);
        }
    }
}

// ── Rendering ────────────────────────────────────────────────────
fn lerp_color(base: u32, tint: u32, t: f32) -> u32 {
    let t = t.min(1.0);
    let r1 = ((base >> 16) & 0xFF) as f32;
    let g1 = ((base >> 8) & 0xFF) as f32;
    let b1 = (base & 0xFF) as f32;

    let r2 = ((tint >> 16) & 0xFF) as f32;
    let g2 = ((tint >> 8) & 0xFF) as f32;
    let b2 = (tint & 0xFF) as f32;

    let r = (r1 + (r2 - r1) * t) as u32;
    let g = (g1 + (g2 - g1) * t) as u32;
    let b = (b1 + (b2 - b1) * t) as u32;

    (r << 16) | (g << 8) | b
}

fn render(
    buffer: &mut [u32],
    vm: &vm::Vm,
    canvas_buffer: &[u32],
    cursor_row: usize,
    cursor_col: usize,
    scroll_offset: usize,
    is_running: bool,
    hit_breakpoint: bool,
    status_msg: &str,
    ram_intensity: &[f32],
    ram_kind: &[vm::MemAccessKind],
    pc_history: &VecDeque<u32>,
    ram_view_base: usize,
) {
    for pixel in buffer.iter_mut() {
        *pixel = BG;
    }

    // ── Canvas grid (with scroll offset) ─────────────────────────
    for vis_row in 0..CANVAS_ROWS {
        let log_row = vis_row + scroll_offset;
        for col in 0..CANVAS_COLS {
            let ram_addr = log_row * CANVAS_COLS + col;
            let intensity = ram_intensity.get(ram_addr).copied().unwrap_or(0.0);
            let kind = ram_kind.get(ram_addr).copied().unwrap_or(vm::MemAccessKind::Read);

            let val = canvas_buffer[log_row * CANVAS_COLS + col];
            let x0 = col * CANVAS_SCALE;
            let y0 = vis_row * CANVAS_SCALE;
            let is_cursor = log_row == cursor_row && col == cursor_col && !is_running;
            let ascii_byte = (val & 0xFF) as u8;

            let use_pixel_font = val != 0 && ascii_byte >= 0x20 && ascii_byte < 0x80;

            // Determine cell base color (with intensity tint)
            let mut tint_color = if kind == vm::MemAccessKind::Write { 0xFF00FF } else { 0x00FFFF };
            let mut final_intensity = intensity;

            // PC trail: bytecode lives at CANVAS_BYTECODE_ADDR (0x1000), so subtract
            // that base to get the canvas cell index for the currently executing word.
            for (i, &past_pc) in pc_history.iter().enumerate() {
                let canvas_idx = (past_pc as usize).wrapping_sub(CANVAS_BYTECODE_ADDR);
                if canvas_idx == ram_addr {
                    let trail_intensity = (i + 1) as f32 / pc_history.len() as f32;
                    if trail_intensity > final_intensity {
                        final_intensity = trail_intensity;
                        tint_color = 0x666666; // white-ish glow for executing PC
                    }
                }
            }

            let cell_bg = if final_intensity > 0.01 {
                lerp_color(GRID_BG, tint_color, final_intensity)
            } else {
                GRID_BG
            };

            if use_pixel_font {
                let fg = syntax_highlight_color(canvas_buffer, log_row, col);
                let glyph = &font::GLYPHS[ascii_byte as usize];

                for dy in 0..CANVAS_SCALE {
                    for dx in 0..CANVAS_SCALE {
                        let px = x0 + dx;
                        let py = y0 + dy;
                        let is_border = dx == CANVAS_SCALE - 1 || dy == CANVAS_SCALE - 1;

                        let gc = dx / 2;
                        let gr = dy / 2;
                        let glyph_on = gc < font::GLYPH_W
                            && gr < font::GLYPH_H
                            && glyph[gr] & (1 << (7 - gc)) != 0;

                        let mut color = if glyph_on {
                            fg
                        } else if is_border {
                            GRID_LINE
                        } else {
                            cell_bg
                        };

                        if is_cursor && is_border {
                            color = CURSOR_COL;
                        }

                        if px < WIDTH && py < HEIGHT {
                            buffer[py * WIDTH + px] = color;
                        }
                    }
                }
            } else {
                // Empty cell
                for dy in 0..CANVAS_SCALE {
                    for dx in 0..CANVAS_SCALE {
                        let px = x0 + dx;
                        let py = y0 + dy;
                        let is_border = dx == CANVAS_SCALE - 1 || dy == CANVAS_SCALE - 1;
                        let mut color = if is_border { GRID_LINE } else { cell_bg };
                        if is_cursor && is_border {
                            color = CURSOR_COL;
                        }
                        if px < WIDTH && py < HEIGHT {
                            buffer[py * WIDTH + px] = color;
                        }
                    }
                }
            }
        }
    }

    // ── Scrollbar (right edge of canvas) ─────────────────────────
    if CANVAS_MAX_ROWS > CANVAS_ROWS {
        let sb_x = CANVAS_COLS * CANVAS_SCALE - 3; // 3px wide bar at right edge
        let sb_height = CANVAS_ROWS * CANVAS_SCALE;
        let max_scroll = CANVAS_MAX_ROWS - CANVAS_ROWS;

        // Background track
        for y in 0..sb_height {
            buffer[y * WIDTH + sb_x] = SCROLLBAR_BG;
            buffer[y * WIDTH + sb_x + 1] = SCROLLBAR_BG;
        }

        // Thumb (proportional to visible/total ratio, minimum 8px)
        let thumb_ratio = (CANVAS_ROWS * CANVAS_SCALE) as f32 / (CANVAS_MAX_ROWS * CANVAS_SCALE) as f32;
        let thumb_height = ((sb_height as f32 * thumb_ratio).max(8.0)) as usize;
        let thumb_max_travel = sb_height - thumb_height;
        let thumb_y = if max_scroll > 0 {
            (scroll_offset * thumb_max_travel) / max_scroll
        } else {
            0
        };

        for y in thumb_y..(thumb_y + thumb_height).min(sb_height) {
            buffer[y * WIDTH + sb_x] = SCROLLBAR_FG;
            buffer[y * WIDTH + sb_x + 1] = SCROLLBAR_FG;
        }
    }

    // ── VM screen ────────────────────────────────────────────────
    for y in 0..256 {
        for x in 0..256 {
            let color = vm.screen[y * 256 + x];
            let sx = VM_SCREEN_X + x;
            let sy = VM_SCREEN_Y + y;
            if sx < WIDTH && sy < HEIGHT {
                buffer[sy * WIDTH + sx] = color;
            }
        }
    }

    // ── RAM Inspector ────────────────────────────────────────────
    // Label rendered inside the panel (first row of tiles, top-left corner)
    let label = format!("RAM [0x{:04X}]", ram_view_base);
    render_text(buffer, RAM_VIEW_X + 2, RAM_VIEW_Y + 2, &label, 0x888899);

    for row in 0..32 {
        for col in 0..32 {
            let addr = ram_view_base + row * 32 + col;
            if addr >= vm.ram.len() { break; }

            let raw_val = vm.ram[addr];
            let intensity = ram_intensity.get(addr).copied().unwrap_or(0.0);
            let kind = ram_kind.get(addr).copied().unwrap_or(vm::MemAccessKind::Read);

            // Base color is the RAM value (masked to 24-bit)
            let base_color = raw_val & 0xFFFFFF;
            
            // Pulse tint
            let tint_color = if kind == vm::MemAccessKind::Write { 0xFF00FF } else { 0x00FFFF };
            let cell_color = if intensity > 0.01 {
                lerp_color(base_color, tint_color, intensity)
            } else {
                base_color
            };

            // Paint 8x8 block
            let x0 = RAM_VIEW_X + col * RAM_VIEW_SCALE;
            let y0 = RAM_VIEW_Y + row * RAM_VIEW_SCALE;
            for dy in 0..RAM_VIEW_SCALE {
                for dx in 0..RAM_VIEW_SCALE {
                    let px = x0 + dx;
                    let py = y0 + dy;
                    if px < WIDTH && py < HEIGHT {
                        buffer[py * WIDTH + px] = cell_color;
                    }
                }
            }
        }
    }

    // ── Global Heatmap ────────────────────────────────────────────
    render_text(buffer, HEATMAP_X + 2, HEATMAP_Y + 2, "64K", 0x888899);
    for i in 0..65536 {
        let addr = i;
        let x = HEATMAP_X + (i % 256);
        let y = HEATMAP_Y + (i / 256);

        let raw_val = vm.ram[addr];
        let intensity = ram_intensity.get(addr).copied().unwrap_or(0.0);
        let kind = ram_kind.get(addr).copied().unwrap_or(vm::MemAccessKind::Read);

        // Base color: Dim gray if data exists, else black
        let base_color = if raw_val > 0 { 0x222222 } else { 0x050505 };
        
        // Pulse tint
        let tint_color = if kind == vm::MemAccessKind::Write { 0xFF00FF } else { 0x00FFFF };
        let mut pixel_color = if intensity > 0.01 {
            lerp_color(base_color, tint_color, intensity)
        } else {
            base_color
        };

        // Current PC is bright white
        if addr == vm.pc as usize {
            pixel_color = 0xFFFFFF;
        }

        if x < WIDTH && y < HEIGHT {
            buffer[y * WIDTH + x] = pixel_color;
        }
    }

    // ── Registers ────────────────────────────────────────────────
    let regs_end_y = REGS_Y + 16 * 14;
    for i in 0..16 {
        let text = format!("r{:02}={:08X}", i, vm.regs[i]);
        render_text(buffer, REGS_X, REGS_Y + i * 14, &text, STATUS_FG);
    }
    for i in 16..32 {
        let text = format!("r{:02}={:08X}", i, vm.regs[i]);
        render_text(
            buffer,
            REGS_X + 200,
            REGS_Y + (i - 16) * 14,
            &text,
            STATUS_FG,
        );
    }

    // ── Disassembly panel ────────────────────────────────────────
    // Show 10 decoded instructions centered on PC
    let disasm_y = regs_end_y + 12;
    let disasm_label_color = 0x888899;
    let disasm_color = 0xBBBBDD;
    let disasm_pc_color = 0x00FF88; // bright green for current instruction
    render_text(buffer, REGS_X, disasm_y, "DISASM", disasm_label_color);

    // Figure out where to start disassembly: scan backwards from PC
    // by trying to decode instruction boundaries. Simple approach:
    // start from a known-good boundary (bytecode base) and walk forward.
    let pc = vm.pc;

    // Build a map of instruction starts from base to PC+some
    let mut inst_starts: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    {
        // Programs are usually at 0 (CLI) or 0x1000 (Canvas)
        let bases = [0u32, CANVAS_BYTECODE_ADDR as u32];
        for &base in &bases {
            // Only scan if PC is in a reasonable range of this base
            if pc >= base && pc < base + 0x1000 {
                let mut addr = base;
                while addr <= pc + 30 {
                    if addr as usize >= vm.ram.len() {
                        break;
                    }
                    let op = vm.ram[addr as usize];
                    // If we hit a zero opcode (empty RAM) past the program, stop
                    if op == 0 && addr > pc + 20 {
                        break;
                    }
                    inst_starts.insert(addr);
                    let (_, len) = vm.disassemble_at(addr);
                    if len == 0 {
                        break;
                    }
                    addr += len as u32;
                }
            }
        }
    }

    // Find the 4 instructions before PC and 5 after
    let mut display_addrs: Vec<u32> = Vec::new();
    let mut before: Vec<u32> = Vec::new();
    let mut after: Vec<u32> = Vec::new();
    let mut found_pc = false;
    for &a in &inst_starts {
        if a < pc {
            before.push(a);
            if before.len() > 4 { before.remove(0); }
        } else if a == pc {
            found_pc = true;
        } else {
            after.push(a);
            if after.len() >= 5 { break; }
        }
    }
    display_addrs.extend_from_slice(&before);
    if found_pc || inst_starts.contains(&pc) {
        display_addrs.push(pc);
    }
    display_addrs.extend_from_slice(&after);
    // Trim to 10 lines
    let total = display_addrs.len();
    if total > 10 {
        // Keep PC visible: if PC is in the list, center around it
        let pc_idx = display_addrs.iter().position(|&a| a == pc).unwrap_or(4);
        let start = if pc_idx > 4 { pc_idx - 4 } else { 0 };
        display_addrs = display_addrs[start..(start + 10).min(total)].to_vec();
    }

    for (i, &addr) in display_addrs.iter().enumerate() {
        let (mnemonic, _) = vm.disassemble_at(addr);
        let is_pc = addr == pc;
        let marker = if is_pc { ">" } else { " " };
        let line = format!("{}{:04X} {}", marker, addr, mnemonic);
        let color = if is_pc { disasm_pc_color } else { disasm_color };
        let line_y = disasm_y + 14 + i as usize * 12;
        if line_y + 12 < HEIGHT - 24 {
            render_text(buffer, REGS_X, line_y, &line, color);
        }
    }

    // ── Status bar ───────────────────────────────────────────────
    let row_info = format!("row {}/{} ", cursor_row + 1, CANVAS_MAX_ROWS);
    let scroll_info = if scroll_offset > 0 || cursor_row >= CANVAS_ROWS {
        format!("[scroll {}-{}] ", scroll_offset + 1, scroll_offset + CANVAS_ROWS)
    } else {
        String::new()
    };
    let pc_text = format!("PC={:04X} {}{}{}", vm.pc, scroll_info, row_info, status_msg);
    render_text(buffer, 8, HEIGHT - 20, &pc_text, STATUS_FG);

    let state_label = if is_running {
        ("RUNNING", 0x00FF00)
    } else if vm.halted {
        ("HALTED", 0xFF4444)
    } else if hit_breakpoint {
        ("BREAK", 0xFF6600)
    } else {
        ("PAUSED", 0xFFAA00)
    };
    render_text(
        buffer,
        WIDTH - 80,
        HEIGHT - 20,
        state_label.0,
        state_label.1,
    );
}

/// Render a text string into the framebuffer using the 8x8 font
fn render_text(buffer: &mut [u32], x0: usize, y0: usize, text: &str, color: u32) {
    let mut cx = x0;
    for ch in text.chars() {
        let idx = ch as usize;
        if idx < 128 {
            let glyph = &font::GLYPHS[idx];
            for row in 0..font::GLYPH_H {
                for col in 0..font::GLYPH_W {
                    if glyph[row] & (1 << (7 - col)) != 0 {
                        let px = cx + col;
                        let py = y0 + row;
                        if px < WIDTH && py < HEIGHT {
                            buffer[py * WIDTH + px] = color;
                        }
                    }
                }
            }
        }
        cx += font::GLYPH_W + 1;
    }
}

/// Get the syntax highlighting color for a character at (row, col) in the canvas.
fn syntax_highlight_color(canvas_buffer: &[u32], row: usize, col: usize) -> u32 {
    // Extract the full line as a string
    let mut line_chars: String = String::with_capacity(CANVAS_COLS);
    for c in 0..CANVAS_COLS {
        let val = canvas_buffer[row * CANVAS_COLS + c];
        if val == 0 {
            // null = newline or end of line
            break;
        }
        let byte = (val & 0xFF) as u8;
        if byte == 0x0A {
            // explicit newline
            break;
        }
        if byte >= 0x20 && byte < 0x80 {
            line_chars.push(byte as char);
        }
    }

    let line = line_chars.trim();
    if line.is_empty() {
        return SYN_DEFAULT;
    }

    // Find the offset of col within the trimmed line
    let trimmed_start = CANVAS_COLS - line_chars.trim_start().len();
    let col_in_trimmed = if col >= trimmed_start {
        col - trimmed_start
    } else {
        return SYN_DEFAULT;
    };

    // Parse the line into syntax spans using the preprocessor's logic
    let spans = preprocessor::parse_syntax_line(line);

    // Find which span contains this column
    for span in &spans {
        if col_in_trimmed >= span.start && col_in_trimmed < span.start + span.len {
            return match span.kind {
                preprocessor::SynTok::Opcode => SYN_OPCODE,
                preprocessor::SynTok::Register => SYN_REGISTER,
                preprocessor::SynTok::Number => SYN_NUMBER,
                preprocessor::SynTok::Label => SYN_LABEL,
                preprocessor::SynTok::Comment => SYN_COMMENT,
                preprocessor::SynTok::Default => SYN_DEFAULT,
            };
        }
    }

    SYN_DEFAULT
}

fn advance_cursor(
    _canvas_buffer: &mut Vec<u32>,
    row: &mut usize,
    col: &mut usize,
    scroll_offset: &mut usize,
) {
    *col += 1;
    if *col >= CANVAS_COLS {
        *col = 0;
        *row += 1;
        if *row >= CANVAS_MAX_ROWS {
            *row = CANVAS_MAX_ROWS - 1;
        }
    }
    ensure_cursor_visible(row, scroll_offset);
}

// ── File listing for Tab completion ────────────────────────────

/// List .asm files in the given directory, returning sorted full paths.
fn list_asm_files(dir: &str) -> Vec<String> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "asm" {
                    if let Some(name) = path.to_str() {
                        files.push(name.to_string());
                    }
                }
            }
        }
    }
    files.sort();
    files
}

// ── Save / Load state ──────────────────────────────────────────

/// Save screen buffer as a PNG file (256x256, RGB).
fn save_screen_png(path: &str, screen: &[u32]) -> std::io::Result<()> {
    let file = std::fs::File::create(path)?;
    let ref mut w = std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(w, 256, 256);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    let mut raw_data = Vec::with_capacity(256 * 256 * 3);
    for pixel in screen {
        raw_data.push((pixel >> 16) as u8); // R
        raw_data.push((pixel >> 8) as u8);  // G
        raw_data.push(*pixel as u8);         // B
    }
    writer.write_image_data(&raw_data)?;
    Ok(())
}

fn save_full_buffer_png(path: &str, buffer: &[u32], w: usize, h: usize) -> std::io::Result<()> {
    let file = std::fs::File::create(path)?;
    let ref mut writer = std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, w as u32, h as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    let mut raw_data = Vec::with_capacity(w * h * 3);
    for &pixel in buffer {
        raw_data.push((pixel >> 16) as u8); // R
        raw_data.push((pixel >> 8) as u8);  // G
        raw_data.push(pixel as u8);         // B
    }
    writer.write_image_data(&raw_data)?;
    Ok(())
}

/// Save full application state (VM + canvas) to a binary file.
/// Format: VM save (see vm.rs) + canvas_len u32 + canvas_buffer + canvas_assembled u8
fn save_state(
    path: &str,
    vm: &vm::Vm,
    canvas_buffer: &[u32],
    canvas_assembled: bool,
) -> std::io::Result<()> {
    use std::io::Write;
    // Save VM state first
    vm.save_to_file(Path::new(path))?;
    // Append canvas data
    let mut f = std::fs::OpenOptions::new().append(true).open(path)?;
    let canvas_len = canvas_buffer.len() as u32;
    f.write_all(&canvas_len.to_le_bytes())?;
    for &v in canvas_buffer {
        f.write_all(&v.to_le_bytes())?;
    }
    f.write_all(&[if canvas_assembled { 1 } else { 0 }])?;
    Ok(())
}

/// Load full application state from a binary file.
/// Returns (vm, canvas_buffer, canvas_assembled) on success.
fn load_state(path: &str) -> std::io::Result<(vm::Vm, Vec<u32>, bool)> {
    use std::io::Read;
    let mut data = Vec::new();
    let mut f = std::fs::File::open(path)?;
    f.read_to_end(&mut data)?;

    // Read VM portion
    let vm_min = 4 + 4 + 1 + 4
        + vm::NUM_REGS * 4
        + vm::RAM_SIZE * 4
        + vm::SCREEN_SIZE * 4;
    if data.len() < vm_min + 4 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "save file too small for canvas trailer",
        ));
    }

    // Parse VM from the raw bytes (same logic as Vm::load_from_file)
    if &data[0..4] != vm::SAVE_MAGIC {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid magic bytes",
        ));
    }
    let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    if version != vm::SAVE_VERSION {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unsupported save version: {}", version),
        ));
    }

    let mut off = 8usize;
    let halted = data[off] != 0;
    off += 1;
    let pc = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
    off += 4;

    let mut regs = [0u32; vm::NUM_REGS];
    for r in regs.iter_mut() {
        *r = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
        off += 4;
    }
    let mut ram = vec![0u32; vm::RAM_SIZE];
    for v in ram.iter_mut() {
        *v = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
        off += 4;
    }
    let mut screen = vec![0u32; vm::SCREEN_SIZE];
    for v in screen.iter_mut() {
        *v = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
        off += 4;
    }

    let rand_state = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
    off += 4;
    let frame_count = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
    off += 4;

    let vm = vm::Vm {
        ram,
        regs,
        pc,
        screen,
        halted,
        frame_ready: false,
        rand_state,
        frame_count,
        beep: None,
        access_log: Vec::new(),
        processes: Vec::new(),
        mode: vm::CpuMode::Kernel,
        kernel_stack: Vec::new(),
        allocated_pages: 0b11,
        current_page_dir: None,
        segfault_pid: 0,
        segfault: false,
        vfs: vfs::Vfs::new(),
        current_pid: 0,
        sched_tick: 0,
        default_time_slice: vm::DEFAULT_TIME_SLICE,
        yielded: false,
        sleep_frames: 0,
        new_priority: 0,
        pipes: Vec::new(),
        pipe_created: false,
        msg_sender: 0,
        msg_data: [0; vm::MSG_WORDS],
        msg_recv_requested: false,
        env_vars: std::collections::HashMap::new(),
        booted: false,
        shutdown_requested: false,
        step_exit_code: None,
        step_zombie: false,
        hypervisor_active: false,
        hypervisor_config: String::new(),
    };


    // Parse canvas trailer
    let canvas_len = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]) as usize;
    off += 4;
    if off + canvas_len * 4 + 1 > data.len() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "save file truncated in canvas data",
        ));
    }
    let mut canvas_buffer = vec![0u32; canvas_len];
    for v in canvas_buffer.iter_mut() {
        *v = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
        off += 4;
    }
    let canvas_assembled = data[off] != 0;

    Ok((vm, canvas_buffer, canvas_assembled))
}

// ── Key mapping ──────────────────────────────────────────────────

fn key_to_ascii(key: Key) -> Option<u8> {
    match key {
        Key::A => Some(b'A'),
        Key::B => Some(b'B'),
        Key::C => Some(b'C'),
        Key::D => Some(b'D'),
        Key::E => Some(b'E'),
        Key::F => Some(b'F'),
        Key::G => Some(b'G'),
        Key::H => Some(b'H'),
        Key::I => Some(b'I'),
        Key::J => Some(b'J'),
        Key::K => Some(b'K'),
        Key::L => Some(b'L'),
        Key::M => Some(b'M'),
        Key::N => Some(b'N'),
        Key::O => Some(b'O'),
        Key::P => Some(b'P'),
        Key::Q => Some(b'Q'),
        Key::R => Some(b'R'),
        Key::S => Some(b'S'),
        Key::T => Some(b'T'),
        Key::U => Some(b'U'),
        Key::V => Some(b'V'),
        Key::W => Some(b'W'),
        Key::X => Some(b'X'),
        Key::Y => Some(b'Y'),
        Key::Z => Some(b'Z'),
        Key::Key0 => Some(b'0'),
        Key::Key1 => Some(b'1'),
        Key::Key2 => Some(b'2'),
        Key::Key3 => Some(b'3'),
        Key::Key4 => Some(b'4'),
        Key::Key5 => Some(b'5'),
        Key::Key6 => Some(b'6'),
        Key::Key7 => Some(b'7'),
        Key::Key8 => Some(b'8'),
        Key::Key9 => Some(b'9'),
        Key::Space => Some(b' '),
        Key::Comma => Some(b','),
        Key::Period => Some(b'.'),
        Key::Slash => Some(b'/'),
        Key::Semicolon => Some(b';'),
        Key::Apostrophe => Some(b'\''),
        Key::Minus => Some(b'-'),
        Key::Equal => Some(b'='),
        Key::LeftBracket => Some(b'['),
        Key::RightBracket => Some(b']'),
        Key::Backslash => Some(b'\\'),
        _ => None,
    }
}

fn key_to_ascii_shifted(key: Key, shift: bool) -> Option<u8> {
    // Letters
    match key {
        Key::A => return Some(if shift { b'A' } else { b'a' }),
        Key::B => return Some(if shift { b'B' } else { b'b' }),
        Key::C => return Some(if shift { b'C' } else { b'c' }),
        Key::D => return Some(if shift { b'D' } else { b'd' }),
        Key::E => return Some(if shift { b'E' } else { b'e' }),
        Key::F => return Some(if shift { b'F' } else { b'f' }),
        Key::G => return Some(if shift { b'G' } else { b'g' }),
        Key::H => return Some(if shift { b'H' } else { b'h' }),
        Key::I => return Some(if shift { b'I' } else { b'i' }),
        Key::J => return Some(if shift { b'J' } else { b'j' }),
        Key::K => return Some(if shift { b'K' } else { b'k' }),
        Key::L => return Some(if shift { b'L' } else { b'l' }),
        Key::M => return Some(if shift { b'M' } else { b'm' }),
        Key::N => return Some(if shift { b'N' } else { b'n' }),
        Key::O => return Some(if shift { b'O' } else { b'o' }),
        Key::P => return Some(if shift { b'P' } else { b'p' }),
        Key::Q => return Some(if shift { b'Q' } else { b'q' }),
        Key::R => return Some(if shift { b'R' } else { b'r' }),
        Key::S => return Some(if shift { b'S' } else { b's' }),
        Key::T => return Some(if shift { b'T' } else { b't' }),
        Key::U => return Some(if shift { b'U' } else { b'u' }),
        Key::V => return Some(if shift { b'V' } else { b'v' }),
        Key::W => return Some(if shift { b'W' } else { b'w' }),
        Key::X => return Some(if shift { b'X' } else { b'x' }),
        Key::Y => return Some(if shift { b'Y' } else { b'y' }),
        Key::Z => return Some(if shift { b'Z' } else { b'z' }),
        _ => {}
    }

    // Numbers and symbols
    match key {
        Key::Key0 => Some(if shift { b')' } else { b'0' }),
        Key::Key1 => Some(if shift { b'!' } else { b'1' }),
        Key::Key2 => Some(if shift { b'@' } else { b'2' }),
        Key::Key3 => Some(if shift { b'#' } else { b'3' }),
        Key::Key4 => Some(if shift { b'$' } else { b'4' }),
        Key::Key5 => Some(if shift { b'%' } else { b'5' }),
        Key::Key6 => Some(if shift { b'^' } else { b'6' }),
        Key::Key7 => Some(if shift { b'&' } else { b'7' }),
        Key::Key8 => Some(if shift { b'*' } else { b'8' }),
        Key::Key9 => Some(if shift { b'(' } else { b'9' }),
        Key::Comma => Some(if shift { b'<' } else { b',' }),
        Key::Period => Some(if shift { b'>' } else { b'.' }),
        Key::Slash => Some(if shift { b'?' } else { b'/' }),
        Key::Semicolon => Some(if shift { b':' } else { b';' }),
        Key::Apostrophe => Some(if shift { b'"' } else { b'\'' }),
        Key::Minus => Some(if shift { b'_' } else { b'-' }),
        Key::Equal => Some(if shift { b'+' } else { b'=' }),
        Key::LeftBracket => Some(if shift { b'{' } else { b'[' }),
        Key::RightBracket => Some(if shift { b'}' } else { b']' }),
        Key::Backslash => Some(if shift { b'|' } else { b'\\' }),
        Key::Backquote => Some(if shift { b'~' } else { b'`' }),
        _ => None,
    }
}
