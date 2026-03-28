// ============================================================================
// TELEMETRY PLANE - GlyphLang → ASCII World Zero-Copy Integration
// ============================================================================
// Architecture:
//   - GlyphLang services write metrics via atomic ops to vm_stats[3-5]
//   - HUD shader reads and renders at Rows 410-419
//   - AI sees token-efficient summary via GET /view
//
// vm_stats Layout:
//   [0] GPU Status (1=ONLINE, 0=OFFLINE)
//   [1] IP (instruction pointer)
//   [2] SP (stack pointer)
//   [3] Request Counter (atomic)
//   [4] Error Counter (atomic)
//   [5] Rolling Latency (ms * 10, fixed-point)
//   [6] Active Route Bitmask
//   [7-10] Reserved
// ============================================================================

// Atomic telemetry update functions for GlyphLang services

// Increment request counter (vm_stats[3])
fn track_request() {
    // In GlyphLang: 3 0 p $ 1 + !
    // Atomic add to request counter
    atomicAdd(&vm_stats[3], 1u);
}

// Increment error counter (vm_stats[4])
fn track_error() {
    // In GlyphLang: 4 0 p $ 1 + !
    atomicAdd(&vm_stats[4], 1u);
}

// Update rolling latency (vm_stats[5])
// latency_ms: actual latency in milliseconds
fn track_latency(latency_ms: f32) {
    // Fixed-point encoding: ms * 10 for 0.1ms precision
    let fixed_point = u32(latency_ms * 10.0);
    
    // Rolling average: 90% old, 10% new
    let current = atomicLoad(&vm_stats[5]);
    let updated = (current * 9u + fixed_point) / 10u;
    atomicStore(&vm_stats[5], updated);
}

// Set active route in bitmask (vm_stats[6])
// route_id: 0-31 (32 possible routes)
fn set_route_active(route_id: u32) {
    let mask = 1u << route_id;
    let current = atomicLoad(&vm_stats[6]);
    atomicStore(&vm_stats[6], current | mask);
}

// Clear route from bitmask
fn set_route_inactive(route_id: u32) {
    let mask = !(1u << route_id);
    let current = atomicLoad(&vm_stats[6]);
    atomicStore(&vm_stats[6], current & mask);
}

// ============================================================================
// HUD RENDERER - Rows 410-419
// ============================================================================

fn render_telemetry_hud() {
    // Read telemetry (non-atomic for HUD, slight race is acceptable)
    let reqs = vm_stats[3u];
    let errs = vm_stats[4u];
    let lat_fixed = vm_stats[5u];
    let routes = vm_stats[6u];
    
    // Convert latency back to ms
    let lat_ms = lat_fixed / 10u;
    
    // Calculate health percentage (lower is better for errors)
    var health_pct = 100u;
    if (reqs > 0u) {
        health_pct = 100u - (errs * 100u / reqs);
    }
    
    // Row 410: "G-LANG: {reqs} ERR:{errs}"
    var cursor_x = 20u;
    let cursor_y = 410u;
    
    cursor_x = draw_telemetry_char('G', cursor_x, cursor_y, telemetry_label_color());
    cursor_x = draw_telemetry_char('-', cursor_x, cursor_y, telemetry_label_color());
    cursor_x = draw_telemetry_char('L', cursor_x, cursor_y, telemetry_label_color());
    cursor_x = draw_telemetry_char('A', cursor_x, cursor_y, telemetry_label_color());
    cursor_x = draw_telemetry_char('N', cursor_x, cursor_y, telemetry_label_color());
    cursor_x = draw_telemetry_char('G', cursor_x, cursor_y, telemetry_label_color());
    cursor_x = draw_telemetry_char(':', cursor_x, cursor_y, telemetry_label_color());
    cursor_x += 5u;
    
    cursor_x = draw_telemetry_number(reqs, cursor_x, cursor_y, value_color());
    cursor_x += 10u;
    
    cursor_x = draw_telemetry_char('E', cursor_x, cursor_y, error_color());
    cursor_x = draw_telemetry_char('R', cursor_x, cursor_y, error_color());
    cursor_x = draw_telemetry_char('R', cursor_x, cursor_y, error_color());
    cursor_x = draw_telemetry_char(':', cursor_x, cursor_y, error_color());
    cursor_x += 5u;
    
    cursor_x = draw_telemetry_number(errs, cursor_x, cursor_y, error_color());
    
    // Row 411: "LAT:{lat_ms}ms ROUTES:{active_count}"
    cursor_x = 20u;
    let cursor_y_411 = 411u;
    
    cursor_x = draw_telemetry_char('L', cursor_x, cursor_y_411, telemetry_label_color());
    cursor_x = draw_telemetry_char('A', cursor_x, cursor_y_411, telemetry_label_color());
    cursor_x = draw_telemetry_char('T', cursor_x, cursor_y_411, telemetry_label_color());
    cursor_x = draw_telemetry_char(':', cursor_x, cursor_y_411, telemetry_label_color());
    cursor_x += 5u;
    
    cursor_x = draw_telemetry_number(lat_ms, cursor_x, cursor_y_411, value_color());
    cursor_x = draw_telemetry_char('m', cursor_x, cursor_y_411, value_color());
    cursor_x = draw_telemetry_char('s', cursor_x, cursor_y_411, value_color());
    cursor_x += 15u;
    
    cursor_x = draw_telemetry_char('R', cursor_x, cursor_y_411, telemetry_label_color());
    cursor_x = draw_telemetry_char('O', cursor_x, cursor_y_411, telemetry_label_color());
    cursor_x = draw_telemetry_char('U', cursor_x, cursor_y_411, telemetry_label_color());
    cursor_x = draw_telemetry_char('T', cursor_x, cursor_y_411, telemetry_label_color());
    cursor_x = draw_telemetry_char('E', cursor_x, cursor_y_411, telemetry_label_color());
    cursor_x = draw_telemetry_char('S', cursor_x, cursor_y_411, telemetry_label_color());
    cursor_x = draw_telemetry_char(':', cursor_x, cursor_y_411, telemetry_label_color());
    cursor_x += 5u;
    
    let active_count = count_bits(routes);
    cursor_x = draw_telemetry_number(active_count, cursor_x, cursor_y_411, value_color());
    
    // Row 412: Health bar [████████████████████████████░] {health_pct}%
    cursor_x = 20u;
    let cursor_y_412 = 412u;
    
    cursor_x = draw_telemetry_char('H', cursor_x, cursor_y_412, telemetry_label_color());
    cursor_x = draw_telemetry_char('E', cursor_x, cursor_y_412, telemetry_label_color());
    cursor_x = draw_telemetry_char('A', cursor_x, cursor_y_412, telemetry_label_color());
    cursor_x = draw_telemetry_char('L', cursor_x, cursor_y_412, telemetry_label_color());
    cursor_x = draw_telemetry_char('T', cursor_x, cursor_y_412, telemetry_label_color());
    cursor_x = draw_telemetry_char('H', cursor_x, cursor_y_412, telemetry_label_color());
    cursor_x = draw_telemetry_char(':', cursor_x, cursor_y_412, telemetry_label_color());
    cursor_x += 5u;
    
    cursor_x = draw_telemetry_char('[', cursor_x, cursor_y_412, bracket_color());
    
    // Draw health bar (20 chars wide)
    let bar_filled = (health_pct * 20u) / 100u;
    var bar_i = 0u;
    loop {
        if (bar_i >= 20u) { break; }
        
        if (bar_i < bar_filled) {
            cursor_x = draw_telemetry_char('█', cursor_x, cursor_y_412, health_good_color());
        } else {
            cursor_x = draw_telemetry_char('░', cursor_x, cursor_y_412, health_dim_color());
        }
        bar_i += 1u;
    }
    
    cursor_x = draw_telemetry_char(']', cursor_x, cursor_y_412, bracket_color());
    cursor_x += 5u;
    
    cursor_x = draw_telemetry_number(health_pct, cursor_x, cursor_y_412, value_color());
    cursor_x = draw_telemetry_char('%', cursor_x, cursor_y_412, value_color());
}

// Helper: Count set bits in route bitmask
fn count_bits(bits: u32) -> u32 {
    var count = 0u;
    var n = bits;
    loop {
        if (n == 0u) { break; }
        count += n & 1u;
        n >>= 1u;
    }
    return count;
}

// Color helpers for telemetry display
fn telemetry_label_color() -> vec3<u32> {
    return vec3<u32>(0u, 200u, 255u);  // Cyan
}

fn health_good_color() -> vec3<u32> {
    return vec3<u32>(0u, 255u, 100u);  // Green
}

fn health_dim_color() -> vec3<u32> {
    return vec3<u32>(40u, 60u, 80u);  // Dark gray
}

fn bracket_color() -> vec3<u32> {
    return vec3<u32>(100u, 100u, 100u);  // Gray
}

fn error_color() -> vec3<u32> {
    return vec3<u32>(255u, 80u, 80u);  // Red
}

fn value_color() -> vec3<u32> {
    return vec3<u32>(255u, 255u, 255u);  // White
}

// Draw single character for telemetry (uses existing font system)
fn draw_telemetry_char(ch: u32, x: u32, y: u32, color: vec3<u32>) -> u32 {
    return draw_char(ch, x, y, color_to_pixel(color));
}

// Draw number for telemetry
fn draw_telemetry_number(num: u32, x: u32, y: u32, color: vec3<u32>) -> u32 {
    return draw_number(num, x, y, color_to_pixel(color));
}

fn color_to_pixel(color: vec3<u32>) -> Pixel {
    var p: Pixel;
    p.r = color.x;
    p.g = color.y;
    p.b = color.z;
    p.a = 255u;
    return p;
}
