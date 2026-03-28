// Telemetry HUD Renderer - GlyphLang metrics at Rows 410-419
// This file is concatenated with sovereign_shell_hud.wgsl

fn render_telemetry_hud(row: u32, col: u32, width: u32) -> vec3<u32> {
    // Only render in telemetry zone (Rows 410-419)
    if (row < 410u || row >= 420u) {
        return vec3<u32>(0u, 0u, 0u);
    }
    
    // Read telemetry data from vm_stats
    let reqs = vm_stats[3u];
    let errs = vm_stats[4u];
    let lat_fixed = vm_stats[5u];
    let routes = vm_stats[6u];
    
    // Convert latency to ms
    let lat_ms = lat_fixed / 10u;
    
    // Calculate health percentage
    var health_pct: u32 = 100u;
    if (reqs > 0u) {
        health_pct = 100u - (errs * 100u / reqs);
    }
    
    // Row 410: "G-LANG: {reqs} ERR:{errs}"
    if (row == 410u) {
        var cursor_x: u32 = 20u;
        
        // Draw "G-LANG:" label in cyan
        cursor_x = draw_char(71u, cursor_x, row, get_color_cyan());   // G
        cursor_x = draw_char(45u, cursor_x, row, get_color_cyan());   // -
        cursor_x = draw_char(76u, cursor_x, row, get_color_cyan());   // L
        cursor_x = draw_char(65u, cursor_x, row, get_color_cyan());   // A
        cursor_x = draw_char(78u, cursor_x, row, get_color_cyan());   // N
        cursor_x = draw_char(71u, cursor_x, row, get_color_cyan());   // G
        cursor_x = draw_char(58u, cursor_x, row, get_color_cyan());   // :
        cursor_x += 5u;
        
        // Draw request count
        cursor_x = draw_number(reqs, cursor_x, row, get_color_white());
        
        // Draw errors if any (in red)
        if (errs > 0u) {
            cursor_x += 10u;
            cursor_x = draw_char(69u, cursor_x, row, get_color_red());   // E
            cursor_x = draw_char(82u, cursor_x, row, get_color_red());   // R
            cursor_x = draw_char(82u, cursor_x, row, get_color_red());   // R
            cursor_x = draw_char(58u, cursor_x, row, get_color_red());   // :
            cursor_x = draw_number(errs, cursor_x, row, get_color_red());
        }
        
        return vec3<u32>(0u, 0u, 0u);
    }
    
    // Row 411: "LAT:{lat_ms}ms ROUTES:{active}"
    if (row == 411u) {
        var cursor_x: u32 = 20u;
        
        cursor_x = draw_char(76u, cursor_x, row, get_color_cyan());   // L
        cursor_x = draw_char(65u, cursor_x, row, get_color_cyan());   // A
        cursor_x = draw_char(84u, cursor_x, row, get_color_cyan());   // T
        cursor_x = draw_char(58u, cursor_x, row, get_color_cyan());   // :
        cursor_x += 5u;
        
        cursor_x = draw_number(lat_ms, cursor_x, row, get_color_white());
        cursor_x = draw_char(109u, cursor_x, row, get_color_white());  // m
        cursor_x = draw_char(115u, cursor_x, row, get_color_white());  // s
        cursor_x += 15u;
        
        cursor_x = draw_char(82u, cursor_x, row, get_color_cyan());   // R
        cursor_x = draw_char(79u, cursor_x, row, get_color_cyan());   // O
        cursor_x = draw_char(85u, cursor_x, row, get_color_cyan());   // U
        cursor_x = draw_char(84u, cursor_x, row, get_color_cyan());   // T
        cursor_x = draw_char(69u, cursor_x, row, get_color_cyan());   // E
        cursor_x = draw_char(83u, cursor_x, row, get_color_cyan());   // S
        cursor_x = draw_char(58u, cursor_x, row, get_color_cyan());   // :
        cursor_x += 5u;
        
        let active_count = count_set_bits(routes);
        cursor_x = draw_number(active_count, cursor_x, row, get_color_white());
        
        return vec3<u32>(0u, 0u, 0u);
    }
    
    // Row 412: Health bar [████████████████████████░] 98%
    if (row == 412u) {
        var cursor_x: u32 = 20u;
        
        cursor_x = draw_char(72u, cursor_x, row, get_color_cyan());   // H
        cursor_x = draw_char(69u, cursor_x, row, get_color_cyan());   // E
        cursor_x = draw_char(65u, cursor_x, row, get_color_cyan());   // A
        cursor_x = draw_char(76u, cursor_x, row, get_color_cyan());   // L
        cursor_x = draw_char(84u, cursor_x, row, get_color_cyan());   // T
        cursor_x = draw_char(72u, cursor_x, row, get_color_cyan());   // H
        cursor_x = draw_char(58u, cursor_x, row, get_color_cyan());   // :
        cursor_x += 5u;
        
        cursor_x = draw_char(91u, cursor_x, row, get_color_gray());  // [
        
        // Draw health bar (20 chars)
        let bar_filled = (health_pct * 20u) / 100u;
        for (var bar_i: u32 = 0u; bar_i < 20u; bar_i += 1u) {
            if (bar_i < bar_filled) {
                cursor_x = draw_char(178u, cursor_x, row, get_color_green());
            } else {
                cursor_x = draw_char(176u, cursor_x, row, get_color_dim());
            }
        }
        
        cursor_x = draw_char(93u, cursor_x, row, get_color_gray());  // ]
        cursor_x += 5u;
        
        cursor_x = draw_number(health_pct, cursor_x, row, get_color_white());
        cursor_x = draw_char(37u, cursor_x, row, get_color_white());  // %
        
        return vec3<u32>(0u, 0u, 0u);
    }
    
    return vec3<u32>(0u, 0u, 0u);
}

// Count set bits in route bitmask
fn count_set_bits(bits: u32) -> u32 {
    var count: u32 = 0u;
    var n = bits;
    for (var i: u32 = 0u; i < 32u; i += 1u) {
        count += n & 1u;
        n >>= 1u;
    }
    return count;
}

// Color helpers
fn get_color_cyan() -> Pixel { return Pixel(0u, 200u, 255u, 255u); }
fn get_color_white() -> Pixel { return Pixel(255u, 255u, 255u, 255u); }
fn get_color_red() -> Pixel { return Pixel(255u, 80u, 80u, 255u); }
fn get_color_green() -> Pixel { return Pixel(0u, 255u, 100u, 255u); }
fn get_color_dim() -> Pixel { return Pixel(40u, 60u, 80u, 255u); }
fn get_color_gray() -> Pixel { return Pixel(100u, 100u, 100u, 255u); }
