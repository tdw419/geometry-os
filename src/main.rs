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
const VM_SCREEN_SCALE: usize = 1;

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
                    // Newline: advance to start of next row
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
                    vm.ram[idx] = 0x20; // space
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
                    if !is_running {
                        if vm.halted {
                            vm.pc = if canvas_assembled {
                                CANVAS_BYTECODE_ADDR as u32
                            } else {
                                0
                            };
                            vm.halted = false;
                        }
                        is_running = true;
                    } else {
                        is_running = false;
                    }
                }
                Key::F8 => {
                    let ctrl = window.is_key_down(Key::LeftCtrl)
                        || window.is_key_down(Key::RightCtrl);
                    if !ctrl {
                        // Assemble canvas text to bytecode
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
                _ => {
                    // Printable character input
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

// ── Canvas assembly: read grid as text, assemble, store bytecode ──
fn canvas_assemble(vm: &mut vm::Vm, canvas_assembled: &mut bool, status_msg: &mut String) {
    let canvas_size = CANVAS_COLS * CANVAS_ROWS;
    let source: String = vm.ram[..canvas_size]
        .iter()
        .map(|&cell| {
            let val = cell & 0xFF;
            if val == 0 {
                '\n'
            } else if val == 0x0A {
                '\n'
            } else {
                (val as u8) as char
            }
        })
        .collect();

    // Collapse consecutive newlines
    let source = source.replace("\n\n", "\n");

    match assembler::assemble(&source) {
        Ok(asm_result) => {
            // Clear bytecode region
            let ram_len = vm.ram.len();
            for v in vm.ram[CANVAS_BYTECODE_ADDR..ram_len.min(CANVAS_BYTECODE_ADDR + 4096)].iter_mut()
            {
                *v = 0;
            }
            // Write bytecode
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
    // Clear background
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
                // Pixel font mode: glyph bits = fg color, gaps = bg color
                let fg = palette_color(val);
                let glyph = &font::GLYPHS[ascii_byte as usize];

                for dy in 0..CANVAS_SCALE {
                    for dx in 0..CANVAS_SCALE {
                        let px = x0 + dx;
                        let py = y0 + dy;
                        let is_border = dx == CANVAS_SCALE - 1 || dy == CANVAS_SCALE - 1;

                        // Map to glyph coordinates at 2x scale
                        let gc = dx / 2;
                        let gr = dy / 2;
                        let glyph_on = if gc < font::GLYPH_W && gr < font::GLYPH_H {
                            glyph[gr] & (1 << (7 - gc)) != 0
                        } else {
                            false
                        };

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
                // Empty cell or non-printable
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
        let val = vm.regs[i];
        let text = format!("r{:02}={:08X}", i, val);
        render_text(buffer, REGS_X, REGS_Y + i * 14, &text, STATUS_FG);
    }
    for i in 16..32 {
        let val = vm.regs[i];
        let text = format!("r{:02}={:08X}", i, val);
        render_text(buffer, REGS_X + 200, REGS_Y + (i - 16) * 14, &text, STATUS_FG);
    }

    // ── Status bar ───────────────────────────────────────────────
    let pc_text = format!("PC={:04X} {}", vm.pc, status_msg);
    render_text(buffer, 8, HEIGHT - 20, &pc_text, STATUS_FG);

    if is_running {
        render_text(buffer, WIDTH - 80, HEIGHT - 20, "RUNNING", 0x00FF00);
    } else if vm.halted {
        render_text(buffer, WIDTH - 60, HEIGHT - 20, "HALTED", 0xFF4444);
    } else {
        render_text(buffer, WIDTH - 60, HEIGHT - 20, "PAUSED", 0xFFAA00);
    }
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
        cx += font::GLYPH_W + 1; // 8 wide + 1 gap
    }
}

/// Map an ASCII value to an HSV-derived color.
/// t = (val - 32) / 94 maps printable ASCII 0x20-0x7E to hue 0-360.
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
        Key::Enter => Some(b'\n'),
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
    // Letters: shift -> uppercase, no shift -> lowercase
    let letter = match key {
        Key::A => Some((b'a', b'A')),
        Key::B => Some((b'b', b'B')),
        Key::C => Some((b'c', b'C')),
        Key::D => Some((b'd', b'D')),
        Key::E => Some((b'e', b'E')),
        Key::F => Some((b'f', b'F')),
        Key::G => Some((b'g', b'G')),
        Key::H => Some((b'h', b'H')),
        Key::I => Some((b'i', b'I')),
        Key::J => Some((b'j', b'J')),
        Key::K => Some((b'k', b'K')),
        Key::L => Some((b'l', b'L')),
        Key::M => Some((b'm', b'M')),
        Key::N => Some((b'n', b'N')),
        Key::O => Some((b'o', b'O')),
        Key::P => Some((b'p', b'P')),
        Key::Q => Some((b'q', b'Q')),
        Key::R => Some((b'r', b'R')),
        Key::S => Some((b's', b'S')),
        Key::T => Some((b't', b'T')),
        Key::U => Some((b'u', b'U')),
        Key::V => Some((b'v', b'V')),
        Key::W => Some((b'w', b'W')),
        Key::X => Some((b'x', b'X')),
        Key::Y => Some((b'y', b'Y')),
        Key::Z => Some((b'z', b'Z')),
        _ => None,
    };
    if let Some((lower, upper)) = letter {
        return Some(if shift { upper } else { lower });
    }

    // Numbers and symbols (all as (normal, shifted) tuples)
    let symbol = match key {
        Key::Key0 => Some((b'0', b')')),
        Key::Key1 => Some((b'1', b'!')),
        Key::Key2 => Some((b'2', b'@')),
        Key::Key3 => Some((b'3', b'#')),
        Key::Key4 => Some((b'4', b'$')),
        Key::Key5 => Some((b'5', b'%')),
        Key::Key6 => Some((b'6', b'^')),
        Key::Key7 => Some((b'7', b'&')),
        Key::Key8 => Some((b'8', b'*')),
        Key::Key9 => Some((b'9', b'(')),
        Key::Minus => Some((b'-', b'_')),
        Key::Equal => Some((b'=', b'+')),
        Key::Comma => Some((b',', b'<')),
        Key::Period => Some((b'.', b'>')),
        Key::Slash => Some((b'/', b'?')),
        Key::Semicolon => Some((b';', b':')),
        Key::Apostrophe => Some((b'\'', b'"')),
        Key::LeftBracket => Some((b'[', b'{')),
        Key::RightBracket => Some((b']', b'}')),
        Key::Backslash => Some((b'\\', b'|')),
        Key::Backquote => Some((b'`', b'~')),
        _ => None,
    };

    if let Some((normal, shifted)) = symbol {
        return Some(if shift { shifted } else { normal });
    }

    None
}
