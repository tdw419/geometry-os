// ============================================================================
// SPATIAL SWARM HUD SHADER
// ============================================================================
// Displays 8 agents with:
//   - Agent ID (0-7) with color coding
//   - POS (x, y) coordinates
//   - VEL (dx, dy) velocity
//   - Status: IT / RUNNER / HALTED
//   - Mailbox indicator (MSG+ if pending)
//   - Trail visualization
// ============================================================================

struct Pixel {
    r: u32,
    g: u32,
    b: u32,
    a: u32,
}

struct AgentState {
    id: u32,
    pos_x: u32,
    pos_y: u32,
    vel_x: i32,
    vel_y: i32,
    color: u32,
    is_it: u32,
    message_waiting: u32,
    trail_len: u32,
    _padding: vec2<u32>,
    trail: array<u32, 32>,
    mailbox: array<u32, 10>,
}

struct Config {
    width: u32,
    height: u32,
    time: f32,
    frame: u32,
    mode: u32,
}

@group(0) @binding(0) var<storage, read_write> buffer_out: array<Pixel>;
@group(0) @binding(1) var<storage, read> agents: array<AgentState>;
@group(0) @binding(2) var<uniform> config: Config;

// ============================================================================
// 5x7 BITMAP FONT
// ============================================================================

fn get_font_column(char_code: u32, col: u32) -> u32 {
    // Digits 0-9
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
    // Letters
    else if (char_code == 65u) {  // 'A'
        if (col == 0u) { return 0x7Eu; }
        if (col == 1u) { return 0x11u; }
        if (col == 2u) { return 0x11u; }
        if (col == 3u) { return 0x11u; }
        if (col == 4u) { return 0x7Eu; }
    } else if (char_code == 67u) {  // 'C'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x41u; }
        if (col == 4u) { return 0x22u; }
    } else if (char_code == 69u) {  // 'E'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x41u; }
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
    } else if (char_code == 77u) {  // 'M'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x04u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x04u; }
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
    } else if (char_code == 82u) {  // 'R'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x09u; }
        if (col == 2u) { return 0x19u; }
        if (col == 3u) { return 0x29u; }
        if (col == 4u) { return 0x46u; }
    } else if (char_code == 83u) {  // 'S'
        if (col == 0u) { return 0x46u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x31u; }
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
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x20u; }
        if (col == 2u) { return 0x10u; }
        if (col == 3u) { return 0x20u; }
        if (col == 4u) { return 0x7Fu; }
    } else if (char_code == 88u) {  // 'X'
        if (col == 0u) { return 0x63u; }
        if (col == 1u) { return 0x14u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x14u; }
        if (col == 4u) { return 0x63u; }
    }
    // Lowercase
    else if (char_code == 105u) {  // 'i'
        if (col == 0u) { return 0x44u; }
        if (col == 1u) { return 0x7Du; }
        return 0u;
    } else if (char_code == 116u) {  // 't'
        if (col == 0u) { return 0x30u; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x78u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x40u; }
    }
    // Special characters
    else if (char_code == 32u) { return 0u; }  // space
    else if (char_code == 43u) {  // '+'
        if (col == 0u) { return 0x08u; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x7Fu; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x08u; }
    } else if (char_code == 44u) {  // ','
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x00u; }
        if (col == 2u) { return 0x60u; }
        if (col == 3u) { return 0x30u; }
        return 0u;
    } else if (char_code == 45u) {  // '-'
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x08u; }
        return 0u;
    } else if (char_code == 58u) {  // ':'
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x00u; }
        if (col == 2u) { return 0x18u; }
        if (col == 3u) { return 0x18u; }
        return 0u;
    } else if (char_code == 40u) {  // '('
        if (col == 0u) { return 0x1Cu; }
        if (col == 1u) { return 0x22u; }
        if (col == 2u) { return 0x41u; }
        return 0u;
    } else if (char_code == 41u) {  // ')'
        if (col == 0u) { return 0x41u; }
        if (col == 1u) { return 0x22u; }
        if (col == 2u) { return 0x1Cu; }
        return 0u;
    }
    
    return 0u;
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn draw_char(start_x: u32, start_y: u32, char_code: u32, color_r: u32, color_g: u32, color_b: u32) {
    // Unrolled 5x7 character
    let col0 = get_font_column(char_code, 0u);
    let col1 = get_font_column(char_code, 1u);
    let col2 = get_font_column(char_code, 2u);
    let col3 = get_font_column(char_code, 3u);
    let col4 = get_font_column(char_code, 4u);
    
    // Column 0
    if ((col0 & 1u) != 0u) {
        let idx = start_y * config.width + start_x;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col0 & 2u) != 0u) {
        let idx = (start_y + 1u) * config.width + start_x;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col0 & 4u) != 0u) {
        let idx = (start_y + 2u) * config.width + start_x;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col0 & 8u) != 0u) {
        let idx = (start_y + 3u) * config.width + start_x;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col0 & 16u) != 0u) {
        let idx = (start_y + 4u) * config.width + start_x;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col0 & 32u) != 0u) {
        let idx = (start_y + 5u) * config.width + start_x;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col0 & 64u) != 0u) {
        let idx = (start_y + 6u) * config.width + start_x;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    
    // Column 1
    if ((col1 & 1u) != 0u) {
        let idx = start_y * config.width + start_x + 1u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col1 & 2u) != 0u) {
        let idx = (start_y + 1u) * config.width + start_x + 1u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col1 & 4u) != 0u) {
        let idx = (start_y + 2u) * config.width + start_x + 1u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col1 & 8u) != 0u) {
        let idx = (start_y + 3u) * config.width + start_x + 1u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col1 & 16u) != 0u) {
        let idx = (start_y + 4u) * config.width + start_x + 1u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col1 & 32u) != 0u) {
        let idx = (start_y + 5u) * config.width + start_x + 1u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col1 & 64u) != 0u) {
        let idx = (start_y + 6u) * config.width + start_x + 1u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    
    // Column 2
    if ((col2 & 1u) != 0u) {
        let idx = start_y * config.width + start_x + 2u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col2 & 2u) != 0u) {
        let idx = (start_y + 1u) * config.width + start_x + 2u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col2 & 4u) != 0u) {
        let idx = (start_y + 2u) * config.width + start_x + 2u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col2 & 8u) != 0u) {
        let idx = (start_y + 3u) * config.width + start_x + 2u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col2 & 16u) != 0u) {
        let idx = (start_y + 4u) * config.width + start_x + 2u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col2 & 32u) != 0u) {
        let idx = (start_y + 5u) * config.width + start_x + 2u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col2 & 64u) != 0u) {
        let idx = (start_y + 6u) * config.width + start_x + 2u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    
    // Column 3
    if ((col3 & 1u) != 0u) {
        let idx = start_y * config.width + start_x + 3u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col3 & 2u) != 0u) {
        let idx = (start_y + 1u) * config.width + start_x + 3u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col3 & 4u) != 0u) {
        let idx = (start_y + 2u) * config.width + start_x + 3u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col3 & 8u) != 0u) {
        let idx = (start_y + 3u) * config.width + start_x + 3u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col3 & 16u) != 0u) {
        let idx = (start_y + 4u) * config.width + start_x + 3u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col3 & 32u) != 0u) {
        let idx = (start_y + 5u) * config.width + start_x + 3u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col3 & 64u) != 0u) {
        let idx = (start_y + 6u) * config.width + start_x + 3u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    
    // Column 4
    if ((col4 & 1u) != 0u) {
        let idx = start_y * config.width + start_x + 4u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col4 & 2u) != 0u) {
        let idx = (start_y + 1u) * config.width + start_x + 4u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col4 & 4u) != 0u) {
        let idx = (start_y + 2u) * config.width + start_x + 4u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col4 & 8u) != 0u) {
        let idx = (start_y + 3u) * config.width + start_x + 4u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col4 & 16u) != 0u) {
        let idx = (start_y + 4u) * config.width + start_x + 4u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col4 & 32u) != 0u) {
        let idx = (start_y + 5u) * config.width + start_x + 4u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
    if ((col4 & 64u) != 0u) {
        let idx = (start_y + 6u) * config.width + start_x + 4u;
        if (idx < arrayLength(&buffer_out)) { buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u); }
    }
}

fn draw_number(x: u32, y: u32, num: u32, color_r: u32, color_g: u32, color_b: u32) -> u32 {
    if (num < 10u) {
        draw_char(x, y, 48u + num, color_r, color_g, color_b);
        return x + 6u;
    } else if (num < 100u) {
        draw_char(x, y, 48u + num / 10u, color_r, color_g, color_b);
        draw_char(x + 6u, y, 48u + num % 10u, color_r, color_g, color_b);
        return x + 12u;
    } else if (num < 1000u) {
        draw_char(x, y, 48u + num / 100u, color_r, color_g, color_b);
        draw_char(x + 6u, y, 48u + (num / 10u) % 10u, color_r, color_g, color_b);
        draw_char(x + 12u, y, 48u + num % 10u, color_r, color_g, color_b);
        return x + 18u;
    } else {
        draw_char(x, y, 48u + num / 1000u, color_r, color_g, color_b);
        draw_char(x + 6u, y, 48u + (num / 100u) % 10u, color_r, color_g, color_b);
        draw_char(x + 12u, y, 48u + (num / 10u) % 10u, color_r, color_g, color_b);
        draw_char(x + 18u, y, 48u + num % 10u, color_r, color_g, color_b);
        return x + 24u;
    }
}

fn draw_signed_number(x: u32, y: u32, num: i32, color_r: u32, color_g: u32, color_b: u32) -> u32 {
    if (num >= 0) {
        draw_char(x, y, 43u, color_r, color_g, color_b);  // +
        return draw_number(x + 6u, y, u32(num), color_r, color_g, color_b);
    } else {
        draw_char(x, y, 45u, color_r, color_g, color_b);  // -
        return draw_number(x + 6u, y, u32(-num), color_r, color_g, color_b);
    }
}

// ============================================================================
// MAIN
// ============================================================================

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    let total_pixels = config.width * config.height;
    
    if (idx >= total_pixels) {
        return;
    }
    
    let x = idx % config.width;
    let y = idx / config.width;
    
    // Initialize to black
    buffer_out[idx] = Pixel(0u, 0u, 0u, 255u);
    
    // ========================================
    // HUD Zone (top 55 rows)
    // ========================================
    if (y < 55u) {
        // Background
        buffer_out[idx] = Pixel(10u, 10u, 20u, 255u);
        
        // Title
        if (y >= 2u && y < 10u && x < 200u) {
            draw_char(10u, 2u, 83u, 0u, 255u, 200u);  // S
            draw_char(16u, 2u, 80u, 0u, 255u, 200u);  // P
            draw_char(22u, 2u, 65u, 0u, 255u, 200u);  // A
            draw_char(28u, 2u, 84u, 0u, 255u, 200u);  // T
            draw_char(34u, 2u, 73u, 0u, 255u, 200u);  // I
            draw_char(40u, 2u, 65u, 0u, 255u, 200u);  // A
            draw_char(46u, 2u, 76u, 0u, 255u, 200u);  // L
            
            draw_char(58u, 2u, 83u, 0u, 255u, 200u);  // S
            draw_char(64u, 2u, 87u, 0u, 255u, 200u);  // W
            draw_char(70u, 2u, 65u, 0u, 255u, 200u);  // A
            draw_char(76u, 2u, 82u, 0u, 255u, 200u);  // R
            draw_char(82u, 2u, 77u, 0u, 255u, 200u);  // M
        }
        
        // Frame counter
        if (y >= 2u && y < 10u && x >= 540u) {
            draw_char(540u, 2u, 70u, 150u, 150u, 150u);  // F
            draw_char(546u, 2u, 82u, 150u, 150u, 150u);  // R
            draw_char(552u, 2u, 65u, 150u, 150u, 150u);  // A
            draw_char(558u, 2u, 77u, 150u, 150u, 150u);  // M
            draw_char(564u, 2u, 69u, 150u, 150u, 150u);  // E
            draw_char(570u, 2u, 58u, 150u, 150u, 150u);  // :
            draw_number(580u, 2u, config.frame, 150u, 150u, 150u);
        }
        
        // Agent panels - draw status for all 8 agents
        // Row 1: Agents 0-3 (y: 15-42)
        // Row 2: Agents 4-7 (y: 15-42, second column)
        
        // Agent 0
        if (y >= 15u && y < 42u && x >= 2u && x < 78u) {
            let agent = agents[0u];
            let acolor = agent.color;
            let ar = (acolor >> 24u) & 0xFFu;
            let ag = (acolor >> 16u) & 0xFFu;
            let ab = (acolor >> 8u) & 0xFFu;
            
            // Border
            if (y == 15u || y == 41u || x == 2u || x == 77u) {
                buffer_out[idx] = Pixel(ar, ag, ab, 255u);
            } else {
                // ID and status
                if (y >= 18u && y < 26u) {
                    draw_char(6u, 18u, 48u, 255u, 255u, 255u);  // 0
                    draw_char(12u, 18u, 58u, 150u, 150u, 150u);  // :
                    if (agent.is_it == 1u) {
                        draw_char(18u, 18u, 73u, 255u, 100u, 100u);  // I
                        draw_char(24u, 18u, 84u, 255u, 100u, 100u);  // T
                    } else {
                        draw_char(18u, 18u, 82u, 100u, 255u, 100u);  // R
                        draw_char(24u, 18u, 85u, 100u, 255u, 100u);  // U
                        draw_char(30u, 18u, 78u, 100u, 255u, 100u);  // N
                    }
                }
                // POS
                if (y >= 26u && y < 34u) {
                    draw_char(6u, 26u, 80u, 200u, 200u, 200u);  // P
                    draw_char(12u, 26u, 58u, 200u, 200u, 200u);  // :
                    let cx = draw_number(18u, 26u, agent.pos_x, 200u, 200u, 200u);
                    draw_char(cx, 26u, 44u, 200u, 200u, 200u);  // ,
                    draw_number(cx + 6u, 26u, agent.pos_y, 200u, 200u, 200u);
                }
                // VEL
                if (y >= 34u && y < 42u) {
                    draw_char(6u, 34u, 86u, 100u, 255u, 100u);  // V
                    draw_char(12u, 34u, 58u, 100u, 255u, 100u);  // :
                    let cx = draw_signed_number(18u, 34u, agent.vel_x, 100u, 255u, 100u);
                    draw_char(cx, 34u, 44u, 100u, 255u, 100u);  // ,
                    draw_signed_number(cx + 6u, 34u, agent.vel_y, 100u, 255u, 100u);
                }
            }
        }
        
        // Agent 1
        if (y >= 15u && y < 42u && x >= 82u && x < 158u) {
            let agent = agents[1u];
            let acolor = agent.color;
            let ar = (acolor >> 24u) & 0xFFu;
            let ag = (acolor >> 16u) & 0xFFu;
            let ab = (acolor >> 8u) & 0xFFu;
            
            if (y == 15u || y == 41u || x == 82u || x == 157u) {
                buffer_out[idx] = Pixel(ar, ag, ab, 255u);
            } else if (y >= 18u && y < 26u) {
                draw_char(86u, 18u, 49u, 255u, 255u, 255u);
                draw_char(92u, 18u, 58u, 150u, 150u, 150u);
                if (agent.is_it == 1u) {
                    draw_char(98u, 18u, 73u, 255u, 100u, 100u);
                    draw_char(104u, 18u, 84u, 255u, 100u, 100u);
                } else {
                    draw_char(98u, 18u, 82u, 100u, 255u, 100u);
                    draw_char(104u, 18u, 85u, 100u, 255u, 100u);
                    draw_char(110u, 18u, 78u, 100u, 255u, 100u);
                }
            } else if (y >= 26u && y < 34u) {
                draw_char(86u, 26u, 80u, 200u, 200u, 200u);
                draw_char(92u, 26u, 58u, 200u, 200u, 200u);
                let cx = draw_number(98u, 26u, agent.pos_x, 200u, 200u, 200u);
                draw_char(cx, 26u, 44u, 200u, 200u, 200u);
                draw_number(cx + 6u, 26u, agent.pos_y, 200u, 200u, 200u);
            } else if (y >= 34u && y < 42u) {
                draw_char(86u, 34u, 86u, 100u, 255u, 100u);
                draw_char(92u, 34u, 58u, 100u, 255u, 100u);
                let cx = draw_signed_number(98u, 34u, agent.vel_x, 100u, 255u, 100u);
                draw_char(cx, 34u, 44u, 100u, 255u, 100u);
                draw_signed_number(cx + 6u, 34u, agent.vel_y, 100u, 255u, 100u);
            }
        }
        
        // Agent 2
        if (y >= 15u && y < 42u && x >= 162u && x < 238u) {
            let agent = agents[2u];
            let acolor = agent.color;
            let ar = (acolor >> 24u) & 0xFFu;
            let ag = (acolor >> 16u) & 0xFFu;
            let ab = (acolor >> 8u) & 0xFFu;
            
            if (y == 15u || y == 41u || x == 162u || x == 237u) {
                buffer_out[idx] = Pixel(ar, ag, ab, 255u);
            } else if (y >= 18u && y < 26u) {
                draw_char(166u, 18u, 50u, 255u, 255u, 255u);
                draw_char(172u, 18u, 58u, 150u, 150u, 150u);
                if (agent.is_it == 1u) {
                    draw_char(178u, 18u, 73u, 255u, 100u, 100u);
                    draw_char(184u, 18u, 84u, 255u, 100u, 100u);
                } else {
                    draw_char(178u, 18u, 82u, 100u, 255u, 100u);
                    draw_char(184u, 18u, 85u, 100u, 255u, 100u);
                    draw_char(190u, 18u, 78u, 100u, 255u, 100u);
                }
            } else if (y >= 26u && y < 34u) {
                draw_char(166u, 26u, 80u, 200u, 200u, 200u);
                draw_char(172u, 26u, 58u, 200u, 200u, 200u);
                let cx = draw_number(178u, 26u, agent.pos_x, 200u, 200u, 200u);
                draw_char(cx, 26u, 44u, 200u, 200u, 200u);
                draw_number(cx + 6u, 26u, agent.pos_y, 200u, 200u, 200u);
            } else if (y >= 34u && y < 42u) {
                draw_char(166u, 34u, 86u, 100u, 255u, 100u);
                draw_char(172u, 34u, 58u, 100u, 255u, 100u);
                let cx = draw_signed_number(178u, 34u, agent.vel_x, 100u, 255u, 100u);
                draw_char(cx, 34u, 44u, 100u, 255u, 100u);
                draw_signed_number(cx + 6u, 34u, agent.vel_y, 100u, 255u, 100u);
            }
        }
        
        // Agent 3
        if (y >= 15u && y < 42u && x >= 242u && x < 318u) {
            let agent = agents[3u];
            let acolor = agent.color;
            let ar = (acolor >> 24u) & 0xFFu;
            let ag = (acolor >> 16u) & 0xFFu;
            let ab = (acolor >> 8u) & 0xFFu;
            
            if (y == 15u || y == 41u || x == 242u || x == 317u) {
                buffer_out[idx] = Pixel(ar, ag, ab, 255u);
            } else if (y >= 18u && y < 26u) {
                draw_char(246u, 18u, 51u, 255u, 255u, 255u);
                draw_char(252u, 18u, 58u, 150u, 150u, 150u);
                if (agent.is_it == 1u) {
                    draw_char(258u, 18u, 73u, 255u, 100u, 100u);
                    draw_char(264u, 18u, 84u, 255u, 100u, 100u);
                } else {
                    draw_char(258u, 18u, 82u, 100u, 255u, 100u);
                    draw_char(264u, 18u, 85u, 100u, 255u, 100u);
                    draw_char(270u, 18u, 78u, 100u, 255u, 100u);
                }
            } else if (y >= 26u && y < 34u) {
                draw_char(246u, 26u, 80u, 200u, 200u, 200u);
                draw_char(252u, 26u, 58u, 200u, 200u, 200u);
                let cx = draw_number(258u, 26u, agent.pos_x, 200u, 200u, 200u);
                draw_char(cx, 26u, 44u, 200u, 200u, 200u);
                draw_number(cx + 6u, 26u, agent.pos_y, 200u, 200u, 200u);
            } else if (y >= 34u && y < 42u) {
                draw_char(246u, 34u, 86u, 100u, 255u, 100u);
                draw_char(252u, 34u, 58u, 100u, 255u, 100u);
                let cx = draw_signed_number(258u, 34u, agent.vel_x, 100u, 255u, 100u);
                draw_char(cx, 34u, 44u, 100u, 255u, 100u);
                draw_signed_number(cx + 6u, 34u, agent.vel_y, 100u, 255u, 100u);
            }
        }
        
        // Agent 4
        if (y >= 15u && y < 42u && x >= 322u && x < 398u) {
            let agent = agents[4u];
            let acolor = agent.color;
            let ar = (acolor >> 24u) & 0xFFu;
            let ag = (acolor >> 16u) & 0xFFu;
            let ab = (acolor >> 8u) & 0xFFu;
            
            if (y == 15u || y == 41u || x == 322u || x == 397u) {
                buffer_out[idx] = Pixel(ar, ag, ab, 255u);
            } else if (y >= 18u && y < 26u) {
                draw_char(326u, 18u, 52u, 255u, 255u, 255u);
                draw_char(332u, 18u, 58u, 150u, 150u, 150u);
                if (agent.is_it == 1u) {
                    draw_char(338u, 18u, 73u, 255u, 100u, 100u);
                    draw_char(344u, 18u, 84u, 255u, 100u, 100u);
                } else {
                    draw_char(338u, 18u, 82u, 100u, 255u, 100u);
                    draw_char(344u, 18u, 85u, 100u, 255u, 100u);
                    draw_char(350u, 18u, 78u, 100u, 255u, 100u);
                }
            } else if (y >= 26u && y < 34u) {
                draw_char(326u, 26u, 80u, 200u, 200u, 200u);
                draw_char(332u, 26u, 58u, 200u, 200u, 200u);
                let cx = draw_number(338u, 26u, agent.pos_x, 200u, 200u, 200u);
                draw_char(cx, 26u, 44u, 200u, 200u, 200u);
                draw_number(cx + 6u, 26u, agent.pos_y, 200u, 200u, 200u);
            } else if (y >= 34u && y < 42u) {
                draw_char(326u, 34u, 86u, 100u, 255u, 100u);
                draw_char(332u, 34u, 58u, 100u, 255u, 100u);
                let cx = draw_signed_number(338u, 34u, agent.vel_x, 100u, 255u, 100u);
                draw_char(cx, 34u, 44u, 100u, 255u, 100u);
                draw_signed_number(cx + 6u, 34u, agent.vel_y, 100u, 255u, 100u);
            }
        }
        
        // Agent 5
        if (y >= 15u && y < 42u && x >= 402u && x < 478u) {
            let agent = agents[5u];
            let acolor = agent.color;
            let ar = (acolor >> 24u) & 0xFFu;
            let ag = (acolor >> 16u) & 0xFFu;
            let ab = (acolor >> 8u) & 0xFFu;
            
            if (y == 15u || y == 41u || x == 402u || x == 477u) {
                buffer_out[idx] = Pixel(ar, ag, ab, 255u);
            } else if (y >= 18u && y < 26u) {
                draw_char(406u, 18u, 53u, 255u, 255u, 255u);
                draw_char(412u, 18u, 58u, 150u, 150u, 150u);
                if (agent.is_it == 1u) {
                    draw_char(418u, 18u, 73u, 255u, 100u, 100u);
                    draw_char(424u, 18u, 84u, 255u, 100u, 100u);
                } else {
                    draw_char(418u, 18u, 82u, 100u, 255u, 100u);
                    draw_char(424u, 18u, 85u, 100u, 255u, 100u);
                    draw_char(430u, 18u, 78u, 100u, 255u, 100u);
                }
            } else if (y >= 26u && y < 34u) {
                draw_char(406u, 26u, 80u, 200u, 200u, 200u);
                draw_char(412u, 26u, 58u, 200u, 200u, 200u);
                let cx = draw_number(418u, 26u, agent.pos_x, 200u, 200u, 200u);
                draw_char(cx, 26u, 44u, 200u, 200u, 200u);
                draw_number(cx + 6u, 26u, agent.pos_y, 200u, 200u, 200u);
            } else if (y >= 34u && y < 42u) {
                draw_char(406u, 34u, 86u, 100u, 255u, 100u);
                draw_char(412u, 34u, 58u, 100u, 255u, 100u);
                let cx = draw_signed_number(418u, 34u, agent.vel_x, 100u, 255u, 100u);
                draw_char(cx, 34u, 44u, 100u, 255u, 100u);
                draw_signed_number(cx + 6u, 34u, agent.vel_y, 100u, 255u, 100u);
            }
        }
        
        // Agent 6
        if (y >= 15u && y < 42u && x >= 482u && x < 558u) {
            let agent = agents[6u];
            let acolor = agent.color;
            let ar = (acolor >> 24u) & 0xFFu;
            let ag = (acolor >> 16u) & 0xFFu;
            let ab = (acolor >> 8u) & 0xFFu;
            
            if (y == 15u || y == 41u || x == 482u || x == 557u) {
                buffer_out[idx] = Pixel(ar, ag, ab, 255u);
            } else if (y >= 18u && y < 26u) {
                draw_char(486u, 18u, 54u, 255u, 255u, 255u);
                draw_char(492u, 18u, 58u, 150u, 150u, 150u);
                if (agent.is_it == 1u) {
                    draw_char(498u, 18u, 73u, 255u, 100u, 100u);
                    draw_char(504u, 18u, 84u, 255u, 100u, 100u);
                } else {
                    draw_char(498u, 18u, 82u, 100u, 255u, 100u);
                    draw_char(504u, 18u, 85u, 100u, 255u, 100u);
                    draw_char(510u, 18u, 78u, 100u, 255u, 100u);
                }
            } else if (y >= 26u && y < 34u) {
                draw_char(486u, 26u, 80u, 200u, 200u, 200u);
                draw_char(492u, 26u, 58u, 200u, 200u, 200u);
                let cx = draw_number(498u, 26u, agent.pos_x, 200u, 200u, 200u);
                draw_char(cx, 26u, 44u, 200u, 200u, 200u);
                draw_number(cx + 6u, 26u, agent.pos_y, 200u, 200u, 200u);
            } else if (y >= 34u && y < 42u) {
                draw_char(486u, 34u, 86u, 100u, 255u, 100u);
                draw_char(492u, 34u, 58u, 100u, 255u, 100u);
                let cx = draw_signed_number(498u, 34u, agent.vel_x, 100u, 255u, 100u);
                draw_char(cx, 34u, 44u, 100u, 255u, 100u);
                draw_signed_number(cx + 6u, 34u, agent.vel_y, 100u, 255u, 100u);
            }
        }
        
        // Agent 7
        if (y >= 15u && y < 42u && x >= 562u && x < 638u) {
            let agent = agents[7u];
            let acolor = agent.color;
            let ar = (acolor >> 24u) & 0xFFu;
            let ag = (acolor >> 16u) & 0xFFu;
            let ab = (acolor >> 8u) & 0xFFu;
            
            if (y == 15u || y == 41u || x == 562u || x == 637u) {
                buffer_out[idx] = Pixel(ar, ag, ab, 255u);
            } else if (y >= 18u && y < 26u) {
                draw_char(566u, 18u, 55u, 255u, 255u, 255u);
                draw_char(572u, 18u, 58u, 150u, 150u, 150u);
                if (agent.is_it == 1u) {
                    draw_char(578u, 18u, 73u, 255u, 100u, 100u);
                    draw_char(584u, 18u, 84u, 255u, 100u, 100u);
                } else {
                    draw_char(578u, 18u, 82u, 100u, 255u, 100u);
                    draw_char(584u, 18u, 85u, 100u, 255u, 100u);
                    draw_char(590u, 18u, 78u, 100u, 255u, 100u);
                }
            } else if (y >= 26u && y < 34u) {
                draw_char(566u, 26u, 80u, 200u, 200u, 200u);
                draw_char(572u, 26u, 58u, 200u, 200u, 200u);
                let cx = draw_number(578u, 26u, agent.pos_x, 200u, 200u, 200u);
                draw_char(cx, 26u, 44u, 200u, 200u, 200u);
                draw_number(cx + 6u, 26u, agent.pos_y, 200u, 200u, 200u);
            } else if (y >= 34u && y < 42u) {
                draw_char(566u, 34u, 86u, 100u, 255u, 100u);
                draw_char(572u, 34u, 58u, 100u, 255u, 100u);
                let cx = draw_signed_number(578u, 34u, agent.vel_x, 100u, 255u, 100u);
                draw_char(cx, 34u, 44u, 100u, 255u, 100u);
                draw_signed_number(cx + 6u, 34u, agent.vel_y, 100u, 255u, 100u);
            }
        }
    }
    
    // ========================================
    // Main Framebuffer Area (rows 60+)
    // ========================================
    if (y >= 60u) {
        // Draw grid
        if (x % 50u == 0u || y % 50u == 0u) {
            buffer_out[idx] = Pixel(15u, 15u, 25u, 255u);
        }
        
        // Draw boundary
        if (x == 10u || x == config.width - 11u || y == 60u || y == config.height - 11u) {
            buffer_out[idx] = Pixel(50u, 50u, 100u, 255u);
        }
        
        // Draw agent trails and positions (unrolled for 8 agents)
        // Agent 0
        {
            let agent = agents[0u];
            let acolor = agent.color;
            let ar = (acolor >> 24u) & 0xFFu;
            let ag = (acolor >> 16u) & 0xFFu;
            let ab = (acolor >> 8u) & 0xFFu;
            
            // Trail points (check first 8)
            if (agent.trail_len > 0u) {
                let packed = agent.trail[0u];
                let tx = packed >> 16u;
                let ty = packed & 0xFFFFu;
                if (ty >= 60u && x == tx && y == ty) {
                    buffer_out[idx] = Pixel(ar/4u, ag/4u, ab/4u, 255u);
                }
            }
            if (agent.trail_len > 4u) {
                let packed = agent.trail[4u];
                let tx = packed >> 16u;
                let ty = packed & 0xFFFFu;
                if (ty >= 60u && x == tx && y == ty) {
                    buffer_out[idx] = Pixel(ar/2u, ag/2u, ab/2u, 255u);
                }
            }
            if (agent.trail_len > 8u) {
                let packed = agent.trail[8u];
                let tx = packed >> 16u;
                let ty = packed & 0xFFFFu;
                if (ty >= 60u && x == tx && y == ty) {
                    buffer_out[idx] = Pixel(ar*3u/4u, ag*3u/4u, ab*3u/4u, 255u);
                }
            }
            
            // Agent position (5x5)
            let ax = agent.pos_x;
            let ay = agent.pos_y;
            if (ay >= 60u && x >= ax && x < ax + 5u && y >= ay && y < ay + 5u) {
                if (agent.is_it == 1u) {
                    let pulse = 200u + ((config.frame % 30u) * 2u);
                    buffer_out[idx] = Pixel(pulse, pulse, pulse, 255u);
                } else {
                    buffer_out[idx] = Pixel(ar, ag, ab, 255u);
                }
            }
        }
        
        // Agent 1
        {
            let agent = agents[1u];
            let acolor = agent.color;
            let ar = (acolor >> 24u) & 0xFFu;
            let ag = (acolor >> 16u) & 0xFFu;
            let ab = (acolor >> 8u) & 0xFFu;
            
            if (agent.trail_len > 0u) {
                let packed = agent.trail[0u];
                let tx = packed >> 16u;
                let ty = packed & 0xFFFFu;
                if (ty >= 60u && x == tx && y == ty) {
                    buffer_out[idx] = Pixel(ar/4u, ag/4u, ab/4u, 255u);
                }
            }
            
            let ax = agent.pos_x;
            let ay = agent.pos_y;
            if (ay >= 60u && x >= ax && x < ax + 5u && y >= ay && y < ay + 5u) {
                if (agent.is_it == 1u) {
                    let pulse = 200u + ((config.frame % 30u) * 2u);
                    buffer_out[idx] = Pixel(pulse, pulse, pulse, 255u);
                } else {
                    buffer_out[idx] = Pixel(ar, ag, ab, 255u);
                }
            }
        }
        
        // Agent 2
        {
            let agent = agents[2u];
            let acolor = agent.color;
            let ar = (acolor >> 24u) & 0xFFu;
            let ag = (acolor >> 16u) & 0xFFu;
            let ab = (acolor >> 8u) & 0xFFu;
            
            if (agent.trail_len > 0u) {
                let packed = agent.trail[0u];
                let tx = packed >> 16u;
                let ty = packed & 0xFFFFu;
                if (ty >= 60u && x == tx && y == ty) {
                    buffer_out[idx] = Pixel(ar/4u, ag/4u, ab/4u, 255u);
                }
            }
            
            let ax = agent.pos_x;
            let ay = agent.pos_y;
            if (ay >= 60u && x >= ax && x < ax + 5u && y >= ay && y < ay + 5u) {
                if (agent.is_it == 1u) {
                    let pulse = 200u + ((config.frame % 30u) * 2u);
                    buffer_out[idx] = Pixel(pulse, pulse, pulse, 255u);
                } else {
                    buffer_out[idx] = Pixel(ar, ag, ab, 255u);
                }
            }
        }
        
        // Agent 3
        {
            let agent = agents[3u];
            let acolor = agent.color;
            let ar = (acolor >> 24u) & 0xFFu;
            let ag = (acolor >> 16u) & 0xFFu;
            let ab = (acolor >> 8u) & 0xFFu;
            
            if (agent.trail_len > 0u) {
                let packed = agent.trail[0u];
                let tx = packed >> 16u;
                let ty = packed & 0xFFFFu;
                if (ty >= 60u && x == tx && y == ty) {
                    buffer_out[idx] = Pixel(ar/4u, ag/4u, ab/4u, 255u);
                }
            }
            
            let ax = agent.pos_x;
            let ay = agent.pos_y;
            if (ay >= 60u && x >= ax && x < ax + 5u && y >= ay && y < ay + 5u) {
                if (agent.is_it == 1u) {
                    let pulse = 200u + ((config.frame % 30u) * 2u);
                    buffer_out[idx] = Pixel(pulse, pulse, pulse, 255u);
                } else {
                    buffer_out[idx] = Pixel(ar, ag, ab, 255u);
                }
            }
        }
        
        // Agent 4
        {
            let agent = agents[4u];
            let acolor = agent.color;
            let ar = (acolor >> 24u) & 0xFFu;
            let ag = (acolor >> 16u) & 0xFFu;
            let ab = (acolor >> 8u) & 0xFFu;
            
            if (agent.trail_len > 0u) {
                let packed = agent.trail[0u];
                let tx = packed >> 16u;
                let ty = packed & 0xFFFFu;
                if (ty >= 60u && x == tx && y == ty) {
                    buffer_out[idx] = Pixel(ar/4u, ag/4u, ab/4u, 255u);
                }
            }
            
            let ax = agent.pos_x;
            let ay = agent.pos_y;
            if (ay >= 60u && x >= ax && x < ax + 5u && y >= ay && y < ay + 5u) {
                if (agent.is_it == 1u) {
                    let pulse = 200u + ((config.frame % 30u) * 2u);
                    buffer_out[idx] = Pixel(pulse, pulse, pulse, 255u);
                } else {
                    buffer_out[idx] = Pixel(ar, ag, ab, 255u);
                }
            }
        }
        
        // Agent 5
        {
            let agent = agents[5u];
            let acolor = agent.color;
            let ar = (acolor >> 24u) & 0xFFu;
            let ag = (acolor >> 16u) & 0xFFu;
            let ab = (acolor >> 8u) & 0xFFu;
            
            if (agent.trail_len > 0u) {
                let packed = agent.trail[0u];
                let tx = packed >> 16u;
                let ty = packed & 0xFFFFu;
                if (ty >= 60u && x == tx && y == ty) {
                    buffer_out[idx] = Pixel(ar/4u, ag/4u, ab/4u, 255u);
                }
            }
            
            let ax = agent.pos_x;
            let ay = agent.pos_y;
            if (ay >= 60u && x >= ax && x < ax + 5u && y >= ay && y < ay + 5u) {
                if (agent.is_it == 1u) {
                    let pulse = 200u + ((config.frame % 30u) * 2u);
                    buffer_out[idx] = Pixel(pulse, pulse, pulse, 255u);
                } else {
                    buffer_out[idx] = Pixel(ar, ag, ab, 255u);
                }
            }
        }
        
        // Agent 6
        {
            let agent = agents[6u];
            let acolor = agent.color;
            let ar = (acolor >> 24u) & 0xFFu;
            let ag = (acolor >> 16u) & 0xFFu;
            let ab = (acolor >> 8u) & 0xFFu;
            
            if (agent.trail_len > 0u) {
                let packed = agent.trail[0u];
                let tx = packed >> 16u;
                let ty = packed & 0xFFFFu;
                if (ty >= 60u && x == tx && y == ty) {
                    buffer_out[idx] = Pixel(ar/4u, ag/4u, ab/4u, 255u);
                }
            }
            
            let ax = agent.pos_x;
            let ay = agent.pos_y;
            if (ay >= 60u && x >= ax && x < ax + 5u && y >= ay && y < ay + 5u) {
                if (agent.is_it == 1u) {
                    let pulse = 200u + ((config.frame % 30u) * 2u);
                    buffer_out[idx] = Pixel(pulse, pulse, pulse, 255u);
                } else {
                    buffer_out[idx] = Pixel(ar, ag, ab, 255u);
                }
            }
        }
        
        // Agent 7
        {
            let agent = agents[7u];
            let acolor = agent.color;
            let ar = (acolor >> 24u) & 0xFFu;
            let ag = (acolor >> 16u) & 0xFFu;
            let ab = (acolor >> 8u) & 0xFFu;
            
            if (agent.trail_len > 0u) {
                let packed = agent.trail[0u];
                let tx = packed >> 16u;
                let ty = packed & 0xFFFFu;
                if (ty >= 60u && x == tx && y == ty) {
                    buffer_out[idx] = Pixel(ar/4u, ag/4u, ab/4u, 255u);
                }
            }
            
            let ax = agent.pos_x;
            let ay = agent.pos_y;
            if (ay >= 60u && x >= ax && x < ax + 5u && y >= ay && y < ay + 5u) {
                if (agent.is_it == 1u) {
                    let pulse = 200u + ((config.frame % 30u) * 2u);
                    buffer_out[idx] = Pixel(pulse, pulse, pulse, 255u);
                } else {
                    buffer_out[idx] = Pixel(ar, ag, ab, 255u);
                }
            }
        }
    }
    
    // ========================================
    // Opcode Reference (bottom)
    // ========================================
    if (y >= config.height - 10u) {
        buffer_out[idx] = Pixel(10u, 10u, 20u, 255u);
        
        if (y >= config.height - 8u && y < config.height - 2u) {
            // Opcodes: $ SPAWN | p POS | > MOVE | x SENSE | ! PUNCH | ^ SEND | ? RECV
            let ref_y = config.height - 8u;
            
            draw_char(10u, ref_y, 36u, 100u, 150u, 200u);  // $
            draw_char(16u, ref_y, 80u, 80u, 80u, 80u);  // P
            draw_char(22u, ref_y, 58u, 80u, 80u, 80u);  // :
            
            draw_char(60u, ref_y, 112u, 100u, 150u, 200u);  // p
            draw_char(66u, ref_y, 80u, 80u, 80u, 80u);  // P
            draw_char(72u, ref_y, 58u, 80u, 80u, 80u);  // :
            
            draw_char(110u, ref_y, 62u, 100u, 150u, 200u);  // >
            draw_char(116u, ref_y, 77u, 80u, 80u, 80u);  // M
            draw_char(122u, ref_y, 58u, 80u, 80u, 80u);  // :
            
            draw_char(160u, ref_y, 120u, 100u, 150u, 200u);  // x
            draw_char(166u, ref_y, 83u, 80u, 80u, 80u);  // S
            draw_char(172u, ref_y, 58u, 80u, 80u, 80u);  // :
            
            draw_char(210u, ref_y, 33u, 100u, 150u, 200u);  // !
            draw_char(216u, ref_y, 67u, 80u, 80u, 80u);  // C
            draw_char(222u, ref_y, 58u, 80u, 80u, 80u);  // :
            
            draw_char(260u, ref_y, 94u, 100u, 150u, 200u);  // ^
            draw_char(266u, ref_y, 84u, 80u, 80u, 80u);  // T
            draw_char(272u, ref_y, 58u, 80u, 80u, 80u);  // :
            
            draw_char(310u, ref_y, 63u, 100u, 150u, 200u);  // ?
            draw_char(316u, ref_y, 82u, 80u, 80u, 80u);  // R
            draw_char(322u, ref_y, 58u, 80u, 80u, 80u);  // :
        }
    }
}
