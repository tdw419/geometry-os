// Logic Expansion Shader — Layer 2 → Layer 1 → Layer 0
//
// Phase 12 Alpha: Double-Unpack for Computational Intelligence
// One instruction pixel → 8×8 logic gate → 3×3 glyphs → physical pixels
//
// Total compression: 576:1 (1 logic pixel → 576 physical pixels)

// ============================================================================
// STORAGE BINDINGS
// ============================================================================

@group(0) @binding(0) var<storage, read> layer_2_logic: array<u32>;     // Instructions (NAND, AND, etc.)
@group(0) @binding(1) var<storage, read> logic_blueprints: array<u64>;  // 8×8 Gate Layouts
@group(0) @binding(2) var<storage, read> glyph_atlas: array<u32>;       // 3×3 Evolved Glyphs
@group(0) @binding(3) var<storage, read> signal_state: array<u32>;      // Current signal values

// Output: Physical framebuffer
@group(0) @binding(4) var<storage, read_write> output_framebuffer: array<vec4f>;

// Constants
const LOGIC_BLOCK_SIZE: i32 = 24;  // 3×3 glyph × 8×8 grid = 24×24 pixels
const GLYPH_SIZE: i32 = 3;
const GRID_SIZE: i32 = 8;

// ============================================================================
// GATE BLUEPRINTS (64-bit masks for 8×8 layouts)
// ============================================================================

// NAND Gate: 0x00FF81818181FF00
// AND Gate:  0x00FF818181818100
// OR Gate:   0x0001818181818100
// XOR Gate:  0x0081810081818100

fn get_gate_blueprint(gate_id: u32) -> u64 {
    switch (gate_id) {
        case 1u: { return 0x00FF81818181FF00u64; }  // NAND
        case 2u: { return 0x00FF818181818100u64; }  // AND
        case 3u: { return 0x0001818181818100u64; }  // OR
        case 4u: { return 0x0081810081818100u64; }  // XOR
        case 5u: { return 0x000000FF00000000u64; }  // WIRE_H (horizontal)
        case 6u: { return 0x0080808080808000u64; }  // WIRE_V (vertical)
        case 7u: { return 0x00000000FF000000u64; }  // INPUT
        case 8u: { return 0x000000000000FF00u64; }  // OUTPUT
        default: { return 0u64; }
    }
}

// ============================================================================
// GLYPH ATLAS (3×3 patterns from Ouroboros evolution)
// ============================================================================

fn get_glyph_pattern(glyph_id: u32) -> u32 {
    // These patterns will be replaced by Ouroboros-evolved glyphs
    switch (glyph_id) {
        case 0u: { return 0b000000000u32; }  // Empty
        case 1u: { return 0b000010000u32; }  // Signal dot (center)
        case 2u: { return 0b000111000u32; }  // Signal bar (horizontal)
        case 3u: { return 0b010010010u32; }  // Signal bar (vertical)
        case 4u: { return 0b111111111u32; }  // Full block
        case 5u: { return 0b010111010u32; }  // High state (filled)
        case 6u: { return 0b010101010u32; }  // Low state (outline)
        case 7u: { return 0b000011100u32; }  // Arrow right
        case 8u: { return 0b111000000u32; }  // Arrow down
        default: { return 0u32; }
    }
}

// ============================================================================
// SIGNAL PROCESSING
// ============================================================================

fn evaluate_gate(gate_id: u32, input_a: bool, input_b: bool) -> bool {
    switch (gate_id) {
        case 1u: { return !(input_a && input_b); }  // NAND
        case 2u: { return input_a && input_b; }      // AND
        case 3u: { return input_a || input_b; }      // OR
        case 4u: { return input_a != input_b; }      // XOR
        default: { return false; }
    }
}

// ============================================================================
// MAIN EXPANSION LOGIC
// ============================================================================

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let phys_x = i32(id.x);
    let phys_y = i32(id.y);
    let phys_pos = vec2i(phys_x, phys_y);
    
    let width = 1280i;
    let height = 800i;
    
    // Bounds check
    if (phys_x >= width || phys_y >= height) { return; }
    
    // Skip HUD area (top 80 pixels)
    if (phys_y < 80) { return; }
    
    // Step 1: Calculate Layer 2 (Logic) position
    // Each logic block is 24×24 pixels (8×8 grid of 3×3 glyphs)
    let l2_x = phys_x / LOGIC_BLOCK_SIZE;
    let l2_y = (phys_y - 80) / LOGIC_BLOCK_SIZE;
    let l2_idx = u32(l2_y * (width / LOGIC_BLOCK_SIZE) + l2_x);
    
    // Step 2: Get the Logic Instruction (what gate lives here?)
    let instruction = layer_2_logic[l2_idx];
    
    if (instruction == 0u) {
        // Empty space - draw background
        output_framebuffer[u32(phys_y * width + phys_x)] = vec4f(0.02, 0.02, 0.05, 1.0);
        return;
    }
    
    // Step 3: Calculate local positions
    let block_local_x = phys_x % LOGIC_BLOCK_SIZE;
    let block_local_y = (phys_y - 80) % LOGIC_BLOCK_SIZE;
    
    // Position within 8×8 grid (Layer 1)
    let l1_x = block_local_x / GLYPH_SIZE;
    let l1_y = block_local_y / GLYPH_SIZE;
    let l1_idx = u32(l1_y * GRID_SIZE + l1_x);
    
    // Position within 3×3 glyph (Layer 0)
    let l0_x = block_local_x % GLYPH_SIZE;
    let l0_y = block_local_y % GLYPH_SIZE;
    let l0_idx = u32(l0_y * GLYPH_SIZE + l0_x);
    
    // Step 4: Unpack Logic Blueprint (Layer 2 → Layer 1)
    let blueprint = get_gate_blueprint(instruction);
    let is_wire = (blueprint >> l1_idx) & 1u;
    
    if (is_wire == 0u) {
        // Not a wire - background
        output_framebuffer[u32(phys_y * width + phys_x)] = vec4f(0.02, 0.02, 0.05, 1.0);
        return;
    }
    
    // Step 5: Determine glyph based on signal state
    let signal_idx = l2_idx * 64u + l1_idx;  // Each logic cell has 64 signals
    let signal_active = (signal_state[signal_idx / 32u] >> (signal_idx % 32u)) & 1u;
    
    // Select glyph based on signal
    let glyph_id = select(6u, 5u, signal_active == 1u);  // Low or High state
    
    // Step 6: Unpack Glyph Pattern (Layer 1 → Layer 0)
    let glyph_pattern = get_glyph_pattern(glyph_id);
    let pixel_active = (glyph_pattern >> l0_idx) & 1u;
    
    if (pixel_active == 1u) {
        // Determine color based on instruction type
        var color = vec3f(0.0, 1.0, 0.0);  // Default: Green (wire)
        
        switch (instruction) {
            case 1u: { color = vec3f(1.0, 0.0, 0.0); }      // NAND - Red
            case 2u: { color = vec3f(1.0, 0.5, 0.0); }      // AND - Orange
            case 3u: { color = vec3f(0.0, 0.5, 1.0); }      // OR - Blue
            case 4u: { color = vec3f(1.0, 0.0, 1.0); }      // XOR - Magenta
            case 7u: { color = vec3f(0.0, 1.0, 1.0); }      // INPUT - Cyan
            case 8u: { color = vec3f(1.0, 1.0, 0.0); }      // OUTPUT - Yellow
            default: { color = vec3f(0.0, 1.0, 0.0); }      // Wire - Green
        }
        
        // Brighten if signal is active
        if (signal_active == 1u) {
            color = color * 1.5;
        }
        
        output_framebuffer[u32(phys_y * width + phys_x)] = vec4f(color, 1.0);
    } else {
        // Empty pixel within wire area
        output_framebuffer[u32(phys_y * width + phys_x)] = vec4f(0.05, 0.05, 0.1, 1.0);
    }
}

// ============================================================================
// GRID LINES (Visual debugging)
// ============================================================================

fn draw_grid_lines(phys_pos: vec2i, color: ptr<function, vec3f>) {
    let block_local_x = phys_pos.x % LOGIC_BLOCK_SIZE;
    let block_local_y = (phys_pos.y - 80) % LOGIC_BLOCK_SIZE;
    
    // Draw logic block boundaries (dim)
    if (block_local_x == 0 || block_local_y == 0) {
        *color = vec3f(0.1, 0.1, 0.15);
    }
    
    // Draw glyph boundaries (very dim)
    let glyph_local_x = phys_pos.x % GLYPH_SIZE;
    let glyph_local_y = phys_pos.y % GLYPH_SIZE;
    
    if (glyph_local_x == 0 || glyph_local_y == 0) {
        *color = vec3f(0.05, 0.05, 0.08);
    }
}
