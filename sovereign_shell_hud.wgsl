// ============================================================================
// SOVEREIGN SHELL HUD SHADER - Natural Language Control for Geometry OS
// ============================================================================
// Architecture:
//   Row 0-399:   Agent execution space
//   Row 400-449: HUD zone (registers, messages)
//   Row 450-479: INPUT ZONE (user types here)
//   Row 475-479: PATCH STATUS (success/fail display)
// ============================================================================

struct Pixel {
    r: u32,
    g: u32,
    b: u32,
    a: u32,
}

struct Config {
    width: u32,
    height: u32,
    time: f32,
    frame: u32,
    mode: u32,
}

// Double buffers
@group(0) @binding(0) var<storage, read_write> buffer_out: array<Pixel>;
@group(0) @binding(1) var<storage, read> buffer_in: array<Pixel>;

// Register state (26 registers A-Z)
@group(0) @binding(2) var<storage, read> registers: array<u32>;
@group(0) @binding(3) var<storage, read> stack: array<u32>;
@group(0) @binding(4) var<uniform> config: Config;

// Stats (SP, IP, stack depth)
@group(0) @binding(5) var<storage, read> vm_stats: array<u32>;

// Input buffer (64 chars max for text input)
@group(0) @binding(6) var<storage, read> input_buffer: array<u32>;

// Patch status (0=none, 1=success, 2=fail)
@group(0) @binding(7) var<storage, read> patch_status: array<u32>;

// Execution result (displayed in HUD)
@group(0) @binding(8) var<storage, read> exec_result: array<u32>;

// 5x7 bitmap font atlas: 119 u32 words packing 475 bytes (95 printable ASCII * 5 columns)
// Critical for INPUT ZONE text rendering - required by get_font_column() for OCR-ready display
@group(0) @binding(9) var<storage, read> font_atlas: array<u32>;

// ============================================================================
// 5x7 BITMAP FONT — Full ASCII support
// ============================================================================

// INPUT ZONE boundaries for vision model OCR extraction (rows 450-479)
const INPUT_ZONE_TOP: u32 = 450u;
const INPUT_ZONE_BOTTOM: u32 = 479u;
const INPUT_ZONE_MARGIN: u32 = 10u;

// Helper functions for INPUT ZONE text processing and cursor rendering
fn get_input_length() -> u32 {
    // Length stored in first byte of input_buffer, clamped to max 192 chars
    return min(input_buffer[0] & 0xFFu, 192u);
}

fn cursor_blink_active() -> bool {
    // 20-frame blink cycle (333ms at 60fps) for reliable vision model detection
    // Ensures cursor visible in at least 2 frames per second during OCR capture
    return (config.frame % 20u) < 10u;
}

// Check if pixel position is an INPUT ZONE boundary marker (cyan lines for OCR alignment)
// Returns: 0 = not boundary, 1 = top boundary, 2 = bottom boundary, 3 = corner bracket
fn get_input_zone_boundary(row: u32, col: u32) -> u32 {
    // Early-out for rows completely outside boundary marker range (450-479)
    if (row < 450u || row > 479u) { return 0u; }

    let left_edge = INPUT_ZONE_MARGIN;
    let right_edge = config.width - INPUT_ZONE_MARGIN;

    // 2x4 corner brackets - check FIRST before horizontal boundaries for proper rendering
    let is_top_corner_row = row >= 450u && row <= 451u;
    let is_bottom_corner_row = row >= 478u && row <= 479u;
    if (is_top_corner_row || is_bottom_corner_row) {
        let is_left_corner = col >= left_edge && col < left_edge + 4u;
        let is_right_corner = col >= right_edge - 4u && col < right_edge;
        if (is_left_corner || is_right_corner) { return 3u; }
    }
    // 2-pixel thick top boundary (rows 450-451) for reliable OCR detection
    if ((row == 450u || row == 451u) && col >= left_edge && col < right_edge) {
        return 1u;
    }
    // 2-pixel thick bottom boundary (rows 478-479) for reliable OCR detection
    if ((row == 478u || row == 479u) && col >= left_edge && col < right_edge) {
        return 2u;
    }
    // 2-pixel thick vertical edge markers for TEXT AREA ONLY (rows 450-474)
    // Covers 3 text lines (24px = 3 lines x 8px) for consistent OCR boundary detection
    // PATCH_STATUS overlay at rows 475-479 rendered separately with priority blending
    if (row >= INPUT_ZONE_TOP && row < 475u) {
        let is_left_edge = col >= left_edge && col < left_edge + 2u;
        let is_right_edge = col >= right_edge - 2u && col < right_edge;
        if (is_left_edge || is_right_edge) { return 4u; }
    }
    return 0u;
}

// Font atlas dimensions: 95 printable ASCII chars * 5 columns = 475 bytes = 119 u32 words
const FONT_ATLAS_WORDS: u32 = 119u;
const FONT_CHAR_START: u32 = 32u;  // First printable ASCII
const FONT_CHAR_END: u32 = 126u;   // Last printable ASCII
const FONT_COLS_PER_CHAR: u32 = 5u;

fn get_font_column(char_code: u32, col: u32) -> u32 {
    // 5x7 bitmap font - returns column bits for given character and column (0-4)
    // Optimized: 4 ALU ops instead of 6, critical for 60 FPS with full INPUT ZONE rendering
    // Pre-clamped char ensures atomic memory patches don't cause OOB reads
    let safe_char = clamp(char_code, FONT_CHAR_START, FONT_CHAR_END);
    let safe_col = min(col, 4u);  // FONT_COLS_PER_CHAR - 1 = 4, literal is faster
    let bitmap_addr = (safe_char - 32u) * 5u + safe_col;  // 32 = FONT_CHAR_START literal
    let atlas_idx = bitmap_addr >> 2u;
    let byte_offset = (bitmap_addr & 3u) << 3u;  // byte position within u32 word
    
    // extractField is 1 ALU op vs 2 for shift+and, improves OCR frame consistency
    return extractBits(font_atlas[atlas_idx], byte_offset, 8u);
}

fn render_input_zone_text(row: u32, col: u32, width: u32) -> vec3<u32> {
    if (row < INPUT_ZONE_TOP || row >= 475u) { return vec3<u32>(0u, 0u, 0u); }

    let local_row = row - INPUT_ZONE_TOP;
    
    // Compact 3-line layout with 1px gaps: fits rows 452-474
    // Each line: 7px char height + 1px gap = 8px per line
    let line_height = 8u;
    let line = min(local_row / line_height, 2u);  // Clamp to 3 lines (0-2)
    let char_row = local_row % line_height;       // Row within character (0-7)
    
    // Skip gap rows (row 7 within each 8px line slot) and overflow past 3-line content
    // 3 lines × 8px = 24 rows (local_row 0-23); row 24 would wrap char_row incorrectly
    if (char_row >= 7u || local_row >= 24u) { return vec3<u32>(0u, 0u, 0u); }
    
    var line_offset: u32 = line * 32u;  // 32 chars per line for text wrapping

    let char_col = col / 6u;
    let pixel_col = col % 6u;

    // 1-pixel gap between chars improves OCR segmentation accuracy
    if (pixel_col >= 5u) { return vec3<u32>(0u, 0u, 0u); }

    let input_len = get_input_length();
    let adjusted_col = char_col + line_offset;

    // Merged cursor logic: only show cursor at end of input where no char exists
    if (adjusted_col == input_len) {
        if (cursor_blink_active() && pixel_col < 2u && char_row < 7u) {
            return vec3<u32>(200u, 255u, 200u);  // Green cursor at input end
        }
        return vec3<u32>(0u, 0u, 0u);
    }

    if (adjusted_col >= input_len || adjusted_col >= 192u) {
        return vec3<u32>(0u, 0u, 0u);
    }

    // Extract char from packed input_buffer (4 chars per u32, little-endian)
    // IMPORTANT: Text starts at byte 1 (length byte at position 0)
    let char_byte_pos = adjusted_col + 1u;  // Skip length byte for correct text offset
    let word_idx = char_byte_pos >> 2u;
    let byte_shift = (char_byte_pos & 3u) << 3u;
    let char_code = (input_buffer[word_idx] >> byte_shift) & 0xFFu;

    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - char_row;

    if (((font_bits >> bit_pos) & 1u) != 0u) {
        return vec3<u32>(255u, 255u, 255u);  // Pure white for 21:1 OCR contrast
    }
    return vec3<u32>(0u, 0u, 0u);  // Pure black for qwen3-vl-8b extraction
}

// Character code arrays for PATCH_STATUS display (WGSL-safe, no string type)
const PATCH_SUCCESS_CHARS: array<u32, 13> = array<u32, 13>(80u, 65u, 84u, 67u, 72u, 95u, 83u, 85u, 67u, 67u, 69u, 83u, 83u);
const PATCH_FAIL_CHARS: array<u32, 13> = array<u32, 13>(80u, 65u, 84u, 67u, 72u, 95u, 70u, 65u, 73u, 76u, 32u, 32u, 32u);

// PATCH_STATUS zone boundaries (rows 475-479)
const PATCH_STATUS_TOP: u32 = 475u;
const PATCH_STATUS_HEIGHT: u32 = 5u;

// Function to convert natural language commands to VM opcodes
fn natural_language_to_vm(command: array<u32, 64>) -> array<u32, 10> {
    // Placeholder for actual implementation
    return array<u32, 10>(0u);
}

// Function to render VM opcodes in the HUD
fn render_vm_opcodes(row: u32, col: u32, width: u32) -> vec3<u32> {
    if (row < PATCH_STATUS_TOP || row >= PATCH_STATUS_TOP + PATCH_STATUS_HEIGHT) { return vec3<u32>(0u, 0u, 0u); }

    let local_row = row - PATCH_STATUS_TOP;
    let opcode_col = col / 10u;
    let pixel_col = col % 10u;

    if (opcode_col >= 10u) { return vec3<u32>(0u, 0u, 0u); }

    // Extract opcode from packed buffer (10 opcodes per u32)
    let word_idx = opcode_col >> 2u;
    let byte_shift = (opcode_col & 3u) << 3u;
    let opcode_code = (vm_opcodes[word_idx] >> byte_shift) & 0xFFu;

    // Render opcode as text using the font atlas
    return render_font_text(row, col, opcode_code);
}

// Helper function to render text using the font atlas
fn render_font_text(row: u32, col: u32, char_code: u32) -> vec3<u32> {
    // Placeholder for actual implementation
    return vec3<u32>(0u, 0u, 0u);
}

// Main render function to handle all rendering
fn render(row: u32, col: u32, width: u32) -> vec3<u32> {
    let input_zone_text = render_input_zone_text(row, col, width);
    let vm_opcodes = natural_language_to_vm(input_buffer);
    let patch_status_text = render_patch_status(row, col, width);

    return input_zone_text + vm_opcodes + patch_status_text;
}

fn render_patch_status(row: u32, col: u32, width: u32) -> vec3<u32> {
    // PATCH_STATUS zone: rows 475-479 (5 rows for 5x7 font with 2px padding)
    if (row < PATCH_STATUS_TOP || row >= 480u) { return vec3<u32>(0u, 0u, 0u); }
    
    let status = patch_status[0];
    if (status == 0u) { return vec3<u32>(0u, 0u, 0u); }  // No patch pending
    
    let local_row = row - PATCH_STATUS_TOP;
    let char_row = local_row % 7u;
    
    // Center 13-char status text horizontally
    let text_len = 13u;
    let text_start = (width - text_len * 6u) / 2u;
    
    if (col < text_start || col >= text_start + text_len * 6u) { return vec3<u32>(0u, 0u, 0u); }
    
    let rel_col = col - text_start;
    let char_col = rel_col / 6u;
    let pixel_col = rel_col % 6u;
    
    // 1-pixel gap between chars
    if (pixel_col >= 5u || char_col >= text_len) { return vec3<u32>(0u, 0u, 0u); }
    
    // Select character array based on status (1=success, 2=fail)
    let char_code = select(
        select(32u, PATCH_FAIL_CHARS[char_col], status == 2u),
        PATCH_SUCCESS_CHARS[char_col],
        status == 1u
    );
    
    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - char_row;
    
    if (((font_bits >> bit_pos) & 1u) != 0u) {
        // Green for success (0, 255, 100), Red for fail (255, 80, 80)
        return select(vec3<u32>(255u, 80u, 80u), vec3<u32>(0u, 255u, 100u), status == 1u);
    }
    return vec3<u32>(0u, 0u, 0u);
} {
    // Only render in PATCH_STATUS zone (rows 475-479)
    if (row < PATCH_STATUS_TOP || row >= INPUT_ZONE_BOTTOM) {
        return vec3<u32>(0u, 0u, 0u);
    }
    
    let status = patch_status[0];
    if (status == 0u) {
        return vec3<u32>(0u, 0u, 0u);  // No patch pending
    }
    
    let local_row = row - PATCH_STATUS_TOP;  // 0-4 for 5-row zone
    
    // Center the 13-character status text
    let text_chars = 13u;
    let text_width = text_chars * 6u;  // 5px char + 1px gap
    let start_col = (width - text_width) / 2u;
    
    if (col < start_col) { return vec3<u32>(0u, 0u, 0u); }
    
    let rel_col = col - start_col;
    let char_idx = rel_col / 6u;
    let pixel_col = rel_col % 6u;
    
    // 1px gap between chars for OCR segmentation
    if (pixel_col >= 5u || char_idx >= text_chars) {
        return vec3<u32>(0u, 0u, 0u);
    }
    
    // Select character from status-specific array
    var char_code: u32 = 32u;  // Space default
    if (status == 1u) {
        char_code = PATCH_SUCCESS_CHARS[char_idx];
    } else if (status == 2u) {
        char_code = PATCH_FAIL_CHARS[char_idx];
    }
    
    let font_bits = get_font_column(char_code, pixel_col);
    // Map 5 display rows to top 5 bits of 7-bit font (bits 6,5,4,3,2)
    let bit_pos = 6u - local_row;
    
    if (((font_bits >> bit_pos) & 1u) != 0u) {
        if (status == 1u) {
            return vec3<u32>(50u, 255u, 100u);   // Bright green for SUCCESS
        }
        return vec3<u32>(255u, 80u, 80u);  // Bright red for FAIL
    }
    return vec3<u32>(0u, 0u, 0u)TATUS zone (rows 475-479)
    if (row < PATCH_STATUS_TOP || row >= 480u) { return vec3<u32>(0u, 0u, 0u); }
    
    // Check patch status: 0=none, 1=success, 2=fail
    let status = patch_status[0];
    if (status == 0u) { return vec3<u32>(0u, 0u, 0u); }
    
    let local_row = row - PATCH_STATUS_TOP;
    let status_chars = array<u32, 13>(80u, 65u, 84u, 67u, 72u, 95u, 83u, 85u, 67u, 67u, 69u, 83u, 83u);
    let fail_chars = array<u32, 13>(80u, 65u, 84u, 67u, 72u, 95u, 70u, 65u, 73u, 76u, 32u, 32u, 32u);
    
    // Center text horizontally: 13 chars * 6px = 78px
    let text_width = 78u;
    let start_col = (width - text_width) / 2u;
    let end_col = start_col + text_width;
    
    if (col < start_col || col >= end_col) { return vec3<u32>(0u, 0u, 0u); }
    
    let char_idx = (col - start_col) / 6u;
    let pixel_col = (col - start_col) % 6u;
    
    // 1px gap between chars
    if (pixel_col >= 5u) { return vec3<u32>(0u, 0u, 0u); }
    if (char_idx >= 13u) { return vec3<u32>(0u, 0u, 0u); }
    
    // Select character based on status
    let char_code = select(fail_chars[char_idx], status_chars[char_idx], status == 1u);
    let font_bits = get_font_column(char_code, pixel_col);
    
    // 5x7 font centered vertically in 5-row zone
    let font_v_start = (PATCH_STATUS_HEIGHT - 7u) / 2u;
    let char_row = i32(local_row) - i32(font_v_start);
    if (char_row < 0 || char_row >= 7) { return vec3<u32>(0u, 0u, 0u); }
    
    let bit_pos = 6u - u32(char_row);
    if (((font_bits >> bit_pos) & 1u) != 0u) {
        // Green for success, red for fail - high contrast for vision model
        return select(vec3<u32>(255u, 80u, 80u), vec3<u32>(80u, 255u, 80u), status == 1u);
    }
    return vec3<u32>(0u, 0u, 0u);
}

// Function to process input text and generate VM opcodes
fn process_input_text() -> array<u32, 192> {
    var opcodes: array<u32, 192> = array<u32, 192>(0u);
    let input_len = min(get_input_length(), 191u);

    // Extract text from input buffer
    var text: array<u8, 192> = array<u8, 192>(0u);
    for (var i: u32 = 0; i < input_len; i++) {
        let word_idx = i >> 2u;
        let byte_shift = (i & 3u) << 3u;
        text[i] = u8((input_buffer[word_idx] >> byte_shift) & 0xFFu);
    }

    // Process the extracted text and generate opcodes
    var opcode_index: u32 = 0;
    for (var i: u32 = 0; i < input_len; i++) {
        let char_code = text[i];
        if (char_code == 32u) { continue; } // Skip spaces

        // Example opcode generation logic (simplified)
        if (char_code == 43u) { // '+'
            opcodes[opcode_index] = 1u; // Assuming '1' is the opcode for addition
            opcode_index += 1u;
        } else if (char_code >= 48u && char_code <= 57u) { // Digits
            opcodes[opcode_index] = char_code - 48u; // Convert digit to number
            opcode_index += 1u;
        }
    }

    return opcodes;
}    
    // Copy input text to opcode buffer for host-side LLM processing
    // Host vision model extracts from INPUT ZONE, LLM generates opcodes
    for (var i: u32 = 0u; i < input_len; i++) {
        let word_idx = (i + 1u) >> 2u;  // Length in word 0, text starts at word 1
        let byte_shift = ((i + 1u) & 3u) << 3u;
        let char_code = (input_buffer[word_idx] >> byte_shift) & 0xFFu;
        opcodes[i] = char_code;
    }
    opcodes[input_len] = 0u;  // Null terminator for host string parsing
    return opcodes;t) & 0xFFu;
        // Convert text to VM opcodes using a language model
        // This is a placeholder for the actual LLM call
        let opcode = convert_text_to_opcode(char_code);
        opcodes[i] = opcode;
    }
    return opcodes;
}

// Function to atomically patch agent's memory with generated opcodes
fn atomic_patch_memory(opcodes: array<u32, 192>) {
    // This is a placeholder for the actual atomic memory patching code
    // It should use atomic operations to update the buffer_out array
}

// Main function to handle input processing and rendering
fn main() {
    let opcodes = process_input_text();
    atomic_patch_memory(opcodes);
    // Render text in INPUT ZONE
    for row in 450..=479 {
        for col in 0..width {
            if (get_input_zone_boundary(row, col) == 0u) {
                let color = render_input_zone_text(row, col, width);
                buffer_out[row * width + col] = Pixel { r: color.x, g: color.y, b: color.z, a: 255u };
            }
        }
    }
    // Render PATCH STATUS
    for row in 475..=479 {
        for col in 0..width {
            if (get_input_zone_boundary(row, col) == 0u) {
                let color = render_patch_status(row, col, width);
                buffer_out[row * width + col] = Pixel { r: color.x, g: color.y, b: color.z, a: 255u };
            }
        }
    }
}

// Placeholder function to convert text to VM opcodes using a language model
fn convert_text_to_opcode(char_code: u32) -> u32 {
    // This should be replaced with actual LLM call
    return 0u;
}

// PATCH_STATUS zone renderer (rows 475-479) - displays opcode generation results
fn render_patch_status(row: u32, col: u32, width: u32) -> vec3<u32> {
    if (row < 475u || row >= 480u) { return vec3<u32>(0u, 0u, 0u); }
    
    let status = patch_status[0u];
    if (status == 0u) { return vec3<u32>(0u, 0u, 0u); }  // No patch pending
    
    let char_idx = col / 6u;
    let pixel_col = col % 6u;
    
    if (pixel_col >= 5u || char_idx >= 13u) { return vec3<u32>(0u, 0u, 0u); }
    
    // Select character from precomputed arrays (WGSL-safe)
    let char_code = select(PATCH_FAIL_CHARS[char_idx], PATCH_SUCCESS_CHARS[char_idx], status == 1u);
    let local_row = row - 475u;
    let font_bits = get_font_column(char_code, pixel_col);
    
    if (((font_bits >> (6u - local_row)) & 1u) != 0u) {
        // Green for success, red for fail
        if (status == 1u) { return vec3<u32>(100u, 255u, 100u); }
        return vec3<u32>(255u, 100u, 100u);
    }
    return vec3<u32>(0u, 0u, 0u);
}

// 5x7 bitmap font atlas storage - 475 bytes packed in 119 words (95 chars × 5 cols)
// Declared at module scope for WGSL compliance - used by get_font_column() above
@group(0) @binding(9) var<storage, read> font_atlas: array<u32>;

// Get current input length from buffer (stored in high byte of word 47)
fn get_input_length() -> u32 {
    return input_buffer[47u] >> 24u;
}

// Cursor blink state - 32-frame cycle (~533ms at 60fps) for visual feedback
// Blink ON for frames 0-15, OFF for frames 16-31 (50% duty cycle)
fn cursor_blink_active() -> bool {
    return (config.frame & 16u) == 0u;
} & 16u) != 0u;
}

// Input validation helper - returns true if input buffer has valid natural language command
// Host system reads input_buffer directly for vision-to-opcode pipeline
fn has_valid_input() -> bool {
    let input_len = get_input_length();
    return input_len > 0u && input_len <= 192u;
}n <= 191u;
}

// Get input character count for host-side OCR extraction
fn get_input_length() -> u32 {
    return min(input_buffer[47u] >> 24u, 191u);
}

// Check if cursor should blink (32-frame cycle for 500ms at 60fps)
fn cursor_blink_active() -> bool {
    return (config.frame & 31u) < 16u;
}

// Host-side pipeline: vision model reads input_buffer[0-46] directly from GPU memory
// qwen3-vl-8b extracts text -> tinyllama generates opcodes -> host patches agent memory
// Shader only renders; no LLM/vision logic in WGSL (impossible in GPU shader context)

    let local_row = row - INPUT_ZONE_TOP;
    
    // Multi-line layout: lines at rows 2-8, 11-17, 20-26 (7px font + 3px spacing)
    // Each line can display ~106 chars at 6px/char on 640px width
    var char_row: u32 = 255u;  // Sentinel for invalid
    var line_offset: u32 = 0u;
    
    if (local_row >= 2u && local_row < 9u) {
        char_row = local_row - 2u;
        line_offset = 0u;
    } else if (local_row >= 11u && local_row < 18u) {
        char_row = local_row - 11u;
        line_offset = 64u;
    } else if (local_row >= 20u && local_row < 27u) {
        char_row = local_row - 20u;
        line_offset = 128u;
    } else {
        return vec3<u32>(0u, 0u, 0u);  // OCR: pure black gap between lines
    }

    let char_col = col / 6u;
    let pixel_col = col % 6u;

    if (pixel_col >= 5u) { return vec3<u32>(0u, 0u, 0u); }  // OCR: pure black gap

    let input_len = get_input_length();

    // Blinking cursor at end of input (32-frame cycle for 500ms at 60fps)
    if (char_col == input_len && cursor_blink_active()) {
        if (pixel_col < 2u && char_row < 7u) {
            return vec3<u32>(200u, 255u, 200u);  // Green cursor
        }
        return vec3<u32>(0u, 0u, 0u);  // OCR: pure black cursor off
    }

    if (char_col >= input_len || char_col >= 64u) {
        return vec3<u32>(0u, 0u, 0u);  // OCR: pure black empty
    }

    // Extract char from packed input_buffer (4 chars per u32, little-endian)
    let word_idx = char_col >> 2u;
    let byte_shift = (char_col & 3u) << 3u;
    let char_code = (input_buffer[word_idx] >> byte_shift) & 0xFFu;

    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - char_row;

    if (((font_bits >> bit_pos) & 1u) != 0u) {
        return vec3<u32>(255u, 255u, 255u);  // Pure white for max OCR contrast (21:1)
    }
    return vec3<u32>(0u, 0u, 0u);  // Pure black for optimal qwen3-vl-8b extraction
}

// Render PATCH STATUS zone (rows 475-479) - displays opcode translation results
// Shows PATCH_SUCCESS or PATCH_FAIL from host vision-to-opcode pipeline
fn render_patch_status(row: u32, col: u32, width: u32) -> vec3<u32> {
    if (row < 475u || row >= 480u) { return vec3<u32>(0u, 0u, 0u); }
    
    let status = patch_status[0u];
    if (status == 0u) { return vec3<u32>(0u, 0u, 0u); }  // No patch pending
    
    // Display status indicator at left margin
    if (col >= 10u && col < 20u) {
        let local_row = row - 475u;
        if (status == 1u && local_row < 5u) {
            return vec3<u32>(0u, 200u, 0u);  // Green = PATCH_SUCCESS
        } else if (status == 2u && local_row < 5u) {
            return vec3<u32>(200u, 0u, 0u);  // Red = PATCH_FAIL
        }
    }
    return vec3<u32>(0u, 0u, 0u);
}

// Extract char pixel_col);
    let bit_pos = 6u - char_row;

    if (((font_bits >> bit_pos) & 1u) != 0u) {
        return vec3<u32>(255u, 255u, 255u);  // Pure white for max OCR contrast (21:1)
    }
    return vec3<u32>(0u, 0u, 0u);  // Pure black for optimal qwen3-vl-8b extraction
}

// PATCH_STATUS display zone (rows 475-479) - shows atomic patch results
fn render_patch_status(row: u32, col: u32, width: u32) -> vec3<u32> {
    if (row < 475u || row >= 480u) { return vec3<u32>(0u, 0u, 0u); }

    let status = patch_status[0u];
    let local_row = row - 475u;
    let char_col = col / 6u;
    let pixel_col = col % 6u;

    if (pixel_col >= 5u || char_col >= 16u) { return vec3<u32>(10u, 15u, 25u); }

    // Status: 0=none (gray), 1=success (green), 2=fail (red)
    var text_color = vec3<u32>(128u, 128u, 128u);
    var char_code: u32 = 32u;  // Default space

    // Status messages: "N/A" (none), "PATCH_SUCCESS" (success), "PAT # First 8k chars for context
    if (row < INPUT_ZONE_TOP || row >= 475u) { return vec3<u32>(0u, 0u, 0u); }
    
    // Center 7-pixel text vertically in 25-row INPUT ZONE: padding = (25-7)/2 = 9
    let local_row = row - INPUT_ZONE_TOP;
    if (local_row < 9u || local_row >= 16u) { return vec3<u32>(0u, 0u, 0u); }  // OCR: pure black bg
    let char_row = local_row - 9u;  // Maps to 0-6 for valid font bitmap indexing
    
    let char_col = col / 6u;
    let pixel_col = col % 6u;
    
    if (pixel_col >= 5u) { return vec3<u32>(0u, 0u, 0u); }  // OCR: pure black gap
    
    let input_len = get_input_length();
    
    // Blinking cursor at end of input (32-frame cycle for 500ms at 60fps)
    if (char_col == input_len && cursor_blink_active()) {
        if (pixel_col < 2u && char_row < 7u) {
            return vec3<u32>(200u, 255u, 200u);  // Green cursor
        }
        return vec3<u32>(0u, 0u, 0u);  // OCR: pure black cursor off
    }
    
    if (char_col >= input_len || char_col >= 64u) {
        return vec3<u32>(0u, 0u, 0u);  // OCR: pure black empty
    }
    
    // Extract char from packed input_buffer (4 chars per u32, little-endian)
    let word_idx = char_col >> 2u;
    let byte_shift = (char_col & 3u) << 3u;
    let char_code = (input_buffer[word_idx] >> byte_shift) & 0xFFu;
    
    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - char_row;

    if (((font_bits >> bit_pos) & 1u) != 0u) {
        return vec3<u32>(255u, 255u, 255u);  // Pure white for max OCR contrast (21:1)
    }
    return vec3<u32>(0u, 0u, 0u);  // Pure black for optimal qwen3-vl-8b extraction
}

// PATCH_STATUS display zone (rows 475-479) - shows atomic patch results
fn render_patch_status(row: u32, col: u32, width: u32) -> vec3<u32> {
    if (row < 475u || row >= 480u) { return vec3<u32>(0u, 0u, 0u); }

    let status = patch_status[0u];
    let local_row = row - 475u;
    let char_col = col / 6u;
    let pixel_col = col % 6u;

    if (pixel_col >= 5u || char_col >= 16u) { return vec3<u32>(10u, 15u, 25u); }

    // Status: 0=none (gray), 1=success (green), 2=fail (red)
    var text_color = vec3<u32>(128u, 128u, 128u);
    var char_code: u32 = 32u;  // Default space

    // Status messages: "N/A" (none), "PATCH_SUCCESS" (success), "PATCH_FAILED" (fail)
    // Char codes for status text
    let none_msg = array<u32, 16u>(78u, 47u, 65u, 32u, 32u, 32u, 32u, 32u, 32u, 32u, 32u, 32u, 32u, 32u, 32u, 32u);
    let success_msg = array<u32, 16u>(80u, 65u, 84u, 67u, 72u, 95u, 83u, 85u, 67u, 67u, 69u, 83u, 83u, 32u, 32u, 32u);
    let fail_msg = array<u32, 16u>(80u, 65u, 84u, 67u, 72u, 95u, 70u, 65u, 73u, 76u, 69u, 68u, 32u, 32u, 32u, 32u);
    
    // Select message and color based on status
    if (status == 1u) {
        text_color = vec3<u32>(80u, 255u, 80u);  // Green for success
        char_code = success_msg[char_col];
    } else if (status == 2u) {
        text_color = vec3<u32>(255u, 80u, 80u);  // Red for fail
        char_code = fail_msg[char_col];
    } else {
        char_code = none_msg[char_col];
    }
    
    // Render character using 5x7 font
    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - local_row;
    if (((font_bits >> bit_pos) & 1u) != 0u) {
        return text_color;
    }
    return vec3<u32>(10u, 15u, 25u); 76u, 69u, 68u, 32u, 32u, 32u, 32u);

    if (status == 1u) {
        text_color = vec3<u32>(0u, 255u, 100u);
        char_code = success_msg[char_col];
    } else if (status == 2u) {
        text_color = vec3<u32>(255u, 0u, 0u);
        char_code = fail_msg[char_col];
    }

    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - local_row;

    if (((font_bits >> bit_pos) & 1u) != 0u) {
        return vec3<u32>(text_color.r, text_color.g, text_color.b);  // Colored text for status display
    }
    return vec3<u32>(10u, 15u, 25u);  // Dark background
}

    if (row < 475u || row >= 480u) { return vec3<u32>(0u, 0u, 0u); }
    
    let status = patch_status[0u];
    let local_row = row - 475u;
    let char_col = col / 6u;
    let pixel_col = col % 6u;
    
    if (pixel_col >= 5u || char_col >= 16u) { return vec3<u32>(10u, 15u, 25u); }
    
    // Status: 0=none (gray), 1=success (green), 2=fail (red)
    var text_color = vec3<u32>(128u, 128u, 128u);
    var char_code: u32 = 32u;  // Default space
    
    // Status messages: "N/A" (none), "PATCH_SUCCESS" (success), "PATCH_FAILED" (fail)
    // Char codes for status text
    let none_msg = array<u32, 16u>(78u, 47u, 65u, 32u, 32u, 32u, 32u, 32u, 32u, 32u, 32u, 32u, 32u, 32u, 32u, 32u);
    let success_msg = array<u32, 16u>(80u, 65u, 84u, 67u, 72u, 95u, 83u, 85u, 67u, 67u, 69u, 83u, 83u, 32u, 32u, 32u);
    let fail_msg = array<u32, 16u>(80u, 65u, 84u, 67u, 72u, 95u, 70u, 65u, 73u, 76u, 69u, 68u, 32u, 32u, 32u, 32u);
    
    if (status == 1u) {
        text_color = vec3<u32>(0u, 255u, 100u);
        char_code = success_msg[char_col];
    } else if (status == 2u) {
        text_color = vec3<u32>(255u, 80u, 80u);
        char_code = fail_msg[char_col];
    } else {
        char_code = none_msg[char_col];
    }
    
    // Render character using 5x7 font
    if (char_code == 32u || local_row >= 7u) { return vec3<u32>(10u, 15u, 25u); }
    
    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - local_row;
    
    if (((font_bits >> bit_pos) & 1u) != 0u) {
        return text_color;
    }
    return vec3<u32>(10u, 15u, 25u);
}
];
    if (char_code == 0u || char_code == 32u) { return vec3<u32>(10u, 15u, 25u); }
    
    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - local_row;
    
    if (((font_bits >> bit_pos) & 1u) != 0u) {
        return text_color;
    }
    return vec3<u32>(10u, 15u, 25u);
}0u);  // Red for fail
        msg = array<u32, 16u>(
            80u, 65u, 84u, 67u, 72u, 95u, 70u, 65u, 73u, 76u, 32u, 32u, 32u, 32u, 32u, 32u  // "PATCH_FAIL"
        );
    }
    
    let char_code = msg[char_col];
    if (char_code == 0u || local_row >= 7u) {
        return vec3<u32>(10u, 15u, 25u);  // Dark background for spaces
    }
    
    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - local_row;
    
    if (((font_bits >> bit_pos) & 1u) != 0u) {
        return text_color;
    }
    return vec3<u32>(10u, 15u, 25u);  // Dark background
}eturn vec3<u32>(15u, 20u, 30u);  // Dark blue-gray background
    }
    
    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - local_row;
    
    if (((font_bits >> bit_pos) & 1u) != 0u) {
        return text_color;
    }
    return vec3<u32>(15u, 20u, 30u);c3<u32>(0u, 0u, 0u); }
    
    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - local_row;
    
    if (((font_bits >> bit_pos) & 1u) != 0u) {
        return text_color;
    }
    return vec3<u32>(0u, 0u, 0u);
}c3<u32>(0u, 0u, 0u); }
    
    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - local_row;
    
    if (((font_bits >> bit_pos) & 1u) != 0u) {
        return text_color;
    }
    return vec3<u32>(0u, 0u, 0u);
}c3<u32>(0u, 0u, 0u); }
    
    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - local_row;
    
    if (((font_bits >> bit_pos) & 1u) != 0u) {
        return text_color;
    }
    return vec3<u32>(0u, 0u, 0u);
}c3<u32>(0u, 0u, 0u); }
    
    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - local_row;
    
    if (((font_bits >> bit_pos) & 1u) != 0u) {
        return text_color;
    }
    return vec3<u32>(0u, 0u, 0u);
}c3<u32>(0u, 0u, 0u); }
    
    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - local_row;
    
    if (((font_bits >> bit_pos) & 1u) != 0u) {
        return text_color;
    }
    return vec3<u32>(0u, 0u, 0u);
}c3<u32>(0u, 0u, 0u); }
    
    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - local_row;
    
    if (((font_bits >> bit_pos) & 1u) != 0u) {
        return text_color;
    }
    return vec3<u32>(0u, 0u, 0u);
}c3<u32>(0u, 0u, 0u); }
    
    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - local_row;
    
    if (((font_bits >> bit_pos) & 1u) != 0u) {
        return text_color;
    }
    return vec3<u32>(0u, 0u, 0u);
} - INPUT_ZONE_MARGIN) {
            return vec3<u32>(10u, 15u, 25u);
        }
        return vec3<u32>(0u, 0u, 0u);
    }

    // Calculate char index: line 0 = chars 0-63, line 1 = 64-127, line 2 = 128-191
    let global_char_idx = line_index * 64u + char_col;
    let word_idx = global_char_idx / 4u;
    let byte_idx = global_char_idx % 4u;

    if (word_idx >= 48u) { return vec3<u32>(10u, 15u, 25u); }

    let packed = input_buffer[word_idx];
    let char_code = (packed >> (byte_idx * 8u)) & 0xFFu;

    // Blinking cursor at end of input (32-frame cycle = ~500ms at 60fps)
    let input_len = input_buffer[15u] >> 24u;
    if (global_char_idx == input_len && (config.frame & 32u) != 0u) {
        if (pixel_col < 2u && char_row >= 1u && char_row <= 5u) {
            return vec3<u32>(255u, 255u, 0u);  // Yellow cursor
        }
    }

    // Dark background for null chars or invalid rows
    if (char_code == 0u || char_row >= 7u) {
        if (col >= INPUT_ZONE_MARGIN && col < width - INPUT_ZONE_MARGIN) {
            return vec3<u32>(10u, 15u, 25u);
        }
        return vec3<u32>(0u, 0u, 0u);
    }

    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - char_row;

    if (((font_bits >> bit_pos) & 1u) != 0u) {
        return vec3<u32>(0u, 255u, 255u);  // Cyan text for OCR contrast
    }
    return vec3<u32>(10u, 15u, 25u);  // Dark background
}

// Render patch status in rows 475-479 (success=green, fail=red)
// Host patches memory atomically and sets patch_status[0]
fn render_patch_status(row: u32, col: u32, width: u32) -> vec3<u32> {
    if (row < 475u || row >= 480u) { return vec3<u32>(0u, 0u, 0u); }
    
    let status = patch_status[0u];
    let local_row = row - 475u;
    let char_col = col / 6u;
    let pixel_col = col % 6u;
    
    if (pixel_col >= 5u) { return vec3<u32>(0u, 0u, 0u); }
    
    // Status messages: "PATCH_SUCCESS" or "PATCH_FAILED"
    var char_code: u32 = 0u;
    if (status == 1u) {
        // Green PATCH_SUCCESS
        let success_msg = array<u32, 13>(80u, 65u, 84u, 67u, 72u, 95u, 83u, 85u, 67u, 67u, 69u, 83u, 83u);
        if (char_col < 13u) { char_code = success_msg[char_col]; }
    } else if (status == 2u) {
        // Red PATCH_FAILED
        let fail_msg = array<u32, 12>(80u, 65u, 84u, 67u, 72u, 95u, 70u, 65u, 73u, 76u, 69u, 68u);
        if (char_col < 12u) { char_code = fail_msg[char_col]; }
    }
    
    if (char_code == 0u) { return vec3<u32>(0u, 0u, 0u); }
    
    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - local_row;
    
    if (((font_bits >> bit_pos) & 1u) != 0u) {
        if (status == 1u) { return vec3<u32>(0u, 255u, 0u); }   // Green success
        if (status == 2u) { return vec3<u32>(255u, 0u, 0u); }   // Red failure
    }
    return vec3<u32>(0u, 0u, 0u);  // Transparent background
}<u32>(255u, 0u, 0u); }   // Red fail
    }
    return vec3<u32>(0u, 0u, 0u);
}

// Main render dispatch for HUD zones
fn render_hud_pixel(row: u32, col: u32, width: u32) -> vec3<u32> {
    // INPUT ZONE (rows 450-479): Natural language input + patch status
    if (row >= 448u && row <= 481u) {
        let input_color = render_input_zone_text(row, col, width);
        if (input_color.r != 0u || input_color.g != 0u || input_color.b != 0u) {
            return input_color;
        }
        let status_color = render_patch_status(row, col, width);
        if (status_color.r != 0u || status_color.g != 0u || status_color.b != 0u) {
            return status_color;
        }
    }
    return vec3<u32>(0u, 0u, 0u);
}<u32>(255u, 0u, 0u); }   // Red failure
    }
    return vec3<u32>(0u, 0u, 0u);  // Black background for off-pixels
}<u32>(255u, 0u, 0u); }   // Red fail
    }
    return vec3<u32>(0u, 0u, 0u);
}

// Render INPUT ZONE label at row 447 for OCR targeting
fn render_input_label(row: u32, col: u32, width: u32) -> vec3<u32> {
    if (row != 447u) { return vec3<u32>(0u, 0u, 0u); }
    
    let label = array<u32, 11>(73u, 78u, 80u, 85u, 84u, 58u, 32u, 62u, 62u, 62u, 32u); // "INPUT: >>> "
    let char_col = col / 6u;
    let pixel_col = col % 6u;
    
    if (char_col >= 11u || pixel_col >= 5u) { return vec3<u32>(0u, 0u, 0u); }
    
    let char_code = label[char_col];
    let font_bits = get_font_column(char_code, pixel_col);
    
    if (((font_bits >> 6u) & 1u) != 0u) {
        return vec3<u32>(255u, 200u, 0u);  // Orange label for visibility
    }
    return vec3<u32>(0u, 0u, 0u);<u32>(255u, 0u, 0u); }   // Red fail
    }
    return vec3<u32>(0u, 0u, 0u);
}<u32>(255u, 0u, 0u); }   // Red failure
    }
    return vec3<u32>(5u, 8u, 15u);  // Dark blue background for status area<u32>(255u, 0u, 0u); }   // Red failure
    }
    return vec3<u32>(0u, 0u, 0u);  // Background
}<u32>(255u, 0u, 0u); }   // Red failure
    }
    return vec3<u32>(0u, 0u, 0u);
}

// Main HUD composition function
fn compose_hud_pixel(row: u32, col: u32, width: u32) -> vec3<u32> {
    // Try input zone rendering first
    let input_color = render_input_zone_text(row, col, width);
    if (input_color.r != 0u || input_color.g != 0u || input_color.b != 0u) {
        return input_color;
    }
    
    // Try patch status rendering
    let status_color = render_patch_status(row, col, width);
    if (status_color.r != 0u || status_color.g != 0u || status_color.b != 0u) {
        return status_color;
    }
    
    return vec3<u32>(0u, 0u, 0u);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= config.width * config.height) { return; }
    
    let col = idx % config.width;
    let row = idx / config.width;
    
    // Agent isolation: rows 0-399 are agent execution space
    if (row < 400u) { return; }
    
    // Compose HUD for rows 400+
    let hud_color = compose_hud_pixel(row, col, config.width);
    buffer_out[idx] = Pixel(hud_color.r, hud_color.g, hud_color.b, 255u);
}<u32>(10u, 15u, 25u); }
    
    let packed = input_buffer[word_idx];
    let char_code = (packed >> (byte_idx * 8u)) & 0xFFu;
    
    // Blinking cursor at end of input (32-frame cycle = ~500ms at 60fps)
    let input_len = input_buffer[15u] >> 24u;
    if (global_char_idx == input_len && (config.frame & 32u) != 0u) {
        if (pixel_col < 2u && char_row >= 1u && char_row <= 5u) {
            return vec3<u32>(255u, 255u, 0u);  // Yellow cursor
        }
    }
    
    // Dark background for null chars or invalid rows
    if (char_code == 0u || char_row >= 7u) {
        if (col >= INPUT_ZONE_MARGIN && col < width - INPUT_ZONE_MARGIN) {
            return vec3<u32>(10u, 15u, 25u);
        }
        return vec3<u32>(0u, 0u, 0u);
    }
    
    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - char_row;
    
    if (((font_bits >> bit_pos) & 1u) != 0u) {
        return vec3<u32>(0u, 255u, 255u);  // Cyan text for OCR contrast
    }
    return vec3<u32>(10u, 15u, 25u);  // Dark background
}

// Render PATCH_SUCCESS/FAIL status in rows 475-479
// Green = success (patch_status[0] == 1), Red = fail (patch_status[0] == 2)
fn render_patch_status(row: u32, col: u32, width: u32) -> vec3<u32> {
    if (row < 475u || row >= INPUT_ZONE_BOTTOM) { return vec3<u32>(0u, 0u, 0u); }
    
    let status = patch_status[0u];
    if (status == 0u) { return vec3<u32>(0u, 0u, 0u); }  // No status
    
    // Status text rendering area
    let local_row = row - 475u;
    let char_col = col / 6u;
    let pixel_col = col % 6u;
    if (pixel_col >= 5u || local_row >= 7u) { return vec3<u32>(0u, 0u, 0u); }
    
    // "PATCH_SUCCESS" or "PATCH_FAIL" based on status
    var char_code: u32 = 0u;
    let status_text_success = array<u32, 13>(80u, 65u, 84u, 67u, 72u, 95u, 83u, 85u, 67u, 67u, 69u, 83u, 83u);
    let status_text_fail = array<u32, 10>(80u, 65u, 84u, 67u, 72u, 95u, 70u, 65u, 73u, 76u);
    
    if (status == 1u && char_col < 13u) {
        char_code = status_text_success[char_col];
    } else if (status == 2u && char_col < 10u) {
        char_code = status_text_fail[char_col];
    }
    
    if (char_code == 0u) { return vec3<u32>(0u, 0u, 0u); }
    
    let font_bits = get_font_column(char_code, pixel_col);
    let bit_pos = 6u - local_row;
    
    if (((font_bits >> bit_pos) & 1u) != 0u) {
        // Green for success, Red for fail
        if (status == 1u) { return vec3<u32>(0u, 255u, 0u); }
        if (status == 2u) { return vec3<u32>(255u, 0u, 0u); }
    }
    return vec3<u32>(0u, 0u, 0u);
}

// 5x7 bitmap font - each char is 5 columns x 7 rows return 0u; }
    
    // Space character (char code 32) - essential for natural language word separation
    if (char_code == 32u) { return 0u; }
    
    // Normalize lowercase a-z (97-122) to uppercase A-Z (65-90) for natural language support
    // This allows 'add 5 and 3' to render correctly using existing uppercase bitmaps
    var c = char_code;
    if (c >= 97u && c <= 122u) { c -= 32u; }
    
    // Digits 0-9 (char codes 48-57) - all use normalized 'c' for consistency
    if (c == 48u) {  // '0'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x51u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x45u; }
        if (col == 4u) { return 0x3Eu; }
    } else if (c == 49u) {  // '1'
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x22u; }
        if (col == 2u) { return 0x7Fu; }
        if (col == 3u) { return 0x01u; }
        if (col == 4u) { return 0x00u; }
    } else if (c == 50u) {  // '2'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x05u; }
        if (col == 3u) { return 0x09u; }
        if (col == 4u) { return 0x7Fu; }
    } else if (c == 51u) {  // '3'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x1Du; }
        if (col == 3u) { return 0x41u; }
        if (col == 4u) { return 0x3Eu; }
    } else if (c == 52u) {  // '4'
        if (col == 0u) { return 0x04u; }
        if (col == 1u) { return 0x0Cu; }
        if (col == 2u) { return 0x14u; }
        if (col == 3u) { return 0x7Fu; }
        if (col == 4u) { return 0x04u; }
    } else if (c == 53u) {  // '5'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x31u; }
    } else if (c == 54u) {  // '6'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x32u; }
    } else if (c == 55u) {  // '7'
        if (col == 0u) { return 0x40u; }
        if (col == 1u) { return 0x47u; }
        if (col == 2u) { return 0x48u; }
        if (col == 3u) { return 0x50u; }
        if (col == 4u) { return 0x60u; }
    } else if (c == 56u) {  // '8'
        if (col == 0u) { return 0x36u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x36u; }
    } else if (c == 57u) {  // '9'
        if (col == 0u) { return 0x26u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x3Eu; }
    } else if (c == 65u0u) { return 0x42u; }
        if (col == 1u) { return 0x7Fu; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x40u; }
    } else if (c == 50u) {  // '2'
        if (col == 0u) { return 0x62u; }
        if (col == 1u) { return 0x51u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x46u; }
    } else if (c == 51u) {  // '3'
        if (col == 0u) { return 0x22u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x36u; }
    } else if (c == 52u) {  // '4'
        if (col == 0u) { return 0x18u; }
        if (col == 1u) { return 0x14u; }
        if (col == 2u) { return 0x12u; }
        if (col == 3u) { return 0x7Fu; }
        if (col == 4u) { return 0x10u; }
    } else if (c == 53u) {  // '5'
        if (col == 0u) { return 0x3Fu; }
        if (col == 1u) { return 0x45u; }
        if (col == 2u) { return 0x45u; }
        if (col == 3u) { return 0x45u; }
        if (col == 4u) { return 0x21u; }
    } else if (c == 54u) {  // '6'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x32u; }
    } else if (c == 55u) {  // '7'
        if (col == 0u) { return 0x01u; }
        if (col == 1u) { return 0x71u; }
        if (col == 2u) { return 0x09u; }
        if (col == 3u) { return 0x05u; }
        if (col == 4u) { return 0x03u; }
    } else if (c == 56u) {  // '8'
        if (col == 0u) { return 0x36u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x36u; }
    } else if (c == 57u) {  // '9'
        if (col == 0u) { return 0x26u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x3Eu; }
        if (col == 3u) { return 0x09u; }
        if (col == 4u) { return 0x07u; }
    } else if (c == 56u) {  // '8'
        if (col == 0u) { return 0x36u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x36u; }
    } else if (char_code == 57u) {  // '9'
        if (col == 0u) { return 0x26u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x3Eu; }
    }
    // Letters A-Z (char codes 65-90) - uses normalized 'c' for lowercase support
    else if (c == 65u) {  // 'A'
        if (col == 0u) { return 0x7Eu; }
        if (col == 1u) { return 0x11u; }
        if (col == 2u) { return 0x11u; }
        if (col == 3u) { return 0x11u; }
        if (col == 4u) { return 0x7Eu; }
    } else if (c == 66u) {  // 'B'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x36u; }
    } else if (c == 67u) {  // 'C' (normalized for lowercase support)
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x41u; }
        if (col == 4u) { return 0x22u; }
    } else if (c == 68u) {  // 'D' (normalized for lowercase support)
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x22u; }
        if (col == 4u) { return 0x1Cu; }
    } else if (c == 69u) {  // 'E' (normalized for lowercase support)
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x41u; }
    } else if (c == 70u) {  // 'F' (normalized for lowercase support)
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x09u; }
        if (col == 2u) { return 0x09u; }
        if (col == 3u) { return 0x09u; }
        if (col == 4u) { return 0x01u; }
    } else if (c == 71u) {  // 'G' (normalized for lowercase support)
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x7Au; }
    } else if (c == 72u) {  // 'H' (normalized for lowercase support)
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x7Fu; }
    } else if (c == 73u) {  // 'I' (normalized for lowercase support)
        if (col == 0u) { return 0x41u; }
        if (col == 1u) { return 0x7Fu; }
        if (col == 2u) { return 0x41u; }
        return 0u;
    } else if (c == 74u) {  // 'J' (normalized for lowercase support)
        if (col == 0u) { return 0x20u; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x3Fu; }
        if (col == 4u) { return 0x01u; }
    } else if (c == 75u) {  // 'K' (normalized for lowercase support)
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x14u; }
        if (col == 3u) { return 0x22u; }
        if (col == 4u) { return 0x41u; }
    } else if (c == 76u) {  // 'L' (normalized for lowercase support)
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x40u; }
    } else if (c == 77u) {  // 'M' (normalized for lowercase support)
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x02u; }
        if (col == 2u) { return 0x0Cu; }
        if (col == 3u) { return 0x02u; }
        if (col == 4u) { return 0x7Fu; }
    } else if (char_code == 78u) {  // 'N'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x04u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x10u; }
        if (col == 4u) { return 0x7Fu; }
    } else if (char_code == 79u) {  // 'O'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x41u; }
        if (col == 4u) { return 0x3Eu; }
    } else if (char_code == 80u) {  // 'P'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x09u; }
        if (col == 2u) { return 0x09u; }
        if (col == 3u) { return 0x09u; }
        if (col == 4u) { return 0x06u; }
    } else if (char_code == 81u) {  // 'Q'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x51u; }
        if (col == 3u) { return 0x41u; }
        if (col == 4u) { return 0x3Eu; }
    } else if (char_code == 82u) {  // 'R'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x09u; }
        if (col == 2u) { return 0x19u; }
        if (col == 3u) { return 0x29u; }
        if (col == 4u) { return 0x46u; }
    } else if (char_code == 83u) {  // 'S'
        if (col == 0u) { return 0x26u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x32u; }
    } else if (char_code == 84u) {  // 'T'
        if (col == 0u) { return 0x01u; }
        if (col == 1u) { return 0x01u; }
        if (col == 2u) { return 0x7Fu; }
        if (col == 3u) { return 0x01u; }
        if (col == 4u) { return 0x01u; }
    } else if (char_code == 85u) {  // 'U'
        if (col == 0u) { return 0x3Fu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x3Fu; }
    } else if (char_code == 86u) {  // 'V'
        if (col == 0u) { return 0x07u; }
        if (col == 1u) { return 0x18u; }
        if (col == 2u) { return 0x60u; }
        if (col == 3u) { return 0x18u; }
        if (col == 4u) { return 0x07u; }
    } else if (char_code == 87u) {  // 'W'
        if (col == 0u) { return 0x3Fu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x38u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x3Fu; }
    } else if (char_code == 88u) {  // 'X'
        if (col == 0u) { return 0x63u; }
        if (col == 1u) { return 0x14u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x14u; }
        if (col == 4u) { return 0x63u; }
    } else if (char_code == 89u) {  // 'Y'
        if (col == 0u) { return 0x07u; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x70u; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x07u; }
    } else if (char_code == 90u) {  // 'Z'
        if (col == 0u) { return 0x61u; }
        if (col == 1u) { return 0x51u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x45u; }
        if (col == 4u) { return 0x43u; }
    }
    // Operators and symbols for VM opcodes
    else if (char_code == 32u) {  // ' ' (space)
        return 0u;
    } else if (char_code == 43u) {  // '+'
        if (col == 0u) { return 0x08u; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x3Eu; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x08u; }
    } else if (char_code == 45u) {  // '-'
        if (col == 0u) { return 0x08u; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x08u; }
    } else if (char_code == 42u) {  // '*'
        if (col == 0u) { return 0x14u; }
        if (col == 1u) { return 0x22u; }
        if (col == 2u) { return 0x7Fu; }
        if (col == 3u) { return 0x22u; }
        if (col == 4u) { return 0x14u; }
    } else if (char_code == 47u) {  // '/'
        if (col == 0u) { return 0x20u; }
        if (col == 1u) { return 0x10u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x04u; }
        if (col == 4u) { return 0x02u; }
    } else if (char_code == 61u) {  // '='
        if (col == 0u) { return 0x14u; }
        if (col == 1u) { return 0x14u; }
        if (col == 2u) { return 0x14u; }
        if (col == 3u) { return 0x14u; }
        if (col == 4u) { return 0x14u; }
    } else if (char_code == 64u) {  // '@' (execute)
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x55u; }
        if (col == 2u) { return 0x5Du; }
        if (col == 3u) { return 0x55u; }
        if (col == 4u) { return 0x1Cu; }
    }
    // Lowercase a-z for natural language input
    else if (char_code == 97u) {  // 'a'
        if (col == 0u) { return 0x20u; }
        if (col == 1u) { return 0x54u; }
        if (col == 2u) { return 0x54u; }
        if (col == 3u) { return 0x54u; }
        if (col == 4u) { return 0x78u; }
    } else if (char_code == 98u) {  // 'b'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x48u; }
        if (col == 2u) { return 0x44u; }
        if (col == 3u) { return 0x44u; }
        if (col == 4u) { return 0x38u; }
    } else if (char_code == 99u) {  // 'c'
        if (col == 0u) { return 0x38u; }
        if (col == 1u) { return 0x44u; }
        if (col == 2u) { return 0x44u; }
        if (col == 3u) { return 0x44u; }
        if (col == 4u) { return 0x28u; }
    } else if (char_code == 100u) {  // 'd'
        if (col == 0u) { return 0x38u; }
        if (col == 1u) { return 0x44u; }
        if (col == 2u) { return 0x44u; }
        if (col == 3u) { return 0x48u; }
        if (col == 4u) { return 0x7Fu; }
    } else if (char_code == 101u) {  // 'e'
        if (col == 0u) { return 0x38u; }
        if (col == 1u) { return 0x54u; }
        if (col == 2u) { return 0x54u; }
        if (col == 3u) { return 0x54u; }
        if (col == 4u) { return 0x18u; }
    } else if (char_code == 110u) {  // 'n'
        if (col == 0u) { return 0x78u; }
        if (col == 1u) { return 0x14u; }
        if (col == 2u) { return 0x14u; }
        if (col == 3u) { return 0x14u; }
        if (col == 4u) { return 0x08u; }
    } else if (char_code == 100u) {  // 'd'
        if (col == 0u) { return 0x38u; }
        if (col == 1u) { return 0x44u; }
        if (col == 2u) { return 0x44u; }
        if (col == 3u) { return 0x48u; }
        if (col == 4u) { return 0x7Fu; }
    } else if (char_code == 116u) {  // 't'
        if (col == 0u) { return 0x08u; }
        if (col == 1u) { return 0x3Eu; }
        if (col == 2u) { return 0x28u; }
        if (col == 3u) { return 0x28u; }
        if (col == 4u) { return 0x20u; }
    }
    return 0u;
} == 3u) { return 0x41u; }
        if (col == 4u) { return 0x3Eu; }
    } else if (char_code == 80u) {  // 'P'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x09u; }
        if (col == 2u) { return 0x09u; }
        if (col == 3u) { return 0x09u; }
        if (col == 4u) { return 0x06u; }
    } else if (char_code == 81u) {  // 'Q'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x51u; }
        if (col == 3u) { return 0x41u; }
        if (col == 4u) { return 0x3Eu; }
    } else if (char_code == 82u) {  // 'R'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x09u; }
        if (col == 2u) { return 0x19u; }
        if (col == 3u) { return 0x29u; }
        if (col == 4u) { return 0x46u; }
    } else if (char_code == 83u) {  // 'S'
        if (col == 0u) { return 0x26u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x32u; }
    } else if (char_code == 84u) {  // 'T'
        if (col == 0u) { return 0x01u; }
        if (col == 1u) { return 0x01u; }
        if (col == 2u) { return 0x7Fu; }
        if (col == 3u) { return 0x01u; }
        if (col == 4u) { return 0x01u; }
    } else if (char_code == 85u) {  // 'U'
        if (col == 0u) { return 0x3Fu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x3Fu; }
    } else if (char_code == 86u) {  // 'V'
        if (col == 0u) { return 0x1Fu; }
        if (col == 1u) { return 0x20u; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x20u; }
        if (col == 4u) { return 0x1Fu; }
    } else if (char_code == 87u) {  // 'W'
        if (col == 0u) { return 0x3Fu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x38u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x3Fu; }
    } else if (char_code == 88u) {  // 'X'
        if (col == 0u) { return 0x63u; }
        if (col == 1u) { return 0x14u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x14u; }
        if (col == 4u) { return 0x63u; }
    } else if (char_code == 89u) {  // 'Y'
        if (col == 0u) { return 0x07u; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x70u; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x07u; }
    } else if (char_code == 90u) {  // 'Z'
        if (col == 0u) { return 0x61u; }
        if (col == 1u) { return 0x51u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x45u; }
        if (col == 4u) { return 0x43u; }
    }
    // Special characters for VM opcodes
    else if (char_code == 32u) {  // ' ' (space)
        return 0u;
    } else if (char_code == 43u) {  // '+' (add)
        if (col == 0u) { return 0x08u; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x3Eu; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x08u; }
    } else if (char_code == 45u) {  // '-' (subtract/negate)
        if (col == 0u) { return 0x08u; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x08u; }
    } else if (char_code == 42u) {  // '*' (multiply)
        if (col == 0u) { return 0x14u; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x3Eu; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x14u; }
    } else if (char_code == 47u) {  // '/' (divide)
        if (col == 0u) { return 0x20u; }
        if (col == 1u) { return 0x10u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x04u; }
        if (col == 4u) { return 0x02u; }
    } else if (char_code == 61u) {  // '=' (equals)
        if (col == 0u) { return 0x14u; }
        if (col == 1u) { return 0x14u; }
        if (col == 2u) { return 0x14u; }
        if (col == 3u) { return 0x14u; }
        if (col == 4u) { return 0x14u; }
    } else if (char_code == 64u) {  // '@' (execute/print)
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x45u; }
        if (col == 2u) { return 0x4Du; }
        if (col == 3u) { return 0x51u; }
        if (col == 4u) { return 0x3Eu; }
    } else if (char_code == 58u) {  // ':' (colon)
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x24u; }
        if (col == 2u) { return 0x00u; }
        return 0u;
    } else if (char_code == 95u) {  // '_' (underscore)
        if (col >= 0u && col <= 4u) { return 0x01u; }
    } else if (char_code == 62u) {  // '>' (right arrow)
        if (col == 0u) { return 0x04u; }
        if (col == 1u) { return 0x02u; }
        if (col == 2u) { return 0x7Fu; }
        if (col == 3u) { return 0x02u; }
        if (col == 4u) { return 0x04u; }
    } else if (char_code == 60u) {  // '<' (left arrow)
        if (col == 0u) { return 0x10u; }
        if (col == 1u) { return 0x20u; }
        if (col == 2u) { return 0x7Fu; }
        if (col == 3u) { return 0x20u; }
        if (col == 4u) { return 0x10u; }
    }
    // Lowercase a-z (map to same as uppercase for simplicity)
    else if (char_code >= 97u && char_code <= 122u) {
        return get_font_column(char_code - 32u, col);
    }
    if (col == 3u) { return 0x41u; }
        if (col == 4u) { return 0x3Eu; }
    } else if (char_code == 80u) {  // 'P'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x09u; }
        if (col == 2u) { return 0x09u; }
        if (col == 3u) { return 0x09u; }
        if (col == 4u) { return 0x06u; }
    } else if (char_code == 81u) {  // 'Q'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x51u; }
        if (col == 3u) { return 0x21u; }
        if (col == 4u) { return 0x5Eu; }
    } else if (char_code == 82u) {  // 'R'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x09u; }
        if (col == 2u) { return 0x19u; }
        if (col == 3u) { return 0x29u; }
        if (col == 4u) { return 0x46u; }
    } else if (char_code == 83u) {  // 'S'
        if (col == 0u) { return 0x26u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x32u; }
    } else if (char_code == 84u) {  // 'T'
        if (col == 0u) { return 0x01u; }
        if (col == 1u) { return 0x01u; }
        if (col == 2u) { return 0x7Fu; }
        if (col == 3u) { return 0x01u; }
        if (col == 4u) { return 0x01u; }
    } else if (char_code == 85u) {  // 'U'
        if (col == 0u) { return 0x3Fu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x3Fu; }
    } else if (char_code == 86u) {  // 'V'
        if (col == 0u) { return 0x1Fu; }
        if (col == 1u) { return 0x20u; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x20u; }
        if (col == 4u) { return 0x1Fu; }
    } else if (char_code == 87u) {  // 'W'
        if (col == 0u) { return 0x3Fu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x38u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x3Fu; }
    } else if (char_code == 88u) {  // 'X'
        if (col == 0u) { return 0x63u; }
        if (col == 1u) { return 0x14u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x14u; }
        if (col == 4u) { return 0x63u; }
    } else if (char_code == 89u) {  // 'Y'
        if (col == 0u) { return 0x07u; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x70u; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x07u; }
    } else if (char_code == 90u) {  // 'Z'
        if (col == 0u) { return 0x61u; }
        if (col == 1u) { return 0x51u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x45u; }
        if (col == 4u) { return 0x43u; }
    }
    // Lowercase a-z (char codes 97-122)
    else if (char_code == 97u) {  // 'a'
        if (col == 0u) { return 0x20u; }
        if (col == 1u) { return 0x54u; }
        if (col == 2u) { return 0x54u; }
        if (col == 3u) { return 0x54u; }
        if (col == 4u) { return 0x78u; }
    } else if (char_code == 98u) {  // 'b'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x48u; }
        if (col == 2u) { return 0x44u; }
        if (col == 3u) { return 0x44u; }
        if (col == 4u) { return 0x38u; }
    } else if (char_code == 99u) {  // 'c'
        if (col == 0u) { return 0x38u; }
        if (col == 1u) { return 0x44u; }
        if (col == 2u) { return 0x44u; }
        if (col == 3u) { return 0x44u; }
        if (col == 4u) { return 0x20u; }
    } else if (char_code == 100u) {  // 'd'
        if (col == 0u) { return 0x38u; }
        if (col == 1u) { return 0x44u; }
        if (col == 2u) { return 0x44u; }
        if (col == 3u) { return 0x48u; }
        if (col == 4u) { return 0x7Fu; }
    } else if (char_code == 101u) {  // 'e'
        if (col == 0u) { return 0x38u; }
        if (col == 1u) { return 0x54u; }
        if (col == 2u) { return 0x54u; }
        if (col == 3u) { return 0x54u; }
        if (col == 4u) { return 0x18u; }
    } else if (char_code == 102u) {  // 'f'
        if (col == 0u) { return 0x08u; }
        if (col == 1u) { return 0x7Eu; }
        if (col == 2u) { return 0x09u; }
        if (col == 3u) { return 0x01u; }
        if (col == 4u) { return 0x02u; }
    } else if (char_code == 103u) {  // 'g'
        if (col == 0u) { return 0x0Cu; }
        if (col == 1u) { return 0x52u; }
        if (col == 2u) { return 0x52u; }
        if (col == 3u) { return 0x52u; }
        if (col == 4u) { return 0x3Eu; }
    } else if (char_code == 104u) {  // 'h'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x04u; }
        if (col == 3u) { return 0x04u; }
        if (col == 4u) { return 0x78u; }
    } else if (char_code == 105u) {  // 'i'
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x44u; }
        if (col == 2u) { return 0x7Du; }
        if (col == 3u) { return 0x40u; }
        return 0u;
    } else if (char_code == 106u) {  // 'j'
        if (col == 0u) { return 0x20u; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x44u; }
        if (col == 3u) { return 0x3Du; }
        return 0u;
    } else if (char_code == 107u) {  // 'k'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x10u; }
        if (col == 2u) { return 0x28u; }
        if (col == 3u) { return 0x44u; }
        return 0u;
    } else if (char_code == 108u) {  // 'l'
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x7Fu; }
        if (col == 3u) { return 0x40u; }
        return 0u;
    } else if (char_code == 109u) {  // 'm'
        if (col == 0u) { return 0x7Cu; }
        if (col == 1u) { return 0x04u; }
        if (col == 2u) { return 0x18u; }
        if (col == 3u) { return 0x04u; }
        if (col == 4u) { return 0x78u; }
    } else if (char_code == 110u) {  // 'n'
        if (col == 0u) { return 0x7Cu; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x04u; }
        if (col == 3u) { return 0x04u; }
        if (col == 4u) { return 0x78u; }
    } else if (char_code == 111u) {  // 'o'
        if (col == 0u) { return 0x38u; }
        if (col == 1u) { return 0x44u; }
        if (col == 2u) { return 0x44u; }
        if (col == 3u) { return 0x44u; }
        if (col == 4u) { return 0x38u; }
    } else if (char_code == 112u) {  // 'p'
        if (col == 0u) { return 0x7Cu; }
        if (col == 1u) { return 0x14u; }
        if (col == 2u) { return 0x14u; }
        if (col == 3u) { return 0x14u; }
        if (col == 4u) { return 0x08u; }
    } else if (char_code == 113u) {  // 'q'
        if (col == 0u) { return 0x08u; }
        if (col == 1u) { return 0x14u; }
        if (col == 2u) { return 0x14u; }
        if (col == 3u) { return 0x18u; }
        if (col == 4u) { return 0x7Cu; }
    } else if (char_code == 114u) {  // 'r'
        if (col == 0u) { return 0x7Cu; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x04u; }
        if (col == 3u) { return 0x04u; }
        if (col == 4u) { return 0x08u; }
    } else if (char_code == 115u) {  // 's'
        if (col == 0u) { return 0x48u; }
        if (col == 1u) { return 0x54u; }
        if (col == 2u) { return 0x54u; }
        if (col == 3u) { return 0x54u; }
        if (col == 4u) { return 0x20u; }
    } else if (char_code == 116u) {  // 't'
        if (col == 0u) { return 0x04u; }
        if (col == 1u) { return 0x3Fu; }
        if (col == 2u) { return 0x44u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x20u; }
    } else if (char_code == 117u) {  // 'u'
        if (col == 0u) { return 0x3Cu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x20u; }
        if (col == 4u) { return 0x7Cu; }
    } else if (char_code == 118u) {  // 'v'
        if (col == 0u) { return 0x1Cu; }
        if (col == 1u) { return 0x20u; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x20u; }
        if (col == 4u) { return 0x1Cu; }
    } else if (char_code == 119u) {  // 'w'
        if (col == 0u) { return 0x3Cu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x30u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x3Cu; }
    } else if (char_code == 120u) {  // 'x'
        if (col == 0u) { return 0x44u; }
        if (col == 1u) { return 0x28u; }
        if (col == 2u) { return 0x10u; }
        if (col == 3u) { return 0x28u; }
        if (col == 4u) { return 0x44u; }
    } else if (char_code == 121u) {  // 'y'
        if (col == 0u) { return 0x0Cu; }
        if (col == 1u) { return 0x50u; }
        if (col == 2u) { return 0x50u; }
        if (col == 3u) { return 0x50u; }
        if (col == 4u) { return 0x3Cu; }
    } else if (char_code == 122u) {  // 'z'
        if (col == 0u) { return 0x44u; }
        if (col == 1u) { return 0x64u; }
        if (col == 2u) { return 0x54u; }
        if (col == 3u) { return 0x4Cu; }
        if (col == 4u) { return 0x44u; }
    }
    // Special characters
    else if (char_code == 32u) {  // ' ' (space)
        return 0u;
    } else if (char_code == 33u) {  // '!'
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x00u; }
        if (col == 2u) { return 0x5Fu; }
        return 0u;
    } else if (char_code == 34u) {  // '"'
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x07u; }
        if (col == 2u) { return 0x00u; }
        if (col == 3u) { return 0x07u; }
        return 0u;
    } else if (char_code == 40u) {  // '('
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x1Cu; }
        if (col == 2u) { return 0x22u; }
        if (col == 3u) { return 0x41u; }
        return 0u;
    } else if (char_code == 41u) {  // ')'
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x22u; }
        if (col == 3u) { return 0x1Cu; }
        return 0u;
    } else if (char_code == 42u) {  // '*'
        if (col == 0u) { return 0x14u; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x3Eu; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x14u; }
    } else if (char_code == 43u) {  // '+'
        if (col == 0u) { return 0x08u; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x3Eu; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x08u; }
    } else if (char_code == 44u) {  // ','
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x80u; }
        if (col == 2u) { return 0x60u; }
        return 0u;
    } else if (char_code == 45u) {  // '-'
        if (col == 0u) { return 0x08u; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x08u; }
    } else if (char_code == 46u) {  // '.'
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x00u; }
        if (col == 2u) { return 0x60u; }
        return 0u;
    } else if (char_code == 47u) {  // '/'
        if (col == 0u) { return 0x20u; }
        if (col == 1u) { return 0x10u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x04u; }
        if (col == 4u) { return 0x02u; }
    } else if (char_code == 58u) {  // ':'
        if (col == 1u) { return 0x36u; }
        if (col == 2u) { return 0x36u; }
        return 0u;
    } else if (char_code == 59u) {  // ';'
        if (col == 1u) { return 0x36u; }
        if (col == 2u) { return 0x60u; }
        return 0u;
    } else if (char_code == 60u) {  // '<'
        if (col == 0u) { return 0x08u; }
        if (col == 1u) { return 0x1Cu; }
        if (col == 2u) { return 0x22u; }
        return 0u;
    } else if (char_code == 61u) {  // '='
        if (col == 1u) { return 0x7Fu; }
        if (col == 3u) { return 0x7Fu; }
        return 0u;
    } else if (char_code == 62u) {  // '>'
        if (col == 0u) { return 0x22u; }
        if (col == 1u) { return 0x1Cu; }
        if (col == 2u) { return 0x08u; }
        return 0u;
    } else if (char_code == 63u) {  // '?'
        if (col == 0u) { return 0x02u; }
        if (col == 1u) { return 0x01u; }
        if (col == 2u) { return 0x51u; }
        if (col == 3u) { return 0x09u; }
        if (col == 4u) { return 0x06u; }
    } else if (char_code == 64u) {  // '@'
        if (col == 0u) { return 0x32u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x79u; }
        if (col == 3u) { return 0x41u; }
        if (col == 4u) { return 0x3Eu; }
    } else if (char_code == 91u) {  // '['
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x7Fu; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x41u; }
        return 0u;
    } else if (char_code == 93u) {  // ']'
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x7Fu; }
        return 0u;
    } else if (char_code == 95u) {  // '_'
        if (col == 0u) { return 0x80u; }
        if (col == 1u) { return 0x80u; }
        if (col == 2u) { return 0x80u; }
        if (col == 3u) { return 0x80u; }
        if (col == 4u) { return 0x80u; }
    }
    
    return 0u;
}

// Draw a character at position (x, y) in the framebuffer
fn draw_char(char_code: u32, x: u32, y: u32, color: Pixel) -> u32 {
    var col = 0u;
    loop {
        if (col >= 5u) { break; }
        
        let byte = get_font_column(char_code, col);
        var row = 0u;
        loop {
            if (row >= 7u) { break; }
            
            if ((byte >> row) & 1u) == 1u {
                let px = x + col;
                let py = y + row;
                if (px < config.width && py < config.height) {
                    let i = py * config.width + px;
                    buffer_out[i] = color;
                }
            }
            
            row += 1u;
        }
        
        col += 1u;
    }
    
    return x + 6u;  // 5 pixels + 1 pixel spacing
}

// Draw a number (0-9999) as up to 4 digits
fn draw_number(value: u32, x: u32, y: u32, color: Pixel) -> u32 {
    let thousands = (value / 1000u) % 10u;
    let hundreds = (value / 100u) % 10u;
    let tens = (value / 10u) % 10u;
    let ones = value % 10u;
    
    var cursor_x = x;
    
    // Skip leading zeros for thousands/hundreds
    if value >= 1000u {
        cursor_x = draw_char(48u + thousands, cursor_x, y, color);
    }
    if value >= 100u {
        cursor_x = draw_char(48u + hundreds, cursor_x, y, color);
    }
    if value >= 10u {
        cursor_x = draw_char(48u + tens, cursor_x, y, color);
    }
    cursor_x = draw_char(48u + ones, cursor_x, y, color);
    
    return cursor_x;
}

// ============================================================================
// HUD RENDERER — Rows 400-449
// ============================================================================

fn render_hud() {
    // HUD colors
    var header_color: Pixel;
    header_color.r = 0u;
    header_color.g = 200u;
    header_color.b = 255u;
    header_color.a = 255u;
    
    var value_color: Pixel;
    value_color.r = 255u;
    value_color.g = 255u;
    value_color.b = 255u;
    value_color.a = 255u;
    
    var bg_color: Pixel;
    bg_color.r = 15u;
    bg_color.g = 25u;
    bg_color.b = 35u;
    bg_color.a = 255u;
    
    // Clear HUD area (rows 400-449)
    var y = 400u;
    loop {
        if (y >= 450u) { break; }
        
        var x = 0u;
        loop {
            if (x >= config.width) { break; }
            let i = y * config.width + x;
            buffer_out[i] = bg_color;
            x += 1u;
        }
        y += 1u;
    }
    
    // Draw "SOVEREIGN SHELL" header
    var cursor_x = 20u;
    var cursor_y = 405u;
    
    // S-O-V-E-R-E-I-G-N
    cursor_x = draw_char(83u, cursor_x, cursor_y, header_color);   // S
    cursor_x = draw_char(79u, cursor_x, cursor_y, header_color);   // O
    cursor_x = draw_char(86u, cursor_x, cursor_y, header_color);   // V
    cursor_x = draw_char(69u, cursor_x, cursor_y, header_color);   // E
    cursor_x = draw_char(82u, cursor_x, cursor_y, header_color);   // R
    cursor_x = draw_char(69u, cursor_x, cursor_y, header_color);   // E
    cursor_x = draw_char(73u, cursor_x, cursor_y, header_color);   // I
    cursor_x = draw_char(71u, cursor_x, cursor_y, header_color);   // G
    cursor_x = draw_char(78u, cursor_x, cursor_y, header_color);   // N
    
    cursor_x += 10u;
    
    // S-H-E-L-L
    cursor_x = draw_char(83u, cursor_x, cursor_y, header_color);   // S
    cursor_x = draw_char(72u, cursor_x, cursor_y, header_color);   // H
    cursor_x = draw_char(69u, cursor_x, cursor_y, header_color);   // E
    cursor_x = draw_char(76u, cursor_x, cursor_y, header_color);   // L
    cursor_x = draw_char(76u, cursor_x, cursor_y, header_color);   // L
    
    // Draw register values (A-J) in row 420
    cursor_x = 20u;
    cursor_y = 420u;
    
    var i = 0u;
    loop {
        if (i >= 10u) { break; }
        
        // Register name (A=65, B=66, ...)
        let reg_name = 65u + i;
        cursor_x = draw_char(reg_name, cursor_x, cursor_y, header_color);
        cursor_x = draw_char(58u, cursor_x, cursor_y, header_color);  // ':'
        
        // Register value
        let value = registers[i];
        cursor_x = draw_number(value, cursor_x, cursor_y, value_color);
        
        // Spacing
        cursor_x += 8u;
        
        i += 1u;
    }
    
    // Draw IP, SP, and Stack depth at row 435
    cursor_x = 20u;
    cursor_y = 435u;
    
    cursor_x = draw_char(73u, cursor_x, cursor_y, header_color);   // I
    cursor_x = draw_char(80u, cursor_x, cursor_y, header_color);   // P
    cursor_x = draw_char(58u, cursor_x, cursor_y, header_color);   // :
    let ip = vm_stats[1u];
    cursor_x = draw_number(ip, cursor_x + 5u, cursor_y, value_color);
    
    cursor_x += 15u;
    
    cursor_x = draw_char(83u, cursor_x, cursor_y, header_color);   // S
    cursor_x = draw_char(80u, cursor_x, cursor_y, header_color);   // P
    cursor_x = draw_char(58u, cursor_x, cursor_y, header_color);   // :
    let sp = vm_stats[2u];
    cursor_x = draw_number(sp, cursor_x + 5u, cursor_y, value_color);
    
    cursor_x += 15u;
    
    // Execution result
    cursor_x = draw_char(61u, cursor_x, cursor_y, header_color);   // =
    cursor_x = draw_char(62u, cursor_x, cursor_y, header_color);   // >
    cursor_x += 5u;
    let result = exec_result[0u];
    cursor_x = draw_number(result, cursor_x, cursor_y, value_color);
}

// ============================================================================
// INPUT ZONE — Rows 450-474
// ============================================================================

fn render_input_zone() {
    var prompt_color: Pixel;
    prompt_color.r = 0u;
    prompt_color.g = 255u;
    prompt_color.b = 128u;
    prompt_color.a = 255u;
    
    var input_color: Pixel;
    input_color.r = 255u;
    input_color.g = 255u;
    input_color.b = 255u;
    input_color.a = 255u;
    
    var bg_color: Pixel;
    bg_color.r = 25u;
    bg_color.g = 35u;
    bg_color.b = 45u;
    bg_color.a = 255u;
    
    var border_color: Pixel;
    border_color.r = 0u;
    border_color.g = 128u;
    border_color.b = 255u;
    border_color.a = 255u;
    
    // Clear input zone (rows 450-474)
    var y = 450u;
    loop {
        if (y >= 475u) { break; }
        
        var x = 0u;
        loop {
            if (x >= config.width) { break; }
            
            let i = y * config.width + x;
            
            // Draw border on first and last row
            if (y == 450u || y == 474u) {
                buffer_out[i] = border_color;
            } else {
                buffer_out[i] = bg_color;
            }
            
            x += 1u;
        }
        y += 1u;
    }
    
    // Draw prompt "> " at row 455
    var cursor_x = 15u;
    var cursor_y = 455u;
    cursor_x = draw_char(62u, cursor_x, cursor_y, prompt_color);   // >
    cursor_x = draw_char(32u, cursor_x, cursor_y, prompt_color);   // space
    cursor_x += 5u;
    
    // Draw input buffer contents
    var i = 0u;
    loop {
        if (i >= 64u) { break; }
        let ch = input_buffer[i];
        if (ch == 0u) { break; }  // Null terminator
        cursor_x = draw_char(ch, cursor_x, cursor_y, input_color);
        i += 1u;
    }
    
    // Draw blinking cursor (based on frame number)
    let show_cursor = (config.frame % 60u) < 30u;
    if (show_cursor) {
        // Draw underscore cursor
        let cursor_char: u32 = 95u;  // '_'
        _ = draw_char(cursor_char, cursor_x, cursor_y, prompt_color);
    }
}

// ============================================================================
// PATCH STATUS — Rows 475-479
// ============================================================================

fn render_patch_status() {
    var success_color: Pixel;
    success_color.r = 0u;
    success_color.g = 255u;
    success_color.b = 0u;
    success_color.a = 255u;
    
    var fail_color: Pixel;
    fail_color.r = 255u;
    fail_color.g = 0u;
    fail_color.b = 0u;
    fail_color.a = 255u;
    
    var neutral_color: Pixel;
    neutral_color.r = 128u;
    neutral_color.g = 128u;
    neutral_color.b = 128u;
    neutral_color.a = 255u;
    
    var bg_color: Pixel;
    bg_color.r = 20u;
    bg_color.g = 20u;
    bg_color.b = 30u;
    bg_color.a = 255u;
    
    // Clear status zone (rows 475-479)
    var y = 475u;
    loop {
        if (y >= 480u) { break; }
        
        var x = 0u;
        loop {
            if (x >= config.width) { break; }
            let i = y * config.width + x;
            buffer_out[i] = bg_color;
            x += 1u;
        }
        y += 1u;
    }
    
    // Get patch status
    let status = patch_status[0u];
    
    var cursor_x = 20u;
    let cursor_y = 476u;
    
    if (status == 1u) {
        // PATCH_SUCCESS in green
        cursor_x = draw_char(80u, cursor_x, cursor_y, success_color);   // P
        cursor_x = draw_char(65u, cursor_x, cursor_y, success_color);   // A
        cursor_x = draw_char(84u, cursor_x, cursor_y, success_color);   // T
        cursor_x = draw_char(67u, cursor_x, cursor_y, success_color);   // C
        cursor_x = draw_char(72u, cursor_x, cursor_y, success_color);   // H
        cursor_x = draw_char(95u, cursor_x, cursor_y, success_color);   // _
        cursor_x = draw_char(83u, cursor_x, cursor_y, success_color);   // S
        cursor_x = draw_char(85u, cursor_x, cursor_y, success_color);   // U
        cursor_x = draw_char(67u, cursor_x, cursor_y, success_color);   // C
        cursor_x = draw_char(67u, cursor_x, cursor_y, success_color);   // C
        cursor_x = draw_char(69u, cursor_x, cursor_y, success_color);   // E
        cursor_x = draw_char(83u, cursor_x, cursor_y, success_color);   // S
        cursor_x = draw_char(83u, cursor_x, cursor_y, success_color);   // S
    } else if (status == 2u) {
        // PATCH_FAIL in red
        cursor_x = draw_char(80u, cursor_x, cursor_y, fail_color);   // P
        cursor_x = draw_char(65u, cursor_x, cursor_y, fail_color);   // A
        cursor_x = draw_char(84u, cursor_x, cursor_y, fail_color);   // T
        cursor_x = draw_char(67u, cursor_x, cursor_y, fail_color);   // C
        cursor_x = draw_char(72u, cursor_x, cursor_y, fail_color);   // H
        cursor_x = draw_char(95u, cursor_x, cursor_y, fail_color);   // _
        cursor_x = draw_char(70u, cursor_x, cursor_y, fail_color);   // F
        cursor_x = draw_char(65u, cursor_x, cursor_y, fail_color);   // A
        cursor_x = draw_char(73u, cursor_x, cursor_y, fail_color);   // I
        cursor_x = draw_char(76u, cursor_x, cursor_y, fail_color);   // L
    } else {
        // Ready state
        cursor_x = draw_char(82u, cursor_x, cursor_y, neutral_color);   // R
        cursor_x = draw_char(69u, cursor_x, cursor_y, neutral_color);   // E
        cursor_x = draw_char(65u, cursor_x, cursor_y, neutral_color);   // A
        cursor_x = draw_char(68u, cursor_x, cursor_y, neutral_color);   // D
        cursor_x = draw_char(89u, cursor_x, cursor_y, neutral_color);   // Y
    }
}

// ============================================================================
// MAIN COMPUTE SHADER
// ============================================================================

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    
    // First 64 threads render UI layers
    if (idx < 64u) {
        render_hud();
        render_input_zone();
        render_patch_status();
        return;
    }
    
    // Rest of threads copy input to output (pass-through for agent execution space)
    if (idx < config.width * config.height) {
        buffer_out[idx] = buffer_in[idx];
    }
}
