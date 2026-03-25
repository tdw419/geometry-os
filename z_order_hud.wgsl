// Z-Order HUD Shader - Phase 8 Delta
//
// Visual z-order:
//   - Foreground wells: Brighter, larger, full color
//   - Background wells: Dimmer, smaller, ghostly
//   - Selected wells: White glow
//
// Competitive gravity visualization

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
    z_index: f32,
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

// Get z-brightness multiplier (foreground = 1.0, background = 0.3)
fn get_z_brightness(z_index: f32) -> f32 {
    // Find max z
    var max_z = 0.0;
    for (var i: u32 = 0u; i < ui.well_count; i = i + 1u) {
        max_z = max(max_z, ui.wells[i].z_index);
    }
    
    if (max_z <= 0.0) {
        return 1.0;
    }
    
    // Normalize: foreground (z=max) = 1.0, background (z=0) = 0.3
    let normalized = z_index / max_z;
    return 0.3 + normalized * 0.7;
}

// Get z-size multiplier (foreground = 1.0, background = 0.6)
fn get_z_size(z_index: f32) -> f32 {
    var max_z = 0.0;
    for (var i: u32 = 0u; i < ui.well_count; i = i + 1u) {
        max_z = max(max_z, ui.wells[i].z_index);
    }
    
    if (max_z <= 0.0) {
        return 1.0;
    }
    
    let normalized = z_index / max_z;
    return 0.6 + normalized * 0.4;
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
        
        // Z-order indicator (stack visualization)
        if (y >= 30u && y < 50u) {
            let well_block = x / 30u;
            if (well_block < ui.well_count) {
                let well = ui.wells[well_block];
                let brightness = get_z_brightness(well.z_index);
                
                if (well.selected > 0.5) {
                    color = vec4f(1.0, 1.0, 1.0, 1.0);
                } else {
                    color = vec4f(0.3 * brightness, 0.5 * brightness, 0.8 * brightness, 1.0);
                }
                
                // Z-number label area
                if (y >= 38u && y < 48u) {
                    let label_x = (x % 30u);
                    if (label_x >= 10u && label_x < 20u) {
                        color = vec4f(brightness, brightness, brightness, 1.0);
                    }
                }
            }
        }
        
        // Legend
        if (y >= 55u && y < 70u) {
            // "FOREGROUND ◄► BACKGROUND"
            if (x >= 20u && x < 200u) {
                let t = f32(x - 20u) / 180.0;
                color = vec4f(0.3 + t * 0.7, 0.5 + t * 0.5, 0.8 + t * 0.2, 1.0);
            }
        }
        
        output[pixel_idx] = pack_color(color);
        return;
    }
    
    // Draw gravity wells with z-order visualization
    for (var i: u32 = 0u; i < ui.well_count; i = i + 1u) {
        let well = ui.wells[i];
        let dx = f32(x) - well.pos_x;
        let dy = f32(y) - well.pos_y;
        let dist = sqrt(dx * dx + dy * dy);
        
        // Z-based modifiers
        let z_brightness = get_z_brightness(well.z_index);
        let z_size = get_z_size(well.z_index);
        
        let base_radius = sqrt(well.strength) * 0.4 * z_size;
        let radius = base_radius * (1.0 + 0.3 * well.selected);
        
        // Gravity glow (intensity based on z)
        if (dist < radius * 1.8 && dist > radius * 0.6) {
            let alpha = 0.15 * (1.0 - (dist - radius * 0.6) / (radius * 1.2));
            let glow_r = 0.3 * z_brightness;
            let glow_g = 0.5 * z_brightness;
            let glow_b = 0.8 * z_brightness;
            color = mix(color, vec4f(glow_r + alpha, glow_g + alpha, glow_b + alpha, 1.0), alpha);
        }
        
        // Main circle (brightness based on z)
        if (dist < radius * 0.6) {
            if (well.selected > 0.5) {
                // Selected: bright white with pulse
                let pulse = 0.8 + 0.2 * sin(config.time * 4.0);
                color = vec4f(pulse, pulse, pulse, 1.0);
            } else {
                // Unselected: z-based brightness
                let intensity = 0.4 + 0.4 * z_brightness;
                color = vec4f(0.3 * intensity, 0.5 * intensity, 0.8 * intensity, 1.0);
            }
        }
        
        // Center point
        if (dist < 8.0 * z_size) {
            if (well.selected > 0.5) {
                color = vec4f(1.0, 1.0, 1.0, 1.0);
            } else {
                color = vec4f(z_brightness, z_brightness, z_brightness, 1.0);
            }
        }
        
        // Z-index label (small number near well)
        if (dist > radius * 0.8 && dist < radius * 1.2) {
            let angle = atan2(dy, dx);
            // Show z-index at top of well
            if (angle < -2.5 && angle > -0.6) {
                let label_brightness = z_brightness * 0.8;
                color = vec4f(label_brightness, label_brightness, label_brightness, 0.6);
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
            let base_color = unpack_color(agent.color);
            
            // Modulate agent brightness by which well they're near
            var near_brightest_z = 0.0;
            for (var j: u32 = 0u; j < ui.well_count; j = j + 1u) {
                let well = ui.wells[j];
                let well_dx = agent.pos_x - well.pos_x;
                let well_dy = agent.pos_y - well.pos_y;
                let well_dist = sqrt(well_dx * well_dx + well_dy * well_dy);
                
                if (well_dist < 150.0) {
                    // Agent is near this well, use its z-brightness
                    let z_bright = get_z_brightness(well.z_index);
                    if (z_bright > near_brightest_z) {
                        near_brightest_z = z_bright;
                    }
                }
            }
            
            // Apply brightness modulation
            if (near_brightest_z > 0.0) {
                color = base_color * (0.5 + near_brightest_z * 0.5);
            } else {
                color = base_color;
            }
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
