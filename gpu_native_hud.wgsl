// ============================================================================
// GPU-Native HUD Shader — Real-Time Visual Proprioception
// ============================================================================
// The shader writes register state directly to framebuffer pixels.
// Zero-latency: Every frame displays current VM state.
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
@group(0) @binding(0) var<storage, read> buffer_in: array<Pixel>;
@group(0) @binding(1) var<storage, read_write> buffer_out: array<Pixel>;

// Register state (26 registers A-Z)
@group(0) @binding(2) var<storage, read> registers: array<u32>;
@group(0) @binding(3) var<storage, read> stack: array<u32>;
@group(0) @binding(4) var<uniform> config: Config;

// Stats (SP, IP, stack depth)
@group(0) @binding(5) var<storage, read> vm_stats: array<u32>;

// ============================================================================
// 5x7 BITMAP FONT — Embedded in shader
// ============================================================================

fn get_font_column(char_code: u32, col: u32) -> u32 {
    // Returns the column byte for a character
    // Each column has 7 bits (rows 0-6)
    
    // Digits 0-9 (char codes 48-57)
    if (char_code == 48u) {  // '0': 0x3E, 0x51, 0x49, 0x45, 0x3E
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x51u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x45u; }
        if (col == 4u) { return 0x3Eu; }
    } else if (char_code == 49u) {  // '1': 0x42, 0x7F, 0x40, 0x00, 0x00
        if (col == 0u) { return 0x42u; }
        if (col == 1u) { return 0x7Fu; }
        if (col == 2u) { return 0x40u; }
        return 0u;
    } else if (char_code == 50u) {  // '2': 0x62, 0x51, 0x49, 0x49, 0x46
        if (col == 0u) { return 0x62u; }
        if (col == 1u) { return 0x51u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x46u; }
    } else if (char_code == 51u) {  // '3': 0x22, 0x49, 0x49, 0x49, 0x36
        if (col == 0u) { return 0x22u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x36u; }
    } else if (char_code == 52u) {  // '4': 0x18, 0x14, 0x12, 0x7F, 0x10
        if (col == 0u) { return 0x18u; }
        if (col == 1u) { return 0x14u; }
        if (col == 2u) { return 0x12u; }
        if (col == 3u) { return 0x7Fu; }
        if (col == 4u) { return 0x10u; }
    } else if (char_code == 53u) {  // '5': 0x27, 0x45, 0x45, 0x45, 0x39
        if (col == 0u) { return 0x27u; }
        if (col == 1u) { return 0x45u; }
        if (col == 2u) { return 0x45u; }
        if (col == 3u) { return 0x45u; }
        if (col == 4u) { return 0x39u; }
    } else if (char_code == 54u) {  // '6': 0x3E, 0x49, 0x49, 0x49, 0x32
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x32u; }
    } else if (char_code == 55u) {  // '7': 0x01, 0x71, 0x09, 0x05, 0x03
        if (col == 0u) { return 0x01u; }
        if (col == 1u) { return 0x71u; }
        if (col == 2u) { return 0x09u; }
        if (col == 3u) { return 0x05u; }
        if (col == 4u) { return 0x03u; }
    } else if (char_code == 56u) {  // '8': 0x36, 0x49, 0x49, 0x49, 0x36
        if (col == 0u) { return 0x36u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x36u; }
    } else if (char_code == 57u) {  // '9': 0x26, 0x49, 0x49, 0x49, 0x3E
        if (col == 0u) { return 0x26u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x3Eu; }
    }
    // Letters A-Z (char codes 65-90)
    else if (char_code == 65u) {  // 'A': 0x7E, 0x11, 0x11, 0x11, 0x7E
        if (col == 0u) { return 0x7Eu; }
        if (col == 1u) { return 0x11u; }
        if (col == 2u) { return 0x11u; }
        if (col == 3u) { return 0x11u; }
        if (col == 4u) { return 0x7Eu; }
    } else if (char_code == 66u) {  // 'B': 0x7F, 0x49, 0x49, 0x49, 0x36
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x36u; }
    } else if (char_code == 67u) {  // 'C': 0x3E, 0x41, 0x41, 0x41, 0x22
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x41u; }
        if (col == 4u) { return 0x22u; }
    } else if (char_code == 68u) {  // 'D': 0x7F, 0x41, 0x41, 0x22, 0x1C
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x22u; }
        if (col == 4u) { return 0x1Cu; }
    } else if (char_code == 69u) {  // 'E': 0x7F, 0x49, 0x49, 0x49, 0x41
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x41u; }
    } else if (char_code == 70u) {  // 'F': 0x7F, 0x09, 0x09, 0x09, 0x01
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x09u; }
        if (col == 2u) { return 0x09u; }
        if (col == 3u) { return 0x09u; }
        if (col == 4u) { return 0x01u; }
    } else if (char_code == 71u) {  // 'G': 0x3E, 0x41, 0x49, 0x49, 0x7A
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x7Au; }
    } else if (char_code == 72u) {  // 'H': 0x7F, 0x08, 0x08, 0x08, 0x7F
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x7Fu; }
    } else if (char_code == 73u) {  // 'I': 0x41, 0x7F, 0x41
        if (col == 0u) { return 0x41u; }
        if (col == 1u) { return 0x7Fu; }
        if (col == 2u) { return 0x41u; }
        return 0u;
    } else if (char_code == 74u) {  // 'J': 0x20, 0x40, 0x41, 0x3F, 0x01
        if (col == 0u) { return 0x20u; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x3Fu; }
        if (col == 4u) { return 0x01u; }
    } else if (char_code == 75u) {  // 'K': 0x7F, 0x08, 0x14, 0x22, 0x41
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x14u; }
        if (col == 3u) { return 0x22u; }
        if (col == 4u) { return 0x41u; }
    } else if (char_code == 76u) {  // 'L': 0x7F, 0x40, 0x40, 0x40, 0x40
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x40u; }
    } else if (char_code == 77u) {  // 'M': 0x7F, 0x02, 0x0C, 0x02, 0x7F
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x02u; }
        if (col == 2u) { return 0x0Cu; }
        if (col == 3u) { return 0x02u; }
        if (col == 4u) { return 0x7Fu; }
    } else if (char_code == 78u) {  // 'N': 0x7F, 0x04, 0x08, 0x10, 0x7F
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x04u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x10u; }
        if (col == 4u) { return 0x7Fu; }
    } else if (char_code == 79u) {  // 'O': 0x3E, 0x41, 0x41, 0x41, 0x3E
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x41u; }
        if (col == 4u) { return 0x3Eu; }
    } else if (char_code == 80u) {  // 'P': 0x7F, 0x09, 0x09, 0x09, 0x06
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x09u; }
        if (col == 2u) { return 0x09u; }
        if (col == 3u) { return 0x09u; }
        if (col == 4u) { return 0x06u; }
    } else if (char_code == 81u) {  // 'Q': 0x3E, 0x41, 0x51, 0x21, 0x5E
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x51u; }
        if (col == 3u) { return 0x21u; }
        if (col == 4u) { return 0x5Eu; }
    } else if (char_code == 82u) {  // 'R': 0x7F, 0x09, 0x19, 0x29, 0x46
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x09u; }
        if (col == 2u) { return 0x19u; }
        if (col == 3u) { return 0x29u; }
        if (col == 4u) { return 0x46u; }
    } else if (char_code == 83u) {  // 'S': 0x26, 0x49, 0x49, 0x49, 0x32
        if (col == 0u) { return 0x26u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x32u; }
    } else if (char_code == 84u) {  // 'T': 0x01, 0x01, 0x7F, 0x01, 0x01
        if (col == 0u) { return 0x01u; }
        if (col == 1u) { return 0x01u; }
        if (col == 2u) { return 0x7Fu; }
        if (col == 3u) { return 0x01u; }
        if (col == 4u) { return 0x01u; }
    } else if (char_code == 85u) {  // 'U': 0x3F, 0x40, 0x40, 0x40, 0x3F
        if (col == 0u) { return 0x3Fu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x3Fu; }
    } else if (char_code == 86u) {  // 'V': 0x1F, 0x20, 0x40, 0x20, 0x1F
        if (col == 0u) { return 0x1Fu; }
        if (col == 1u) { return 0x20u; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x20u; }
        if (col == 4u) { return 0x1Fu; }
    } else if (char_code == 87u) {  // 'W': 0x3F, 0x40, 0x38, 0x40, 0x3F
        if (col == 0u) { return 0x3Fu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x38u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x3Fu; }
    } else if (char_code == 88u) {  // 'X': 0x63, 0x14, 0x08, 0x14, 0x63
        if (col == 0u) { return 0x63u; }
        if (col == 1u) { return 0x14u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x14u; }
        if (col == 4u) { return 0x63u; }
    } else if (char_code == 89u) {  // 'Y': 0x07, 0x08, 0x70, 0x08, 0x07
        if (col == 0u) { return 0x07u; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x70u; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x07u; }
    } else if (char_code == 90u) {  // 'Z': 0x61, 0x51, 0x49, 0x45, 0x43
        if (col == 0u) { return 0x61u; }
        if (col == 1u) { return 0x51u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x45u; }
        if (col == 4u) { return 0x43u; }
    }
    // Special characters
    else if (char_code == 58u) {  // ':'
        if (col == 1u) { return 0x36u; }
        if (col == 2u) { return 0x36u; }
        return 0u;
    } else if (char_code == 61u) {  // '='
        if (col == 1u) { return 0x7Fu; }
        if (col == 3u) { return 0x7Fu; }
        return 0u;
    } else if (char_code == 45u) {  // '-'
        if (col == 2u) { return 0x7Fu; }
        return 0u;
    }
    
    return 0u;
}

// Draw a character at position (x, y) in the framebuffer
// Returns the next x position after the character
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

// Draw a number (0-999) as 3 digits
fn draw_number(value: u32, x: u32, y: u32, color: Pixel) -> u32 {
    let hundreds = (value / 100u) % 10u;
    let tens = (value / 10u) % 10u;
    let ones = value % 10u;
    
    var cursor_x = x;
    cursor_x = draw_char(48u + hundreds, cursor_x, y, color);  // '0' + digit
    cursor_x = draw_char(48u + tens, cursor_x, y, color);
    cursor_x = draw_char(48u + ones, cursor_x, y, color);
    
    return cursor_x;
}

// ============================================================================
// HUD RENDERER — Writes register state to framebuffer
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
    bg_color.r = 20u;
    bg_color.g = 30u;
    bg_color.b = 40u;
    bg_color.a = 255u;
    
    // Clear HUD area (rows 0-7)
    var y = 0u;
    loop {
        if (y >= 8u) { break; }
        
        var x = 0u;
        loop {
            if (x >= config.width) { break; }
            
            let i = y * config.width + x;
            buffer_out[i] = bg_color;
            
            x += 1u;
        }
        
        y += 1u;
    }
    
    // Draw "REGISTERS:" header at row 0, col 20
    var cursor_x = 20u;
    cursor_x = draw_char(82u, cursor_x, 2u, header_color);   // R
    cursor_x = draw_char(69u, cursor_x, 2u, header_color);   // E
    cursor_x = draw_char(71u, cursor_x, 2u, header_color);   // G
    cursor_x = draw_char(73u, cursor_x, 2u, header_color);   // I
    cursor_x = draw_char(83u, cursor_x, 2u, header_color);   // S
    cursor_x = draw_char(84u, cursor_x, 2u, header_color);   // T
    cursor_x = draw_char(69u, cursor_x, 2u, header_color);   // E
    cursor_x = draw_char(82u, cursor_x, 2u, header_color);   // R
    cursor_x = draw_char(83u, cursor_x, 2u, header_color);   // S
    cursor_x = draw_char(58u, cursor_x, 2u, header_color);   // :
    
    // Draw register values (A-J) in 2 rows of 5
    cursor_x = 20u;
    var cursor_y = 14u;
    
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
        cursor_x += 10u;
        
        // Wrap to next row after 5 registers
        if (i == 4u) {
            cursor_x = 20u;
            cursor_y = 24u;
        }
        
        i += 1u;
    }
    
    // Draw "STACK:" at row 6
    cursor_x = 20u;
    cursor_y = 40u;
    cursor_x = draw_char(83u, cursor_x, cursor_y, header_color);   // S
    cursor_x = draw_char(84u, cursor_x, cursor_y, header_color);   // T
    cursor_x = draw_char(65u, cursor_x, cursor_y, header_color);   // A
    cursor_x = draw_char(67u, cursor_x, cursor_y, header_color);   // C
    cursor_x = draw_char(75u, cursor_x, cursor_y, header_color);   // K
    cursor_x = draw_char(58u, cursor_x, cursor_y, header_color);   // :
    
    // Draw stack depth
    let stack_depth = vm_stats[0u];
    cursor_x = draw_number(stack_depth, cursor_x + 10u, cursor_y, value_color);
    
    // Draw "IP:" and "SP:" at row 7
    cursor_x = 20u;
    cursor_y = 52u;
    cursor_x = draw_char(73u, cursor_x, cursor_y, header_color);   // I
    cursor_x = draw_char(80u, cursor_x, cursor_y, header_color);   // P
    cursor_x = draw_char(58u, cursor_x, cursor_y, header_color);   // :
    let ip = vm_stats[1u];
    cursor_x = draw_number(ip, cursor_x + 5u, cursor_y, value_color);
    
    cursor_x = 120u;
    cursor_x = draw_char(83u, cursor_x, cursor_y, header_color);   // S
    cursor_x = draw_char(80u, cursor_x, cursor_y, header_color);   // P
    cursor_x = draw_char(58u, cursor_x, cursor_y, header_color);   // :
    let sp = vm_stats[2u];
    cursor_x = draw_number(sp, cursor_x + 5u, cursor_y, value_color);
}

// ============================================================================
// MAIN COMPUTE SHADER
// ============================================================================

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    
    // First 64 threads render HUD
    if (idx < 64u) {
        render_hud();
        return;
    }
    
    // Rest of threads copy input to output (pass-through for now)
    if (idx < config.width * config.height) {
        buffer_out[idx] = buffer_in[idx];
    }
}
