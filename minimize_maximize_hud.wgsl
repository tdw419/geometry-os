// Minimize/Maximize HUD Shader - Phase 8 Zeta
//
// Visual collapse and expand:
//   - Collapse: Wells shrink and move to taskbar, color shifts to tribe
//   - Expand: Wells explode from taskbar with strength surge
//   - Transition: Higher friction to prevent overshooting
//   - Taskbar: Bottom of screen shows minimized wells

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
    is_minimized: f32,
    transition_progress: f32,
    _pad0: f32, _pad1: f32,
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

// Get visual radius (shrinks during minimize, surges during expand)
fn get_visual_radius(well: Well) -> f32 {
    let base_radius = sqrt(well.strength) * 0.4;
    
    if (well.transition_progress > 0.01 && well.transition_progress < 0.99) {
        // During transition: interpolate size
        if (well.is_minimized > 0.5) {
            // Collapsing: shrink
            let shrink_factor = 1.0 - well.transition_progress * 0.7;
            return base_radius * shrink_factor;
        } else {
            // Expanding: grow (with initial pop)
            let grow_factor = 0.3 + well.transition_progress * 0.7;
            // Add pop effect in early expansion
            if (well.transition_progress < 0.3) {
                let pop = 1.0 + (1.0 - well.transition_progress / 0.3) * 0.3;
                return base_radius * grow_factor * pop;
            }
            return base_radius * grow_factor;
        }
    }
    
    // Static state
    if (well.is_minimized > 0.5) {
        return 10.0;  // Small icon size
    }
    
    return base_radius;
}

// Get color during transition (shifts to tribe color when minimized)
fn get_transition_color(well: Well, base_color: vec4f, tribe: u32) -> vec4f {
    if (well.transition_progress > 0.01 && well.transition_progress < 0.99) {
        let tribe_color = get_tribe_color(tribe);
        
        if (well.is_minimized > 0.5) {
            // Collapsing: shift to tribe color
            return mix(base_color, tribe_color, well.transition_progress);
        } else {
            // Expanding: shift from tribe color to base
            return mix(tribe_color, base_color, well.transition_progress);
        }
    }
    
    if (well.is_minimized > 0.5) {
        return get_tribe_color(tribe);
    }
    
    return base_color;
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
    
    // Taskbar (bottom 50 pixels)
    if (y >= config.height - 50u) {
        color = vec4f(0.15, 0.15, 0.2, 1.0);
        
        // Minimized well icons
        let icon_spacing = config.width / (ui.well_count + 1u);
        for (var i: u32 = 0u; i < ui.well_count; i = i + 1u) {
            let well = ui.wells[i];
            if (well.is_minimized > 0.5 && well.transition_progress > 0.99) {
                let icon_x = icon_spacing * (i + 1u);
                let icon_y = config.height - 25u;
                
                let dx = f32(x) - f32(icon_x);
                let dy = f32(y) - f32(icon_y);
                let dist = sqrt(dx * dx + dy * dy);
                
                if (dist < 15.0) {
                    color = get_tribe_color(i);
                }
                if (dist < 5.0) {
                    color = vec4f(1.0, 1.0, 1.0, 1.0);
                }
            }
        }
        
        output[pixel_idx] = pack_color(color);
        return;
    }
    
    // HUD background (top 80 pixels)
    if (y < 80u) {
        color = vec4f(0.1, 0.1, 0.15, 1.0);
        
        // State indicator (MIN/EXP)
        if (y >= 30u && y < 50u) {
            let well_block = x / 60u;
            if (well_block < ui.well_count) {
                let well = ui.wells[well_block];
                
                if (well.is_minimized > 0.5) {
                    // Minimized: red tint
                    color = vec4f(0.6, 0.2, 0.2, 1.0);
                } else if (well.transition_progress > 0.01 && well.transition_progress < 0.99) {
                    // Transitioning: yellow pulse
                    let pulse = 0.7 + 0.3 * sin(config.time * 8.0);
                    color = vec4f(pulse, pulse * 0.8, 0.2, 1.0);
                } else {
                    // Expanded: blue
                    color = vec4f(0.2, 0.5, 0.8, 1.0);
                }
            }
        }
        
        // Legend
        if (y >= 55u && y < 70u) {
            // "CLICK TO MINIMIZE/MAXIMIZE"
            if (x >= 20u && x < 250u) {
                let t = f32(x - 20u) / 230.0;
                color = vec4f(0.3 + t * 0.3, 0.5, 0.8 - t * 0.3, 1.0);
            }
        }
        
        output[pixel_idx] = pack_color(color);
        return;
    }
    
    // Draw gravity wells with collapse/expand visualization
    for (var i: u32 = 0u; i < ui.well_count; i = i + 1u) {
        let well = ui.wells[i];
        let dx = f32(x) - well.pos_x;
        let dy = f32(y) - well.pos_y;
        let dist = sqrt(dx * dx + dy * dy);
        
        let radius = get_visual_radius(well);
        let base_color = vec4f(0.3, 0.5, 0.8, 1.0);
        let well_color = get_transition_color(well, base_color, i);
        
        // Outer gravity glow (intensity based on transition)
        var glow_intensity = 1.0;
        if (well.transition_progress > 0.01 && well.transition_progress < 0.99) {
            glow_intensity = 1.0 + abs(well.transition_progress - 0.5) * 0.5;
        }
        
        if (dist < radius * 1.8 && dist > radius * 0.6) {
            let alpha = 0.15 * (1.0 - (dist - radius * 0.6) / (radius * 1.2)) * glow_intensity;
            color = mix(color, well_color, alpha);
        }
        
        // Main circle
        if (dist < radius * 0.6) {
            if (well.transition_progress > 0.01 && well.transition_progress < 0.99) {
                // Transitioning: pulse
                let pulse = 0.7 + 0.3 * sin(config.time * 8.0);
                color = well_color * pulse;
            } else {
                color = well_color;
            }
        }
        
        // Center point
        if (dist < 6.0) {
            if (well.is_minimized > 0.5) {
                color = get_tribe_color(i);
            } else {
                color = vec4f(1.0, 1.0, 1.0, 1.0);
            }
        }
        
        // Transition effect: radiating rings during expand
        if (well.transition_progress > 0.01 && well.transition_progress < 0.5 && well.is_minimized < 0.5) {
            // Expanding: show radiating rings
            let ring_radius = radius * 2.0 * well.transition_progress;
            let ring_dist = abs(dist - ring_radius);
            if (ring_dist < 3.0) {
                let alpha = 0.5 * (1.0 - well.transition_progress * 2.0);
                color = mix(color, vec4f(1.0, 1.0, 1.0, 1.0), alpha);
            }
        }
        
        // Transition effect: collapsing spiral during minimize
        if (well.transition_progress > 0.01 && well.transition_progress < 0.99 && well.is_minimized > 0.5) {
            // Collapsing: show inward spiral
            let angle = atan2(dy, dx);
            let spiral_angle = angle + well.transition_progress * 6.28;
            let spiral_x = well.pos_x + cos(spiral_angle) * radius * (1.0 - well.transition_progress);
            let spiral_y = well.pos_y + sin(spiral_angle) * radius * (1.0 - well.transition_progress);
            
            let spiral_dx = f32(x) - spiral_x;
            let spiral_dy = f32(y) - spiral_y;
            let spiral_dist = sqrt(spiral_dx * spiral_dx + spiral_dy * spiral_dy);
            
            if (spiral_dist < 5.0) {
                let alpha = 0.5 * (1.0 - well.transition_progress);
                color = mix(color, vec4f(1.0, 1.0, 0.5, 1.0), alpha);
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
            // Check if near a transitioning well (for visual effect)
            var near_transition = false;
            for (var j: u32 = 0u; j < ui.well_count; j = j + 1u) {
                let well = ui.wells[j];
                if (well.transition_progress > 0.01 && well.transition_progress < 0.99) {
                    let well_dx = agent.pos_x - well.pos_x;
                    let well_dy = agent.pos_y - well.pos_y;
                    let well_dist = sqrt(well_dx * well_dx + well_dy * well_dy);
                    if (well_dist < 200.0) {
                        near_transition = true;
                        break;
                    }
                }
            }
            
            let base_color = unpack_color(agent.color);
            
            if (near_transition) {
                // Brighter during transition
                color = base_color * 1.3;
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
