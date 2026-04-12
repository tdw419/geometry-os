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
use std::path::PathBuf;

// ── Layout constants ─────────────────────────────────────────────
const WIDTH: usize = 1024;
const HEIGHT: usize = 768;

// Canvas grid
const CANVAS_SCALE: usize = 16; // 16x16 screen pixels per cell
const CANVAS_COLS: usize = 32;
const CANVAS_ROWS: usize = 32;

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

// ── Colors ───────────────────────────────────────────────────────
const BG: u32 = 0x050508;
const GRID_BG: u32 = 0x0A0A14;
const GRID_LINE: u32 = 0x141420;
const CURSOR_COL: u32 = 0x00FFFF;
const STATUS_FG: u32 = 0x888899;

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

    // Cursor position on canvas
    let mut cursor_row: usize = 0;
    let mut cursor_col: usize = 0;

    // Status bar message
    let mut status_msg = String::from("[TEXT mode: type assembly, F8=assemble, F5=run]");

    // Last loaded file (for Ctrl+F8 reload)
    let mut loaded_file: Option<PathBuf> = None;

    // Load file from command-line argument at startup
    if let Some(path_str) = std::env::args().nth(1) {
        let path = PathBuf::from(&path_str);
        if let Ok(source) = std::fs::read_to_string(&path) {
            load_source_to_canvas(&mut vm, &source, &mut cursor_row, &mut cursor_col);
            status_msg = format!("[loaded: {}]", path.display());
            loaded_file = Some(path);
        } else {
            status_msg = format!("[error: could not read {}]", path_str);
        }
    }

    // ── Main loop ────────────────────────────────────────────────
    while window.is_open() && !window.is_key_down(Key::Escape) {
        // ── Handle input ─────────────────────────────────────────
        for key in window.get_keys_pressed(KeyRepeat::No) {
            if is_running {
                // Runtime: send keys to VM keyboard port
                if let Some(ch) = key_to_ascii(key) {
                    vm.ram[KEY_PORT] = ch as u32;
                }
                continue;
            }

            // Canvas editing (VM paused)
            match key {
                Key::Enter => {
                    let idx = cursor_row * CANVAS_COLS + cursor_col;
                    vm.ram[idx] = '\n' as u32;
                    cursor_col = 0;
                    cursor_row += 1;
                    if cursor_row >= CANVAS_ROWS {
                        cursor_row = 0;
                    }
                }
                Key::Space => {
                    let idx = cursor_row * CANVAS_COLS + cursor_col;
                    vm.ram[idx] = 0x20;
                    advance_cursor(&mut cursor_row, &mut cursor_col);
                }
                Key::Backspace => {
                    if cursor_col > 0 {
                        cursor_col -= 1;
                    } else if cursor_row > 0 {
                        cursor_row -= 1;
                        cursor_col = CANVAS_COLS - 1;
                    }
                    vm.ram[cursor_row * CANVAS_COLS + cursor_col] = 0;
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
                    let ctrl = window.is_key_down(Key::LeftCtrl)
                        || window.is_key_down(Key::RightCtrl);
                    if ctrl {
                        // Ctrl+F8: reload the last loaded file onto the canvas
                        if let Some(ref path) = loaded_file.clone() {
                            if let Ok(source) = std::fs::read_to_string(path) {
                                load_source_to_canvas(&mut vm, &source, &mut cursor_row, &mut cursor_col);
                                status_msg = format!("[reloaded: {}]", path.display());
                            } else {
                                status_msg = format!("[error: could not reload {}]", path.display());
                            }
                        } else {
                            status_msg = String::from("[no file loaded -- run with: cargo run -- path/to/file.asm]");
                        }
                    } else {
                        canvas_assemble(&mut vm, &mut canvas_assembled, &mut status_msg);
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
                }
                Key::Down => {
                    if cursor_row < CANVAS_ROWS - 1 {
                        cursor_row += 1;
                    }
                }
                Key::V => {
                    let ctrl = window.is_key_down(Key::LeftCtrl)
                        || window.is_key_down(Key::RightCtrl);
                    if ctrl {
                        match arboard::Clipboard::new() {
                            Ok(mut clipboard) => match clipboard.get_text() {
                                Ok(text) => {
                                    let pasted = paste_text_to_canvas(
                                        &mut vm,
                                        &text,
                                        &mut cursor_row,
                                        &mut cursor_col,
                                    );
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
                            vm.ram[idx] = ch as u32;
                            advance_cursor(&mut cursor_row, &mut cursor_col);
                        }
                    }
                }
                _ => {
                    let shift = window.is_key_down(Key::LeftShift)
                        || window.is_key_down(Key::RightShift);
                    if let Some(ch) = key_to_ascii_shifted(key, shift) {
                        let idx = cursor_row * CANVAS_COLS + cursor_col;
                        vm.ram[idx] = ch as u32;
                        advance_cursor(&mut cursor_row, &mut cursor_col);
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
        render(&mut buffer, &vm, cursor_row, cursor_col, is_running, &status_msg);
        window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();
    }
}

// ── Load source text from a string onto the canvas grid ──────────
fn load_source_to_canvas(
    vm: &mut vm::Vm,
    source: &str,
    cursor_row: &mut usize,
    cursor_col: &mut usize,
) {
    // Clear canvas grid
    for cell in vm.ram[..CANVAS_COLS * CANVAS_ROWS].iter_mut() {
        *cell = 0;
    }

    let mut row = 0usize;
    let mut col = 0usize;

    for ch in source.chars() {
        if row >= CANVAS_ROWS {
            break;
        }
        if ch == '\n' {
            row += 1;
            col = 0;
        } else if col < CANVAS_COLS {
            vm.ram[row * CANVAS_COLS + col] = ch as u32;
            col += 1;
        }
        // characters beyond column 32 on a single line are dropped
    }

    *cursor_row = 0;
    *cursor_col = 0;
}

// ── Paste text from clipboard onto the canvas grid at cursor position ──
fn paste_text_to_canvas(
    vm: &mut vm::Vm,
    text: &str,
    cursor_row: &mut usize,
    cursor_col: &mut usize,
) -> usize {
    let mut row = *cursor_row;
    let mut col = *cursor_col;
    let mut count = 0usize;

    for ch in text.chars() {
        if row >= CANVAS_ROWS {
            break;
        }
        if ch == '\n' {
            row += 1;
            col = 0;
        } else if ch == '\r' {
            // Skip carriage returns
            continue;
        } else if col < CANVAS_COLS {
            vm.ram[row * CANVAS_COLS + col] = ch as u32;
            col += 1;
            if col >= CANVAS_COLS {
                row += 1;
                col = 0;
            }
            count += 1;
        }
    }

    *cursor_row = row.min(CANVAS_ROWS - 1);
    *cursor_col = col.min(CANVAS_COLS - 1);
    count
}

// ── Canvas assembly: read grid as text, assemble, store bytecode ──
fn canvas_assemble(vm: &mut vm::Vm, canvas_assembled: &mut bool, status_msg: &mut String) {
    let canvas_size = CANVAS_COLS * CANVAS_ROWS;
    let source: String = vm.ram[..canvas_size]
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
    cursor_row: usize,
    cursor_col: usize,
    is_running: bool,
    status_msg: &str,
) {
    for pixel in buffer.iter_mut() {
        *pixel = BG;
    }

    // ── Canvas grid ──────────────────────────────────────────────
    for row in 0..CANVAS_ROWS {
        for col in 0..CANVAS_COLS {
            let val = vm.ram[row * CANVAS_COLS + col];
            let x0 = col * CANVAS_SCALE;
            let y0 = row * CANVAS_SCALE;
            let is_cursor = row == cursor_row && col == cursor_col && !is_running;
            let ascii_byte = (val & 0xFF) as u8;

            let use_pixel_font = val != 0 && ascii_byte >= 0x20 && ascii_byte < 0x80;

            if use_pixel_font {
                let fg = palette_color(val);
                let glyph = &font::GLYPHS[ascii_byte as usize];

                for dy in 0..CANVAS_SCALE {
                    for dx in 0..CANVAS_SCALE {
                        let px = x0 + dx;
                        let py = y0 + dy;
                        let is_border = dx == CANVAS_SCALE - 1 || dy == CANVAS_SCALE - 1;

                        let gc = dx / 2;
                        let gr = dy / 2;
                        let glyph_on = gc < font::GLYPH_W && gr < font::GLYPH_H
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
        render_text(buffer, REGS_X + 200, REGS_Y + (i - 16) * 14, &text, STATUS_FG);
    }

    // ── Status bar ───────────────────────────────────────────────
    let pc_text = format!("PC={:04X} {}", vm.pc, status_msg);
    render_text(buffer, 8, HEIGHT - 20, &pc_text, STATUS_FG);

    let state_label = if is_running {
        ("RUNNING", 0x00FF00)
    } else if vm.halted {
        ("HALTED", 0xFF4444)
    } else {
        ("PAUSED", 0xFFAA00)
    };
    render_text(buffer, WIDTH - 80, HEIGHT - 20, state_label.0, state_label.1);
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

/// Map an ASCII value to an HSV-derived color.
fn palette_color(val: u32) -> u32 {
    let v = (val & 0xFF) as f32;
    let t = if v >= 32.0 && v <= 126.0 {
        (v - 32.0) / 94.0
    } else {
        v / 255.0
    };
    let hue = t * 360.0;
    hsv_to_rgb(hue, 0.8, 1.0)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> u32 {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as i32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let r = ((r + m) * 255.0) as u8;
    let g = ((g + m) * 255.0) as u8;
    let b = ((b + m) * 255.0) as u8;
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

fn advance_cursor(row: &mut usize, col: &mut usize) {
    *col += 1;
    if *col >= CANVAS_COLS {
        *col = 0;
        *row += 1;
        if *row >= CANVAS_ROWS {
            *row = 0;
        }
    }
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
