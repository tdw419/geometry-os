// ============================================================================
// SPATIAL PHYSICS HUD SHADER
// ============================================================================
// Displays:
//   - Agent position (POS_X, POS_Y)
//   - Velocity vectors (VEL_X, VEL_Y)
//   - Agent trail (last N positions)
//   - Collision indicator
//   - Output framebuffer content
// ============================================================================

struct Pixel {
    r: u32,
    g: u32,
    b: u32,
    a: u32,
}

struct AgentState {
    pos_x: u32,
    pos_y: u32,
    vel_x: i32,
    vel_y: i32,
    collision: u32,
    trail_len: u32,
    _padding: vec2<u32>,
    trail: array<u32, 32>  // Packed (x << 16 | y)
}

struct Config {
    width: u32,
    height: u32,
    time: f32,
    frame: u32,
    mode: u32,
}

@group(0) @binding(0) var<storage, read_write> buffer_out: array<Pixel>;
@group(0) @binding(1) var<storage, read> agent: AgentState;
@group(0) @binding(2) var<uniform> config: Config;
@group(0) @binding(3) var<storage, read> trail_data: array<u32>;

// ============================================================================
// 5x7 BITMAP FONT - Minimal set
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
    // Letters A-Z
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
    } else if (char_code == 70u) {  // 'F'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x09u; }
        if (col == 2u) { return 0x09u; }
        if (col == 3u) { return 0x09u; }
        if (col == 4u) { return 0x01u; }
    } else if (char_code == 73u) {  // 'I'
        if (col == 0u) { return 0x41u; }
        if (col == 1u) { return 0x7Fu; }
        if (col == 2u) { return 0x41u; }
        return 0u;
    } else if (char_code == 76u) {  // 'L'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x40u; }
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
    }
    // Special characters
    else if (char_code == 32u) {  // ' ' (space)
        return 0u;
    } else if (char_code == 43u) {  // '+'
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
    }
    
    return 0u;
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn draw_char(start_x: u32, start_y: u32, char_code: u32, color_r: u32, color_g: u32, color_b: u32) {
    // Draw 5x7 character unrolled
    let columns = array<u32, 5>(0u, 1u, 2u, 3u, 4u);
    
    // col 0
    let col0 = get_font_column(char_code, 0u);
    if ((col0 & 1u) != 0u) { 
        let idx = start_y * config.width + start_x;
        if (idx < arrayLength(&buffer_out)) {
            buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u);
        }
    }
    if ((col0 & 2u) != 0u) { 
        let idx = (start_y + 1u) * config.width + start_x;
        if (idx < arrayLength(&buffer_out)) {
            buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u);
        }
    }
    if ((col0 & 4u) != 0u) { 
        let idx = (start_y + 2u) * config.width + start_x;
        if (idx < arrayLength(&buffer_out)) {
            buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u);
        }
    }
    if ((col0 & 8u) != 0u) { 
        let idx = (start_y + 3u) * config.width + start_x;
        if (idx < arrayLength(&buffer_out)) {
            buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u);
        }
    }
    if ((col0 & 16u) != 0u) { 
        let idx = (start_y + 4u) * config.width + start_x;
        if (idx < arrayLength(&buffer_out)) {
            buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u);
        }
    }
    if ((col0 & 32u) != 0u) { 
        let idx = (start_y + 5u) * config.width + start_x;
        if (idx < arrayLength(&buffer_out)) {
            buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u);
        }
    }
    if ((col0 & 64u) != 0u) { 
        let idx = (start_y + 6u) * config.width + start_x;
        if (idx < arrayLength(&buffer_out)) {
            buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u);
        }
    }
    
    // col 1
    let col1 = get_font_column(char_code, 1u);
    if ((col1 & 1u) != 0u) { 
        let idx = start_y * config.width + start_x + 1u;
        if (idx < arrayLength(&buffer_out)) {
            buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u);
        }
    }
    if ((col1 & 2u) != 0u) { 
        let idx = (start_y + 1u) * config.width + start_x + 1u;
        if (idx < arrayLength(&buffer_out)) {
            buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u);
        }
    }
    if ((col1 & 4u) != 0u) { 
        let idx = (start_y + 2u) * config.width + start_x + 1u;
        if (idx < arrayLength(&buffer_out)) {
            buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u);
        }
    }
    if ((col1 & 8u) != 0u) { 
        let idx = (start_y + 3u) * config.width + start_x + 1u;
        if (idx < arrayLength(&buffer_out)) {
            buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u);
        }
    }
    if ((col1 & 16u) != 0u) { 
        let idx = (start_y + 4u) * config.width + start_x + 1u;
        if (idx < arrayLength(&buffer_out)) {
            buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u);
        }
    }
    if ((col1 & 32u) != 0u) { 
        let idx = (start_y + 5u) * config.width + start_x + 1u;
        if (idx < arrayLength(&buffer_out)) {
            buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u);
        }
    }
    if ((col1 & 64u) != 0u) { 
        let idx = (start_y + 6u) * config.width + start_x + 1u;
        if (idx < arrayLength(&buffer_out)) {
            buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u);
        }
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
    
    // Draw grid (faint)
    if (x % 50u == 0u || y % 50u == 0u) {
        buffer_out[idx] = Pixel(20u, 20u, 30u, 255u);
    }
    
    // Draw boundary box (yellow)
    let margin = 10u;
    if (x >= margin && x < config.width - margin && 
        y >= margin && y < config.height - margin) {
        if (x == margin || x == config.width - margin - 1u ||
            y == margin || y == config.height - margin - 1u) {
            buffer_out[idx] = Pixel(255u, 255u, 0u, 255u);
        }
    }
    
    // Draw agent trail (cyan, fading)
    let trail_len = agent.trail_len;
    if (trail_len > 0u) {
        // Unrolled check for first 8 trail points
        if (trail_len > 0u) {
            let packed = agent.trail[0u];
            let tx = packed >> 16u;
            let ty = packed & 0xFFFFu;
            if (x == tx && y == ty) {
                buffer_out[idx] = Pixel(0u, 50u, 50u, 255u);
            }
        }
        if (trail_len > 4u) {
            let packed = agent.trail[4u];
            let tx = packed >> 16u;
            let ty = packed & 0xFFFFu;
            if (x == tx && y == ty) {
                buffer_out[idx] = Pixel(0u, 100u, 100u, 255u);
            }
        }
        if (trail_len > 8u) {
            let packed = agent.trail[8u];
            let tx = packed >> 16u;
            let ty = packed & 0xFFFFu;
            if (x == tx && y == ty) {
                buffer_out[idx] = Pixel(0u, 150u, 150u, 255u);
            }
        }
        if (trail_len > 16u) {
            let packed = agent.trail[16u];
            let tx = packed >> 16u;
            let ty = packed & 0xFFFFu;
            if (x == tx && y == ty) {
                buffer_out[idx] = Pixel(0u, 180u, 180u, 255u);
            }
        }
        if (trail_len > 24u) {
            let packed = agent.trail[24u];
            let tx = packed >> 16u;
            let ty = packed & 0xFFFFu;
            if (x == tx && y == ty) {
                buffer_out[idx] = Pixel(0u, 200u, 200u, 255u);
            }
        }
        if (trail_len > 31u && trail_len <= 32u) {
            let packed = agent.trail[31u];
            let tx = packed >> 16u;
            let ty = packed & 0xFFFFu;
            if (x == tx && y == ty) {
                buffer_out[idx] = Pixel(0u, 220u, 220u, 255u);
            }
        }
    }
    
    // Draw agent current position (white, 3x3)
    let ax = agent.pos_x;
    let ay = agent.pos_y;
    if (x >= ax && x < ax + 3u && y >= ay && y < ay + 3u) {
        if (agent.collision == 1u) {
            buffer_out[idx] = Pixel(255u, 0u, 100u, 255u);  // Red on collision
        } else {
            buffer_out[idx] = Pixel(255u, 255u, 255u, 255u);  // White normally
        }
    }
    
    // Draw velocity vector (green line from agent)
    if (agent.vel_x != 0 || agent.vel_y != 0) {
        let vx = agent.vel_x;
        let vy = agent.vel_y;
        
        // Draw line points unrolled
        // t=2
        {
            let line_x = ax + 1u + u32(i32(2u) * vx / 5);
            let line_y = ay + 1u + u32(i32(2u) * vy / 5);
            if (x == line_x && y == line_y) {
                buffer_out[idx] = Pixel(0u, 255u, 100u, 255u);
            }
        }
        // t=4
        {
            let line_x = ax + 1u + u32(i32(4u) * vx / 5);
            let line_y = ay + 1u + u32(i32(4u) * vy / 5);
            if (x == line_x && y == line_y) {
                buffer_out[idx] = Pixel(0u, 255u, 100u, 255u);
            }
        }
        // t=6
        {
            let line_x = ax + 1u + u32(i32(6u) * vx / 5);
            let line_y = ay + 1u + u32(i32(6u) * vy / 5);
            if (x == line_x && y == line_y) {
                buffer_out[idx] = Pixel(0u, 255u, 100u, 255u);
            }
        }
        // t=8
        {
            let line_x = ax + 1u + u32(i32(8u) * vx / 5);
            let line_y = ay + 1u + u32(i32(8u) * vy / 5);
            if (x == line_x && y == line_y) {
                buffer_out[idx] = Pixel(0u, 255u, 100u, 255u);
            }
        }
    }
    
    // ========================================
    // HUD Zone (top 50 rows)
    // ========================================
    if (y < 50u) {
        // Background
        buffer_out[idx] = Pixel(15u, 15u, 25u, 255u);
        
        // Draw "SPATIAL PHYSICS" header
        if (y >= 3u && y < 10u && x < 120u) {
            draw_char(10u, 3u, 83u, 0u, 255u, 200u);  // S
            draw_char(16u, 3u, 80u, 0u, 255u, 200u);  // P
            draw_char(22u, 3u, 65u, 0u, 255u, 200u);  // A
            draw_char(28u, 3u, 84u, 0u, 255u, 200u);  // T
            draw_char(34u, 3u, 73u, 0u, 255u, 200u);  // I
            draw_char(40u, 3u, 65u, 0u, 255u, 200u);  // A
            draw_char(46u, 3u, 76u, 0u, 255u, 200u);  // L
            
            draw_char(58u, 3u, 80u, 0u, 255u, 200u);  // P
            draw_char(64u, 3u, 72u, 0u, 255u, 200u);  // H
            draw_char(70u, 3u, 89u, 0u, 255u, 200u);  // Y
            draw_char(76u, 3u, 83u, 0u, 255u, 200u);  // S
            draw_char(82u, 3u, 73u, 0u, 255u, 200u);  // I
            draw_char(88u, 3u, 67u, 0u, 255u, 200u);  // C
            draw_char(94u, 3u, 83u, 0u, 255u, 200u);  // S
        }
        
        // Draw POS
        if (y >= 22u && y < 30u && x < 150u) {
            draw_char(10u, 22u, 80u, 255u, 255u, 255u);  // P
            draw_char(16u, 22u, 79u, 255u, 255u, 255u);  // O
            draw_char(22u, 22u, 83u, 255u, 255u, 255u);  // S
            draw_char(28u, 22u, 58u, 255u, 255u, 255u);  // :
            let cx = draw_number(40u, 22u, agent.pos_x, 255u, 255u, 255u);
            draw_char(cx, 22u, 44u, 255u, 255u, 255u);  // ,
            draw_number(cx + 6u, 22u, agent.pos_y, 255u, 255u, 255u);
        }
        
        // Draw VEL
        if (y >= 34u && y < 42u && x < 150u) {
            draw_char(10u, 34u, 86u, 0u, 255u, 100u);  // V
            draw_char(16u, 34u, 69u, 0u, 255u, 100u);  // E
            draw_char(22u, 34u, 76u, 0u, 255u, 100u);  // L
            draw_char(28u, 34u, 58u, 0u, 255u, 100u);  // :
            
            // Signed velocity X
            var cx = 40u;
            if (agent.vel_x >= 0) {
                draw_char(cx, 34u, 43u, 0u, 255u, 100u);  // +
                cx = cx + 6u;
                cx = draw_number(cx, 34u, u32(agent.vel_x), 0u, 255u, 100u);
            } else {
                draw_char(cx, 34u, 45u, 0u, 255u, 100u);  // -
                cx = cx + 6u;
                cx = draw_number(cx, 34u, u32(-agent.vel_x), 0u, 255u, 100u);
            }
            
            draw_char(cx, 34u, 44u, 0u, 255u, 100u);  // ,
            
            // Signed velocity Y
            cx = cx + 6u;
            if (agent.vel_y >= 0) {
                draw_char(cx, 34u, 43u, 0u, 255u, 100u);  // +
                cx = cx + 6u;
                draw_number(cx, 34u, u32(agent.vel_y), 0u, 255u, 100u);
            } else {
                draw_char(cx, 34u, 45u, 0u, 255u, 100u);  // -
                cx = cx + 6u;
                draw_number(cx, 34u, u32(-agent.vel_y), 0u, 255u, 100u);
            }
        }
        
        // Draw collision indicator
        if (agent.collision == 1u && y >= 22u && y < 42u && x >= 200u && x < 300u) {
            draw_char(200u, 28u, 67u, 255u, 0u, 100u);  // C
            draw_char(206u, 28u, 79u, 255u, 0u, 100u);  // O
            draw_char(212u, 28u, 76u, 255u, 0u, 100u);  // L
            draw_char(218u, 28u, 76u, 255u, 0u, 100u);  // L
            draw_char(224u, 28u, 73u, 255u, 0u, 100u);  // I
            draw_char(230u, 28u, 83u, 255u, 0u, 100u);  // S
            draw_char(236u, 28u, 73u, 255u, 0u, 100u);  // I
            draw_char(242u, 28u, 79u, 255u, 0u, 100u);  // O
            draw_char(248u, 28u, 78u, 255u, 0u, 100u);  // N
        }
        
        // Draw trail length
        if (y >= 22u && y < 30u && x >= 320u && x < 420u) {
            draw_char(320u, 22u, 84u, 0u, 200u, 255u);  // T
            draw_char(326u, 22u, 82u, 0u, 200u, 255u);  // R
            draw_char(332u, 22u, 65u, 0u, 200u, 255u);  // A
            draw_char(338u, 22u, 73u, 0u, 200u, 255u);  // I
            draw_char(344u, 22u, 76u, 0u, 200u, 255u);  // L
            draw_char(350u, 22u, 58u, 0u, 200u, 255u);  // :
            draw_number(362u, 22u, agent.trail_len, 0u, 200u, 255u);
        }
        
        // Draw frame counter
        if (y >= 34u && y < 42u && x >= 320u && x < 420u) {
            draw_char(320u, 34u, 70u, 150u, 150u, 150u);  // F
            draw_char(326u, 34u, 82u, 150u, 150u, 150u);  // R
            draw_char(332u, 34u, 65u, 150u, 150u, 150u);  // A
            draw_char(338u, 34u, 77u, 150u, 150u, 150u);  // M
            draw_char(344u, 34u, 69u, 150u, 150u, 150u);  // E
            draw_char(350u, 34u, 58u, 150u, 150u, 150u);  // :
            draw_number(362u, 34u, config.frame, 150u, 150u, 150u);
        }
    }
    
    // ========================================
    // Opcodes reference (bottom 20 rows)
    // ========================================
    if (y >= config.height - 20u) {
        // Background
        buffer_out[idx] = Pixel(10u, 10u, 20u, 255u);
        
        // Draw opcodes reference
        if (y >= config.height - 16u && y < config.height - 8u && x < 350u) {
            draw_char(10u, config.height - 16u, 112u, 100u, 100u, 150u);  // p
            draw_char(16u, config.height - 16u, 32u, 100u, 100u, 150u);   // space
            draw_char(22u, config.height - 16u, 80u, 100u, 100u, 150u);  // P
            draw_char(28u, config.height - 16u, 79u, 100u, 100u, 150u);  // O
            draw_char(34u, config.height - 16u, 83u, 100u, 100u, 150u);  // S
            
            draw_char(50u, config.height - 16u, 62u, 100u, 100u, 150u);  // >
            draw_char(56u, config.height - 16u, 32u, 100u, 100u, 150u);  // space
            draw_char(62u, config.height - 16u, 77u, 100u, 100u, 150u);  // M
            draw_char(68u, config.height - 16u, 79u, 100u, 100u, 150u);  // O
            draw_char(74u, config.height - 16u, 86u, 100u, 100u, 150u);  // V
            draw_char(80u, config.height - 16u, 69u, 100u, 100u, 150u);  // E
            
            draw_char(100u, config.height - 16u, 120u, 100u, 100u, 150u);  // x
            draw_char(106u, config.height - 16u, 32u, 100u, 100u, 150u);  // space
            draw_char(112u, config.height - 16u, 83u, 100u, 100u, 150u);  // S
            draw_char(118u, config.height - 16u, 69u, 100u, 100u, 150u);  // E
            draw_char(124u, config.height - 16u, 78u, 100u, 100u, 150u);  // N
            draw_char(130u, config.height - 16u, 83u, 100u, 100u, 150u);  // S
            draw_char(136u, config.height - 16u, 69u, 100u, 100u, 150u);  // E
            
            draw_char(160u, config.height - 16u, 33u, 100u, 100u, 150u);  // !
            draw_char(166u, config.height - 16u, 32u, 100u, 100u, 150u);  // space
            draw_char(172u, config.height - 16u, 80u, 100u, 100u, 150u);  // P
            draw_char(178u, config.height - 16u, 85u, 100u, 100u, 150u);  // U
            draw_char(184u, config.height - 16u, 78u, 100u, 100u, 150u);  // N
            draw_char(190u, config.height - 16u, 67u, 100u, 100u, 150u);  // C
            draw_char(196u, config.height - 16u, 72u, 100u, 100u, 150u);  // H
        }
    }
}
