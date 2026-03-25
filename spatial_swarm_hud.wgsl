// ============================================================================
// SPATIAL SWARM HUD SHADER — 64-AGENT COLLECTIVE
// ============================================================================
// Phase 7 Gamma: Hive Mind HUD
//   - 8×8 grid of mini-status tiles (one per agent)
//   - Each tile: ID, status (IT/RUN), tribe color, MSG flag
//   - Collective statistics: total messages, collisions, avg velocity
//   - Tribe clustering visualization
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
    tribe: u32,
    is_it: u32,
    message_waiting: u32,
    trail_len: u32,
    collision_count: u32,
    message_count: u32,
    _padding: u32,
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
// 5x7 BITMAP FONT (compact)
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
    } else if (char_code == 76u) {  // 'L'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x40u; }
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
    let col0 = get_font_column(char_code, 0u);
    let col1 = get_font_column(char_code, 1u);
    let col2 = get_font_column(char_code, 2u);
    let col3 = get_font_column(char_code, 3u);
    let col4 = get_font_column(char_code, 4u);
    
    // Column 0
    for r in 0u..7u {
        if ((col0 >> r) & 1u) != 0u {
            let idx = (start_y + r) * config.width + start_x;
            if (idx < arrayLength(&buffer_out)) {
                buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u);
            }
        }
    }
    
    // Column 1
    for r in 0u..7u {
        if ((col1 >> r) & 1u) != 0u {
            let idx = (start_y + r) * config.width + start_x + 1u;
            if (idx < arrayLength(&buffer_out)) {
                buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u);
            }
        }
    }
    
    // Column 2
    for r in 0u..7u {
        if ((col2 >> r) & 1u) != 0u {
            let idx = (start_y + r) * config.width + start_x + 2u;
            if (idx < arrayLength(&buffer_out)) {
                buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u);
            }
        }
    }
    
    // Column 3
    for r in 0u..7u {
        if ((col3 >> r) & 1u) != 0u {
            let idx = (start_y + r) * config.width + start_x + 3u;
            if (idx < arrayLength(&buffer_out)) {
                buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u);
            }
        }
    }
    
    // Column 4
    for r in 0u..7u {
        if ((col4 >> r) & 1u) != 0u {
            let idx = (start_y + r) * config.width + start_x + 4u;
            if (idx < arrayLength(&buffer_out)) {
                buffer_out[idx] = Pixel(color_r, color_g, color_b, 255u);
            }
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
    } else {
        draw_char(x, y, 48u + num / 100u, color_r, color_g, color_b);
        draw_char(x + 6u, y, 48u + (num / 10u) % 10u, color_r, color_g, color_b);
        draw_char(x + 12u, y, 48u + num % 10u, color_r, color_g, color_b);
        return x + 18u;
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

// Get tribe color from tribe ID (0-7)
fn get_tribe_color(tribe: u32) -> vec3<u32> {
    let tribe_colors = array<vec3<u32>, 8>(
        vec3<u32>(255u, 64u, 64u),    // Tribe 0: Red
        vec3<u32>(64u, 255u, 64u),    // Tribe 1: Green
        vec3<u32>(64u, 64u, 255u),    // Tribe 2: Blue
        vec3<u32>(255u, 255u, 64u),   // Tribe 3: Yellow
        vec3<u32>(255u, 64u, 255u),   // Tribe 4: Magenta
        vec3<u32>(64u, 255u, 255u),   // Tribe 5: Cyan
        vec3<u32>(255u, 128u, 64u),   // Tribe 6: Orange
        vec3<u32>(128u, 64u, 255u),   // Tribe 7: Purple
    );
    return tribe_colors[tribe % 8u];
}

// Draw mini tile for agent (compact 30x35)
fn draw_agent_tile(tile_x: u32, tile_y: u32, agent: AgentState) {
    let tribe_color = get_tribe_color(agent.tribe);
    let tr = tribe_color.x;
    let tg = tribe_color.y;
    let tb = tribe_color.z;
    
    // Tile border (tribe color)
    let border_color = Pixel(tr, tg, tb, 255u);
    
    // Draw border
    for dx in 0u..30u {
        let top_idx = tile_y * config.width + tile_x + dx;
        let bot_idx = (tile_y + 34u) * config.width + tile_x + dx;
        if (top_idx < arrayLength(&buffer_out)) { buffer_out[top_idx] = border_color; }
        if (bot_idx < arrayLength(&buffer_out)) { buffer_out[bot_idx] = border_color; }
    }
    for dy in 0u..35u {
        let left_idx = (tile_y + dy) * config.width + tile_x;
        let right_idx = (tile_y + dy) * config.width + tile_x + 29u;
        if (left_idx < arrayLength(&buffer_out)) { buffer_out[left_idx] = border_color; }
        if (right_idx < arrayLength(&buffer_out)) { buffer_out[right_idx] = border_color; }
    }
    
    // Agent ID (2 digits)
    let id_tens = agent.id / 10u;
    let id_ones = agent.id % 10u;
    draw_char(tile_x + 3u, tile_y + 3u, 48u + id_tens, 255u, 255u, 255u);
    draw_char(tile_x + 9u, tile_y + 3u, 48u + id_ones, 255u, 255u, 255u);
    
    // Status indicator (IT/RUN)
    if (agent.is_it == 1u) {
        draw_char(tile_x + 3u, tile_y + 12u, 73u, 255u, 100u, 100u);  // I
        draw_char(tile_x + 9u, tile_y + 12u, 84u, 255u, 100u, 100u);  // T
    } else {
        draw_char(tile_x + 3u, tile_y + 12u, 82u, 100u, 255u, 100u);  // R
    }
    
    // Tribe indicator (colored bar)
    for dx in 0u..24u {
        let idx = (tile_y + 22u) * config.width + tile_x + 3u + dx;
        if (idx < arrayLength(&buffer_out)) {
            buffer_out[idx] = Pixel(tr, tg, tb, 255u);
        }
    }
    
    // Message waiting indicator
    if (agent.message_waiting == 1u) {
        draw_char(tile_x + 20u, tile_y + 3u, 77u, 255u, 255u, 0u);  // M (yellow)
    }
    
    // Collision indicator (small dot if recent collision)
    if (agent.collision_count > 0u) {
        let dot_idx = (tile_y + 27u) * config.width + tile_x + 3u;
        if (dot_idx < arrayLength(&buffer_out)) {
            buffer_out[dot_idx] = Pixel(255u, 128u, 0u, 255u);
        }
    }
}

// Calculate collective stats
fn calculate_total_messages() -> u32 {
    var total = 0u;
    for (i in 0u..64u) {
        total += agents[i].message_count;
    }
    return total;
}

fn calculate_total_collisions() -> u32 {
    var total = 0u;
    for (i in 0u..64u) {
        total += agents[i].collision_count;
    }
    return total;
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
    // HUD Zone (top 95 rows)
    // ========================================
    if (y < 95u) {
        // Background
        buffer_out[idx] = Pixel(10u, 10u, 20u, 255u);
        
        // Title
        if (y >= 2u && y < 10u && x < 350u) {
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
            
            draw_char(94u, 2u, 45u, 150u, 150u, 150u);  // -
            
            draw_char(100u, 2u, 54u, 0u, 255u, 200u);  // 6
            draw_char(106u, 2u, 52u, 0u, 255u, 200u);  // 4
            
            draw_char(118u, 2u, 65u, 255u, 200u, 0u);  // A
            draw_char(124u, 2u, 71u, 255u, 200u, 0u);  // G
            draw_char(130u, 2u, 69u, 255u, 200u, 0u);  // E
            draw_char(136u, 2u, 78u, 255u, 200u, 0u);  // N
            draw_char(142u, 2u, 84u, 255u, 200u, 0u);  // T
            draw_char(148u, 2u, 83u, 255u, 200u, 0u);  // S
        }
        
        // Frame counter (top right)
        if (y >= 2u && y < 10u && x >= 1180u) {
            draw_char(1180u, 2u, 70u, 150u, 150u, 150u);  // F
            draw_char(1186u, 2u, 82u, 150u, 150u, 150u);  // R
            draw_char(1192u, 2u, 65u, 150u, 150u, 150u);  // A
            draw_char(1198u, 2u, 77u, 150u, 150u, 150u);  // M
            draw_char(1204u, 2u, 69u, 150u, 150u, 150u);  // E
            draw_char(1210u, 2u, 58u, 150u, 150u, 150u);  // :
            draw_number(1220u, 2u, config.frame, 150u, 150u, 150u);
        }
        
        // Collective stats (top right, row 2)
        if (y >= 12u && y < 20u && x >= 1050u) {
            let total_msgs = calculate_total_messages();
            let total_colls = calculate_total_collisions();
            
            draw_char(1050u, 12u, 77u, 255u, 255u, 0u);  // M
            draw_char(1056u, 12u, 83u, 255u, 255u, 0u);  // S
            draw_char(1062u, 12u, 71u, 255u, 255u, 0u);  // G
            draw_char(1068u, 12u, 58u, 255u, 255u, 0u);  // :
            draw_number(1078u, 12u, total_msgs, 255u, 255u, 0u);
            
            draw_char(1120u, 12u, 67u, 255u, 128u, 0u);  // C
            draw_char(1126u, 12u, 79u, 255u, 128u, 0u);  // O
            draw_char(1132u, 12u, 76u, 255u, 128u, 0u);  // L
            draw_char(1138u, 12u, 76u, 255u, 128u, 0u);  // L
            draw_char(1144u, 12u, 58u, 255u, 128u, 0u);  // :
            draw_number(1154u, 12u, total_colls, 255u, 128u, 0u);
        }
        
        // 8×8 Agent tile grid
        // Each tile: 30x35 pixels
        // Grid: 8 cols × 2 rows (wrapping)
        // Actually: single row of 8 columns, each containing 8 agents stacked
        
        // Layout: 4 rows of 16 agents each
        // Row 0: agents 0-15
        // Row 1: agents 16-31
        // Row 2: agents 32-47
        // Row 3: agents 48-63
        
        let tile_width = 32u;
        let tile_height = 36u;
        let grid_start_x = 10u;
        let grid_start_y = 22u;
        
        // Draw all 64 agent tiles
        for (agent_idx in 0u..64u) {
            let row = agent_idx / 16u;
            let col = agent_idx % 16u;
            
            let tile_x = grid_start_x + col * tile_width;
            let tile_y = grid_start_y + row * tile_height;
            
            // Check if current pixel is in this tile
            if (x >= tile_x && x < tile_x + 30u && y >= tile_y && y < tile_y + 35u) {
                draw_agent_tile(tile_x, tile_y, agents[agent_idx]);
            }
        }
        
        // Tribe legend (right side)
        if (x >= 1150u && x < 1270u && y >= 40u && y < 95u) {
            let legend_y = 45u;
            for (tribe in 0u..8u) {
                let tc = get_tribe_color(tribe);
                let ly = legend_y + tribe * 6u;
                
                // Color swatch
                if (y >= ly && y < ly + 5u && x >= 1150u && x < 1160u) {
                    buffer_out[idx] = Pixel(tc.x, tc.y, tc.z, 255u);
                }
                
                // Tribe label
                if (y >= ly && y < ly + 5u && x >= 1165u) {
                    draw_char(1165u, ly, 84u, tc.x, tc.y, tc.z);  // T
                    draw_char(1171u, ly, 58u, tc.x, tc.y, tc.z);  // :
                    draw_number(1177u, ly, tribe, tc.x, tc.y, tc.z);
                }
            }
        }
    }
    
    // ========================================
    // Main Framebuffer Area (rows 100+)
    // ========================================
    if (y >= 100u) {
        // Draw grid
        if (x % 50u == 0u || y % 50u == 0u) {
            buffer_out[idx] = Pixel(15u, 15u, 25u, 255u);
        }
        
        // Draw boundary
        if (x == 10u || x == config.width - 11u || y == 100u || y == config.height - 11u) {
            buffer_out[idx] = Pixel(50u, 50u, 100u, 255u);
        }
        
        // Draw all 64 agents (with trails)
        for (agent_idx in 0u..64u) {
            let agent = agents[agent_idx];
            let acolor = agent.color;
            let ar = (acolor >> 24u) & 0xFFu;
            let ag = (acolor >> 16u) & 0xFFu;
            let ab = (acolor >> 8u) & 0xFFu;
            
            // Trail (first point only for performance)
            if (agent.trail_len > 0u) {
                let packed = agent.trail[0u];
                let tx = packed >> 16u;
                let ty = packed & 0xFFFFu;
                if (ty >= 100u && x == tx && y == ty) {
                    buffer_out[idx] = Pixel(ar/4u, ag/4u, ab/4u, 255u);
                }
            }
            
            // Agent position (3x3 for compactness with 64 agents)
            let ax = agent.pos_x;
            let ay = agent.pos_y;
            if (ay >= 100u && x >= ax && x < ax + 3u && y >= ay && y < ay + 3u) {
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
            let ref_y = config.height - 8u;
            
            // Compact opcode display
            draw_char(10u, ref_y, 36u, 100u, 150u, 200u);  // $
            draw_char(16u, ref_y, 112u, 80u, 80u, 80u);    // p
            draw_char(28u, ref_y, 62u, 100u, 150u, 200u);  // >
            draw_char(34u, ref_y, 109u, 80u, 80u, 80u);    // m
            draw_char(46u, ref_y, 120u, 100u, 150u, 200u); // x
            draw_char(52u, ref_y, 115u, 80u, 80u, 80u);    // s
            draw_char(64u, ref_y, 33u, 100u, 150u, 200u);  // !
            draw_char(70u, ref_y, 99u, 80u, 80u, 80u);     // c
            draw_char(82u, ref_y, 94u, 100u, 150u, 200u);  // ^
            draw_char(88u, ref_y, 116u, 80u, 80u, 80u);    // t
            draw_char(100u, ref_y, 63u, 100u, 150u, 200u); // ?
            draw_char(106u, ref_y, 114u, 80u, 80u, 80u);   // r
        }
    }
}
