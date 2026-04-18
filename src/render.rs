// render.rs -- Rendering pipeline for Geometry OS

use crate::font;
use crate::preprocessor;
use crate::vm;
use std::collections::VecDeque;

// ── Layout constants ─────────────────────────────────────────────
pub const WIDTH: usize = 1024;
pub const HEIGHT: usize = 768;

pub const CANVAS_SCALE: usize = 16;
pub const CANVAS_COLS: usize = 32;
pub const CANVAS_ROWS: usize = 32;
pub const CANVAS_MAX_ROWS: usize = 128;

pub const VM_SCREEN_X: usize = 640;
pub const VM_SCREEN_Y: usize = 64;

pub const REGS_X: usize = 640;
pub const REGS_Y: usize = 340;

pub const RAM_VIEW_X: usize = 0;
pub const RAM_VIEW_Y: usize = 512;
pub const RAM_VIEW_SCALE: usize = 8;

pub const HEATMAP_X: usize = 256;
pub const HEATMAP_Y: usize = 512;

pub const CANVAS_BYTECODE_ADDR: usize = 0x1000;

// ── Colors ───────────────────────────────────────────────────────
const BG: u32 = 0x050508;
const GRID_BG: u32 = 0x0A0A14;
const GRID_LINE: u32 = 0x141420;
const CURSOR_COL: u32 = 0x00FFFF;
const STATUS_FG: u32 = 0x888899;
const SCROLLBAR_BG: u32 = 0x181828;
const SCROLLBAR_FG: u32 = 0x334466;

// ── Syntax highlighting colors ──────────────────────────────────
const SYN_OPCODE: u32 = 0x00CCFF;
const SYN_REGISTER: u32 = 0x44FF88;
const SYN_NUMBER: u32 = 0xFFAA33;
const SYN_LABEL: u32 = 0xFFDD44;
const SYN_COMMENT: u32 = 0x555566;
const SYN_DEFAULT: u32 = 0xAAAA88;

pub fn lerp_color(base: u32, tint: u32, t: f32) -> u32 {
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

#[allow(clippy::too_many_arguments)]
pub fn render(
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
            let kind = ram_kind
                .get(ram_addr)
                .copied()
                .unwrap_or(vm::MemAccessKind::Read);

            let val = canvas_buffer[log_row * CANVAS_COLS + col];
            let x0 = col * CANVAS_SCALE;
            let y0 = vis_row * CANVAS_SCALE;
            let is_cursor = log_row == cursor_row && col == cursor_col && !is_running;
            let ascii_byte = (val & 0xFF) as u8;

            let use_pixel_font = val != 0 && (0x20..0x80).contains(&ascii_byte);

            // Determine cell base color (with intensity tint)
            let mut tint_color = if kind == vm::MemAccessKind::Write {
                0xFF00FF
            } else {
                0x00FFFF
            };
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
        let thumb_ratio =
            (CANVAS_ROWS * CANVAS_SCALE) as f32 / (CANVAS_MAX_ROWS * CANVAS_SCALE) as f32;
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
            if addr >= vm.ram.len() {
                break;
            }

            let raw_val = vm.ram[addr];
            let intensity = ram_intensity.get(addr).copied().unwrap_or(0.0);
            let kind = ram_kind
                .get(addr)
                .copied()
                .unwrap_or(vm::MemAccessKind::Read);

            // Base color is the RAM value (masked to 24-bit)
            let base_color = raw_val & 0xFFFFFF;

            // Pulse tint
            let tint_color = if kind == vm::MemAccessKind::Write {
                0xFF00FF
            } else {
                0x00FFFF
            };
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
        let kind = ram_kind
            .get(addr)
            .copied()
            .unwrap_or(vm::MemAccessKind::Read);

        // Base color: Dim gray if data exists, else black
        let base_color = if raw_val > 0 { 0x222222 } else { 0x050505 };

        // Pulse tint
        let tint_color = if kind == vm::MemAccessKind::Write {
            0xFF00FF
        } else {
            0x00FFFF
        };
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
            if before.len() > 4 {
                before.remove(0);
            }
        } else if a == pc {
            found_pc = true;
        } else {
            after.push(a);
            if after.len() >= 5 {
                break;
            }
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
        let start = pc_idx.saturating_sub(4);
        display_addrs = display_addrs[start..(start + 10).min(total)].to_vec();
    }

    for (i, &addr) in display_addrs.iter().enumerate() {
        let (mnemonic, _) = vm.disassemble_at(addr);
        let is_pc = addr == pc;
        let marker = if is_pc { ">" } else { " " };
        let line = format!("{}{:04X} {}", marker, addr, mnemonic);
        let color = if is_pc { disasm_pc_color } else { disasm_color };
        let line_y = disasm_y + 14 + i * 12;
        if line_y + 12 < HEIGHT - 24 {
            render_text(buffer, REGS_X, line_y, &line, color);
        }
    }

    // ── Status bar ───────────────────────────────────────────────
    let row_info = format!("row {}/{} ", cursor_row + 1, CANVAS_MAX_ROWS);
    let scroll_info = if scroll_offset > 0 || cursor_row >= CANVAS_ROWS {
        format!(
            "[scroll {}-{}] ",
            scroll_offset + 1,
            scroll_offset + CANVAS_ROWS
        )
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
pub fn render_text(buffer: &mut [u32], x0: usize, y0: usize, text: &str, color: u32) {
    let mut cx = x0;
    for ch in text.chars() {
        let idx = ch as usize;
        if idx < 128 {
            let glyph = &font::GLYPHS[idx];
            for (row, &glyph_row) in glyph.iter().enumerate().take(font::GLYPH_H) {
                for col in 0..font::GLYPH_W {
                    if glyph_row & (1 << (7 - col)) != 0 {
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
pub fn syntax_highlight_color(canvas_buffer: &[u32], row: usize, col: usize) -> u32 {
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
        if (0x20..0x80).contains(&byte) {
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
                preprocessor::SynTok::Formula => SYN_OPCODE, // formula ops highlighted like opcodes
            };
        }
    }

    SYN_DEFAULT
}
