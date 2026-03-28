// Telemetry HUD - GlyphLang metrics at Rows 410-419
fn render_telemetry_hud(row: u32, col: u32, width: u32) -> vec3<u32> {
    if (row < 410u || row >= 420u) {
        return vec3<u32>(0u, 1u, 1u);
    }
    
    let reqs = vm_stats[3u];
    let errs = vm_stats[4u];
    
    // Row 410: "G-LANG: {reqs}"
    if (row == 410u) {
        var cursor_x: u32 = 20u;
        cursor_x = draw_char(71u, cursor_x, row, Pixel(0u, 200u, 255u, 255u)); // G
        cursor_x = draw_char(45u, cursor_x, row, Pixel(0u, 200u, 255u, 255u)); // -
        cursor_x = draw_char(76u, cursor_x, row, Pixel(0u, 200u, 255u, 255u)); // L
        cursor_x = draw_char(65u, cursor_x, row, Pixel(0u, 200u, 255u, 255u)); // A
        cursor_x = draw_char(78u, cursor_x, row, Pixel(0u, 200u, 255u, 255u)); // N
        cursor_x = draw_char(71u, cursor_x, row, Pixel(0u, 200u, 255u, 255u)); // G
        cursor_x = draw_char(58u, cursor_x, row, Pixel(0u, 200u, 255u, 255u)); // :
        cursor_x += 5u;
        cursor_x = draw_number(reqs, cursor_x, row, Pixel(255u, 255u, 255u, 255u));
    }
    
    return vec3<u32>(1u, 1u, 1u);
}
