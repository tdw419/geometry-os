// ============================================================================
// TELEMETRY BRIDGE - Particle Spawning on vm_stats Delta
// ============================================================================
// This module adds particle-based visual feedback when GlyphLang routes execute.
// Particles spawn at Row 410 + route_id and travel rightward.
// ============================================================================

// --- Particle System Constants ---
const MAX_PARTICLES: u32 = 256u;
const PARTICLE_LIFETIME: f32 = 60.0;  // frames
const PARTICLE_BASE_SPEED: f32 = 2.0;
const PARTICLE_MAX_SPEED: f32 = 12.0;

// --- Particle Structure ---
struct Particle {
    pos: vec2<f32>,
    vel: vec2<f32>,
    color: vec4<f32>,
    life: f32,
    route_id: u32,
    result_val: u32,
}

// --- Storage Buffers (add these to main shader bindings) ---
// @group(0) @binding(9)  var<storage, read_write> prev_vm_stats: array<u32, 11>;
// @group(0) @binding(10) var<storage, read_write> particles: array<Particle>;
// @group(0) @binding(11) var<storage, read_write> particle_counter: atomic<u32>;

// --- Route Color Palette ---
// Each route ID gets a distinct color for visual identification
fn get_route_color(route_id: u32) -> vec4<f32> {
    let r = f32((route_id * 73u) % 256u) / 255.0;
    let g = f32((route_id * 151u) % 256u) / 255.0;
    let b = f32((route_id * 199u) % 256u) / 255.0;
    return vec4<f32>(r, g, b, 1.0);
}

// --- Core: Spawn Route Event Particle ---
// Call this when vm_stats[idx] changes
fn spawn_route_event(route_id: u32, val: u32, 
    particles: ptr<function, read_write> array<Particle>, 
    particle_counter: ptr<function, read_write> atomic<u32>) 
{
    // Get next particle slot (ring buffer)
    let p_idx = atomicAdd(&particle_counter, 1u) % MAX_PARTICLES;
    
    // Spawn at HUD telemetry row for this route
    // Row 410 is the start of telemetry zone
    let spawn_y = 410.0 + f32(route_id) * 2.0;  // Stagger vertically
    
    particles[p_idx].pos = vec2<f32>(10.0, spawn_y);
    
    // Velocity: magnitude tied to result value
    // Clamp to reasonable range
    let speed = clamp(
        PARTICLE_BASE_SPEED + f32(val % 10u), 
        PARTICLE_BASE_SPEED, 
        PARTICLE_MAX_SPEED
    );
    particles[p_idx].vel = vec2<f32>(speed, 0.0);
    
    // Color from route ID
    particles[p_idx].color = get_route_color(route_id);
    
    // Full lifetime
    particles[p_idx].life = PARTICLE_LIFETIME;
    
    // Tag with source info
    particles[p_idx].route_id = route_id;
    particles[p_idx].result_val = val;
}

// --- Delta Detection: Check vm_stats for Changes ---
// Returns bitmask of which stats changed
fn detect_vm_stats_delta(
    vm_stats: ptr<function, read_write> array<atomic<u32>, 11>,
    prev_vm_stats: ptr<function, read_write> array<u32, 11>
) -> u32 {
    var changed_mask = 0u;
    
    // Check indices 3-6 (Requests, Errors, Latency, Routes)
    // These are the telemetry fields that indicate route execution
    var i = 3u;
    loop {
        if (i >= 7u) { break; }
        
        let current_val = atomicLoad(&vm_stats[i]);
        let prev_val = prev_vm_stats[i];
        
        if (current_val != prev_val) {
            // Mark this index as changed
            changed_mask |= (1u << i);
            
            // Update prev for next frame
            prev_vm_stats[i] = current_val;
        }
        
        i += 1u;
    }
    
    return changed_mask;
}

// --- Main Telemetry Pulse Checker ---
// Call this from main() compute shader
// Spawns particles for any vm_stats changes
fn check_telemetry_pulses(
    vm_stats: ptr<function, read_write> array<atomic<u32>, 11>,
    prev_vm_stats: ptr<function, read_write> array<u32, 11>,
    particles: ptr<function, read_write> array<Particle>,
    particle_counter: ptr<function, read_write> atomic<u32>
) {
    let changed = detect_vm_stats_delta(vm_stats, prev_vm_stats);
    
    if (changed == 0u) { return; }  // No changes
    
    // Spawn particles for each changed stat
    // Index 3 = Requests (main execution indicator)
    if ((changed & 0x08u) != 0u) {  // bit 3
        let val = atomicLoad(&vm_stats[3]);
        spawn_route_event(3u, val, particles, particle_counter);
    }
    
    // Index 4 = Errors (spawn red particle)
    if ((changed & 0x10u) != 0u) {  // bit 4
        let val = atomicLoad(&vm_stats[4]);
        spawn_route_event(4u, val, particles, particle_counter);
    }
    
    // Index 5 = Latency (spawn yellow particle)
    if ((changed & 0x20u) != 0u) {  // bit 5
        let val = atomicLoad(&vm_stats[5]);
        spawn_route_event(5u, val, particles, particle_counter);
    }
    
    // Index 6 = Route mask (spawn cyan particle)
    if ((changed & 0x40u) != 0u) {  // bit 6
        let val = atomicLoad(&vm_stats[6]);
        spawn_route_event(6u, val, particles, particle_counter);
    }
}

// --- Particle Physics Update ---
// Call this each frame to move particles
fn update_particles(
    particles: ptr<function, read_write> array<Particle>,
    buffer_out: ptr<function, read_write> array<Pixel>,
    width: u32,
    height: u32
) {
    var i = 0u;
    loop {
        if (i >= MAX_PARTICLES) { break; }
        
        let p = particles[i];
        
        // Skip dead particles
        if (p.life <= 0.0) {
            i += 1u;
            continue;
        }
        
        // Update position
        let new_pos = p.pos + p.vel;
        particles[i].pos = new_pos;
        
        // Decay life
        particles[i].life = p.life - 1.0;
        
        // Render particle (single pixel for now)
        let px = u32(new_pos.x);
        let py = u32(new_pos.y);
        
        if (px < width && py < height) {
            let idx = py * width + px;
            
            // Alpha fade based on life
            let alpha = p.life / PARTICLE_LIFETIME;
            let color = vec4<f32>(
                p.color.r,
                p.color.g,
                p.color.b,
                alpha
            );
            
            buffer_out[idx] = Pixel(
                u32(color.r * 255.0),
                u32(color.g * 255.0),
                u32(color.b * 255.0),
                u32(color.a * 255.0)
            );
        }
        
        i += 1u;
    }
}

// ============================================================================
// INTEGRATION NOTES
// ============================================================================
// 
// To integrate into sovereign_shell_hud.wgsl:
//
// 1. Add these bindings after @binding(8):
//    @group(0) @binding(9)  var<storage, read_write> prev_vm_stats: array<u32, 11>;
//    @group(0) @binding(10) var<storage, read_write> particles: array<Particle>;
//    @group(0) @binding(11) var<storage, read_write> particle_counter: atomic<u32>;
//
// 2. Add to main() after HUD rendering:
//    check_telemetry_pulses(vm_stats, prev_vm_stats, particles, particle_counter);
//    update_particles(particles, buffer_out, config.width, config.height);
//
// 3. Initialize prev_vm_stats to zero on first run (host-side)
// 4. Initialize particle_counter to zero (host-side)
//
// ============================================================================
