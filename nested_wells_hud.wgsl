// Nested Wells HUD Shader - Phase 8 Gamma
//
// Visual hierarchy:
//   - Root wells: Large, solid circles
//   - Child wells: Smaller, with connecting lines to parent
//   - Depth indicator: Size/color varies by depth
//   - Selected: White glow with pulsing
//
// Connecting lines show the parent-child relationships

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
    message_count: u32,
    _padding: u32,
    trail: array<u32, 32>,
    mailbox: array<u32, 10>,
}

struct Config {
    width: u32, height: u32,
    time: f32,
    frame: u32,
    mode: u32,
}

struct Well {
    pos_x: f32, pos_y: f32,
    strength: f32,
    selected: f32,
    parent_idx: i32,
    offset_x: f32, offset_y: f32,
    depth: f32,
    _padding: vec4f,  // 16 bytes padding
}

struct UIState {
    well_count: u32,
    _pad0: u32, _pad1: u32, _pad2: u32,
    wells: array<Well, 16>,
}

@group(0) @binding(0) var<storage, read_write> output: array<Pixel>;
@group(0) @binding(1) var<storage, read> agents: array<AgentGpuState>;
@group(0) @binding(2) var<uniform> config: Config;
@group(0) @binding(3) var<uniform> ui: UIState;

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
    switch(tribe % 8u) {
        case 0u: { return vec4f(0.4, 0.2, 0.2, 1.0); }
        case 1u: { return vec4f(0.2, 0.4, 0.2, 1.0); }
        case 2u: { return vec4f(0.2, 0.2, 0.4, 1.0); }
        case 3u: { return vec4f(0.4, 0.4, 0.2, 1.0); }
        case 4u: { return vec4f(0.4, 0.2, 0.4, 1.0); }
        case 5u: { return vec4f(0.2, 0.4, 0.4, 1.0); }
        case 6u: { return vec4f(0.4, 0.3, 0.2, 1.0); }
        case 7u: { return vec4f(0.3, 0.2, 0.4, 1.0); }
        default: { return vec4f(0.3, 0.3, 0.3, 1.0); }
    }
}

// Get color based on depth (for hierarchy visualization)
fn get_depth_color(depth: f32, selected: f32) -> vec4f {
    let d = u32(depth);
    
    // Selected wells are white-ish
    if (selected > 0.5) {
        let pulse = 0.8 + 0.2 * sin(config.time * 4.0);
        return vec4f(pulse, pulse, pulse, 1.0);
    }
    
    switch(d) {
        case 0u: { return vec4f(0.3, 0.5, 0.8, 1.0); }  // Root: Blue
        case 1u: { return vec4f(0.5, 0.7, 0.3, 1.0); }  // Level 1: Green
        case 2u: { return vec4f(0.8, 0.5, 0.3, 1.0); }  // Level 2: Orange
        case 3u: { return vec4f(0.7, 0.3, 0.7, 1.0); }  // Level 3: Purple
        default: { return vec4f(0.5, 0.5, 0.5, 1.0); }
    }
}

// Distance from point to line segment (for drawing connections)
fn dist_to_segment(p: vec2f, a: vec2f, b: vec2f) -> f32 {
    let ab = b - a;
    let ap = p - a;
    let t = clamp(dot(ap, ab) / dot(ab, ab), 0.0, 1.0);
    let closest = a + t * ab;
    return length(p - closest);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3u) {
    let pixel_idx = global_id.x;
    let total_pixels = config.width * config.height;
    
    if (pixel_idx >= total_pixels) { return; }
    
    let x = pixel_idx % config.width;
    let y = pixel_idx / config.width;
    let pos = vec2f(f32(x), f32(y));
    
    // Background
    var color = vec4f(0.02, 0.02, 0.05, 1.0);
    
    // HUD background (top 80 pixels)
    if (y < 80u) {
        color = vec4f(0.1, 0.1, 0.15, 1.0);
        
        // Well count indicator with depth coloring
        if (y >= 30u && y < 40u) {
            let well_block = x / 20u;
            if (well_block < ui.well_count) {
                let well = ui.wells[well_block];
                color = get_depth_color(well.depth, well.selected);
            }
        }
        
        // Hierarchy legend
        if (y >= 50u && y < 70u) {
            let legend_x = x;
            // "DESKTOP → DOCS → RESUME"
            if (legend_x >= 20u && legend_x < 250u) {
                let segment = (legend_x - 20u) / 80u;
                if (segment < 3u) {
                    color = get_depth_color(f32(segment), 0.0);
                }
            }
        }
        
        output[pixel_idx] = pack_color(color);
        return;
    }
    
    // Draw connecting lines between parent and child wells
    for (var i: u32 = 0u; i < ui.well_count; i = i + 1u) {
        let well = ui.wells[i];
        if (well.parent_idx >= 0) {
            let parent = ui.wells[u32(well.parent_idx)];
            let dist = dist_to_segment(pos, vec2f(parent.pos_x, parent.pos_y), vec2f(well.pos_x, well.pos_y));
            
            // Draw line with slight glow
            if (dist < 3.0) {
                let alpha = 1.0 - dist / 3.0;
                let line_color = get_depth_color(well.depth, 0.0);
                color = mix(color, line_color, alpha * 0.5);
            }
        }
    }
    
    // Draw gravity wells with hierarchy visualization
    for (var i: u32 = 0u; i < ui.well_count; i = i + 1u) {
        let well = ui.wells[i];
        let dx = f32(x) - well.pos_x;
        let dy = f32(y) - well.pos_y;
        let dist = sqrt(dx * dx + dy * dy);
        
        // Size based on depth (root = largest)
        let base_size = 40.0 - well.depth * 8.0;
        let radius = max(base_size, 15.0) * (1.0 + 0.3 * well.selected);
        
        // Effective strength (with inheritance)
        let effective_strength = well.strength * (1.0 + well.selected);
        let grav_radius = sqrt(effective_strength) * 0.4;
        
        // Outer gravity glow
        if (dist < grav_radius && dist > radius * 0.5) {
            let alpha = 0.1 * (1.0 - (dist - radius * 0.5) / (grav_radius - radius * 0.5));
            let glow_color = get_depth_color(well.depth, well.selected);
            color = mix(color, glow_color, alpha);
        }
        
        // Main well circle
        if (dist < radius * 0.5) {
            let intensity = 0.6 + 0.4 * well.selected;
            color = get_depth_color(well.depth, well.selected) * intensity;
        }
        
        // Center indicator
        if (dist < 6.0) {
            if (well.selected > 0.5) {
                color = vec4f(1.0, 1.0, 1.0, 1.0);
            } else {
                color = get_depth_color(well.depth, 0.0) * 1.5;
            }
        }
        
        // Depth ring (concentric circles showing depth)
        if (well.depth > 0.5) {
            let ring_dist = abs(dist - radius * 0.7);
            if (ring_dist < 1.5) {
                color = mix(color, vec4f(1.0, 1.0, 1.0, 0.3), 0.5);
            }
        }
    }
    
    // Draw agents
    for (var i: u32 = 0u; i < MAX_AGENTS; i = i + 1u) {
        let agent = agents[i];
        let dx = f32(x) - agent.pos_x;
        let dy = f32(y) - agent.pos_y;
        let dist = sqrt(dx * dx + dy * dy);
        
        let size = select(5.0, 7.0, agent.is_it == 1u);
        
        if (dist < size) {
            color = unpack_color(agent.color);
        }
        
        // Velocity indicator
        if (dist < size && dist > size - 2.0) {
            let vel_len = sqrt(agent.vel_x * agent.vel_x + agent.vel_y * agent.vel_y);
            if (vel_len > 0.5) {
                let dir_x = agent.vel_x / vel_len;
                let dir_y = agent.vel_y / vel_len;
                let dot = (dx / dist) * dir_x + (dy / dist) * dir_y;
                if (dot > 0.7) {
                    color = vec4f(1.0, 1.0, 1.0, 0.8);
                }
            }
        }
    }
    
    // Mini agent tiles (right side)
    if (x >= config.width - 74u && x < config.width - 10u && y >= 10u && y < 74u) {
        let tile_x = (x - (config.width - 74u)) / 8u;
        let tile_y = (y - 10u) / 8u;
        let agent_idx = tile_y * 8u + tile_x;
        
        if (agent_idx < MAX_AGENTS) {
            let agent = agents[agent_idx];
            color = get_tribe_color(agent.tribe);
            
            if (agent.is_it == 1u) {
                let local_x = (x - (config.width - 74u)) % 8u;
                let local_y = (y - 10u) % 8u;
                if (local_x == 0u || local_x == 6u || local_y == 0u || local_y == 6u) {
                    color = vec4f(1.0, 1.0, 1.0, 1.0);
                }
            }
        }
    }
    
    output[pixel_idx] = pack_color(color);
}
