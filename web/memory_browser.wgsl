struct Memory {
    id: f32,
    opcode: f32,
    priority: f32,
    unused: f32,
}

struct Params {
    tension: f32,
    time: f32,
    memory_count: u32,
    zoom: f32,
    view_matrix: mat4x4<f32>,
    proj_matrix: mat4x4<f32>,
}

@group(0) @binding(0) var<storage, read> embeddings: array<f32>; // memory_count * 384
@group(0) @binding(1) var<storage, read> memories: array<Memory>;
@group(0) @binding(2) var<storage, read_write> positions: array<vec4<f32>>;
@group(0) @binding(3) var<uniform> params: Params;

// --- COMPUTE SHADER (Gravity) ---

fn get_hilbert_pos(index: u32) -> vec2<f32> {
    // Simplified 2D Hilbert decode for a 32x32 grid (1024 slots)
    var d = index;
    var x = 0u;
    var y = 0u;
    var s = 1u;
    while (s < 32u) {
        let rx = 1u & (d / 2u);
        let ry = 1u & (d ^ rx);
        
        // rot
        if (ry == 0u) {
            if (rx == 1u) {
                x = s - 1u - x;
                y = s - 1u - y;
            }
            let t = x;
            x = y;
            y = t;
        }
        
        x = x + s * rx;
        y = y + s * ry;
        d = d / 4u;
        s = s * 2u;
    }
    return vec2<f32>(f32(x), f32(y));
}

fn get_embedding(mem_idx: u32, element: u32) -> f32 {
    return embeddings[mem_idx * 384u + element];
}

@compute @workgroup_size(64)
fn compute_gravity(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let i = global_id.x;
    if (i >= params.memory_count) { return; }

    let mem = memories[i];
    let base_pos = get_hilbert_pos(i);
    
    var force = vec2<f32>(0.0, 0.0);
    
    // Semantic Gravity: Compare with top-K neighbors
    // For simplicity in this MVP, we compare with a stride of neighbors
    for (var j = 0u; j < params.memory_count; j = j + (params.memory_count / 8u)) {
        if (i == j) { continue; }
        
        // Dot product in 384-dimensions
        var sim = 0.0;
        for (var k = 0u; k < 384u; k = k + 8u) { // Sparse dot product for performance
            sim += get_embedding(i, k) * get_embedding(j, k);
        }
        
        let other_base = get_hilbert_pos(j);
        let dir = normalize(other_base - base_pos);
        force += dir * clamp(sim, 0.0, 1.0) * params.tension;
    }

    let final_x = base_pos.x + force.x;
    let final_y = base_pos.y + force.y;
    let final_z = mem.priority * 10.0;
    
    positions[i] = vec4<f32>(final_x, final_y, final_z, 1.0);
}

// --- RENDER SHADER (Particles) ---

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) memory_id: u32,
    @location(2) color: vec3<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) v_idx: u32,
    @builtin(instance_index) i_idx: u32
) -> VertexOutput {
    let pos = positions[i_idx].xyz;
    let mem = memories[i_idx];
    
    // Quad vertices (-0.5 to 0.5)
    var quad = array<vec2<f32>, 4>(
        vec2<f32>(-0.5, -0.5),
        vec2<f32>(0.5, -0.5),
        vec2<f32>(-0.5, 0.5),
        vec2<f32>(0.5, 0.5)
    );
    
    let offset = quad[v_idx] * 0.8; // Particle size
    let world_pos = vec4<f32>(pos.x + offset.x, pos.y + offset.y, pos.z, 1.0);
    
    var out: VertexOutput;
    out.position = params.proj_matrix * params.view_matrix * world_pos;
    out.uv = quad[v_idx] + 0.5;
    out.memory_id = i_idx;
    
    // Color by type
    if (mem.opcode == 16.0) { out.color = vec3<f32>(0.0, 1.0, 0.8); } // Note
    else if (mem.opcode == 96.0) { out.color = vec3<f32>(0.0, 1.0, 0.0); } // Code
    else { out.color = vec3<f32>(1.0, 1.0, 0.0); }
    
    return out;
}

@group(1) @binding(0) var atlas_sampler: sampler;
@group(1) @binding(1) var atlas_texture: texture_2d<f32>;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // For MVP, render as colored dots
    // Later integration will use atlas UVs from GeometryFont
    let dist = length(in.uv - 0.5);
    if (dist > 0.5) { discard; }
    
    let alpha = 1.0 - smoothstep(0.4, 0.5, dist);
    return vec4<f32>(in.color, alpha);
}

// --- PICKING SHADER ---

@fragment
fn fs_picking(in: VertexOutput) -> @location(0) u32 {
    return in.memory_id + 1u; // 0 is "no selection"
}
