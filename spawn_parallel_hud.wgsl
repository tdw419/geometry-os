// ============================================================================
// SPAWN Parallel HUD Shader — Multi-Agent Visual Telemetry
// ============================================================================
// Renders HUD for each active thread at its designated row offset
// ============================================================================

struct Pixel {
    r: u32,
    g: u32,
    b: u32,
    a: u32,
}

struct ThreadState {
    is_active: u32,
    ip: u32,
    sp: u32,
    row_offset: u32,
    _padding: u32,
    registers: array<u32, 26>,
    stack: array<u32, 32>,
}

struct Config {
    width: u32,
    height: u32,
    time: f32,
    frame: u32,
    mode: u32,
}

@group(0) @binding(0) var<storage, read_write> buffer_out: array<Pixel>;
@group(0) @binding(1) var<storage, read> threads: array<ThreadState>;
@group(0) @binding(2) var<uniform> config: Config;

// 5x7 bitmap font (same as gpu_native_hud.wgsl)
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
    } else if (char_code == 35u) {  // '#'
        if (col == 0u) { return 0x14u; }
        if (col == 1u) { return 0x7Fu; }
        if (col == 2u) { return 0x14u; }
        if (col == 3u) { return 0x7Fu; }
        if (col == 4u) { return 0x14u; }
    }
    
    return 0u;
}

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
    
    return x + 6u;
}

fn draw_number(value: u32, x: u32, y: u32, color: Pixel) -> u32 {
    let hundreds = (value / 100u) % 10u;
    let tens = (value / 10u) % 10u;
    let ones = value % 10u;
    
    var cursor_x = x;
    cursor_x = draw_char(48u + hundreds, cursor_x, y, color);
    cursor_x = draw_char(48u + tens, cursor_x, y, color);
    cursor_x = draw_char(48u + ones, cursor_x, y, color);
    
    return cursor_x;
}

// Render HUD for a single thread
fn render_thread_hud(thread_idx: u32, thread: ThreadState) {
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
    
    var thread_color: Pixel;
    if (thread_idx == 0u) {
        thread_color.r = 100u;
        thread_color.g = 255u;
        thread_color.b = 100u;
        thread_color.a = 255u;
    } else {
        thread_color.r = 255u;
        thread_color.g = 200u;
        thread_color.b = 100u;
        thread_color.a = 255u;
    }
    
    let base_y = thread.row_offset;
    
    // Draw thread header: "#0:" or "#1:"
    var cursor_x = 20u;
    cursor_x = draw_char(35u, cursor_x, base_y + 2u, thread_color);  // '#'
    cursor_x = draw_char(48u + thread_idx, cursor_x, base_y + 2u, thread_color);  // thread number
    cursor_x = draw_char(58u, cursor_x, base_y + 2u, thread_color);  // ':'
    
    // Draw registers A-J (unrolled to avoid dynamic indexing)
    cursor_x = 60u;
    
    // A
    cursor_x = draw_char(65u, cursor_x, base_y + 2u, header_color);
    cursor_x = draw_char(58u, cursor_x, base_y + 2u, header_color);
    cursor_x = draw_number(thread.registers[0u], cursor_x, base_y + 2u, value_color);
    cursor_x += 8u;
    
    // B
    cursor_x = draw_char(66u, cursor_x, base_y + 2u, header_color);
    cursor_x = draw_char(58u, cursor_x, base_y + 2u, header_color);
    cursor_x = draw_number(thread.registers[1u], cursor_x, base_y + 2u, value_color);
    cursor_x += 8u;
    
    // C
    cursor_x = draw_char(67u, cursor_x, base_y + 2u, header_color);
    cursor_x = draw_char(58u, cursor_x, base_y + 2u, header_color);
    cursor_x = draw_number(thread.registers[2u], cursor_x, base_y + 2u, value_color);
    cursor_x += 8u;
    
    // D
    cursor_x = draw_char(68u, cursor_x, base_y + 2u, header_color);
    cursor_x = draw_char(58u, cursor_x, base_y + 2u, header_color);
    cursor_x = draw_number(thread.registers[3u], cursor_x, base_y + 2u, value_color);
    cursor_x += 8u;
    
    // E
    cursor_x = draw_char(69u, cursor_x, base_y + 2u, header_color);
    cursor_x = draw_char(58u, cursor_x, base_y + 2u, header_color);
    cursor_x = draw_number(thread.registers[4u], cursor_x, base_y + 2u, value_color);
    cursor_x += 8u;
    
    // Draw IP and SP on second line
    cursor_x = 20u;
    cursor_x = draw_char(73u, cursor_x, base_y + 12u, header_color);  // I
    cursor_x = draw_char(80u, cursor_x, base_y + 12u, header_color);  // P
    cursor_x = draw_char(58u, cursor_x, base_y + 12u, header_color);  // :
    cursor_x = draw_number(thread.ip, cursor_x + 5u, base_y + 12u, value_color);
    
    cursor_x = 100u;
    cursor_x = draw_char(83u, cursor_x, base_y + 12u, header_color);  // S
    cursor_x = draw_char(80u, cursor_x, base_y + 12u, header_color);  // P
    cursor_x = draw_char(58u, cursor_x, base_y + 12u, header_color);  // :
    cursor_x = draw_number(thread.sp, cursor_x + 5u, base_y + 12u, value_color);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    
    // First 64 threads render HUDs
    if (idx < 64u) {
        // Clear background
        var y = 0u;
        loop {
            if (y >= config.height) { break; }
            
            var x = 0u;
            loop {
                if (x >= config.width) { break; }
                
                let i = y * config.width + x;
                buffer_out[i].r = 15u;
                buffer_out[i].g = 25u;
                buffer_out[i].b = 35u;
                buffer_out[i].a = 255u;
                
                x += 1u;
            }
            
            y += 1u;
        }
        
        // Render HUD for each active thread
        var thread_idx = 0u;
        loop {
            if (thread_idx >= 8u) { break; }
            
            let thread = threads[thread_idx];
            if (thread.is_active == 1u) {
                render_thread_hud(thread_idx, thread);
            }
            
            thread_idx += 1u;
        }
    }
}
