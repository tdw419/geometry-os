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

fn get_font_column(char_code: u32, col: u32) -> u32 {
    const font_table: array<u32, 10> = array<48u, 49u, 50u, 51u, 52u, 53u, 54u, 55u, 56u, 57u, 65u, 66u, 67u, 68u, 69u, 70u, 71u, 72u, 73u, 74u, 75u, 76u, 77u, 78u, 79u, 80u>;
    const font_data: array<u32, 10> = array<0x3Eu, 0x51u, 0x49u, 0x45u, 0x3Eu, 0x42u, 0x7Fu, 0x40u, 0x62u, 0x51u, 0x49u, 0x49u, 0x49u, 0x46u, 0x22u, 0x49u, 0x49u, 0x49u, 0x36u, 0x18u, 0x14u, 0x12u, 0x7Fu, 0x10u, 0x27u, 0x45u, 0x45u, 0x45u, 0x39u, 0x3Eu, 0x49u, 0x49u, 0x49u, 0x32u, 0x01u, 0x71u, 0x09u, 0x05u, 0x03u, 0x36u, 0x49u, 0x49u, 0x49u, 0x36u, 0x26u, 0x49u, 0x49u, 0x49u, 0x3Eu>;
    let index = match char_code {
        48..=57 => font_table[char_code - 48] as usize,
        65..=90 => font_table[char_code - 65 + 10] as usize,
        _ => return 0u,
    };
    let column_data = font_data[index];
    match col {
        0 => (column_data & 0x3F) >> 0,
        1 => (column_data & 0xC0) >> 6,
        2 => (column_data & 0x300) >> 8,
        3 => (column_data & 0xC00) >> 10,
        4 => (column_data & 0x3F000) >> 12,
        _ => return 0u,
    }
}

    const font_table: array<u32, 10> = array<48u, 49u, 50u, 51u, 52u, 53u, 54u, 55u, 56u, 57u, 65u, 66u, 67u, 68u, 69u, 70u, 71u, 72u, 73u, 74u, 75u, 76u, 77u, 78u, 79u, 80u>;
    const font_data: array<u32, 10> = array<0x3Eu, 0x51u, 0x49u, 0x45u, 0x3Eu, 0x42u, 0x7Fu, 0x40u, 0x62u, 0x51u, 0x49u, 0x49u, 0x49u, 0x46u, 0x22u, 0x49u, 0x49u, 0x49u, 0x36u, 0x18u, 0x14u, 0x12u, 0x7Fu, 0x10u, 0x27u, 0x45u, 0x45u, 0x45u, 0x39u, 0x3Eu, 0x49u, 0x49u, 0x49u, 0x32u, 0x01u, 0x71u, 0x09u, 0x05u, 0x03u, 0x36u, 0x49u, 0x49u, 0x49u, 0x36u, 0x26u, 0x49u, 0x49u, 0x49u, 0x3Eu>;
    let index = match char_code {
        48..=57 => font_table[char_code - 48] as usize,
        65..=90 => font_table[char_code - 65 + 10] as usize,
        _ => return 0u,
    };
    let column_data = font_data[index];
    match col {
        0 => (column_data & 0x3F) >> 0,
        1 => (column_data & 0xC0) >> 6,
        2 => (column_data & 0x300) >> 8,
        3 => (column_data & 0xC00) >> 10,
        4 => (column_data & 0x3F000) >> 12,
        _ => return 0u,
    }
}
    // Digits 0-9 (char codes 48-57)
    if (char_code == 48u) {  // '0'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x51u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x45u; }
        if (col == 4u) { return 0x3Eu; }
    } else if (char_code == 49u) {  // '1'
        if (col == 0u) { return 0x42u; }
        if (col == 1u) { return 0x7Fu; }
        if (col == 2u) { return 0x40u; }
        return 0u;
    } else if (char_code == 50u) {  // '2'
        if (col == 0u) { return 0x62u; }
        if (col == 1u) { return 0x51u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x46u; }
    } else if (char_code == 51u) {  // '3'
        if (col == 0u) { return 0x22u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x36u; }
    } else if (char_code == 52u) {  // '4'
        if (col == 0u) { return 0x18u; }
        if (col == 1u) { return 0x14u; }
        if (col == 2u) { return 0x12u; }
        if (col == 3u) { return 0x7Fu; }
        if (col == 4u) { return 0x10u; }
    } else if (char_code == 53u) {  // '5'
        if (col == 0u) { return 0x27u; }
        if (col == 1u) { return 0x45u; }
        if (col == 2u) { return 0x45u; }
        if (col == 3u) { return 0x45u; }
        if (col == 4u) { return 0x39u; }
    } else if (char_code == 54u) {  // '6'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x32u; }
    } else if (char_code == 55u) {  // '7'
        if (col == 0u) { return 0x01u; }
        if (col == 1u) { return 0x71u; }
        if (col == 2u) { return 0x09u; }
        if (col == 3u) { return 0x05u; }
        if (col == 4u) { return 0x03u; }
    } else if (char_code == 56u) {  // '8'
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
    // Letters A-Z (char codes 65-90)
    else if (char_code == 65u) {  // 'A'
        if (col == 0u) { return 0x7Eu; }
        if (col == 1u) { return 0x11u; }
        if (col == 2u) { return 0x11u; }
        if (col == 3u) { return 0x11u; }
        if (col == 4u) { return 0x7Eu; }
    } else if (char_code == 66u) {  // 'B'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x36u; }
    } else if (char_code == 67u) {  // 'C'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x41u; }
        if (col == 4u) { return 0x22u; }
    } else if (char_code == 68u) {  // 'D'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x22u; }
        if (col == 4u) { return 0x1Cu; }
    } else if (char_code == 69u) {  // 'E'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x41u; }
    } else if (char_code == 70u) {  // 'F'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x09u; }
        if (col == 2u) { return 0x09u; }
        if (col == 3u) { return 0x09u; }
        if (col == 4u) { return 0x01u; }
    } else if (char_code == 71u) {  // 'G'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x7Au; }
    } else if (char_code == 72u) {  // 'H'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x7Fu; }
    } else if (char_code == 73u) {  // 'I'
        if (col == 0u) { return 0x41u; }
        if (col == 1u) { return 0x7Fu; }
        if (col == 2u) { return 0x41u; }
        return 0u;
    } else if (char_code == 74u) {  // 'J'
        if (col == 0u) { return 0x20u; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x3Fu; }
        if (col == 4u) { return 0x01u; }
    } else if (char_code == 75u) {  // 'K'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x14u; }
        if (col == 3u) { return 0x22u; }
        if (col == 4u) { return 0x41u; }
    } else if (char_code == 76u) {  // 'L'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x40u; }
    } else if (char_code == 77u) {  // 'M'
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
