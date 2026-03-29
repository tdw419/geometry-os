// ============================================================================
// SOVEREIGN SHELL HUD SHADER - Natural Language Control for Geometry OS
// ============================================================================
// Architecture:
//   Row 0-399:   Agent execution space
//   Row 400-409: HUD header (SOVEREIGN SHELL)
//   Row 410-419: TELEMETRY ZONE (vm_stats readout)
//   Row 420-449: HUD zone (registers, messages)
//   Row 450-474: INPUT ZONE (user types here)
//   Row 475-479: PATCH STATUS (success/fail display)
// ============================================================================

// Telemetry zone constants
const TELEMETRY_ZONE_TOP: u32 = 410u;
const TELEMETRY_ZONE_BOTTOM: u32 = 420u;

struct Pixel {
    r: u32,
    g: u32,
    b: u32,
    a: u32,
}

struct Config {
    width: u32,
    height: u32,
    time: f32,
    frame: u32,
    mode: u32,
}

// Double buffers
@group(0) @binding(0) var<storage, read_write> buffer_out: array<Pixel>;
@group(0) @binding(1) var<storage, read> buffer_in: array<Pixel>;

// Register state (26 registers A-Z)
@group(0) @binding(2) var<storage, read> registers: array<u32>;
@group(0) @binding(3) var<storage, read> stack: array<u32>;
@group(0) @binding(4) var<uniform> config: Config;

// Stats (GPU status, IP, SP, telemetry)
@group(0) @binding(5) var<storage, read_write> vm_stats: array<atomic<u32>, 11>;

// ============================================================================
// CONSOLIDATED I/O STATE BUFFER (binding 6)
// Layout: input_text[64] | patch_status[1] | exec_result[1] | _pad[2]
// ============================================================================
struct IOState {
    input_text: array<u32, 64>,   // 256 bytes
    patch_status: u32,            // offset 256
    exec_result: u32,             // offset 260
    _pad: array<u32, 2>,          // align to 264 bytes
}

@group(0) @binding(6) var<storage, read> io_state: IOState;

// ============================================================================
// TELEMETRY BRIDGE (binding 7) - Particle System for Route Visualization
// ============================================================================
const MAX_PARTICLES: u32 = 256u;
const PARTICLE_LIFETIME: f32 = 300.0;  // 5 seconds at 60fps
const PARTICLE_BASE_SPEED: f32 = 0.5;  // Slower for visibility
const PARTICLE_MAX_SPEED: f32 = 3.0;

struct Particle {
    pos: vec2<f32>,
    vel: vec2<f32>,
    color: vec4<f32>,
    life: f32,
    route_id: u32,
    result_val: u32,
}

// Layout:
//   [0..44]   prev_vm_stats (11 u32s)
//   [48..52]  particle_counter (1 u32, atomic)
//   [64+]     particles (256 * 48 bytes each)
struct TelemetryBridge {
    prev_vm_stats: array<u32, 11>,
    _pad1: u32,  // offset 44
    _pad2: u32,  // offset 48 - will use this for counter
    _pad3: u32,  // offset 52
    particle_counter: atomic<u32>,  // offset 56
    _pad4: array<u32, 1>,  // align to 64 bytes
    particles: array<Particle, 256>,
}

@group(0) @binding(7) var<storage, read_write> telemetry_bridge: TelemetryBridge;

// ============================================================================
// 5x7 BITMAP FONT — Full ASCII support
// ============================================================================

fn get_font_column(char_code: u32, col: u32) -> u32 {
    // Digits 0-9 (char codes 48-57)
    if (char_code == 48u) {  // '0'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x51u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x45u; }
        if (col == 4u) { return 0x3Eu; }
    } else if (char_code == 49u) {  // '1'
        if (col == 0u) { return 0x42u; }
        if (col == 1u) { return 0x7Fu; }
        if (col == 2u) { return 0x40u; }
        return 0u;
    } else if (char_code == 50u) {  // '2'
        if (col == 0u) { return 0x62u; }
        if (col == 1u) { return 0x51u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x46u; }
    } else if (char_code == 51u) {  // '3'
        if (col == 0u) { return 0x22u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x36u; }
    } else if (char_code == 52u) {  // '4'
        if (col == 0u) { return 0x18u; }
        if (col == 1u) { return 0x14u; }
        if (col == 2u) { return 0x12u; }
        if (col == 3u) { return 0x7Fu; }
        if (col == 4u) { return 0x10u; }
    } else if (char_code == 53u) {  // '5'
        if (col == 0u) { return 0x27u; }
        if (col == 1u) { return 0x45u; }
        if (col == 2u) { return 0x45u; }
        if (col == 3u) { return 0x45u; }
        if (col == 4u) { return 0x39u; }
    } else if (char_code == 54u) {  // '6'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x32u; }
    } else if (char_code == 55u) {  // '7'
        if (col == 0u) { return 0x01u; }
        if (col == 1u) { return 0x71u; }
        if (col == 2u) { return 0x09u; }
        if (col == 3u) { return 0x05u; }
        if (col == 4u) { return 0x03u; }
    } else if (char_code == 56u) {  // '8'
        if (col == 0u) { return 0x36u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x36u; }
    } else if (char_code == 57u) {  // '9'
        if (col == 0u) { return 0x26u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x3Eu; }
    }
    // Letters A-Z (char codes 65-90)
    else if (char_code == 65u) {  // 'A'
        if (col == 0u) { return 0x7Eu; }
        if (col == 1u) { return 0x11u; }
        if (col == 2u) { return 0x11u; }
        if (col == 3u) { return 0x11u; }
        if (col == 4u) { return 0x7Eu; }
    } else if (char_code == 66u) {  // 'B'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x36u; }
    } else if (char_code == 67u) {  // 'C'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x41u; }
        if (col == 4u) { return 0x22u; }
    } else if (char_code == 68u) {  // 'D'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x22u; }
        if (col == 4u) { return 0x1Cu; }
    } else if (char_code == 69u) {  // 'E'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x41u; }
    } else if (char_code == 70u) {  // 'F'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x09u; }
        if (col == 2u) { return 0x09u; }
        if (col == 3u) { return 0x09u; }
        if (col == 4u) { return 0x01u; }
    } else if (char_code == 71u) {  // 'G'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x7Au; }
    } else if (char_code == 72u) {  // 'H'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x7Fu; }
    } else if (char_code == 73u) {  // 'I'
        if (col == 0u) { return 0x41u; }
        if (col == 1u) { return 0x7Fu; }
        if (col == 2u) { return 0x41u; }
        return 0u;
    } else if (char_code == 74u) {  // 'J'
        if (col == 0u) { return 0x20u; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x3Fu; }
        if (col == 4u) { return 0x01u; }
    } else if (char_code == 75u) {  // 'K'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x14u; }
        if (col == 3u) { return 0x22u; }
        if (col == 4u) { return 0x41u; }
    } else if (char_code == 76u) {  // 'L'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x40u; }
    } else if (char_code == 77u) {  // 'M'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x02u; }
        if (col == 2u) { return 0x0Cu; }
        if (col == 3u) { return 0x02u; }
        if (col == 4u) { return 0x7Fu; }
    } else if (char_code == 78u) {  // 'N'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x04u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x10u; }
        if (col == 4u) { return 0x7Fu; }
    } else if (char_code == 79u) {  // 'O'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x41u; }
        if (col == 4u) { return 0x3Eu; }
    } else if (char_code == 80u) {  // 'P'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x09u; }
        if (col == 2u) { return 0x09u; }
        if (col == 3u) { return 0x09u; }
        if (col == 4u) { return 0x06u; }
    } else if (char_code == 81u) {  // 'Q'
        if (col == 0u) { return 0x3Eu; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x51u; }
        if (col == 3u) { return 0x21u; }
        if (col == 4u) { return 0x5Eu; }
    } else if (char_code == 82u) {  // 'R'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x09u; }
        if (col == 2u) { return 0x19u; }
        if (col == 3u) { return 0x29u; }
        if (col == 4u) { return 0x46u; }
    } else if (char_code == 83u) {  // 'S'
        if (col == 0u) { return 0x26u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x49u; }
        if (col == 4u) { return 0x32u; }
    } else if (char_code == 84u) {  // 'T'
        if (col == 0u) { return 0x01u; }
        if (col == 1u) { return 0x01u; }
        if (col == 2u) { return 0x7Fu; }
        if (col == 3u) { return 0x01u; }
        if (col == 4u) { return 0x01u; }
    } else if (char_code == 85u) {  // 'U'
        if (col == 0u) { return 0x3Fu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x3Fu; }
    } else if (char_code == 86u) {  // 'V'
        if (col == 0u) { return 0x1Fu; }
        if (col == 1u) { return 0x20u; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x20u; }
        if (col == 4u) { return 0x1Fu; }
    } else if (char_code == 87u) {  // 'W'
        if (col == 0u) { return 0x3Fu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x38u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x3Fu; }
    } else if (char_code == 88u) {  // 'X'
        if (col == 0u) { return 0x63u; }
        if (col == 1u) { return 0x14u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x14u; }
        if (col == 4u) { return 0x63u; }
    } else if (char_code == 89u) {  // 'Y'
        if (col == 0u) { return 0x07u; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x70u; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x07u; }
    } else if (char_code == 90u) {  // 'Z'
        if (col == 0u) { return 0x61u; }
        if (col == 1u) { return 0x51u; }
        if (col == 2u) { return 0x49u; }
        if (col == 3u) { return 0x45u; }
        if (col == 4u) { return 0x43u; }
    }
    // Lowercase a-z (char codes 97-122)
    else if (char_code == 97u) {  // 'a'
        if (col == 0u) { return 0x20u; }
        if (col == 1u) { return 0x54u; }
        if (col == 2u) { return 0x54u; }
        if (col == 3u) { return 0x54u; }
        if (col == 4u) { return 0x78u; }
    } else if (char_code == 98u) {  // 'b'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x48u; }
        if (col == 2u) { return 0x44u; }
        if (col == 3u) { return 0x44u; }
        if (col == 4u) { return 0x38u; }
    } else if (char_code == 99u) {  // 'c'
        if (col == 0u) { return 0x38u; }
        if (col == 1u) { return 0x44u; }
        if (col == 2u) { return 0x44u; }
        if (col == 3u) { return 0x44u; }
        if (col == 4u) { return 0x20u; }
    } else if (char_code == 100u) {  // 'd'
        if (col == 0u) { return 0x38u; }
        if (col == 1u) { return 0x44u; }
        if (col == 2u) { return 0x44u; }
        if (col == 3u) { return 0x48u; }
        if (col == 4u) { return 0x7Fu; }
    } else if (char_code == 101u) {  // 'e'
        if (col == 0u) { return 0x38u; }
        if (col == 1u) { return 0x54u; }
        if (col == 2u) { return 0x54u; }
        if (col == 3u) { return 0x54u; }
        if (col == 4u) { return 0x18u; }
    } else if (char_code == 102u) {  // 'f'
        if (col == 0u) { return 0x08u; }
        if (col == 1u) { return 0x7Eu; }
        if (col == 2u) { return 0x09u; }
        if (col == 3u) { return 0x01u; }
        if (col == 4u) { return 0x02u; }
    } else if (char_code == 103u) {  // 'g'
        if (col == 0u) { return 0x0Cu; }
        if (col == 1u) { return 0x52u; }
        if (col == 2u) { return 0x52u; }
        if (col == 3u) { return 0x52u; }
        if (col == 4u) { return 0x3Eu; }
    } else if (char_code == 104u) {  // 'h'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x04u; }
        if (col == 3u) { return 0x04u; }
        if (col == 4u) { return 0x78u; }
    } else if (char_code == 105u) {  // 'i'
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x44u; }
        if (col == 2u) { return 0x7Du; }
        if (col == 3u) { return 0x40u; }
        return 0u;
    } else if (char_code == 106u) {  // 'j'
        if (col == 0u) { return 0x20u; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x44u; }
        if (col == 3u) { return 0x3Du; }
        return 0u;
    } else if (char_code == 107u) {  // 'k'
        if (col == 0u) { return 0x7Fu; }
        if (col == 1u) { return 0x10u; }
        if (col == 2u) { return 0x28u; }
        if (col == 3u) { return 0x44u; }
        return 0u;
    } else if (char_code == 108u) {  // 'l'
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x7Fu; }
        if (col == 3u) { return 0x40u; }
        return 0u;
    } else if (char_code == 109u) {  // 'm'
        if (col == 0u) { return 0x7Cu; }
        if (col == 1u) { return 0x04u; }
        if (col == 2u) { return 0x18u; }
        if (col == 3u) { return 0x04u; }
        if (col == 4u) { return 0x78u; }
    } else if (char_code == 110u) {  // 'n'
        if (col == 0u) { return 0x7Cu; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x04u; }
        if (col == 3u) { return 0x04u; }
        if (col == 4u) { return 0x78u; }
    } else if (char_code == 111u) {  // 'o'
        if (col == 0u) { return 0x38u; }
        if (col == 1u) { return 0x44u; }
        if (col == 2u) { return 0x44u; }
        if (col == 3u) { return 0x44u; }
        if (col == 4u) { return 0x38u; }
    } else if (char_code == 112u) {  // 'p'
        if (col == 0u) { return 0x7Cu; }
        if (col == 1u) { return 0x14u; }
        if (col == 2u) { return 0x14u; }
        if (col == 3u) { return 0x14u; }
        if (col == 4u) { return 0x08u; }
    } else if (char_code == 113u) {  // 'q'
        if (col == 0u) { return 0x08u; }
        if (col == 1u) { return 0x14u; }
        if (col == 2u) { return 0x14u; }
        if (col == 3u) { return 0x18u; }
        if (col == 4u) { return 0x7Cu; }
    } else if (char_code == 114u) {  // 'r'
        if (col == 0u) { return 0x7Cu; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x04u; }
        if (col == 3u) { return 0x04u; }
        if (col == 4u) { return 0x08u; }
    } else if (char_code == 115u) {  // 's'
        if (col == 0u) { return 0x48u; }
        if (col == 1u) { return 0x54u; }
        if (col == 2u) { return 0x54u; }
        if (col == 3u) { return 0x54u; }
        if (col == 4u) { return 0x20u; }
    } else if (char_code == 116u) {  // 't'
        if (col == 0u) { return 0x04u; }
        if (col == 1u) { return 0x3Fu; }
        if (col == 2u) { return 0x44u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x20u; }
    } else if (char_code == 117u) {  // 'u'
        if (col == 0u) { return 0x3Cu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x20u; }
        if (col == 4u) { return 0x7Cu; }
    } else if (char_code == 118u) {  // 'v'
        if (col == 0u) { return 0x1Cu; }
        if (col == 1u) { return 0x20u; }
        if (col == 2u) { return 0x40u; }
        if (col == 3u) { return 0x20u; }
        if (col == 4u) { return 0x1Cu; }
    } else if (char_code == 119u) {  // 'w'
        if (col == 0u) { return 0x3Cu; }
        if (col == 1u) { return 0x40u; }
        if (col == 2u) { return 0x30u; }
        if (col == 3u) { return 0x40u; }
        if (col == 4u) { return 0x3Cu; }
    } else if (char_code == 120u) {  // 'x'
        if (col == 0u) { return 0x44u; }
        if (col == 1u) { return 0x28u; }
        if (col == 2u) { return 0x10u; }
        if (col == 3u) { return 0x28u; }
        if (col == 4u) { return 0x44u; }
    } else if (char_code == 121u) {  // 'y'
        if (col == 0u) { return 0x0Cu; }
        if (col == 1u) { return 0x50u; }
        if (col == 2u) { return 0x50u; }
        if (col == 3u) { return 0x50u; }
        if (col == 4u) { return 0x3Cu; }
    } else if (char_code == 122u) {  // 'z'
        if (col == 0u) { return 0x44u; }
        if (col == 1u) { return 0x64u; }
        if (col == 2u) { return 0x54u; }
        if (col == 3u) { return 0x4Cu; }
        if (col == 4u) { return 0x44u; }
    }
    // Special characters
    else if (char_code == 32u) {  // ' ' (space)
        return 0u;
    } else if (char_code == 33u) {  // '!'
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x00u; }
        if (col == 2u) { return 0x5Fu; }
        return 0u;
    } else if (char_code == 34u) {  // '"'
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x07u; }
        if (col == 2u) { return 0x00u; }
        if (col == 3u) { return 0x07u; }
        return 0u;
    } else if (char_code == 40u) {  // '('
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x1Cu; }
        if (col == 2u) { return 0x22u; }
        if (col == 3u) { return 0x41u; }
        return 0u;
    } else if (char_code == 41u) {  // ')'
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x22u; }
        if (col == 3u) { return 0x1Cu; }
        return 0u;
    } else if (char_code == 42u) {  // '*'
        if (col == 0u) { return 0x14u; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x3Eu; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x14u; }
    } else if (char_code == 43u) {  // '+'
        if (col == 0u) { return 0x08u; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x3Eu; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x08u; }
    } else if (char_code == 44u) {  // ','
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x80u; }
        if (col == 2u) { return 0x60u; }
        return 0u;
    } else if (char_code == 45u) {  // '-'
        if (col == 0u) { return 0x08u; }
        if (col == 1u) { return 0x08u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x08u; }
        if (col == 4u) { return 0x08u; }
    } else if (char_code == 46u) {  // '.'
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x00u; }
        if (col == 2u) { return 0x60u; }
        return 0u;
    } else if (char_code == 47u) {  // '/'
        if (col == 0u) { return 0x20u; }
        if (col == 1u) { return 0x10u; }
        if (col == 2u) { return 0x08u; }
        if (col == 3u) { return 0x04u; }
        if (col == 4u) { return 0x02u; }
    } else if (char_code == 58u) {  // ':'
        if (col == 1u) { return 0x36u; }
        if (col == 2u) { return 0x36u; }
        return 0u;
    } else if (char_code == 59u) {  // ';'
        if (col == 1u) { return 0x36u; }
        if (col == 2u) { return 0x60u; }
        return 0u;
    } else if (char_code == 60u) {  // '<'
        if (col == 0u) { return 0x08u; }
        if (col == 1u) { return 0x1Cu; }
        if (col == 2u) { return 0x22u; }
        return 0u;
    } else if (char_code == 61u) {  // '='
        if (col == 1u) { return 0x7Fu; }
        if (col == 3u) { return 0x7Fu; }
        return 0u;
    } else if (char_code == 62u) {  // '>'
        if (col == 0u) { return 0x22u; }
        if (col == 1u) { return 0x1Cu; }
        if (col == 2u) { return 0x08u; }
        return 0u;
    } else if (char_code == 63u) {  // '?'
        if (col == 0u) { return 0x02u; }
        if (col == 1u) { return 0x01u; }
        if (col == 2u) { return 0x51u; }
        if (col == 3u) { return 0x09u; }
        if (col == 4u) { return 0x06u; }
    } else if (char_code == 64u) {  // '@'
        if (col == 0u) { return 0x32u; }
        if (col == 1u) { return 0x49u; }
        if (col == 2u) { return 0x79u; }
        if (col == 3u) { return 0x41u; }
        if (col == 4u) { return 0x3Eu; }
    } else if (char_code == 91u) {  // '['
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x7Fu; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x41u; }
        return 0u;
    } else if (char_code == 93u) {  // ']'
        if (col == 0u) { return 0x00u; }
        if (col == 1u) { return 0x41u; }
        if (col == 2u) { return 0x41u; }
        if (col == 3u) { return 0x7Fu; }
        return 0u;
    } else if (char_code == 95u) {  // '_'
        if (col == 0u) { return 0x80u; }
        if (col == 1u) { return 0x80u; }
        if (col == 2u) { return 0x80u; }
        if (col == 3u) { return 0x80u; }
        if (col == 4u) { return 0x80u; }
    }
    
    return 0u;
}

// Draw a character at position (x, y) in the framebuffer
fn draw_char(char_code: u32, x: u32, y: u32, color: Pixel) -> u32 {
    var col = 0u;
    loop {
        if (col >= 5u) { break; }
        
        let byte = get_font_column(char_code, col);
        var row = 0u;
        loop {
            if (row >= 7u) { break; }
            
            if ((byte >> row) & 1u) == 1u {
                let px = x + col;
                let py = y + row;
                if (px < config.width && py < config.height) {
                    let i = py * config.width + px;
                    buffer_out[i] = color;
                }
            }
            
            row += 1u;
        }
        
        col += 1u;
    }
    
    return x + 6u;  // 5 pixels + 1 pixel spacing
}

// Draw a number (0-9999) as up to 4 digits
fn draw_number(value: u32, x: u32, y: u32, color: Pixel) -> u32 {
    let thousands = (value / 1000u) % 10u;
    let hundreds = (value / 100u) % 10u;
    let tens = (value / 10u) % 10u;
    let ones = value % 10u;
    
    var cursor_x = x;
    
    // Skip leading zeros for thousands/hundreds
    if value >= 1000u {
        cursor_x = draw_char(48u + thousands, cursor_x, y, color);
    }
    if value >= 100u {
        cursor_x = draw_char(48u + hundreds, cursor_x, y, color);
    }
    if value >= 10u {
        cursor_x = draw_char(48u + tens, cursor_x, y, color);
    }
    cursor_x = draw_char(48u + ones, cursor_x, y, color);
    
    return cursor_x;
}

// ============================================================================
// HUD RENDERER — Rows 400-449
// ============================================================================

fn render_hud() {
    // HUD colors
    var header_color: Pixel;
    header_color.r = 0u;
    header_color.g = 200u;
    header_color.b = 255u;
    header_color.a = 255u;
    
    var value_color: Pixel;
    value_color.r = 255u;
    value_color.g = 255u;
    value_color.b = 255u;
    value_color.a = 255u;
    
    var bg_color: Pixel;
    bg_color.r = 15u;
    bg_color.g = 25u;
    bg_color.b = 35u;
    bg_color.a = 255u;
    
    // Clear HUD area (rows 400-449)
    var y = 400u;
    loop {
        if (y >= 450u) { break; }
        
        var x = 0u;
        loop {
            if (x >= config.width) { break; }
            let i = y * config.width + x;
            buffer_out[i] = bg_color;
            x += 1u;
        }
        y += 1u;
    }
    
    // Draw "SOVEREIGN SHELL" header
    var cursor_x = 20u;
    var cursor_y = 405u;
    
    // S-O-V-E-R-E-I-G-N
    cursor_x = draw_char(83u, cursor_x, cursor_y, header_color);   // S
    cursor_x = draw_char(79u, cursor_x, cursor_y, header_color);   // O
    cursor_x = draw_char(86u, cursor_x, cursor_y, header_color);   // V
    cursor_x = draw_char(69u, cursor_x, cursor_y, header_color);   // E
    cursor_x = draw_char(82u, cursor_x, cursor_y, header_color);   // R
    cursor_x = draw_char(69u, cursor_x, cursor_y, header_color);   // E
    cursor_x = draw_char(73u, cursor_x, cursor_y, header_color);   // I
    cursor_x = draw_char(71u, cursor_x, cursor_y, header_color);   // G
    cursor_x = draw_char(78u, cursor_x, cursor_y, header_color);   // N
    
    cursor_x += 10u;
    
    // S-H-E-L-L
    cursor_x = draw_char(83u, cursor_x, cursor_y, header_color);   // S
    cursor_x = draw_char(72u, cursor_x, cursor_y, header_color);   // H
    cursor_x = draw_char(69u, cursor_x, cursor_y, header_color);   // E
    cursor_x = draw_char(76u, cursor_x, cursor_y, header_color);   // L
    cursor_x = draw_char(76u, cursor_x, cursor_y, header_color);   // L
    
    // Draw register values (A-J) in row 420
    cursor_x = 20u;
    cursor_y = 420u;
    
    var i = 0u;
    loop {
        if (i >= 10u) { break; }
        
        // Register name (A=65, B=66, ...)
        let reg_name = 65u + i;
        cursor_x = draw_char(reg_name, cursor_x, cursor_y, header_color);
        cursor_x = draw_char(58u, cursor_x, cursor_y, header_color);  // ':'
        
        // Register value
        let value = registers[i];
        cursor_x = draw_number(value, cursor_x, cursor_y, value_color);
        
        // Spacing
        cursor_x += 8u;
        
        i += 1u;
    }
    
    // Draw IP, SP, and Stack depth at row 435
    cursor_x = 20u;
    cursor_y = 435u;
    
    cursor_x = draw_char(73u, cursor_x, cursor_y, header_color);   // I
    cursor_x = draw_char(80u, cursor_x, cursor_y, header_color);   // P
    cursor_x = draw_char(58u, cursor_x, cursor_y, header_color);   // :
    let ip = vm_stats[1u];
    cursor_x = draw_number(ip, cursor_x + 5u, cursor_y, value_color);
    
    cursor_x += 15u;
    
    cursor_x = draw_char(83u, cursor_x, cursor_y, header_color);   // S
    cursor_x = draw_char(80u, cursor_x, cursor_y, header_color);   // P
    cursor_x = draw_char(58u, cursor_x, cursor_y, header_color);   // :
    let sp = vm_stats[2u];
    cursor_x = draw_number(sp, cursor_x + 5u, cursor_y, value_color);
    
    cursor_x += 15u;
    
    // Execution result
    cursor_x = draw_char(61u, cursor_x, cursor_y, header_color);   // =
    cursor_x = draw_char(62u, cursor_x, cursor_y, header_color);   // >
    cursor_x += 5u;
    let result = io_state.exec_result;
    cursor_x = draw_number(result, cursor_x, cursor_y, value_color);
}

// ============================================================================
// INPUT ZONE — Rows 450-474
// ============================================================================

fn render_input_zone() {
    var prompt_color: Pixel;
    prompt_color.r = 0u;
    prompt_color.g = 255u;
    prompt_color.b = 128u;
    prompt_color.a = 255u;
    
    var input_color: Pixel;
    input_color.r = 255u;
    input_color.g = 255u;
    input_color.b = 255u;
    input_color.a = 255u;
    
    var bg_color: Pixel;
    bg_color.r = 25u;
    bg_color.g = 35u;
    bg_color.b = 45u;
    bg_color.a = 255u;
    
    var border_color: Pixel;
    border_color.r = 0u;
    border_color.g = 128u;
    border_color.b = 255u;
    border_color.a = 255u;
    
    // Clear input zone (rows 450-474)
    var y = 450u;
    loop {
        if (y >= 475u) { break; }
        
        var x = 0u;
        loop {
            if (x >= config.width) { break; }
            
            let i = y * config.width + x;
            
            // Draw border on first and last row
            if (y == 450u || y == 474u) {
                buffer_out[i] = border_color;
            } else {
                buffer_out[i] = bg_color;
            }
            
            x += 1u;
        }
        y += 1u;
    }
    
    // Draw prompt "> " at row 455
    var cursor_x = 15u;
    var cursor_y = 455u;
    cursor_x = draw_char(62u, cursor_x, cursor_y, prompt_color);   // >
    cursor_x = draw_char(32u, cursor_x, cursor_y, prompt_color);   // space
    cursor_x += 5u;
    
    // Draw input buffer contents
    var i = 0u;
    loop {
        if (i >= 64u) { break; }
        let ch = io_state.input_text[i];
        if (ch == 0u) { break; }  // Null terminator
        cursor_x = draw_char(ch, cursor_x, cursor_y, input_color);
        i += 1u;
    }
    
    // Draw blinking cursor (based on frame number)
    let show_cursor = (config.frame % 60u) < 30u;
    if (show_cursor) {
        // Draw underscore cursor
        let cursor_char: u32 = 95u;  // '_'
        _ = draw_char(cursor_char, cursor_x, cursor_y, prompt_color);
    }
}

// ============================================================================
// PATCH STATUS — Rows 475-479
// ============================================================================

fn render_patch_status() {
    var success_color: Pixel;
    success_color.r = 0u;
    success_color.g = 255u;
    success_color.b = 0u;
    success_color.a = 255u;
    
    var fail_color: Pixel;
    fail_color.r = 255u;
    fail_color.g = 0u;
    fail_color.b = 0u;
    fail_color.a = 255u;
    
    var neutral_color: Pixel;
    neutral_color.r = 128u;
    neutral_color.g = 128u;
    neutral_color.b = 128u;
    neutral_color.a = 255u;
    
    var bg_color: Pixel;
    bg_color.r = 20u;
    bg_color.g = 20u;
    bg_color.b = 30u;
    bg_color.a = 255u;
    
    // Clear status zone (rows 475-479)
    var y = 475u;
    loop {
        if (y >= 480u) { break; }
        
        var x = 0u;
        loop {
            if (x >= config.width) { break; }
            let i = y * config.width + x;
            buffer_out[i] = bg_color;
            x += 1u;
        }
        y += 1u;
    }
    
    // Get patch status
    let status = io_state.patch_status;
    
    var cursor_x = 20u;
    let cursor_y = 476u;
    
    if (status == 1u) {
        // PATCH_SUCCESS in green
        cursor_x = draw_char(80u, cursor_x, cursor_y, success_color);   // P
        cursor_x = draw_char(65u, cursor_x, cursor_y, success_color);   // A
        cursor_x = draw_char(84u, cursor_x, cursor_y, success_color);   // T
        cursor_x = draw_char(67u, cursor_x, cursor_y, success_color);   // C
        cursor_x = draw_char(72u, cursor_x, cursor_y, success_color);   // H
        cursor_x = draw_char(95u, cursor_x, cursor_y, success_color);   // _
        cursor_x = draw_char(83u, cursor_x, cursor_y, success_color);   // S
        cursor_x = draw_char(85u, cursor_x, cursor_y, success_color);   // U
        cursor_x = draw_char(67u, cursor_x, cursor_y, success_color);   // C
        cursor_x = draw_char(67u, cursor_x, cursor_y, success_color);   // C
        cursor_x = draw_char(69u, cursor_x, cursor_y, success_color);   // E
        cursor_x = draw_char(83u, cursor_x, cursor_y, success_color);   // S
        cursor_x = draw_char(83u, cursor_x, cursor_y, success_color);   // S
    } else if (status == 2u) {
        // PATCH_FAIL in red
        cursor_x = draw_char(80u, cursor_x, cursor_y, fail_color);   // P
        cursor_x = draw_char(65u, cursor_x, cursor_y, fail_color);   // A
        cursor_x = draw_char(84u, cursor_x, cursor_y, fail_color);   // T
        cursor_x = draw_char(67u, cursor_x, cursor_y, fail_color);   // C
        cursor_x = draw_char(72u, cursor_x, cursor_y, fail_color);   // H
        cursor_x = draw_char(95u, cursor_x, cursor_y, fail_color);   // _
        cursor_x = draw_char(70u, cursor_x, cursor_y, fail_color);   // F
        cursor_x = draw_char(65u, cursor_x, cursor_y, fail_color);   // A
        cursor_x = draw_char(73u, cursor_x, cursor_y, fail_color);   // I
        cursor_x = draw_char(76u, cursor_x, cursor_y, fail_color);   // L
    } else {
        // Ready state
        cursor_x = draw_char(82u, cursor_x, cursor_y, neutral_color);   // R
        cursor_x = draw_char(69u, cursor_x, cursor_y, neutral_color);   // E
        cursor_x = draw_char(65u, cursor_x, cursor_y, neutral_color);   // A
        cursor_x = draw_char(68u, cursor_x, cursor_y, neutral_color);   // D
        cursor_x = draw_char(89u, cursor_x, cursor_y, neutral_color);   // Y
    }
}

// ============================================================================
// TELEMETRY HUD — Rows 410-419 (GlyphLang Metrics)
// ============================================================================
// vm_stats layout:
//   [0] GPU Status (1=ONLINE)
//   [1] IP (instruction pointer)
//   [2] SP (stack pointer)
//   [3] Requests (atomic)
//   [4] Errors (atomic)
//   [5] Latency (fixed-point ms*10)
//   [6] Active Routes bitmask
//   [7-10] Reserved
// ============================================================================

fn render_telemetry_hud() {
    var label_color: Pixel;
    label_color.r = 100u;
    label_color.g = 180u;
    label_color.b = 255u;
    label_color.a = 255u;

    var value_color: Pixel;
    value_color.r = 0u;
    value_color.g = 255u;
    value_color.b = 100u;
    value_color.a = 255u;

    var error_color: Pixel;
    error_color.r = 255u;
    error_color.g = 80u;
    error_color.b = 80u;
    error_color.a = 255u;

    var dim_color: Pixel;
    dim_color.r = 80u;
    dim_color.g = 80u;
    dim_color.b = 100u;
    dim_color.a = 255u;

    var bg_color: Pixel;
    bg_color.r = 10u;
    bg_color.g = 15u;
    bg_color.b = 25u;
    bg_color.a = 255u;

    // Clear telemetry zone (rows 410-419) but skip particle lanes
    // Particles spawn at y = 410 + route_id*2 for routes 3-6
    // So skip rows: 416 (route 3), 418 (route 4), 420 (route 5), 422 (route 6)
    var y = TELEMETRY_ZONE_TOP;
    loop {
        if (y >= TELEMETRY_ZONE_BOTTOM) { break; }
        
        // Skip particle lanes (rows 416, 418, 420, 422)
        let is_particle_lane = (y >= 416u && y <= 422u && y % 2u == 0u);
        
        var x = 0u;
        loop {
            if (x >= config.width) { break; }
            
            if (!is_particle_lane) {
                let i = y * config.width + x;
                buffer_out[i] = bg_color;
            }
            
            x += 1u;
        }
        y += 1u;
    }

    var cursor_x: u32;
    var cursor_y: u32;

    // Row 410: GPU Status indicator
    cursor_x = 10u;
    cursor_y = TELEMETRY_ZONE_TOP;
    cursor_x = draw_char(91u, cursor_x, cursor_y, label_color);   // [
    cursor_x = draw_char(71u, cursor_x, cursor_y, label_color);   // G
    cursor_x = draw_char(80u, cursor_x, cursor_y, label_color);   // P
    cursor_x = draw_char(85u, cursor_x, cursor_y, label_color);   // U
    cursor_x = draw_char(58u, cursor_x, cursor_y, label_color);   // :

    let gpu_status = vm_stats[0];
    if (gpu_status == 1u) {
        cursor_x = draw_char(32u, cursor_x, cursor_y, value_color);  // space
        cursor_x = draw_char(79u, cursor_x, cursor_y, value_color);  // O
        cursor_x = draw_char(78u, cursor_x, cursor_y, value_color);  // N
        cursor_x = draw_char(76u, cursor_x, cursor_y, value_color);  // L
        cursor_x = draw_char(73u, cursor_x, cursor_y, value_color);  // I
        cursor_x = draw_char(78u, cursor_x, cursor_y, value_color);  // N
        cursor_x = draw_char(69u, cursor_x, cursor_y, value_color);  // E
    } else {
        cursor_x = draw_char(32u, cursor_x, cursor_y, error_color);  // space
        cursor_x = draw_char(79u, cursor_x, cursor_y, error_color);  // O
        cursor_x = draw_char(70u, cursor_x, cursor_y, error_color);  // F
        cursor_x = draw_char(70u, cursor_x, cursor_y, error_color);  // F
        cursor_x = draw_char(76u, cursor_x, cursor_y, error_color);  // L
        cursor_x = draw_char(73u, cursor_x, cursor_y, error_color);  // I
        cursor_x = draw_char(78u, cursor_x, cursor_y, error_color);  // N
        cursor_x = draw_char(69u, cursor_x, cursor_y, error_color);  // E
    }
    cursor_x = draw_char(32u, cursor_x, cursor_y, label_color);   // space
    cursor_x = draw_char(93u, cursor_x, cursor_y, label_color);   // ]

    // Row 411: IP and SP
    cursor_x = 10u;
    cursor_y = TELEMETRY_ZONE_TOP + 1u;
    cursor_x = draw_char(73u, cursor_x, cursor_y, label_color);   // I
    cursor_x = draw_char(80u, cursor_x, cursor_y, label_color);   // P
    cursor_x = draw_char(58u, cursor_x, cursor_y, label_color);   // :
    cursor_x = draw_char(32u, cursor_x, cursor_y, label_color);   // space
    cursor_x = draw_char(48u, cursor_x, cursor_y, dim_color);     // 0
    cursor_x = draw_char(120u, cursor_x, cursor_y, dim_color);    // x
    cursor_x = draw_hex_u32(vm_stats[1], cursor_x, cursor_y, value_color);

    cursor_x += 10u;  // Gap
    cursor_x = draw_char(83u, cursor_x, cursor_y, label_color);   // S
    cursor_x = draw_char(80u, cursor_x, cursor_y, label_color);   // P
    cursor_x = draw_char(58u, cursor_x, cursor_y, label_color);   // :
    cursor_x = draw_char(32u, cursor_x, cursor_y, label_color);   // space
    cursor_x = draw_char(48u, cursor_x, cursor_y, dim_color);     // 0
    cursor_x = draw_char(120u, cursor_x, cursor_y, dim_color);    // x
    cursor_x = draw_hex_u32(vm_stats[2], cursor_x, cursor_y, value_color);

    // Row 412: Request counter with bar
    cursor_x = 10u;
    cursor_y = TELEMETRY_ZONE_TOP + 2u;
    cursor_x = draw_char(82u, cursor_x, cursor_y, label_color);   // R
    cursor_x = draw_char(69u, cursor_x, cursor_y, label_color);   // E
    cursor_x = draw_char(81u, cursor_x, cursor_y, label_color);   // Q
    cursor_x = draw_char(58u, cursor_x, cursor_y, label_color);   // :
    cursor_x = draw_char(32u, cursor_x, cursor_y, label_color);   // space
    cursor_x = draw_char(91u, cursor_x, cursor_y, dim_color);     // [

    // Draw request bar (max 10 chars)
    let req_count = vm_stats[3];
    let req_bar_len = min(req_count, 10u);
    var bar_i = 0u;
    loop {
        if (bar_i >= 10u) { break; }
        if (bar_i < req_bar_len) {
            cursor_x = draw_char(124u, cursor_x, cursor_y, value_color);  // |
        } else {
            cursor_x = draw_char(46u, cursor_x, cursor_y, dim_color);     // .
        }
        bar_i += 1u;
    }
    cursor_x = draw_char(93u, cursor_x, cursor_y, dim_color);     // ]
    cursor_x = draw_char(32u, cursor_x, cursor_y, label_color);   // space
    cursor_x = draw_decimal_u32(req_count, cursor_x, cursor_y, value_color);

    // Row 413: Error counter with indicator
    cursor_x = 10u;
    cursor_y = TELEMETRY_ZONE_TOP + 3u;
    cursor_x = draw_char(69u, cursor_x, cursor_y, label_color);   // E
    cursor_x = draw_char(82u, cursor_x, cursor_y, label_color);   // R
    cursor_x = draw_char(82u, cursor_x, cursor_y, label_color);   // R
    cursor_x = draw_char(58u, cursor_x, cursor_y, label_color);   // :
    cursor_x = draw_char(32u, cursor_x, cursor_y, label_color);   // space
    cursor_x = draw_char(91u, cursor_x, cursor_y, dim_color);     // [

    let err_count = vm_stats[4];
    if (err_count > 0u) {
        cursor_x = draw_char(33u, cursor_x, cursor_y, error_color);   // !
    } else {
        cursor_x = draw_char(32u, cursor_x, cursor_y, value_color);   // space
    }

    var err_i = 0u;
    loop {
        if (err_i >= 10u) { break; }
        cursor_x = draw_char(46u, cursor_x, cursor_y, dim_color);  // .
        err_i += 1u;
    }
    cursor_x = draw_char(93u, cursor_x, cursor_y, dim_color);     // ]
    cursor_x = draw_char(32u, cursor_x, cursor_y, label_color);   // space
    cursor_x = draw_decimal_u32(err_count, cursor_x, cursor_y, error_color);

    // Row 414: Latency (fixed-point ms*10)
    cursor_x = 10u;
    cursor_y = TELEMETRY_ZONE_TOP + 4u;
    cursor_x = draw_char(76u, cursor_x, cursor_y, label_color);   // L
    cursor_x = draw_char(65u, cursor_x, cursor_y, label_color);   // A
    cursor_x = draw_char(84u, cursor_x, cursor_y, label_color);   // T
    cursor_x = draw_char(58u, cursor_x, cursor_y, label_color);   // :
    cursor_x = draw_char(32u, cursor_x, cursor_y, label_color);   // space

    let latency_raw = vm_stats[5];
    let latency_ms = latency_raw / 10u;
    let latency_frac = latency_raw % 10u;
    cursor_x = draw_decimal_u32(latency_ms, cursor_x, cursor_y, value_color);
    cursor_x = draw_char(46u, cursor_x, cursor_y, value_color);   // .
    cursor_x = draw_decimal_u32(latency_frac, cursor_x, cursor_y, value_color);
    cursor_x = draw_char(109u, cursor_x, cursor_y, label_color);  // m
    cursor_x = draw_char(115u, cursor_x, cursor_y, label_color);  // s

    // Row 415: Route bitmask (binary)
    cursor_x = 10u;
    cursor_y = TELEMETRY_ZONE_TOP + 5u;
    cursor_x = draw_char(82u, cursor_x, cursor_y, label_color);   // R
    cursor_x = draw_char(84u, cursor_x, cursor_y, label_color);   // T
    cursor_x = draw_char(95u, cursor_x, cursor_y, label_color);   // _
    cursor_x = draw_char(77u, cursor_x, cursor_y, label_color);   // M
    cursor_x = draw_char(65u, cursor_x, cursor_y, label_color);   // A
    cursor_x = draw_char(83u, cursor_x, cursor_y, label_color);   // S
    cursor_x = draw_char(75u, cursor_x, cursor_y, label_color);   // K
    cursor_x = draw_char(58u, cursor_x, cursor_y, label_color);   // :
    cursor_x = draw_char(32u, cursor_x, cursor_y, label_color);   // space

    let route_mask = vm_stats[6];
    cursor_x = draw_binary_u8(route_mask & 255u, cursor_x, cursor_y, value_color);

    // Rows 416-419: Reserved (clear only)
    // Already cleared by the initial loop
}

// Helper: Draw 4-digit hex
fn draw_hex_u32(val: u32, x: u32, y: u32, color: Pixel) -> u32 {
    var cursor_x = x;
    cursor_x = draw_hex_digit((val >> 12u) & 15u, cursor_x, y, color);
    cursor_x = draw_hex_digit((val >> 8u) & 15u, cursor_x, y, color);
    cursor_x = draw_hex_digit((val >> 4u) & 15u, cursor_x, y, color);
    cursor_x = draw_hex_digit(val & 15u, cursor_x, y, color);
    return cursor_x;
}

// Helper: Draw single hex digit
fn draw_hex_digit(digit: u32, x: u32, y: u32, color: Pixel) -> u32 {
    let char_code = select(digit + 48u, digit + 55u, digit > 9u);  // 0-9 or A-F
    return draw_char(char_code, x, y, color);
}

// Helper: Draw decimal number (max 10 digits)
fn draw_decimal_u32(val: u32, x: u32, y: u32, color: Pixel) -> u32 {
    var cursor_x = x;

    if (val == 0u) {
        return draw_char(48u, cursor_x, y, color);
    }

    var num = val;
    var digit_count = 0u;

    // Count digits first
    var temp = num;
    loop {
        if (temp == 0u) { break; }
        temp /= 10u;
        digit_count += 1u;
    }

    // Draw digits from most significant
    var divisor = 1u;
    var j = 1u;
    loop {
        if (j >= digit_count) { break; }
        divisor *= 10u;
        j += 1u;
    }

    loop {
        if (divisor == 0u) { break; }
        let digit = (num / divisor) % 10u;
        cursor_x = draw_char(48u + digit, cursor_x, y, color);
        divisor /= 10u;
    }

    return cursor_x;
}

// Helper: Draw 8-bit binary from u32 (uses low byte)
fn draw_binary_u8(val: u32, x: u32, y: u32, color: Pixel) -> u32 {
    var cursor_x = x;
    var i = 0u;
    loop {
        if (i >= 8u) { break; }
        let bit = (val >> (7u - i)) & 1u;
        if (bit == 1u) {
            cursor_x = draw_char(49u, cursor_x, y, color);  // 1
        } else {
            cursor_x = draw_char(48u, cursor_x, y, color);  // 0
        }
        i += 1u;
    }
    return cursor_x;
}

// ============================================================================
// TELEMETRY BRIDGE HELPER FUNCTIONS
// ============================================================================

// Route color palette - each route gets distinct color
fn get_route_color(route_id: u32) -> vec4<f32> {
    let r = f32((route_id * 73u) % 256u) / 255.0;
    let g = f32((route_id * 151u) % 256u) / 255.0;
    let b = f32((route_id * 199u) % 256u) / 255.0;
    return vec4<f32>(r, g, b, 1.0);
}

// Spawn a particle when vm_stats change
fn spawn_telemetry_particle(route_id: u32, val: u32) {
    let p_idx = atomicAdd(&telemetry_bridge.particle_counter, 1u) % MAX_PARTICLES;
    
    // Spawn at telemetry row for this route
    let spawn_y = f32(TELEMETRY_ZONE_TOP) + f32(route_id) * 2.0;
    
    telemetry_bridge.particles[p_idx].pos = vec2<f32>(10.0, spawn_y);
    
    // Velocity based on result value
    let speed = clamp(PARTICLE_BASE_SPEED + f32(val % 10u), PARTICLE_BASE_SPEED, PARTICLE_MAX_SPEED);
    telemetry_bridge.particles[p_idx].vel = vec2<f32>(speed, 0.0);
    
    // Color from route ID
    telemetry_bridge.particles[p_idx].color = get_route_color(route_id);
    telemetry_bridge.particles[p_idx].life = PARTICLE_LIFETIME;
    telemetry_bridge.particles[p_idx].route_id = route_id;
    telemetry_bridge.particles[p_idx].result_val = val;
}

// Check for vm_stats changes and spawn particles
fn check_telemetry_pulses() {
    // TEST MODE: Spawn particles every 20 frames when mode == 1
    if (config.mode == 1u && config.frame % 20u == 0u) {
        let test_route = (config.frame / 20u) % 4u + 3u;  // Cycle through routes 3-6
        spawn_telemetry_particle(test_route, config.frame);
    }
    
    // Check indices 3-6 (Requests, Errors, Latency, Routes)
    var i = 3u;
    loop {
        if (i >= 7u) { break; }
        
        let current_val = atomicLoad(&vm_stats[i]);
        let prev_val = telemetry_bridge.prev_vm_stats[i];
        
        if (current_val != prev_val) {
            // Spawn particle for this change
            spawn_telemetry_particle(i, current_val);
            
            // Update prev for next frame
            telemetry_bridge.prev_vm_stats[i] = current_val;
        }
        
        i += 1u;
    }
}

// Update particle physics and render
fn update_particles() {
    var i = 0u;
    loop {
        if (i >= MAX_PARTICLES) { break; }
        
        var p = telemetry_bridge.particles[i];
        
        // Skip dead particles
        if (p.life <= 0.0) {
            i += 1u;
            continue;
        }
        
        // Update position
        p.pos = p.pos + p.vel;
        telemetry_bridge.particles[i].pos = p.pos;
        
        // Decay life
        p.life = p.life - 1.0;
        telemetry_bridge.particles[i].life = p.life;
        
        // Render particle as 3-pixel wide streak with ADDITIVE BLENDING
        let px = u32(p.pos.x);
        let py = u32(p.pos.y);
        
        if (py < config.height) {
            let alpha = p.life / PARTICLE_LIFETIME;
            
            // Draw 3 pixels horizontally with additive blend
            var dx = 0u;
            loop {
                if (dx >= 3u) { break; }
                let x = px + dx;
                if (x < config.width) {
                    let idx = py * config.width + x;
                    
                    // ADDITIVE BLEND: Add particle color to existing pixel
                    var existing = buffer_out[idx];
                    var pr = u32(p.color.r * 255.0 * alpha);
                    var pg = u32(p.color.g * 255.0 * alpha);
                    var pb = u32(p.color.b * 255.0 * alpha);
                    
                    // Blend and clamp
                    existing.r = min(255u, existing.r + pr);
                    existing.g = min(255u, existing.g + pg);
                    existing.b = min(255u, existing.b + pb);
                    existing.a = 255u;
                    
                    buffer_out[idx] = existing;
                }
                dx += 1u;
            }
        }
        
        i += 1u;
    }
}

// ============================================================================
// MAIN COMPUTE SHADER - Layered Rendering with Sequential Execution
// ============================================================================

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    let total_pixels = config.width * config.height;
    
    // ═════════════════════════════════════════════════════════════════
    // LAYER 1: Copy base layer (protect HUD zone) - PARALLEL
    // ═════════════════════════════════════════════════════════════════
    if (idx < total_pixels) {
        let row = idx / config.width;
        if (row < 400u || row >= 480u) {
            buffer_out[idx] = buffer_in[idx];
        } else {
            // Clear HUD zone background
            var bg: Pixel;
            bg.r = 10u; bg.g = 15u; bg.b = 25u; bg.a = 255u;
            buffer_out[idx] = bg;
        }
    }
    
    // ENSURE base layer is complete before HUD renders
    storageBarrier();
    workgroupBarrier();
    
    // ═════════════════════════════════════════════════════════════════
    // LAYER 2+3: HUD + Particles (SINGLE THREAD for sequential execution)
    // ═════════════════════════════════════════════════════════════════
    // Barriers don't serialize different thread IDs within a workgroup!
    // We must use the SAME thread ID for sequential operations.
    if (idx == 0u) {
        // Step 1: Spawn particles based on vm_stats
        check_telemetry_pulses();
        
        // Step 2: Render HUD text
        render_hud();
        render_telemetry_hud();
        render_input_zone();
        render_patch_status();
        
        // Step 3: Overlay particles with additive blending (AFTER HUD)
        update_particles();
    }
}
