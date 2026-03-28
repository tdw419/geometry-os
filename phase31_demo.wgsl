// Phase 31: Full GPU Demo Shader
// Target: 3000+ FPS on RTX 5090
//
// Combines:
// - Spatial physics (signal propagation)
// - Agent simulation (gravity wells)
// - Neural saccades (comet rendering)

// ============================================================================
// STRUCTS
// ============================================================================

struct Pixel {
    r: u32, g: u32, b: u32, a: u32,
}

struct AgentGpuState {
    pos_x: f32, pos_y: f32,
    vel_x: f32, vel_y: f32,
    color: u32,
    tribe: u32,
    signal_strength: f32,
    _padding: f32,
}

struct Well {
    pos_x: f32, pos_y: f32,
    strength: f32,
    z_index: f32,
}

struct SaccadePath {
    start_x: f32, start_y: f32,
    control_x: f32, control_y: f32,
    end_x: f32, end_y: f32,
    similarity: f32,
    effort: f32,
}

struct Uniforms {
    time: f32,
    frame: u32,
    width: u32,
    height: u32,
    agent_count: u32,
    well_count: u32,
    saccade_count: u32,
    mode: u32,
}

// ============================================================================
// BINDINGS
// ============================================================================

@group(0) @binding(0) var<storage, read_write> output: array<Pixel>;
@group(0) @binding(1) var<storage, read> agents: array<AgentGpuState>;
@group(0) @binding(2) var<storage, read> wells: array<Well>;
@group(0) @binding(3) var<storage, read> saccades: array<SaccadePath>;
@group(0) @binding(4) var<uniform> uniforms: Uniforms;

// ============================================================================
// UTILITIES
// ============================================================================

fn unpack_color(packed: u32) -> vec4f {
    return vec4f(
        f32((packed >> 24u) & 0xFFu) / 255.0,
        f32((packed >> 16u) & 0xFFu) / 255.0,
        f32((packed >> 8u) & 0xFFu) / 255.0,
        f32(packed & 0xFFu) / 255.0,
    );
}

fn pack_color(r: f32, g: f32, b: f32, a: f32) -> u32 {
    return (u32(r * 255.0) << 24u)
         | (u32(g * 255.0) << 16u)
         | (u32(b * 255.0) << 8u)
         | u32(a * 255.0);
}

fn get_pixel(x: u32, y: u32) -> vec4f {
    let idx = y * uniforms.width + x;
    let p = output[idx];
    return vec4f(
        f32(p.r) / 255.0,
        f32(p.g) / 255.0,
        f32(p.b) / 255.0,
        f32(p.a) / 255.0,
    );
}

fn set_pixel(x: u32, y: u32, color: vec4f) {
    if (x >= uniforms.width || y >= uniforms.height) { return; }
    let idx = y * uniforms.width + x;
    output[idx] = Pixel(
        u32(color.r * 255.0),
        u32(color.g * 255.0),
        u32(color.b * 255.0),
        u32(color.a * 255.0),
    );
}

fn blend_pixel(x: u32, y: u32, color: vec4f) {
    if (x >= uniforms.width || y >= uniforms.height) { return; }
    let existing = get_pixel(x, y);
    let blended = max(existing, color);
    set_pixel(x, y, blended);
}

// ============================================================================
// BEZIER
// ============================================================================

fn bezier_point(t: f32, p0: vec2f, p1: vec2f, p2: vec2f) -> vec2f {
    let one_minus_t = 1.0 - t;
    return one_minus_t * one_minus_t * p0
         + 2.0 * one_minus_t * t * p1
         + t * t * p2;
}

// ============================================================================
// COMPUTE MAIN
// ============================================================================

@compute @workgroup_size(8, 8, 1)
fn compute_main(@builtin(global_invocation_id) global_id: vec3u) {
    let x = global_id.x;
    let y = global_id.y;

    if (x >= uniforms.width || y >= uniforms.height) { return; }

    // Background
    var color = vec4f(0.02, 0.02, 0.04, 1.0);

    // Render wells
    for (var i = 0u; i < uniforms.well_count; i++) {
        let well = wells[i];
        let dx = f32(x) - well.pos_x;
        let dy = f32(y) - well.pos_y;
        let dist = sqrt(dx * dx + dy * dy);
        let radius = sqrt(well.strength) / 10.0;

        if (dist <= radius && dist >= radius - 5.0) {
            let alpha = 0.3 + 0.7 * well.z_index / 16.0;
            color = mix(color, vec4f(0.2, 0.4, 0.8, 1.0), alpha);
        }
    }

    // Render agents
    for (var i = 0u; i < uniforms.agent_count; i++) {
        let agent = agents[i];
        let dx = f32(x) - agent.pos_x;
        let dy = f32(y) - agent.pos_y;
        let dist = sqrt(dx * dx + dy * dy);

        // Agent body (3x3)
        if (dist <= 2.0) {
            let agent_color = unpack_color(agent.color);
            let intensity = select(0.5, 1.0, dist <= 1.0);
            color = max(color, vec4f(agent_color.rgb * intensity, 1.0));
        }

        // Signal glow
        if (agent.signal_strength > 0.0) {
            let glow_radius = agent.signal_strength * 10.0;
            if (dist <= glow_radius) {
                let alpha = (1.0 - dist / glow_radius) * agent.signal_strength;
                color = max(color, vec4f(1.0, 0.3, 0.0, alpha));
            }
        }
    }

    // Render saccades (neural paths)
    if (uniforms.mode >= 1u) {
        for (var i = 0u; i < uniforms.saccade_count; i++) {
            let path = saccades[i];

            let t = fract(uniforms.time * 0.5 + f32(i) * 0.1);
            let p0 = vec2f(path.start_x, path.start_y);
            let p1 = vec2f(path.control_x, path.control_y);
            let p2 = vec2f(path.end_x, path.end_y);

            let comet_pos = bezier_point(t, p0, p1, p2);

            let dx = f32(x) - comet_pos.x;
            let dy = f32(y) - comet_pos.y;
            let dist = sqrt(dx * dx + dy * dy);

            let size = 5.0 + path.effort * 10.0;

            if (dist <= size) {
                var base_color: vec3f;
                if (path.similarity >= 0.8) {
                    base_color = vec3f(0.0, 1.0, 0.0);  // Green
                } else if (path.similarity >= 0.5) {
                    base_color = vec3f(1.0, 1.0, 0.0);  // Yellow
                } else {
                    base_color = vec3f(1.0, 0.0, 0.0);  // Red
                }

                let alpha = (1.0 - dist / size) * 0.8;
                let shimmer = sin(uniforms.time * 10.0 + f32(i)) * 0.2 + 0.8;
                color = max(color, vec4f(base_color * shimmer, alpha));
            }
        }
    }

    // Scanline effect
    let scanline = sin(f32(y) * 0.5) * 0.03;
    color = vec4f(color.rgb - scanline, color.a);

    set_pixel(x, y, color);
}
