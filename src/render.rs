// render.rs -- Rendering pipeline for Geometry OS

use crate::font;
use crate::preprocessor;
use crate::vm;
use std::collections::VecDeque;

// ── Building Icon Cache ─────────────────────────────────────────
/// A scaled-down RGBA thumbnail of a pixelpack PNG, for use as a building icon.
pub struct BuildingIcon {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>, // RGBA packed as 0xRRGGBBAA
}

/// Cache of building icons keyed by app name (e.g., "tetris").
/// Loaded once at startup, passed to render each frame.
pub struct BuildingIconCache {
    icons: std::collections::HashMap<String, BuildingIcon>,
}

impl BuildingIconCache {
    pub fn new() -> Self {
        Self {
            icons: std::collections::HashMap::new(),
        }
    }

    /// Load a pixelpack PNG, downscale to target size, and cache it.
    pub fn load_icon(
        &mut self,
        name: &str,
        pxpk_path: &str,
        target_w: usize,
        target_h: usize,
    ) -> bool {
        let data = match std::fs::read(pxpk_path) {
            Ok(d) => d,
            Err(_) => return false,
        };

        // Decode PNG to RGBA pixels using png crate directly
        let decoder = png::Decoder::new(std::io::Cursor::new(data));
        let mut reader = match decoder.read_info() {
            Ok(r) => r,
            Err(_) => return false,
        };

        let info = reader.info().clone();
        let w = info.width as usize;
        let h = info.height as usize;
        let mut buf = vec![0u8; w * h * 4];
        if reader.next_frame(&mut buf).is_err() {
            return false;
        }

        // Downscale using nearest-neighbor sampling to target_w x target_h
        let mut pixels = Vec::with_capacity(target_w * target_h);
        for ty in 0..target_h {
            let src_y = ty * h / target_h;
            for tx in 0..target_w {
                let src_x = tx * w / target_w;
                let off = (src_y * w + src_x) * 4;
                let r = buf[off] as u32;
                let g = buf[off + 1] as u32;
                let b = buf[off + 2] as u32;
                // Pack as RGB for the display buffer
                pixels.push((r << 16) | (g << 8) | b);
            }
        }

        self.icons.insert(
            name.to_string(),
            BuildingIcon {
                width: target_w,
                height: target_h,
                pixels,
            },
        );
        true
    }

    pub fn get(&self, name: &str) -> Option<&BuildingIcon> {
        self.icons.get(name)
    }

    /// Load an icon from raw PNG data (e.g., freshly encoded pixelpack PNG).
    pub fn load_icon_from_data(
        &mut self,
        name: &str,
        png_data: &[u8],
        target_w: usize,
        target_h: usize,
    ) -> bool {
        let decoder = png::Decoder::new(std::io::Cursor::new(png_data));
        let mut reader = match decoder.read_info() {
            Ok(r) => r,
            Err(_) => return false,
        };

        let info = reader.info().clone();
        let w = info.width as usize;
        let h = info.height as usize;
        let mut buf = vec![0u8; w * h * 4];
        if reader.next_frame(&mut buf).is_err() {
            return false;
        }

        let mut pixels = Vec::with_capacity(target_w * target_h);
        for ty in 0..target_h {
            let src_y = ty * h / target_h;
            for tx in 0..target_w {
                let src_x = tx * w / target_w;
                let off = (src_y * w + src_x) * 4;
                let r = buf[off] as u32;
                let g = buf[off + 1] as u32;
                let b = buf[off + 2] as u32;
                pixels.push((r << 16) | (g << 8) | b);
            }
        }

        self.icons.insert(
            name.to_string(),
            BuildingIcon {
                width: target_w,
                height: target_h,
                pixels,
            },
        );
        true
    }
}

// ── Layout constants ─────────────────────────────────────────────
pub const WIDTH: usize = 1024;
pub const HEIGHT: usize = 768;

pub const CANVAS_COLS: usize = 128; // buffer stride (max displayable columns)
pub const CANVAS_ROWS: usize = 32; // default visible rows (Normal mode)
pub const CANVAS_MAX_ROWS: usize = 128;

// ── Font Mode ────────────────────────────────────────────────────
/// Runtime font mode controlling cell size and visible grid dimensions.
/// F1 = Medium (big, readable), F2 = Small (dense, 80-col terminal),
/// F3/F4 = zoom in/out cycling through modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontMode {
    Small,  // cell=12, 85 cols × 64 rows -- dense 80-col terminal feel
    Normal, // cell=16, 32 cols × 48 rows -- default canvas grid
    Medium, // cell=24, 42 cols × 32 rows -- big readable text
}

impl FontMode {
    pub fn cell_size(self) -> usize {
        match self {
            FontMode::Small => 12,
            FontMode::Normal => 16,
            FontMode::Medium => 24,
        }
    }

    pub fn vis_cols(self) -> usize {
        match self {
            FontMode::Small => 85,  // 85 × 12 = 1020px
            FontMode::Normal => 32, // 32 × 16 = 512px (right panel for VM screen)
            FontMode::Medium => 42, // 42 × 24 = 1008px
        }
    }

    pub fn vis_rows(self) -> usize {
        match self {
            FontMode::Small => 64,  // 64 × 12 = 768px
            FontMode::Normal => 48, // 48 × 16 = 768px
            FontMode::Medium => 32, // 32 × 24 = 768px
        }
    }

    /// Whether this mode fills the full render buffer width (no side panel)
    pub fn is_fullwidth(self) -> bool {
        self != FontMode::Normal
    }

    pub fn name(self) -> &'static str {
        match self {
            FontMode::Small => "SMALLTEXT",
            FontMode::Normal => "NORMAL",
            FontMode::Medium => "MEDTEXT",
        }
    }

    pub fn zoom_in(self) -> FontMode {
        match self {
            FontMode::Small => FontMode::Normal,
            FontMode::Normal => FontMode::Medium,
            FontMode::Medium => FontMode::Medium,
        }
    }

    pub fn zoom_out(self) -> FontMode {
        match self {
            FontMode::Small => FontMode::Small,
            FontMode::Normal => FontMode::Small,
            FontMode::Medium => FontMode::Normal,
        }
    }
}

/// Cursor style for terminal/editor modes.
/// Toggled via Ctrl+click on the canvas cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorStyle {
    Block,     // Full cell background highlight (default)
    Underline, // Bottom 2 rows of cell
    Bar,       // Left 2 columns of cell
}

impl CursorStyle {
    pub fn next(self) -> CursorStyle {
        match self {
            CursorStyle::Block => CursorStyle::Underline,
            CursorStyle::Underline => CursorStyle::Bar,
            CursorStyle::Bar => CursorStyle::Block,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            CursorStyle::Block => "BLOCK",
            CursorStyle::Underline => "UNDERLINE",
            CursorStyle::Bar => "BAR",
        }
    }
}

// Legacy constant for code that still uses it
pub const CANVAS_SCALE: usize = 16;

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

// Window title bar colors
const TITLE_BAR_BG: u32 = 0x2A2A3A;
const TITLE_BAR_BG_ACTIVE: u32 = 0x3A3A5A;
const TITLE_BAR_TEXT: u32 = 0xCCCCCC;
const TITLE_BAR_CLOSE: u32 = 0xFF6666;
const TITLE_BAR_CLOSE_BG: u32 = 0x4A2A2A;
const WINDOW_BORDER_COLOR: u32 = 0x444466;
const TITLE_BAR_HIGHLIGHT: u32 = 0x5A5A7A; // lighter top edge = raised/clickable
const TITLE_BAR_SHADOW: u32 = 0x1A1A2A; // darker bottom edge = raised/clickable
const TITLE_BAR_HEIGHT: usize = 12;
const CLOSE_BTN_SIZE: usize = 8;
const CLOSE_BTN_MARGIN: usize = 2;

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
    icon_cache: Option<&BuildingIconCache>,
    text_sel: Option<((usize, usize), (usize, usize))>, // Phase 157: selection highlight
    font_mode: FontMode,                                // Phase 159: runtime font switching
    cursor_style: CursorStyle,                          // Phase 162: block/underline/bar
    cursor_blink_on: bool,                              // Phase 162: blink state (toggles 500ms)
) {
    let cell_size = font_mode.cell_size();
    let vis_cols = font_mode.vis_cols();
    let vis_rows = font_mode.vis_rows();

    for pixel in buffer.iter_mut() {
        *pixel = BG;
    }

    // ── Canvas grid (with scroll offset) ─────────────────────────
    for vis_row in 0..vis_rows {
        let log_row = vis_row + scroll_offset;
        if log_row >= CANVAS_MAX_ROWS {
            break;
        }
        for col in 0..vis_cols {
            if col >= CANVAS_COLS {
                break;
            }
            let ram_addr = log_row * CANVAS_COLS + col;
            let intensity = ram_intensity.get(ram_addr).copied().unwrap_or(0.0);
            let kind = ram_kind
                .get(ram_addr)
                .copied()
                .unwrap_or(vm::MemAccessKind::Read);

            let val = canvas_buffer[log_row * CANVAS_COLS + col];
            let x0 = col * cell_size;
            let y0 = vis_row * cell_size;
            let is_cursor = log_row == cursor_row && col == cursor_col && !is_running;
            // Blinking: if cursor should blink, check blink state
            let show_cursor = is_cursor && (is_running || cursor_blink_on);
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

            // Phase 157: Check if this cell is in the text selection range
            let in_selection = if let Some((sel_start, sel_end)) = text_sel {
                // Normalize selection to top-left..bottom-right
                let (tl, br) = if sel_start.0 < sel_end.0
                    || (sel_start.0 == sel_end.0 && sel_start.1 <= sel_end.1)
                {
                    (sel_start, sel_end)
                } else {
                    (sel_end, sel_start)
                };
                let cell = (log_row, col);
                // Cell is in selection if: same row as tl and col >= tl.1, or
                // same row as br and col <= br.1, or row strictly between tl and br
                if cell.0 == tl.0 && cell.0 == br.0 {
                    cell.1 >= tl.1 && cell.1 <= br.1
                } else if cell.0 == tl.0 {
                    cell.1 >= tl.1
                } else if cell.0 == br.0 {
                    cell.1 <= br.1
                } else {
                    cell.0 > tl.0 && cell.0 < br.0
                }
            } else {
                false
            };
            // Selection colors: blue background, white text
            let sel_bg: u32 = 0x264F78;
            let sel_fg: u32 = 0xFFFFFF;
            let cell_bg = if in_selection { sel_bg } else { cell_bg };

            // Phase 162: cursor style check -- which pixels are cursor-highlighted
            // is_cursor_pixel(dx, dy) returns true if this pixel should be cursor-colored
            let is_cursor_pixel = |dx: usize, dy: usize| -> bool {
                if !show_cursor {
                    return false;
                }
                match cursor_style {
                    CursorStyle::Block => true, // entire cell
                    CursorStyle::Underline => dy >= cell_size.saturating_sub(2), // bottom 2 rows
                    CursorStyle::Bar => dx < 2, // left 2 columns
                }
            };

            if use_pixel_font {
                let fg = if in_selection {
                    sel_fg
                } else {
                    syntax_highlight_color(canvas_buffer, log_row, col)
                };
                let glyph = &font::GLYPHS[ascii_byte as usize];

                for dy in 0..cell_size {
                    for dx in 0..cell_size {
                        let px = x0 + dx;
                        let py = y0 + dy;
                        let is_border = dx == cell_size - 1 || dy == cell_size - 1;

                        // Scale 8x8 glyph to cell_size × cell_size
                        let gc = dx * font::GLYPH_W / cell_size;
                        let gr = dy * font::GLYPH_H / cell_size;
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

                        // Phase 162: style-aware cursor drawing
                        if is_cursor_pixel(dx, dy) {
                            if cursor_style == CursorStyle::Block {
                                // Block cursor: invert fg/bg -- glyph pixels get bg color, background gets CURSOR_COL
                                color = if glyph_on { cell_bg } else { CURSOR_COL };
                            } else {
                                color = CURSOR_COL;
                            }
                        }

                        if px < WIDTH && py < HEIGHT {
                            buffer[py * WIDTH + px] = color;
                        }
                    }
                }
            } else {
                // Empty cell
                for dy in 0..cell_size {
                    for dx in 0..cell_size {
                        let px = x0 + dx;
                        let py = y0 + dy;
                        let is_border = dx == cell_size - 1 || dy == cell_size - 1;
                        let mut color = if is_border { GRID_LINE } else { cell_bg };
                        // Phase 162: style-aware cursor for empty cells
                        if is_cursor_pixel(dx, dy) {
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
    if CANVAS_MAX_ROWS > vis_rows {
        let sb_x = vis_cols * cell_size - 3; // 3px wide bar at right edge
        let sb_height = vis_rows * cell_size;
        let max_scroll = CANVAS_MAX_ROWS.saturating_sub(vis_rows);

        // Background track
        for y in 0..sb_height {
            buffer[y * WIDTH + sb_x] = SCROLLBAR_BG;
            buffer[y * WIDTH + sb_x + 1] = SCROLLBAR_BG;
        }

        // Thumb (proportional to visible/total ratio, minimum 8px)
        let thumb_ratio = (vis_rows * cell_size) as f32 / (CANVAS_MAX_ROWS * cell_size) as f32;
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

    // ── VM screen (only in Normal mode where side panel exists) ───
    if !font_mode.is_fullwidth() {
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
    }

    // ── Building icon overlay ───────────────────────────────────
    // After VM screen blit, overlay pixelpack PNG icons on buildings.
    // Read building table from RAM (same layout as world_desktop.asm).
    // RAM[0x7500-0x757F]: buildings [world_x, world_y, type_color, name_addr] x 10
    // RAM[0x7580]: building count
    // RAM[0x7800]: camera_x, RAM[0x7801]: camera_y
    if let Some(icon_cache) = icon_cache {
        let bldg_count = vm.ram.get(0x7580).copied().unwrap_or(0).min(32) as usize;
        let cam_x = vm.ram.get(0x7800).copied().unwrap_or(0) as i32;
        let cam_y = vm.ram.get(0x7801).copied().unwrap_or(0) as i32;

        for i in 0..bldg_count {
            let base = 0x7500 + i * 4;
            let bldg_x = vm.ram.get(base).copied().unwrap_or(0) as i32;
            let bldg_y = vm.ram.get(base + 1).copied().unwrap_or(0) as i32;
            let name_addr = vm.ram.get(base + 3).copied().unwrap_or(0) as usize;

            // Read building name from RAM
            let mut name = String::new();
            for j in 0..16 {
                if name_addr + j >= vm.ram.len() {
                    break;
                }
                let ch = vm.ram[name_addr + j];
                if ch == 0 || ch > 127 {
                    break;
                }
                name.push(ch as u8 as char);
            }

            if let Some(icon) = icon_cache.get(&name) {
                // Compute screen position: (bldg_x - cam_x) * 4 + VM_SCREEN_X, etc.
                let scr_x = (bldg_x - cam_x) * 4;
                let scr_y = (bldg_y - cam_y) * 4;

                // Skip if off-screen
                if scr_x + (icon.width as i32) < 0
                    || scr_x >= 256
                    || scr_y + (icon.height as i32) < 0
                    || scr_y >= 256
                {
                    continue;
                }

                // Overlay icon pixels onto the VM screen area of the display buffer
                for iy in 0..icon.height {
                    for ix in 0..icon.width {
                        let px = scr_x + ix as i32;
                        let py = scr_y + iy as i32;
                        if px >= 0 && px < 256 && py >= 0 && py < 256 {
                            let color = icon.pixels[iy * icon.width + ix];
                            let sx = VM_SCREEN_X + px as usize;
                            let sy = VM_SCREEN_Y + py as usize;
                            if sx < WIDTH && sy < HEIGHT {
                                buffer[sy * WIDTH + sx] = color;
                            }
                        }
                    }
                }
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
    let scroll_info = if scroll_offset > 0 || cursor_row >= vis_rows {
        format!(
            "[scroll {}-{}] ",
            scroll_offset + 1,
            scroll_offset + vis_rows
        )
    } else {
        String::new()
    };
    let pc_text = format!(
        "PC={:04X} {}{}[{} {}×{}] cur={}{} ",
        vm.pc,
        scroll_info,
        row_info,
        font_mode.name(),
        vis_cols,
        vis_rows,
        cursor_style.name(),
        if cursor_blink_on { "●" } else { "○" }
    );
    render_text(buffer, 8, HEIGHT - 20, &pc_text, STATUS_FG);
    // Append status_msg separately (may be long)
    if !status_msg.is_empty() {
        let sm_x = 8 + pc_text.len() * 6;
        if sm_x < WIDTH - 250 {
            render_text(buffer, sm_x, HEIGHT - 20, status_msg, STATUS_FG);
        }
    }

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
        WIDTH - 200,
        HEIGHT - 20,
        state_label.0,
        state_label.1,
    );

    // ── Desktop polish: notification count ──────────────────────
    let notif_count = vm.ram.get(0x7600).copied().unwrap_or(0);
    if notif_count > 0 {
        let notif_text = format!("[{} notif]", notif_count.min(99));
        render_text(buffer, WIDTH - 300, HEIGHT - 20, &notif_text, 0xFFAA00);
    }
}

// ── Fullscreen Map Rendering ────────────────────────────────────
// RAM ports for map control (written by host, read by asm + render):
//   0x7800: camera_x (integer tile coords, written by asm or host)
//   0x7801: camera_y (integer tile coords)
//   0x7802: frame_counter
//   0x7810: camera_frac_x (fixed-point 16.16 fractional camera offset)
//   0x7811: camera_frac_y (fixed-point 16.16 fractional camera offset)
//   0x7812: zoom_level (0=far/1px tiles, 1=2px, 2=4px default, 3=8px, 4=16px)
//   0x7813: map_flags (bit0=fullscreen mode active, bit1=show_debug_hud)

/// Scale the 256x256 VM screen to fill the 1024x768 window.
/// Uses nearest-neighbor scaling. The map is 768x768 (3x scale) centered
/// with a 128px sidebar on the right for debug info.
pub fn render_fullscreen_map(
    buffer: &mut [u32],
    vm: &vm::Vm,
    icon_cache: Option<&BuildingIconCache>,
) {
    // Clear to black
    for pixel in buffer.iter_mut() {
        *pixel = 0x050508;
    }

    // Zoom level determines how much of the 256x256 VM screen we show:
    //   0 = show all 256x256 at ~2x (512x512, centered in 768 area with padding)
    //   1 = show all 256x256 at 3x (768x768) -- standard view
    //   2 = show center 128x128 at 6x (768x768) -- zoomed in 2x
    //   3 = show center 64x64 at 12x (768x768) -- zoomed in 4x
    //   4 = show center 32x32 at 24x (768x768) -- zoomed in 8x
    let zoom = vm.ram.get(0x7812).copied().unwrap_or(2).min(4);
    let (src_region, scale) = match zoom {
        0 => (256, 2), // 256px source, 2x scale -> 512x512
        1 => (256, 3), // 256px source, 3x scale -> 768x768
        2 => (128, 6), // 128px center, 6x scale -> 768x768
        3 => (64, 12), // 64px center, 12x scale -> 768x768
        4 => (32, 24), // 32px center, 24x scale -> 768x768
        _ => (256, 3),
    };
    let src_offset = (256 - src_region) / 2; // center crop
    let map_display_size = 768; // the 768x768 area
    let map_offset = (map_display_size - src_region * scale) / 2; // center if < 768

    let sidebar_x = 768; // sidebar always starts at x=768

    // ── Blit VM screen with zoom-dependent crop and scale ──────
    for sy in 0..src_region {
        for sx in 0..src_region {
            let vm_x = src_offset + sx;
            let vm_y = src_offset + sy;
            if vm_x >= 256 || vm_y >= 256 {
                continue;
            }
            let color = vm.screen[vm_y * 256 + vm_x];
            let base_x = map_offset + sx * scale;
            let base_y = map_offset + sy * scale;
            for dy in 0..scale {
                for dx in 0..scale {
                    let px = base_x + dx;
                    let py = base_y + dy;
                    if px < WIDTH && py < HEIGHT {
                        buffer[py * WIDTH + px] = color;
                    }
                }
            }
        }
    }

    // ── Phase 107: Blit world-space windows (scaled with zoom) ──
    // World-space windows are positioned in tile coordinates and pan with the camera.
    // Uses the same crop+scale mapping as the terrain blit to stay aligned.
    {
        use crate::vm::types::WORLD_COORD_UNSET;

        let cam_x_tiles = vm.ram.get(0x7800).copied().unwrap_or(0) as i32;
        let cam_y_tiles = vm.ram.get(0x7801).copied().unwrap_or(0) as i32;

        // Collect world-space windows sorted by z_order
        let mut world_wins: Vec<&crate::vm::Window> = vm
            .windows
            .iter()
            .filter(|w| {
                w.active
                    && w.is_world_space()
                    && w.world_x != WORLD_COORD_UNSET
                    && w.world_y != WORLD_COORD_UNSET
            })
            .collect();
        world_wins.sort_by_key(|w| w.z_order);

        for win in &world_wins {
            // Window position in VM framebuffer coords (same space as terrain)
            let vm_fb_x = (win.world_x as i32 - cam_x_tiles) * 8;
            let vm_fb_y = (win.world_y as i32 - cam_y_tiles) * 8;

            // Visibility check: any part of the window within the cropped region?
            let so = src_offset as i32;
            let sr = src_region as i32;
            if vm_fb_x + win.w as i32 <= so
                || vm_fb_y + win.h as i32 <= so
                || vm_fb_x >= so + sr
                || vm_fb_y >= so + sr
            {
                continue;
            }

            // Apply same crop + scale as terrain blit:
            // display = map_offset + (vm_fb - src_offset) * scale
            let sx = map_offset as i32 + (vm_fb_x - so) * scale as i32;
            let sy = map_offset as i32 + (vm_fb_y - so) * scale as i32;
            let s_scale = scale as i32;

            // Blit offscreen buffer with scale
            let ww = win.w as usize;
            for py in 0..win.h as usize {
                for px in 0..ww {
                    let color = win.offscreen_buffer[py * ww + px];
                    if color == 0 {
                        continue; // transparent
                    }
                    // Scale each pixel
                    let base_x = sx + (px as i32) * s_scale;
                    let base_y = sy + (py as i32) * s_scale;
                    for dy in 0..s_scale {
                        for dx in 0..s_scale {
                            let px_out = base_x + dx;
                            let py_out = base_y + dy;
                            if px_out >= 0
                                && px_out < WIDTH as i32
                                && py_out >= 0
                                && py_out < HEIGHT as i32
                            {
                                buffer[(py_out as usize) * WIDTH + (px_out as usize)] = color;
                            }
                        }
                    }
                }
            }

            // Draw title bar overlay on world-space window
            let title = win.read_title(&vm.ram);
            let display_title = if title.is_empty() {
                format!("Win {}", win.id)
            } else {
                title
            };
            let is_active = win.z_order == world_wins.iter().map(|w| w.z_order).max().unwrap_or(0);
            draw_title_bar(
                buffer,
                WIDTH,
                sx,
                sy,
                win.w,
                &display_title,
                is_active,
                s_scale,
            );
        }
    }

    // ── Taskbar overlay (always on top of world-space windows) ───────
    // Z-order: terrain < windows < taskbar. The taskbar occupies y=240..255
    // in the 256x256 VM screen. Re-blit these rows on top of windows.
    {
        let tbar_y: i32 = vm::types::TASKBAR_Y as i32;
        let tbar_h: i32 = vm::types::TASKBAR_H as i32;
        let so = src_offset as i32;
        let sr = src_region as i32;
        let sc = scale as i32;

        for ty in 0..tbar_h {
            let vm_y = tbar_y + ty;
            let crop_y = vm_y - so;
            if crop_y < 0 || crop_y >= sr {
                continue; // taskbar row outside cropped view
            }
            let disp_base_y = map_offset as i32 + crop_y * sc;
            for tx in 0..256i32 {
                let crop_x = tx - so;
                if crop_x < 0 || crop_x >= sr {
                    continue; // pixel outside cropped view
                }
                let color = vm.screen[(vm_y as usize) * 256 + (tx as usize)];
                if color == 0 {
                    continue; // transparent
                }
                let disp_base_x = map_offset as i32 + crop_x * sc;
                for dy in 0..sc {
                    for dx in 0..sc {
                        let px = disp_base_x + dx;
                        let py = disp_base_y + dy;
                        if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                            buffer[(py as usize) * WIDTH + (px as usize)] = color;
                        }
                    }
                }
            }
        }
    }

    // ── Building icon overlay (scaled with zoom) ───────────────
    if let Some(icon_cache) = icon_cache {
        let bldg_count = vm.ram.get(0x7580).copied().unwrap_or(0).min(32) as usize;
        let cam_x = vm.ram.get(0x7800).copied().unwrap_or(0) as i32;
        let cam_y = vm.ram.get(0x7801).copied().unwrap_or(0) as i32;

        for i in 0..bldg_count {
            let base = 0x7500 + i * 4;
            let bldg_x = vm.ram.get(base).copied().unwrap_or(0) as i32;
            let bldg_y = vm.ram.get(base + 1).copied().unwrap_or(0) as i32;
            let name_addr = vm.ram.get(base + 3).copied().unwrap_or(0) as usize;

            let mut name = String::new();
            for j in 0..16 {
                if name_addr + j >= vm.ram.len() {
                    break;
                }
                let ch = vm.ram[name_addr + j];
                if ch == 0 || ch > 127 {
                    break;
                }
                name.push(ch as u8 as char);
            }

            if let Some(icon) = icon_cache.get(&name) {
                // Building position in VM screen pixels
                let scr_x = (bldg_x - cam_x) * 4;
                let scr_y = (bldg_y - cam_y) * 4;

                // Skip if off the VM screen
                if scr_x + (icon.width as i32) < 0
                    || scr_x >= 256
                    || scr_y + (icon.height as i32) < 0
                    || scr_y >= 256
                {
                    continue;
                }

                // Apply zoom crop: convert VM coords to display coords
                for iy in 0..icon.height {
                    for ix in 0..icon.width {
                        let px = scr_x + ix as i32;
                        let py = scr_y + iy as i32;
                        if px >= 0 && px < 256 && py >= 0 && py < 256 {
                            // Check if this pixel falls within the cropped region
                            let crop_px = px - src_offset as i32;
                            let crop_py = py - src_offset as i32;
                            if crop_px < 0
                                || crop_px >= src_region as i32
                                || crop_py < 0
                                || crop_py >= src_region as i32
                            {
                                continue;
                            }
                            let color = icon.pixels[iy * icon.width + ix];
                            // Draw at zoom scale
                            let disp_x = map_offset + crop_px as usize * scale;
                            let disp_y = map_offset + crop_py as usize * scale;
                            for dy in 0..scale {
                                for dx in 0..scale {
                                    let dpx = disp_x + dx;
                                    let dpy = disp_y + dy;
                                    if dpx < WIDTH && dpy < HEIGHT {
                                        buffer[dpy * WIDTH + dpx] = color;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ── Sidebar: debug HUD ──────────────────────────────────────
    let hud_fg = 0x888899;
    let hud_bright = 0xCCCCEE;

    // Camera position
    let cam_x = vm.ram.get(0x7800).copied().unwrap_or(0);
    let cam_y = vm.ram.get(0x7801).copied().unwrap_or(0);
    let zoom = vm.ram.get(0x7812).copied().unwrap_or(2);
    render_text(buffer, sidebar_x + 4, 4, "MAP VIEW", hud_bright);
    render_text(
        buffer,
        sidebar_x + 4,
        20,
        &format!("cam ({},{})", cam_x, cam_y),
        hud_fg,
    );
    render_text(buffer, sidebar_x + 4, 34, &format!("zoom {}", zoom), hud_fg);

    // Player position
    let px = vm.ram.get(0x7808).copied().unwrap_or(0);
    let py = vm.ram.get(0x7809).copied().unwrap_or(0);
    render_text(
        buffer,
        sidebar_x + 4,
        54,
        &format!("player ({},{})", px, py),
        hud_fg,
    );

    // Building count
    let bldg_count = vm.ram.get(0x7580).copied().unwrap_or(0).min(32);
    render_text(
        buffer,
        sidebar_x + 4,
        74,
        &format!("buildings {}", bldg_count),
        hud_fg,
    );

    // Building list
    for i in 0..bldg_count as usize {
        let base = 0x7500 + i * 4;
        let bx = vm.ram.get(base).copied().unwrap_or(0);
        let by = vm.ram.get(base + 1).copied().unwrap_or(0);
        let name_addr = vm.ram.get(base + 3).copied().unwrap_or(0) as usize;

        let mut name = String::new();
        for j in 0..12 {
            if name_addr + j >= vm.ram.len() {
                break;
            }
            let ch = vm.ram[name_addr + j];
            if ch == 0 || ch > 127 {
                break;
            }
            name.push(ch as u8 as char);
        }

        let y_pos = 96 + i * 14;
        if y_pos + 12 < HEIGHT {
            // Highlight if player is near
            let dist = (bx as i32 - px as i32).abs() + (by as i32 - py as i32).abs();
            let color = if dist < 8 { 0x44FF44 } else { hud_fg };
            render_text(
                buffer,
                sidebar_x + 4,
                y_pos,
                &format!("{} ({},{})", name, bx, by),
                color,
            );
        }
    }

    // Controls help at bottom of sidebar
    render_text(
        buffer,
        sidebar_x + 4,
        HEIGHT - 80,
        "WASD/arrows: move",
        0x555566,
    );
    render_text(
        buffer,
        sidebar_x + 4,
        HEIGHT - 64,
        "drag: pan map",
        0x555566,
    );
    render_text(buffer, sidebar_x + 4, HEIGHT - 48, "scroll: zoom", 0x555566);
    render_text(
        buffer,
        sidebar_x + 4,
        HEIGHT - 32,
        "dblclick: enter",
        0x555566,
    );
    render_text(buffer, sidebar_x + 4, HEIGHT - 16, "Esc: exit", 0x555566);
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

/// Draw a window title bar into a pixel buffer.
/// Used by both screen-space windows (scale=1, buf_width=256) and
/// world-space windows (scale=zoom, buf_width=1024).
pub fn draw_title_bar(
    buffer: &mut [u32],
    buf_width: usize,
    x0: i32,
    y0: i32,
    win_w: u32,
    title: &str,
    is_active: bool,
    scale: i32,
) {
    let bar_h = (TITLE_BAR_HEIGHT as i32) * scale;
    let w = win_w as i32;
    let bg_color = if is_active {
        TITLE_BAR_BG_ACTIVE
    } else {
        TITLE_BAR_BG
    };

    // Draw title bar background
    for dy in 0..bar_h {
        let py = y0 + dy;
        if py < 0 {
            continue;
        }
        for dx in 0..w * scale {
            let px = x0 + dx;
            if px < 0 {
                continue;
            }
            let idx = (py as usize) * buf_width + (px as usize);
            if idx < buffer.len() {
                buffer[idx] = bg_color;
            }
        }
    }

    // Draw raised edge: lighter top row + left column (clickable affordance)
    {
        // Top edge (highlight)
        let py = y0;
        if py >= 0 {
            for dx in 0..w * scale {
                let px = x0 + dx;
                if px >= 0 {
                    let idx = (py as usize) * buf_width + (px as usize);
                    if idx < buffer.len() {
                        buffer[idx] = TITLE_BAR_HIGHLIGHT;
                    }
                }
            }
        }
        // Left edge (highlight)
        let px = x0;
        if px >= 0 {
            for dy in 0..bar_h {
                let py = y0 + dy;
                if py >= 0 {
                    let idx = (py as usize) * buf_width + (px as usize);
                    if idx < buffer.len() {
                        buffer[idx] = TITLE_BAR_HIGHLIGHT;
                    }
                }
            }
        }
        // Right edge (shadow)
        let px = x0 + w * scale - 1;
        if px >= 0 {
            for dy in 0..bar_h {
                let py = y0 + dy;
                if py >= 0 {
                    let idx = (py as usize) * buf_width + (px as usize);
                    if idx < buffer.len() {
                        buffer[idx] = TITLE_BAR_SHADOW;
                    }
                }
            }
        }
    }

    // Draw border line under title bar
    let border_y = y0 + bar_h;
    if border_y >= 0 {
        for dx in 0..w * scale {
            let px = x0 + dx;
            if px < 0 {
                continue;
            }
            let idx = (border_y as usize) * buf_width + (px as usize);
            if idx < buffer.len() {
                buffer[idx] = WINDOW_BORDER_COLOR;
            }
        }
    }

    // Draw close button background rectangle
    {
        let cb_x0 = x0 + (w - CLOSE_BTN_MARGIN as i32 - CLOSE_BTN_SIZE as i32 - 1) * scale;
        let cb_y0 = y0 + (CLOSE_BTN_MARGIN as i32 - 1) * scale;
        let cb_w = (CLOSE_BTN_SIZE as i32 + 2) * scale;
        let cb_h = (CLOSE_BTN_SIZE as i32 + 2) * scale;
        for dy in 0..cb_h {
            let py = cb_y0 + dy;
            if py < 0 {
                continue;
            }
            for dx in 0..cb_w {
                let px = cb_x0 + dx;
                if px < 0 {
                    continue;
                }
                let idx = (py as usize) * buf_width + (px as usize);
                if idx < buffer.len() {
                    buffer[idx] = TITLE_BAR_CLOSE_BG;
                }
            }
        }
    }

    // Draw title text (8x8 font, scaled)
    let char_w = (font::GLYPH_W as i32) * scale;
    let max_chars = (w * scale) / (char_w + scale) - 3;
    let mut cx = x0 + 2 * scale;
    let cy = y0 + 2 * scale;
    for (i, ch) in title.chars().enumerate() {
        if i as i32 >= max_chars {
            break;
        }
        let idx = ch as usize;
        if idx < 128 {
            let glyph = &font::GLYPHS[idx];
            for (row, &glyph_row) in glyph.iter().enumerate().take(font::GLYPH_H) {
                for col in 0..font::GLYPH_W {
                    if glyph_row & (1 << (7 - col)) != 0 {
                        for sy in 0..scale {
                            for sx in 0..scale {
                                let px = cx + (col as i32) * scale + sx;
                                let py = cy + (row as i32) * scale + sy;
                                if px >= 0 && py >= 0 {
                                    let buf_idx = (py as usize) * buf_width + (px as usize);
                                    if buf_idx < buffer.len() {
                                        buffer[buf_idx] = TITLE_BAR_TEXT;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        cx += char_w + scale;
    }

    // Draw close button (X glyph) at top-right corner
    let btn_x = x0 + (w - CLOSE_BTN_MARGIN as i32 - CLOSE_BTN_SIZE as i32) * scale;
    let btn_y = y0 + CLOSE_BTN_MARGIN as i32 * scale;
    let x_glyph = &font::GLYPHS['X' as usize];
    for (row, &glyph_row) in x_glyph.iter().enumerate().take(font::GLYPH_H) {
        for col in 0..font::GLYPH_W {
            if glyph_row & (1 << (7 - col)) != 0 {
                for sy in 0..scale {
                    for sx in 0..scale {
                        let px = btn_x + (col as i32) * scale + sx;
                        let py = btn_y + (row as i32) * scale + sy;
                        if px >= 0 && py >= 0 {
                            let buf_idx = (py as usize) * buf_width + (px as usize);
                            if buf_idx < buffer.len() {
                                buffer[buf_idx] = TITLE_BAR_CLOSE;
                            }
                        }
                    }
                }
            }
        }
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
