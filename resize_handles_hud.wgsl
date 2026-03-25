// Resize Handles HUD Shader - Phase 8 Epsilon
//
// Visual resize handles:
//   - Wells sized by strength (radius = sqrt(strength) * 0.4)
//   - Resize handle at bottom-right corner
//   - Handle grows when resizing is active
//   - Brightness indicates strength

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
    resizing: f32,
    _pad0: f32, _pad1: f32, _pad2: f32,
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

// Get brightness based on strength (100 = dim, 5000 = bright)
fn get_strength_brightness(strength: f32) -> f32 {
    // Normalize: 100 → 0.4, 5000 → 1.0
    return 0.4 + (strength - 100.0) / 4900.0 * 0.6;
}

// Get well radius from strength
fn get_well_radius(strength: f32) -> f32 {
    return sqrt(strength) * 0.4;
}

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
        
        // Strength indicator (bars)
        if (y >= 30u && y < 50u) {
            let well_block = x / 60u;
            if (well_block < ui.well_count) {
                let well = ui.wells[well_block];
                let brightness = get_strength_brightness(well.strength);
                
                // Bar height based on strength
                let bar_height = (well.strength - 100.0) / 4900.0 * 16.0;
                let y_in_block = y - 30u;
                
                if (f32(y_in_block) < bar_height) {
                    if (well.resizing > 0.5) {
                        // Resizing: yellow pulse
                        let pulse = 0.8 + 0.2 * sin(config.time * 6.0);
                        color = vec4f(pulse, pulse * 0.8, 0.2, 1.0);
                    } else if (well.selected > 0.5) {
                        color = vec4f(1.0, 1.0, 1.0, 1.0);
                    } else {
                        color = vec4f(0.3 * brightness, 0.5 * brightness, 0.8 * brightness, 1.0);
                    }
                } else {
                    color = vec4f(0.15, 0.15, 0.2, 1.0);
                }
            }
        }
        
        // Legend
        if (y >= 55u && y < 70u) {
            // "DRAG CORNER TO RESIZE"
            if (x >= 20u && x < 220u) {
                let t = f32(x - 20u) / 200.0;
                color = vec4f(0.3 + t * 0.5, 0.5 + t * 0.3, 0.8 - t * 0.3, 1.0);
            }
        }
        
        output[pixel_idx] = pack_color(color);
        return;
    }
    
    // Draw gravity wells with resize handles
    for (var i: u32 = 0u; i < ui.well_count; i = i + 1u) {
        let well = ui.wells[i];
        let dx = f32(x) - well.pos_x;
        let dy = f32(y) - well.pos_y;
        let dist = sqrt(dx * dx + dy * dy);
        
        let radius = get_well_radius(well.strength);
        let brightness = get_strength_brightness(well.strength);
        
        // Outer gravity glow
        if (dist < radius * 1.5 && dist > radius * 0.8) {
            let alpha = 0.15 * (1.0 - (dist - radius * 0.8) / (radius * 0.7));
            let glow_r = 0.3 * brightness;
            let glow_g = 0.5 * brightness;
            let glow_b = 0.8 * brightness;
            color = mix(color, vec4f(glow_r + alpha, glow_g + alpha, glow_b + alpha, 1.0), alpha);
        }
        
        // Main circle (size based on strength)
        if (dist < radius * 0.8) {
            if (well.selected > 0.5) {
                let pulse = 0.8 + 0.2 * sin(config.time * 4.0);
                color = vec4f(pulse, pulse, pulse, 1.0);
            } else if (well.resizing > 0.5) {
                // Resizing: yellow tint with pulse
                let pulse = 0.7 + 0.3 * sin(config.time * 6.0);
                color = vec4f(0.8 * pulse, 0.7 * pulse, 0.3 * pulse, 1.0);
            } else {
                let intensity = 0.4 + 0.3 * brightness;
                color = vec4f(0.3 * intensity, 0.5 * intensity, 0.8 * intensity, 1.0);
            }
        }
        
        // Center point
        if (dist < 6.0) {
            color = vec4f(brightness, brightness, brightness, 1.0);
        }
        
        // Resize handle (bottom-right corner)
        let handle_offset_x = radius * 0.6;
        let handle_offset_y = radius * 0.6;
        let handle_x = well.pos_x + handle_offset_x;
        let handle_y = well.pos_y + handle_offset_y;
        let handle_dx = f32(x) - handle_x;
        let handle_dy = f32(y) - handle_y;
        let handle_dist = sqrt(handle_dx * handle_dx + handle_dy * handle_dy);
        
        // Handle size grows when resizing
        let handle_radius = select(15.0, 20.0, well.resizing > 0.5);
        
        // Draw resize handle
        if (handle_dist < handle_radius) {
            if (well.resizing > 0.5) {
                // Active resizing: bright yellow pulse
                let pulse = 0.8 + 0.2 * sin(config.time * 8.0);
                color = vec4f(pulse, pulse * 0.9, 0.2, 1.0);
            } else if (well.selected > 0.5) {
                // Selected but not resizing: white
                color = vec4f(0.9, 0.9, 0.9, 1.0);
            } else {
                // Normal: cyan handle
                color = vec4f(0.5, 0.8, 0.9, 1.0);
            }
        }
        
        // Handle border
        if (handle_dist > handle_radius - 2.0 && handle_dist < handle_radius) {
            if (well.resizing > 0.5) {
                color = vec4f(1.0, 1.0, 0.5, 1.0);
            } else {
                color = vec4f(0.8, 0.9, 1.0, 1.0);
            }
        }
        
        // Corner lines (visual hint for resize direction)
        if (well.resizing > 0.5 || well.selected > 0.5) {
            // Draw diagonal lines from handle
            if (handle_dist < handle_radius * 2.0 && handle_dist > handle_radius) {
                let angle = atan2(handle_dy, handle_dx);
                if (angle > 0.3 && angle < 1.2) {
                    let alpha = 0.3 * (1.0 - (handle_dist - handle_radius) / handle_radius);
                    color = mix(color, vec4f(1.0, 1.0, 1.0, 1.0), alpha);
                }
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
