// ============================================================================
// RADIAL DRIFT HUD SHADER — Self-Organizing Memory Map
// ============================================================================
// Phase 8 Eta: Semantic Defrag
//   - Master gravity well at center (0.5, 0.5)
//   - Radial priority: center = active, periphery = archive
//   - Tribe-based color sorting (Green tribe → Green data)
//   - Drift physics: unused wells migrate outward
//   - "Pull to center" on interaction
// ============================================================================

const WIDTH: u32 = 1280u;
const HEIGHT: u32 = 800u;
const MAX_AGENTS: u32 = 64u;
const MAX_WELLS: u32 = 16u;

struct Pixel {
    r: u32, g: u32, b: u32, a: u32,
}

struct AgentGpuState {
    id: u32,
    pos_x: f32, pos_y: f32,
    vel_x: f32, vel_y: f32,
    color: u32,
    tribe: u32,
    is_it: u32,
    message_waiting: u32,
    trail_len: u32,
    collision_count: u32,
    message_count: u32,        // deliveries
    cargo: f32,                // 0.0 = Empty, 1.0 = Green, 2.0 = Blue
    distance_traveled: f32,    // Energy metric
    trail: array<u32, 30>,
    mailbox: array<u32, 10>,
}

struct Config {
    width: u32, height: u32,
    time: f32,
    frame: u32,
    mode: u32,
}

struct Well {
    pos_x: f32, pos_y: f32,          // 8 bytes
    strength: f32,                    // 4 bytes
    drift_rate: f32,                  // 4 bytes
    last_access: f32,                 // 4 bytes
    tribe: u32,                       // 4 bytes
    is_active: u32,                   // 4 bytes
    _padding: u32, _padding2: u32, _padding3: u32, _padding4: u32, _padding5: u32,  // 20 bytes = 48 total
}

struct RadialState {
    master_well_x: f32,               // Center X (usually 0.5)
    master_well_y: f32,               // Center Y (usually 0.5)
    core_radius: f32,                 // High-priority zone (0.0-0.2)
    inner_radius: f32,                // Medium-priority zone (0.2-0.4)
    drift_enabled: u32,               // Toggle radial drift
    color_sorting: u32,               // Toggle tribe-based sorting
    _padding: u32, _padding2: u32,
}

struct UIState {
    well_count: u32,
    _pad0: u32, _pad1: u32, _pad2: u32,
    wells: array<Well, 16>,
    radial: RadialState,
}

@group(0) @binding(0) var<storage, read_write> output: array<Pixel>;
@group(0) @binding(1) var<storage, read> agents: array<AgentGpuState>;
@group(0) @binding(2) var<uniform> config: Config;
@group(0) @binding(3) var<uniform> ui: UIState;

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

fn unpack_color(packed: u32) -> vec4f {
    return vec4f(
        f32((packed >> 24u) & 0xFFu) / 255.0,
        f32((packed >> 16u) & 0xFFu) / 255.0,
        f32((packed >> 8u) & 0xFFu) / 255.0,
        f32(packed & 0xFFu) / 255.0,
    );
}

fn pack_color(c: vec4f) -> Pixel {
    return Pixel(
        u32(clamp(c.r * 255.0, 0.0, 255.0)),
        u32(clamp(c.g * 255.0, 0.0, 255.0)),
        u32(clamp(c.b * 255.0, 0.0, 255.0)),
        u32(clamp(c.a * 255.0, 0.0, 255.0)),
    );
}

fn get_tribe_color(tribe: u32) -> vec4f {
    // Same palette as spatial_swarm_hud.wgsl
    switch(tribe % 8u) {
        case 0u: { return vec4f(1.0, 0.25, 0.25, 1.0); }  // Red
        case 1u: { return vec4f(0.25, 1.0, 0.25, 1.0); }  // Green
        case 2u: { return vec4f(0.25, 0.25, 1.0, 1.0); }  // Blue
        case 3u: { return vec4f(1.0, 1.0, 0.25, 1.0); }  // Yellow
        case 4u: { return vec4f(1.0, 0.25, 1.0, 1.0); }  // Magenta
        case 5u: { return vec4f(0.25, 1.0, 1.0, 1.0); }  // Cyan
        case 6u: { return vec4f(1.0, 0.5, 0.25, 1.0); }  // Orange
        case 7u: { return vec4f(0.5, 0.25, 1.0, 1.0); }  // Purple
        default: { return vec4f(0.5, 0.5, 0.5, 1.0); }
    }
}

// ============================================================================
// RADIAL PRIORITY CALCULATIONS
// ============================================================================

fn get_distance_from_center(pos: vec2f) -> f32 {
    let center = vec2f(ui.radial.master_well_x, ui.radial.master_well_y);
    return distance(pos, center);
}

fn get_radial_priority(pos: vec2f) -> f32 {
    // Priority decays with distance from center
    // 1.0 at center, 0.1 at edges
    let dist = get_distance_from_center(pos);
    return 1.0 / (1.0 + dist * 10.0);
}

fn get_priority_zone(pos: vec2f) -> u32 {
    // Returns: 0 = Core, 1 = Inner, 2 = Periphery
    let dist = get_distance_from_center(pos);
    
    if (dist < ui.radial.core_radius) {
        return 0u;  // Core - high frequency, 100% opacity
    } else if (dist < ui.radial.inner_radius) {
        return 1u;  // Inner - medium frequency, cohesive swarm
    } else {
        return 2u;  // Periphery - low energy, background
    }
}

// ============================================================================
// DRIFT PHYSICS
// ============================================================================

fn apply_radial_drift(well: Well, delta_time: f32) -> vec2f {
    // Unused wells drift outward from center
    if (ui.radial.drift_enabled == 0u) {
        return vec2f(well.pos_x, well.pos_y);
    }
    
    let center = vec2f(ui.radial.master_well_x, ui.radial.master_well_y);
    let pos = vec2f(well.pos_x, well.pos_y);
    
    // Time since last access (in seconds)
    let time_since_access = config.time - well.last_access;
    
    // Drift only if unused for > 5 seconds
    if (time_since_access < 5.0) {
        return pos;
    }
    
    // Drift direction: away from center
    let drift_dir = normalize(pos - center);
    
    // Drift rate increases with time since access
    let drift_speed = well.drift_rate * min(time_since_access / 10.0, 3.0);
    
    // Apply drift
    var new_pos = pos + drift_dir * drift_speed * delta_time;
    
    // Clamp to screen bounds (leave 10% margin)
    new_pos.x = clamp(new_pos.x, 0.1, 0.9);
    new_pos.y = clamp(new_pos.y, 0.1, 0.9);
    
    return new_pos;
}

fn pull_to_center(well: Well, strength: f32) -> vec2f {
    // When clicked/interacted, well snaps toward center
    let center = vec2f(ui.radial.master_well_x, ui.radial.master_well_y);
    let pos = vec2f(well.pos_x, well.pos_y);
    
    return mix(pos, center, strength);
}

// ============================================================================
// TRIBE-BASED COLOR SORTING
// ============================================================================

fn get_sorting_target(tribe: u32) -> vec2f {
    // Each tribe has a "home" position where they sort their color
    // Arranged in a circle around the center
    
    let angle = f32(tribe) * (6.28318 / 8.0);  // 2π / 8 tribes
    let radius = ui.radial.inner_radius * 0.7;  // Just inside inner ring
    
    let center = vec2f(ui.radial.master_well_x, ui.radial.master_well_y);
    
    return center + vec2f(
        cos(angle) * radius,
        sin(angle) * radius
    );
}

fn should_agent_sort_color(agent: AgentGpuState) -> bool {
    // Agent sorts if color sorting enabled and agent is in inner ring
    if (ui.radial.color_sorting == 0u) {
        return false;
    }
    
    let pos = vec2f(agent.pos_x / f32(WIDTH), agent.pos_y / f32(HEIGHT));
    let zone = get_priority_zone(pos);
    
    // Only inner ring agents actively sort
    return zone == 1u;
}

// ============================================================================
// GRAVITY ATTRACTION (with radial modification)
// ============================================================================

fn calculate_gravity_with_drift(agent: AgentGpuState) -> vec2f {
    // Combined gravity: master well + tribe sorting + data wells
    
    let agent_pos = vec2f(
        agent.pos_x / f32(WIDTH),
        agent.pos_y / f32(HEIGHT)
    );
    
    var total_force = vec2f(0.0, 0.0);
    
    // 1. Master well attraction (always present)
    let center = vec2f(ui.radial.master_well_x, ui.radial.master_well_y);
    let to_center = center - agent_pos;
    let dist_to_center = length(to_center);
    
    // Master gravity scales with distance (stronger at edges to pull in)
    let master_strength = 0.5 + dist_to_center * 2.0;
    total_force += normalize(to_center) * master_strength * 0.1;
    
    // 2. Tribe sorting target (if enabled and in inner ring)
    if (should_agent_sort_color(agent)) {
        let sorting_target = get_sorting_target(agent.tribe);
        let to_sort = sorting_target - agent_pos;
        total_force += normalize(to_sort) * 0.05;
    }
    
    // 3. Data well attractions
    for (var i = 0u; i < ui.well_count; i++) {
        let well = ui.wells[i];
        if (well.is_active == 0u) { continue; }
        
        let well_pos = apply_radial_drift(well, 0.016);  // ~60fps delta
        let to_well = well_pos - agent_pos;
        let dist = length(to_well);
        
        // Wells in core have stronger attraction
        let well_zone = get_priority_zone(well_pos);
        let zone_multiplier = select(
            select(1.5, 1.0, well_zone == 1u),
            2.0,
            well_zone == 0u
        );
        
        if (dist > 0.01) {
            total_force += normalize(to_well) * well.strength * zone_multiplier / (dist * dist + 0.1);
        }
    }
    
    return total_force;
}

// ============================================================================
// RENDERING
// ============================================================================

fn render_priority_zones(color: ptr<function, vec4f>, x: u32, y: u32) {
    // Subtle background coloring to show zones
    
    let pos = vec2f(f32(x) / f32(WIDTH), f32(y) / f32(HEIGHT));
    let zone = get_priority_zone(pos);
    
    // Very subtle tint
    var tint = vec4f(0.0, 0.0, 0.0, 0.0);
    
    if (zone == 0u) {
        // Core - warm tint (active)
        tint = vec4f(0.02, 0.01, 0.0, 0.0);
    } else if (zone == 1u) {
        // Inner - neutral
        tint = vec4f(0.0, 0.0, 0.0, 0.0);
    } else {
        // Periphery - cool tint (archive)
        tint = vec4f(0.0, 0.0, 0.02, 0.0);
    }
    
    *color = *color + tint;
}

fn render_master_well(color: ptr<function, vec4f>, x: u32, y: u32) {
    // Render the central master well as a subtle glow
    
    let center_x = u32(ui.radial.master_well_x * f32(WIDTH));
    let center_y = u32(ui.radial.master_well_y * f32(HEIGHT));
    
    let dx = abs(i32(x) - i32(center_x));
    let dy = abs(i32(y) - i32(center_y));
    let dist = sqrt(f32(dx * dx + dy * dy));
    
    // Soft glow with multiple rings
    if (dist < 30.0) {
        let glow = 1.0 - (dist / 30.0);
        let pulse = sin(config.time * 2.0) * 0.1 + 0.9;
        *color = *color + vec4f(0.3, 0.3, 0.4, 0.0) * glow * glow * pulse;
    }
    
    // Core ring
    if (dist >= 28.0 && dist < 32.0) {
        *color = mix(*color, vec4f(0.5, 0.5, 0.6, 1.0), 0.3);
    }
}

fn render_tribe_zones(color: ptr<function, vec4f>, x: u32, y: u32) {
    // Render tribe sorting zones as colored arcs
    
    if (ui.radial.color_sorting == 0u) { return; }
    
    let pos = vec2f(f32(x) / f32(WIDTH), f32(y) / f32(HEIGHT));
    let center = vec2f(ui.radial.master_well_x, ui.radial.master_well_y);
    let dist = distance(pos, center);
    
    // Only render in inner ring
    if (dist < ui.radial.core_radius || dist > ui.radial.inner_radius) {
        return;
    }
    
    // Calculate which tribe zone this pixel is in
    let angle = atan2(pos.y - center.y, pos.x - center.x);
    let normalized_angle = (angle + 3.14159) / 6.28318;  // 0-1
    let tribe = u32(normalized_angle * 8.0) % 8u;
    
    let tribe_color = get_tribe_color(tribe);
    
    // Very subtle coloring
    *color = mix(*color, tribe_color, 0.05);
}

fn render_well_with_priority(color: ptr<function, vec4f>, x: u32, y: u32) {
    // Render data wells with radial priority visualization
    
    for (var i = 0u; i < ui.well_count; i++) {
        let well = ui.wells[i];
        if (well.is_active == 0u) { continue; }
        
        let well_pos = apply_radial_drift(well, 0.016);
        let wx = u32(well_pos.x * f32(WIDTH));
        let wy = u32(well_pos.y * f32(HEIGHT));
        
        let dx = abs(i32(x) - i32(wx));
        let dy = abs(i32(y) - i32(wy));
        let dist = sqrt(f32(dx * dx + dy * dy));
        
        // Well size and intensity based on priority zone
        let zone = get_priority_zone(well_pos);
        var well_radius: f32;
        var well_intensity: f32;
        
        if (zone == 0u) {
            well_radius = 25.0;
            well_intensity = 0.8;
        } else if (zone == 1u) {
            well_radius = 20.0;
            well_intensity = 0.6;
        } else {
            well_radius = 15.0;
            well_intensity = 0.4;
        }
        
        if (dist < well_radius) {
            let glow = 1.0 - (dist / well_radius);
            let well_color = get_tribe_color(well.tribe);
            *color = mix(*color, well_color, glow * well_intensity);
        }
        
        // Well border
        if (dist >= well_radius - 2.0 && dist < well_radius) {
            let border_color = get_tribe_color(well.tribe);
            *color = mix(*color, border_color, 0.5);
        }
    }
}

fn render_agent_with_priority(color: ptr<function, vec4f>, x: u32, y: u32) {
    // Render agents with radial priority affecting appearance
    
    for (var i = 0u; i < MAX_AGENTS; i++) {
        let agent = agents[i];
        
        let ax = u32(agent.pos_x);
        let ay = u32(agent.pos_y);
        
        let dx = abs(i32(x) - i32(ax));
        let dy = abs(i32(y) - i32(ay));
        let dist = sqrt(f32(dx * dx + dy * dy));
        
        if (dist < 8.0) {
            let agent_pos_norm = vec2f(agent.pos_x / f32(WIDTH), agent.pos_y / f32(HEIGHT));
            let priority = get_radial_priority(agent_pos_norm);
            
            let base_color = get_tribe_color(agent.tribe);
            
            // Priority affects size and brightness
            let size = 4.0 + priority * 4.0;
            let brightness = 0.5 + priority * 0.5;
            
            if (dist < size) {
                let glow = 1.0 - (dist / size);
                let final_color = base_color * brightness;
                *color = mix(*color, final_color, glow);
            }
        }
    }
}

// ============================================================================
// MAIN COMPUTE SHADER
// ============================================================================

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3u) {
    let pixel_idx = global_id.x;
    let total_pixels = config.width * config.height;
    
    if (pixel_idx >= total_pixels) { return; }
    
    let x = pixel_idx % config.width;
    let y = pixel_idx / config.width;
    
    // Background
    var color = vec4f(0.02, 0.02, 0.05, 1.0);
    
    // HUD background (top 80 pixels)
    if (y < 80u) {
        color = vec4f(0.1, 0.1, 0.15, 1.0);
    }
    
    // Render layers (back to front)
    render_priority_zones(&color, x, y);
    render_master_well(&color, x, y);
    render_tribe_zones(&color, x, y);
    render_well_with_priority(&color, x, y);
    render_agent_with_priority(&color, x, y);
    
    // Output
    output[pixel_idx] = pack_color(color);
}
