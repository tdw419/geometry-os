// main.rs -- Geometry OS Canvas Text Surface
//
// The canvas grid IS a text editor. Type assembly, press F8 to assemble,
// press F5 to run. Each keystroke writes a colored pixel glyph.
//
// Build: cargo run
// Test:  cargo test

mod assembler;
mod font;
mod vm;

use minifb::{Key, KeyRepeat, Window, WindowOptions};
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

// ── Memory map ───────────────────────────────────────────────────
// 0x000-0x3FF   Canvas grid (source text, 1024 cells visible on 32x32 grid)
// 0x1000-0x1FFF Assembled bytecode output (F8 writes here)
// 0xFFF         Keyboard port (memory-mapped I/O)
const CANVAS_BYTECODE_ADDR: usize = 0x1000;
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
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "  reset             Reset VM state");
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
                *output_row = write_line_to_canvas(canvas_buffer, *output_row, "Usage: load <file>");
                *output_row = write_line_to_canvas(canvas_buffer, *output_row, "geo> ");
                ensure_scroll(*output_row, scroll_offset);
                return (false, false);
            }
            let mut filename = parts[1..].join(" ");
            if !filename.ends_with(".asm") {
                filename.push_str(".asm");
            }
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

            match assembler::assemble(&source) {
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
                        &format!("ASM ERROR line {}: {}", e.line, e.message),
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
        "reset" => {
            vm.reset();
            *output_row = write_line_to_canvas(canvas_buffer, *output_row, "VM reset");
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

fn main() {
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

    // Cursor position on canvas (logical coordinates, can exceed visible area)
    let mut cursor_row: usize = 0;
    let mut cursor_col: usize = 0;

    // Scroll offset: which logical row is at the top of the visible window
    let mut scroll_offset: usize = 0;

    // Canvas backing buffer (separate from VM RAM to allow > 32 rows
    // without overlapping bytecode at 0x1000)
    let mut canvas_buffer: Vec<u32> = vec![0; CANVAS_MAX_ROWS * CANVAS_COLS];

    // Status bar message
    let mut status_msg = String::from("[TERM: type commands, Enter=run]");

    // Last loaded file (for Ctrl+F8 reload)
    let mut loaded_file: Option<PathBuf> = None;

    // ── Mode state ──────────────────────────────────────────────
    let mut mode = Mode::Terminal;
    // In terminal mode, track which row the prompt is on
    let mut term_prompt_row: usize = 0;
    // The "output row" for terminal -- where next line goes
    let mut term_output_row: usize = 0;

    // Boot: write welcome banner + first prompt into canvas
    {
        term_output_row = write_line_to_canvas(&mut canvas_buffer, 0, "Geometry OS v0.2.0");
        term_output_row = write_line_to_canvas(&mut canvas_buffer, term_output_row, "25 opcodes | 32 regs | 256x256");
        term_output_row = write_line_to_canvas(&mut canvas_buffer, term_output_row, "Type 'help' for commands.");
        term_output_row = write_line_to_canvas(&mut canvas_buffer, term_output_row, "");
        term_prompt_row = term_output_row;
        term_output_row = write_line_to_canvas(&mut canvas_buffer, term_output_row, "geo> ");
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
                    is_running = !is_running;
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
                Key::PageUp => {
                    if scroll_offset > 0 {
                        scroll_offset = scroll_offset.saturating_sub(CANVAS_ROWS);
                        // Move cursor to center of visible area
                        let new_cursor = scroll_offset + CANVAS_ROWS / 2;
                        if new_cursor < cursor_row || cursor_row < scroll_offset {
                            cursor_row = new_cursor.min(CANVAS_MAX_ROWS - 1);
                        }
                    }
                }
                Key::PageDown => {
                    let max_scroll = CANVAS_MAX_ROWS.saturating_sub(CANVAS_ROWS);
                    if scroll_offset < max_scroll {
                        scroll_offset = (scroll_offset + CANVAS_ROWS).min(max_scroll);
                        // Move cursor to center of visible area
                        let new_cursor = scroll_offset + CANVAS_ROWS / 2;
                        if new_cursor > cursor_row
                            || cursor_row >= scroll_offset + CANVAS_ROWS
                        {
                            cursor_row = new_cursor.min(CANVAS_MAX_ROWS - 1);
                        }
                    }
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
            for _ in 0..4096 {
                if !vm.step() {
                    is_running = false;
                    break;
                }
            }
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
            &status_msg,
        );
        window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();
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

    match assembler::assemble(&source) {
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
fn render(
    buffer: &mut [u32],
    vm: &vm::Vm,
    canvas_buffer: &[u32],
    cursor_row: usize,
    cursor_col: usize,
    scroll_offset: usize,
    is_running: bool,
    status_msg: &str,
) {
    for pixel in buffer.iter_mut() {
        *pixel = BG;
    }

    // ── Canvas grid (with scroll offset) ─────────────────────────
    for vis_row in 0..CANVAS_ROWS {
        let log_row = vis_row + scroll_offset;
        for col in 0..CANVAS_COLS {
            let val = canvas_buffer[log_row * CANVAS_COLS + col];
            let x0 = col * CANVAS_SCALE;
            let y0 = vis_row * CANVAS_SCALE;
            let is_cursor = log_row == cursor_row && col == cursor_col && !is_running;
            let ascii_byte = (val & 0xFF) as u8;

            let use_pixel_font = val != 0 && ascii_byte >= 0x20 && ascii_byte < 0x80;

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
                            GRID_BG
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
                        let mut color = if is_border { GRID_LINE } else { GRID_BG };
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

    // ── Registers ────────────────────────────────────────────────
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

/// Valid opcodes for syntax highlighting (same set as assembler.rs)
const OPCODES: &[&str] = &[
    "HALT", "NOP", "LDI", "LOAD", "STORE", "ADD", "SUB", "MUL", "DIV",
    "AND", "OR", "XOR", "SHL", "SHR", "MOD", "JMP", "JZ", "JNZ",
    "CALL", "RET", "BLT", "BGE", "PSET", "PSETI", "FILL", "RECTF",
    "TEXT", "CMP", "PUSH", "POP",
];

/// Token types produced by the syntax highlighter.
#[derive(Clone, Copy, PartialEq)]
enum SynTok {
    Opcode,
    Register,
    Number,
    Label,
    Comment,
    Default,
}

/// A single token with its start column and length.
struct SynSpan {
    kind: SynTok,
    start: usize,
    len: usize,
}

/// Parse a line of assembly text into syntax spans for highlighting.
/// Returns spans covering the line -- characters not in any span get SYN_DEFAULT.
fn parse_syntax_line(line: &str) -> Vec<SynSpan> {
    let mut spans: Vec<SynSpan> = Vec::new();
    let trimmed = line.trim();

    if trimmed.is_empty() {
        return spans;
    }

    // Check if entire line (after trim) is a comment
    if trimmed.starts_with(';') {
        spans.push(SynSpan { kind: SynTok::Comment, start: 0, len: line.len() });
        return spans;
    }

    // Check for label definition: word followed by ':'
    // The assembler does label.find(':') before stripping comments, so
    // we need to handle "label: instruction ; comment" style lines.
    let first_start = line.len() - trimmed.len();
    let mut pos = first_start;

    // Check if line starts with a label (identifier followed by ':')
    if let Some(colon_pos) = line[pos..].find(':') {
        let label_end = pos + colon_pos;
        // Make sure it's not inside a comment
        if line[pos..label_end].chars().all(|c| c.is_alphanumeric() || c == '_') {
            spans.push(SynSpan { kind: SynTok::Label, start: pos, len: colon_pos });
            pos = label_end + 1; // skip the colon
            // skip whitespace after colon
            while pos < line.len() && line.as_bytes()[pos] == b' ' {
                pos += 1;
            }
        }
    }

    // Now parse instruction tokens from current position
    // First check for inline comment
    let comment_start = line[pos..].find(';').map(|i| pos + i);
    let code_end = comment_start.unwrap_or(line.len());

    // Extract the code portion (without comment)
    let code = &line[pos..code_end];
    let code_offset = pos; // offset of code start from line start

    if code.is_empty() {
        // Only whitespace before comment
        if let Some(cs) = comment_start {
            spans.push(SynSpan { kind: SynTok::Comment, start: cs, len: line.len() - cs });
        }
        return spans;
    }

    // Tokenize the code portion by splitting on commas and whitespace
    let mut token_pos = 0;
    let mut is_first_token = true;
    let tokens_str: Vec<&str> = code.split(|c: char| c == ',' || c == ' ' || c == '\t')
        .filter(|s| !s.is_empty())
        .collect();

    for token in &tokens_str {
        // Find the actual position of this token in the code string
        let actual_start = code[token_pos..].find(*token).unwrap_or(token_pos);
        let abs_start = code_offset + actual_start;

        // Determine token type
        if is_first_token {
            // First token: check if it's an opcode
            let upper: String = token.chars().map(|c| c.to_ascii_uppercase()).collect();
            if OPCODES.contains(&upper.as_str()) {
                spans.push(SynSpan { kind: SynTok::Opcode, start: abs_start, len: token.len() });
            } else {
                spans.push(SynSpan { kind: SynTok::Default, start: abs_start, len: token.len() });
            }
            is_first_token = false;
        } else {
            // Subsequent tokens: register, number, or label reference
            if token.starts_with('r') || token.starts_with('R') {
                // Could be a register: r0-r31
                let reg_part = &token[1..];
                if reg_part.parse::<u32>().is_ok() {
                    spans.push(SynSpan { kind: SynTok::Register, start: abs_start, len: token.len() });
                    token_pos = actual_start + token.len();
                    continue;
                }
            }
            // Check if it's a number (decimal, hex 0x, binary 0b)
            let is_number = token.chars().next().map_or(false, |c| c.is_ascii_digit())
                || token.starts_with("0x") || token.starts_with("0X")
                || token.starts_with("0b") || token.starts_with("0B")
                || (token.starts_with('-') && token.len() > 1 && token[1..].chars().next().map_or(false, |c| c.is_ascii_digit()));
            if is_number {
                spans.push(SynSpan { kind: SynTok::Number, start: abs_start, len: token.len() });
            } else {
                // Label reference (e.g. JMP loop)
                spans.push(SynSpan { kind: SynTok::Label, start: abs_start, len: token.len() });
            }
        }

        token_pos = actual_start + token.len();
    }

    // Add comment span
    if let Some(cs) = comment_start {
        spans.push(SynSpan { kind: SynTok::Comment, start: cs, len: line.len() - cs });
    }

    spans
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

    // Parse the line into syntax spans
    let spans = parse_syntax_line(line);

    // Find which span contains this column
    for span in &spans {
        if col_in_trimmed >= span.start && col_in_trimmed < span.start + span.len {
            return match span.kind {
                SynTok::Opcode => SYN_OPCODE,
                SynTok::Register => SYN_REGISTER,
                SynTok::Number => SYN_NUMBER,
                SynTok::Label => SYN_LABEL,
                SynTok::Comment => SYN_COMMENT,
                SynTok::Default => SYN_DEFAULT,
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

    let vm = vm::Vm {
        ram,
        regs,
        pc,
        screen,
        halted,
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
