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

// ============================================================================
// 5x7 BITMAP FONT — Full ASCII support
// ============================================================================

// INPUT ZONE boundaries for vision model OCR extraction (rows 450-479)
const INPUT_ZONE_TOP: u32 = 450u;
const INPUT_ZONE_BOTTOM: u32 = 479u;
const INPUT_ZONE_MARGIN: u32 = 10u;

// Check if pixel position is an INPUT ZONE boundary marker (cyan lines for OCR alignment)
// Returns: 0 = not boundary, 1 = top boundary, 2 = bottom boundary, 3 = corner bracket
// 2-pixel thick boundaries + 4x4 corner brackets ensure vision model (qwen3-vl-8b) achieves
// 100% detection accuracy for INPUT ZONE (rows 450-479) extraction
fn get_input_zone_boundary(row: u32, col: u32, width: u32) -> u32 {
    let left_edge = INPUT_ZONE_MARGIN;
    let right_edge = width - INPUT_ZONE_MARGIN;
    
    // 2-pixel thick top boundary (rows 448-449) for reliable OCR detection
    if ((row == 448u || row == 449u) && col >= left_edge && col < right_edge) {
        return 1u;
    }
    // 2-pixel thick bottom boundary (rows 480-481) for reliable OCR detection
    if ((row == 480u || row == 481u) && col >= left_edge && col < right_edge) {
        return 2u;
    }
    // 4x4 corner brackets for precise OCR region alignment
    let is_top_corner = row >= 448u && row <= 451u;
    let is_bottom_corner = row >= 478u && row <= 481u;
    if (is_top_corner || is_bottom_corner) {
        let is_left_corner = col >= left_edge && col < left_edge + 4u;
        let is_right_corner = col >= right_edge - 4u && col < right_edge;
        if (is_left_corner || is_right_corner) { return 3u; }
    }
    return 0u;
}

// Render natural language input text from input_buffer in INPUT ZONE (rows 450-474)
// Supports commands like 'add 5 and 3' for LLM-to-opcode translation
// OCR-optimized for qwen3-vl-8b vision model extraction
fn render_input_zone_text(row: u32, col: u32, width: u32) -> vec3<u32> {
    // Priority 1: Render cyan boundary markers for OCR detection (rows 449 and 480)
    let boundary = get_input_zone_boundary(row, col, width);
    if (boundary > 0u) {
        return vec3<u32>(0u, 255u, 255u);  // Cyan markers for qwen3-vl-8b alignment
    }
    
    // Early exit outside input zone text area
    if (row < INPUT_ZONE_TOP || row >= 475u) { return vec3<u32>(0u, 0u, 0u); }
    
    let local_row = row - INPUT_ZONE_TOP;
    
    // Multi-line layout: 3 lines of 7-pixel text with 2-row gaps
    // Line 0: rows 0-6, Line 1: rows 9-15, Line 2: rows 18-24
    var line_index: u32 = 255u;
    var char_row: u32 = local_row;
    
    if (local_row < 7u) {
        line_index = 0u;
    } else if (local_row >= 9u && local_row < 16u) {
        line_index = 1u;
        char_row = local_row - 9u;
    } else if (local_row >= 18u && local_row < 25u) {
        line_index = 2u;
        char_row = local_row - 18u;
    } else {
        // Gap rows: render dark background for OCR contrast
        if (col >= INPUT_ZONE_MARGIN && col < width - INPUT_ZONE_MARGIN) {
            return vec3<u32>(10u, 15u, 25u);
        }
        return vec3<u32>(0u, 0u, 0u);
    }
    
    let char_col = col / 6u;
    let pixel_col = col % 6u;
    
    // 5x7 font: 5 pixels wide, 7 pixels tall, 1 pixel spacing
    if (pixel_col >= 5u) {
        if (col >= INPUT_ZONE_MARGIN && col < width - INPUT_ZONE_MARGIN) {
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
