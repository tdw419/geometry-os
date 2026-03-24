// ============================================================================
// Pixel Agent Shader — Self-Propagating Computational Universe
//
// Double-buffered compute shader where pixels are autonomous agents.
// Each pixel can read neighbors, modify state, and replicate.
// ============================================================================

// Agent opcodes (stored in pixel.r)
const OP_NOP: u32 = 0x00u;        // No operation
const OP_IDLE: u32 = 0x01u;       // Idle (just render color)
const OP_MOVE_RIGHT: u32 = 0x02u; // Move agent to right neighbor
const OP_MOVE_LEFT: u32 = 0x03u;  // Move agent to left neighbor
const OP_MOVE_UP: u32 = 0x04u;    // Move agent up
const OP_MOVE_DOWN: u32 = 0x05u;  // Move agent down
const OP_REPLICATE: u32 = 0x06u;  // Copy self to all neighbors
const OP_INFECT: u32 = 0x07u;     // Convert neighbor to self
const OP_DIE: u32 = 0x0Fu;        // Remove self

// Sensing opcodes
const OP_READ_N: u32 = 0x08u;     // Read north neighbor's red value into reg
const OP_READ_S: u32 = 0x09u;     // Read south neighbor
const OP_READ_E: u32 = 0x0Au;     // Read east neighbor
const OP_READ_W: u32 = 0x0Bu;     // Read west neighbor

// Conditional opcodes
const OP_IF_RED: u32 = 0x10u;     // If reg == target_red, exec next; else skip
const OP_IF_GREEN: u32 = 0x11u;   // If reg == target_green
const OP_IF_EMPTY: u32 = 0x12u;   // If reg == TYPE_EMPTY
const OP_IF_AGENT: u32 = 0x13u;   // If reg == TYPE_AGENT

// Signal opcodes
const OP_EMIT_SIGNAL: u32 = 0x20u; // Wake up all neighbors
const OP_SLEEP: u32 = 0x21u;       // Become dormant (a=TYPE_EMPTY)
const OP_WIRE: u32 = 0x22u;        // Propagate signal from west to east
const OP_CLOCK: u32 = 0x23u;       // Toggle signal based on frame counter
const OP_SIGNAL_SOURCE: u32 = 0x24u; // Constant signal source (always high)

// Logic opcodes (for building circuits) - REMAPPED to symbols to avoid digit collision
// Digits 0-9 (0x30-0x39) are now PASS-THROUGH for text display
const OP_AND: u32 = 0x26u;        // '&' - Output high if N AND W have signal
const OP_XOR: u32 = 0x5Eu;        // '^' - Output high if N XOR W have signal
const OP_OR: u32 = 0x7Cu;         // '|' - Output high if N OR W have signal
const OP_NOT: u32 = 0x7Eu;        // '~' - Output high if W does NOT have signal
const OP_RANDOM: u32 = 0x40u;     // Randomly choose action

// Digit range (0x30-0x39) - Pass through as text, don't execute as opcodes
const DIGIT_MIN: u32 = 0x30u;     // '0'
const DIGIT_MAX: u32 = 0x39u;     // '9'

// Portal opcodes (cross-zone signal teleportation)
const OP_PORTAL_IN: u32 = 0x50u;  // Teleport signal: g=target_x, b=target_y
const OP_PORTAL_OUT: u32 = 0x51u; // Receive teleported signal (becomes active)
const OP_PORTAL_BIDIR: u32 = 0x52u; // Bidirectional portal

// Neural Pipe opcodes (LLM integration)
const OP_GENERATE: u32 = 0x53u;   // Request LLM generation
const OP_PROMPT: u32 = 0x54u;     // Mark pixel as part of prompt zone
const OP_RESPONSE: u32 = 0x55u;   // Mark pixel as part of response zone

// Execution handover opcodes
const OP_JMP_RESPONSE: u32 = 0x56u;  // Jump IP to response zone (row 20)
const OP_RETURN: u32 = 0x57u;        // Return from response zone to caller

// Agent type flags (stored in pixel.a)
const TYPE_EMPTY: u32 = 0u;       // Empty cell
const TYPE_AGENT: u32 = 254u;     // Active agent
const TYPE_CODE: u32 = 255u;      // Executable code

struct Pixel {
    r: u32,  // Opcode or Red channel
    g: u32,  // Register A / Green
    b: u32,  // Register B / Blue
    a: u32,  // Type flag or Alpha
}

struct Config {
    width: u32,
    height: u32,
    time: f32,
    frame: u32,
    mode: u32,  // 0=agent, 1=formula
}

// Double buffers
@group(0) @binding(0) var<storage, read> buffer_in: array<Pixel>;
@group(0) @binding(1) var<storage, read_write> buffer_out: array<Pixel>;

// Formula bytecode (for legacy mode)
@group(0) @binding(2) var<storage, read> bytecode: array<u32>;
@group(0) @binding(3) var<storage, read> constants: array<f32>;

// Config
@group(0) @binding(4) var<uniform> config: Config;

// Stats buffer (for neural pipe signaling)
@group(0) @binding(5) var<storage, read_write> stats: array<u32>;

// Helper: Check if coordinates are in bounds
fn in_bounds(x: u32, y: u32) -> bool {
    return x < config.width && y < config.height;
}

// Helper: Get pixel index
fn idx(x: u32, y: u32) -> u32 {
    return y * config.width + x;
}

// Helper: Read neighbor (returns empty if out of bounds)
fn read_neighbor(x: u32, y: u32, dx: i32, dy: i32) -> Pixel {
    let nx = i32(x) + dx;
    let ny = i32(y) + dy;
    
    if (nx >= 0 && nx < i32(config.width) && ny >= 0 && ny < i32(config.height)) {
        return buffer_in[idx(u32(nx), u32(ny))];
    }
    
    var empty: Pixel;
    empty.r = 0u;
    empty.g = 0u;
    empty.b = 0u;
    empty.a = TYPE_EMPTY;
    return empty;
}

// Helper: Write to output buffer (atomic-like behavior via last-write-wins)
fn write_pixel(x: u32, y: u32, pixel: Pixel) {
    if (in_bounds(x, y)) {
        let i = idx(x, y);
        // Only write if target is empty or we're overwriting with higher priority
        if (buffer_out[i].a == TYPE_EMPTY || pixel.a == TYPE_CODE) {
            buffer_out[i] = pixel;
        }
    }
}

// Helper: Create empty pixel
fn empty_pixel() -> Pixel {
    var p: Pixel;
    p.r = 0u;
    p.g = 0u;
    p.b = 0u;
    p.a = TYPE_EMPTY;
    return p;
}

// Helper: Copy pixel
fn copy_pixel(p: Pixel) -> Pixel {
    var copy: Pixel;
    copy.r = p.r;
    copy.g = p.g;
    copy.b = p.b;
    copy.a = p.a;
    return copy;
}

// ============================================================================
// AGENT MODE — Autonomous pixel behaviors
// ============================================================================

fn execute_agent(px: u32, py: u32, cell: Pixel) {
    let x = px;
    let y = py;
    
    switch cell.r {
        case OP_NOP: {
            // Just pass through
            write_pixel(x, y, cell);
        }
        case OP_IDLE: {
            // Render as color, stay in place
            write_pixel(x, y, cell);
        }
        case OP_MOVE_RIGHT: {
            // Move to right neighbor (leave empty behind)
            let neighbor = read_neighbor(x, y, 1, 0);
            if (neighbor.a == TYPE_EMPTY) {
                write_pixel(x, y, empty_pixel());
                write_pixel(x + 1u, y, cell);
            } else {
                // Blocked, stay in place
                write_pixel(x, y, cell);
            }
        }
        case OP_MOVE_LEFT: {
            let neighbor = read_neighbor(x, y, -1, 0);
            if (neighbor.a == TYPE_EMPTY) {
                write_pixel(x, y, empty_pixel());
                write_pixel(x - 1u, y, cell);
            } else {
                write_pixel(x, y, cell);
            }
        }
        case OP_MOVE_UP: {
            let neighbor = read_neighbor(x, y, 0, -1);
            if (neighbor.a == TYPE_EMPTY) {
                write_pixel(x, y, empty_pixel());
                write_pixel(x, y - 1u, cell);
            } else {
                write_pixel(x, y, cell);
            }
        }
        case OP_MOVE_DOWN: {
            let neighbor = read_neighbor(x, y, 0, 1);
            if (neighbor.a == TYPE_EMPTY) {
                write_pixel(x, y, empty_pixel());
                write_pixel(x, y + 1u, cell);
            } else {
                write_pixel(x, y, cell);
            }
        }
        case OP_REPLICATE: {
            // Stay in place, copy to all empty neighbors
            write_pixel(x, y, cell);
            
            // Children inherit parent's opcode (keep replicating!)
            var child = copy_pixel(cell);
            
            // Unrolled neighbor checks
            if (x > 0u && y > 0u) {
                let n = read_neighbor(x, y, -1, -1);
                if (n.a == TYPE_EMPTY) {
                    write_pixel(x - 1u, y - 1u, child);
                }
            }
            if (y > 0u) {
                let n = read_neighbor(x, y, 0, -1);
                if (n.a == TYPE_EMPTY) {
                    write_pixel(x, y - 1u, child);
                }
            }
            if (x + 1u < config.width && y > 0u) {
                let n = read_neighbor(x, y, 1, -1);
                if (n.a == TYPE_EMPTY) {
                    write_pixel(x + 1u, y - 1u, child);
                }
            }
            if (x > 0u) {
                let n = read_neighbor(x, y, -1, 0);
                if (n.a == TYPE_EMPTY) {
                    write_pixel(x - 1u, y, child);
                }
            }
            if (x + 1u < config.width) {
                let n = read_neighbor(x, y, 1, 0);
                if (n.a == TYPE_EMPTY) {
                    write_pixel(x + 1u, y, child);
                }
            }
            if (x > 0u && y + 1u < config.height) {
                let n = read_neighbor(x, y, -1, 1);
                if (n.a == TYPE_EMPTY) {
                    write_pixel(x - 1u, y + 1u, child);
                }
            }
            if (y + 1u < config.height) {
                let n = read_neighbor(x, y, 0, 1);
                if (n.a == TYPE_EMPTY) {
                    write_pixel(x, y + 1u, child);
                }
            }
            if (x + 1u < config.width && y + 1u < config.height) {
                let n = read_neighbor(x, y, 1, 1);
                if (n.a == TYPE_EMPTY) {
                    write_pixel(x + 1u, y + 1u, child);
                }
            }
        }
        case OP_INFECT: {
            // Convert all neighbors to self
            write_pixel(x, y, cell);
            
            // Top
            if (y > 0u) {
                var n = read_neighbor(x, y, 0, -1);
                if (n.a != TYPE_EMPTY && n.a != TYPE_CODE) {
                    n.r = cell.r;
                    n.g = cell.g;
                    n.b = cell.b;
                    write_pixel(x, y - 1u, n);
                }
            }
            // Left
            if (x > 0u) {
                var n = read_neighbor(x, y, -1, 0);
                if (n.a != TYPE_EMPTY && n.a != TYPE_CODE) {
                    n.r = cell.r;
                    n.g = cell.g;
                    n.b = cell.b;
                    write_pixel(x - 1u, y, n);
                }
            }
            // Right
            if (x + 1u < config.width) {
                var n = read_neighbor(x, y, 1, 0);
                if (n.a != TYPE_EMPTY && n.a != TYPE_CODE) {
                    n.r = cell.r;
                    n.g = cell.g;
                    n.b = cell.b;
                    write_pixel(x + 1u, y, n);
                }
            }
            // Bottom
            if (y + 1u < config.height) {
                var n = read_neighbor(x, y, 0, 1);
                if (n.a != TYPE_EMPTY && n.a != TYPE_CODE) {
                    n.r = cell.r;
                    n.g = cell.g;
                    n.b = cell.b;
                    write_pixel(x, y + 1u, n);
                }
            }
        }
        case OP_DIE: {
            // Remove self (don't write anything)
        }
        
        // ===== SENSING OPCODES =====
        case OP_READ_N: {
            // Read north neighbor's opcode into cell.g (register)
            var updated = cell;
            if (y > 0u) {
                let n = read_neighbor(x, y, 0, -1);
                updated.g = n.r;  // Store neighbor's opcode in register
            }
            write_pixel(x, y, updated);
        }
        case OP_READ_S: {
            var updated = cell;
            if (y + 1u < config.height) {
                let n = read_neighbor(x, y, 0, 1);
                updated.g = n.r;
            }
            write_pixel(x, y, updated);
        }
        case OP_READ_E: {
            var updated = cell;
            if (x + 1u < config.width) {
                let n = read_neighbor(x, y, 1, 0);
                updated.g = n.r;
            }
            write_pixel(x, y, updated);
        }
        case OP_READ_W: {
            var updated = cell;
            if (x > 0u) {
                let n = read_neighbor(x, y, -1, 0);
                updated.g = n.r;
            }
            write_pixel(x, y, updated);
        }
        
        // ===== CONDITIONAL OPCODES =====
        case OP_IF_RED: {
            // If register (cell.g) has red, replicate; else just pass through
            write_pixel(x, y, cell);
            if (cell.g > 200u) {
                // Condition met: replicate
                var child = copy_pixel(cell);
                child.r = OP_REPLICATE;
                if (x + 1u < config.width) {
                    let n = read_neighbor(x, y, 1, 0);
                    if (n.a == TYPE_EMPTY) {
                        write_pixel(x + 1u, y, child);
                    }
                }
            }
        }
        case OP_IF_GREEN: {
            write_pixel(x, y, cell);
            if (cell.g > 200u) {
                var child = copy_pixel(cell);
                child.r = OP_REPLICATE;
                if (y + 1u < config.height) {
                    let n = read_neighbor(x, y, 0, 1);
                    if (n.a == TYPE_EMPTY) {
                        write_pixel(x, y + 1u, child);
                    }
                }
            }
        }
        case OP_IF_EMPTY: {
            // Only replicate if neighbor is empty
            write_pixel(x, y, cell);
            let target_x = (cell.b >> 8u) & 0xFFu;  // Target encoded in b
            let target_y = cell.b & 0xFFu;
            if (target_x < config.width && target_y < config.height) {
                let n = read_neighbor(x, y, i32(target_x) - i32(x), i32(target_y) - i32(y));
                if (n.a == TYPE_EMPTY) {
                    var child = copy_pixel(cell);
                    child.r = OP_REPLICATE;
                    write_pixel(target_x, target_y, child);
                }
            }
        }
        case OP_IF_AGENT: {
            write_pixel(x, y, cell);
            // Check if north neighbor is an agent
            if (y > 0u) {
                let n = read_neighbor(x, y, 0, -1);
                if (n.a == TYPE_AGENT) {
                    // Condition met: do something
                    var child = copy_pixel(cell);
                    child.r = OP_MOVE_RIGHT;
                    write_pixel(x, y, child);
                }
            }
        }
        
        // ===== SIGNAL OPCODES =====
        case OP_EMIT_SIGNAL: {
            // Wake up all neighbors (set them to TYPE_AGENT if they were empty)
            write_pixel(x, y, cell);
            
            // Wake all 4 cardinal neighbors
            if (y > 0u) {
                var n = read_neighbor(x, y, 0, -1);
                if (n.a == TYPE_EMPTY) {
                    n.a = TYPE_AGENT;
                    n.r = OP_IDLE;
                    n.g = 100u;  // Signal color
                    n.b = 100u;
                    write_pixel(x, y - 1u, n);
                }
            }
            if (x > 0u) {
                var n = read_neighbor(x, y, -1, 0);
                if (n.a == TYPE_EMPTY) {
                    n.a = TYPE_AGENT;
                    n.r = OP_IDLE;
                    n.g = 100u;
                    n.b = 100u;
                    write_pixel(x - 1u, y, n);
                }
            }
            if (x + 1u < config.width) {
                var n = read_neighbor(x, y, 1, 0);
                if (n.a == TYPE_EMPTY) {
                    n.a = TYPE_AGENT;
                    n.r = OP_IDLE;
                    n.g = 100u;
                    n.b = 100u;
                    write_pixel(x + 1u, y, n);
                }
            }
            if (y + 1u < config.height) {
                var n = read_neighbor(x, y, 0, 1);
                if (n.a == TYPE_EMPTY) {
                    n.a = TYPE_AGENT;
                    n.r = OP_IDLE;
                    n.g = 100u;
                    n.b = 100u;
                    write_pixel(x, y + 1u, n);
                }
            }
        }
        case OP_SLEEP: {
            // Become dormant (remove agent flag)
            var dormant = cell;
            dormant.a = TYPE_EMPTY;
            write_pixel(x, y, dormant);
        }
        case OP_WIRE: {
            // Propagate signal through wire
            // b=0: horizontal wire (reads west neighbor)
            // b=1: vertical wire (reads north neighbor)
            // Signal is stored in green channel (g > 128 = high)
            var wire = copy_pixel(cell);
            wire.a = TYPE_AGENT;
            
            var signal_high = false;
            if (cell.b == 1u) {
                // Vertical wire: read from north
                if (y > 0u) {
                    let n = read_neighbor(x, y, 0, -1);
                    signal_high = (n.g > 128u);
                }
            } else {
                // Horizontal wire: read from west
                if (x > 0u) {
                    let w = read_neighbor(x, y, -1, 0);
                    signal_high = (w.g > 128u);
                }
            }
            
            wire.g = select(0u, 255u, signal_high);
            // b stays as direction flag (copy_pixel preserves it)
            write_pixel(x, y, wire);
        }
        case OP_CLOCK: {
            // Toggle signal based on frame counter
            // b = period (half-cycle length in frames), stored permanently
            // g = output signal (0 or 255)
            // Period is stored in b and NEVER overwritten
            let period = max(1u, cell.b);
            let tick = config.frame % (period * 2u);
            let signal_high = tick < period;

            var clock = copy_pixel(cell);
            clock.a = TYPE_AGENT;
            clock.g = select(0u, 255u, signal_high);
            // b stays as period (copy_pixel preserves it)
            write_pixel(x, y, clock);
        }
        case OP_SIGNAL_SOURCE: {
            // Constant high signal source
            var source = copy_pixel(cell);
            source.a = TYPE_AGENT;
            source.g = 255u; // Always high
            source.b = 50u;  // Slight blue tint
            write_pixel(x, y, source);
        }
        
        // ===== LOGIC OPCODES (check signal strength g > 128) =====
        case OP_AND: {
            // Output high if BOTH north AND west have signal (g > 128)
            var gate = copy_pixel(cell);
            gate.a = TYPE_AGENT;
            
            var north_high = false;
            var west_high = false;
            
            if (y > 0u) {
                let n = read_neighbor(x, y, 0, -1);
                north_high = (n.g > 128u);
            }
            if (x > 0u) {
                let w = read_neighbor(x, y, -1, 0);
                west_high = (w.g > 128u);
            }
            
            // AND: output high only if both inputs high
            gate.g = select(0u, 255u, north_high && west_high);
            gate.b = 255u; // Green gates render with blue tint
            write_pixel(x, y, gate);
        }
        case OP_XOR: {
            // Output high if exactly one input has signal
            var gate = copy_pixel(cell);
            gate.a = TYPE_AGENT;
            
            var north_high = false;
            var west_high = false;
            
            if (y > 0u) {
                let n = read_neighbor(x, y, 0, -1);
                north_high = (n.g > 128u);
            }
            if (x > 0u) {
                let w = read_neighbor(x, y, -1, 0);
                west_high = (w.g > 128u);
            }
            
            // XOR: output high if exactly one input high
            gate.g = select(0u, 255u, north_high != west_high);
            gate.b = 200u;
            write_pixel(x, y, gate);
        }
        case OP_OR: {
            // Output high if EITHER north OR west has signal
            var gate = copy_pixel(cell);
            gate.a = TYPE_AGENT;
            
            var north_high = false;
            var west_high = false;
            
            if (y > 0u) {
                let n = read_neighbor(x, y, 0, -1);
                north_high = (n.g > 128u);
            }
            if (x > 0u) {
                let w = read_neighbor(x, y, -1, 0);
                west_high = (w.g > 128u);
            }
            
            gate.g = select(0u, 255u, north_high || west_high);
            gate.b = 150u;
            write_pixel(x, y, gate);
        }
        case OP_NOT: {
            // Output high if west input is LOW (inverter)
            var gate = copy_pixel(cell);
            gate.a = TYPE_AGENT;
            
            var west_high = false;
            if (x > 0u) {
                let w = read_neighbor(x, y, -1, 0);
                west_high = (w.g > 128u);
            }
            
            // NOT: output is inverse of input
            gate.g = select(255u, 0u, west_high);
            gate.b = 100u;
            write_pixel(x, y, gate);
        }
        case OP_RANDOM: {
            // Randomly choose to move or replicate
            write_pixel(x, y, cell);
            
            // Use frame as pseudo-random seed
            let rand = (config.frame * 1234567u + x * 7919u + y * 104729u) % 100u;
            
            if (rand < 30u) {
                // 30% chance: replicate
                var child = copy_pixel(cell);
                child.r = OP_REPLICATE;
                if (x + 1u < config.width) {
                    let n = read_neighbor(x, y, 1, 0);
                    if (n.a == TYPE_EMPTY) {
                        write_pixel(x + 1u, y, child);
                    }
                }
            } else if (rand < 60u) {
                // 30% chance: move
                var mover = copy_pixel(cell);
                mover.r = OP_MOVE_RIGHT;
                write_pixel(x, y, empty_pixel());
                if (x + 1u < config.width) {
                    let n = read_neighbor(x, y, 1, 0);
                    if (n.a == TYPE_EMPTY) {
                        write_pixel(x + 1u, y, mover);
                    }
                }
            }
            // 40% chance: stay idle
        }
        
        // ===== PORTAL OPCODES (Cross-Zone Signal Teleportation) =====
        case OP_PORTAL_IN: {
            // Teleport signal to target coordinates
            // g = target_x, b = target_y
            let target_x = cell.g;
            let target_y = cell.b;
            
            // Validate target is in bounds
            if (target_x < config.width && target_y < config.height) {
                let target_idx = target_y * config.width + target_x;
                
                // Read current target state
                let dest_pixel = buffer_in[target_idx];
                
                // Only teleport if target is empty or a portal receiver
                if (dest_pixel.a == TYPE_EMPTY || dest_pixel.r == OP_PORTAL_OUT) {
                    // Create signal at target
                    var signal = copy_pixel(cell);
                    signal.r = OP_PORTAL_OUT;  // Becomes active receiver
                    signal.a = TYPE_AGENT;
                    signal.g = 255u;  // Signal strength
                    write_pixel(target_x, target_y, signal);
                }
            }
            
            // Portal IN stays active (persist)
            write_pixel(x, y, cell);
        }
        case OP_PORTAL_OUT: {
            // Receive teleported signal
            // This pixel becomes "hot" when a portal sends to it
            if (cell.g > 200u) {
                // Signal received - emit to neighbors
                write_pixel(x, y, cell);
                
                // Wake up neighbors
                if (y > 0u) {
                    var n = read_neighbor(x, y, 0, -1);
                    if (n.a == TYPE_EMPTY) {
                        n.a = TYPE_AGENT;
                        n.r = OP_IDLE;
                        n.g = 150u;
                        n.b = 150u;
                        write_pixel(x, y - 1u, n);
                    }
                }
                if (x + 1u < config.width) {
                    var n = read_neighbor(x, y, 1, 0);
                    if (n.a == TYPE_EMPTY) {
                        n.a = TYPE_AGENT;
                        n.r = OP_IDLE;
                        n.g = 150u;
                        n.b = 150u;
                        write_pixel(x + 1u, y, n);
                    }
                }
                if (y + 1u < config.height) {
                    var n = read_neighbor(x, y, 0, 1);
                    if (n.a == TYPE_EMPTY) {
                        n.a = TYPE_AGENT;
                        n.r = OP_IDLE;
                        n.g = 150u;
                        n.b = 150u;
                        write_pixel(x, y + 1u, n);
                    }
                }
                
                // Decay signal strength
                var decayed = cell;
                decayed.g = cell.g - 50u;
                if (decayed.g < 50u) {
                    // Signal faded, return to dormant portal
                    decayed.g = 0u;
                }
                write_pixel(x, y, decayed);
            } else {
                // Dormant - just stay as portal receiver
                write_pixel(x, y, cell);
            }
        }
        case OP_PORTAL_BIDIR: {
            // Bidirectional portal - can send and receive
            // g = target_x, b = target_y
            let target_x = cell.g;
            let target_y = cell.b;
            
            // Check if we received a signal (high green value from neighbor)
            var received_signal = false;
            if (y > 0u) {
                let n = read_neighbor(x, y, 0, -1);
                if (n.a == TYPE_AGENT && n.g > 200u) {
                    received_signal = true;
                }
            }
            if (x > 0u) {
                let n = read_neighbor(x, y, -1, 0);
                if (n.a == TYPE_AGENT && n.g > 200u) {
                    received_signal = true;
                }
            }
            
            if (received_signal && target_x < config.width && target_y < config.height) {
                // Forward signal to target
                var signal = empty_pixel();
                signal.r = OP_PORTAL_BIDIR;
                signal.g = 255u;  // Signal strength
                signal.a = TYPE_AGENT;
                write_pixel(target_x, target_y, signal);
            }
            
            // Persist portal
            write_pixel(x, y, cell);
        }
        
        // ===== NEURAL PIPE OPCODES (LLM Integration) =====
        case OP_GENERATE: {
            // Request LLM generation - only if not already pending
            // This prevents continuous triggering while host processes
            
            // Only set signal if currently idle (stats[0] == 0)
            if (stats[0] == 0u) {
                stats[0] = 1u;  // NEURAL_PIPE_REQUEST
                stats[1] = cell.g;  // prompt_start_row (or default 10)
                stats[2] = cell.b;  // response_start_row (or default 20)
            }
            
            // Mark this pixel as waiting for generation
            var gen = copy_pixel(cell);
            gen.a = TYPE_AGENT;
            gen.g = 255u;  // Signal: waiting for response
            write_pixel(x, y, gen);
        }
        case OP_PROMPT: {
            // Mark pixel as part of prompt zone
            // Just renders with a special color to show it's prompt area
            var prompt = copy_pixel(cell);
            prompt.a = TYPE_AGENT;
            prompt.g = 100u;  // Dim green = prompt zone
            prompt.b = 200u;  // Blue tint
            write_pixel(x, y, prompt);
        }
        case OP_RESPONSE: {
            // Mark pixel as part of response zone
            // Host writes LLM output here
            var resp = copy_pixel(cell);
            resp.a = TYPE_AGENT;
            resp.g = 200u;  // Brighter green = response zone
            resp.b = 100u;  // Less blue
            write_pixel(x, y, resp);
        }
        case OP_JMP_RESPONSE: {
            // Jump to response zone (row 20) - Execution Handover
            // This creates an "agent" at (0, 20) that will execute LLM code
            // The agent's r channel holds the instruction pointer offset
            
            // Create execution agent at start of response zone
            let response_start_x = 0u;
            let response_start_y = 20u;  // RESPONSE_START_ROW
            
            var exec_agent = copy_pixel(cell);
            exec_agent.r = OP_IDLE;  // Will execute whatever is there
            exec_agent.g = 255u;     // Bright = executing
            exec_agent.b = 0u;       // No blue
            exec_agent.a = TYPE_AGENT;
            
            write_pixel(response_start_x, response_start_y, exec_agent);
            
            // Remove the JMP instruction (one-shot)
            // Don't write ourselves back - we've handed over
        }
        case OP_RETURN: {
            // Return from response zone to caller
            // For now, just die (halt execution)
            // In future, could restore previous IP from stack
        }

        // ===== DIGIT PASS-THROUGH (0-9) =====
        // Digits are text literals, not opcodes
        // This allows code like "0a 1b 10i" to display without being executed
        case 0x30u: { write_pixel(x, y, cell); }  // '0'
        case 0x31u: { write_pixel(x, y, cell); }  // '1'
        case 0x32u: { write_pixel(x, y, cell); }  // '2'
        case 0x33u: { write_pixel(x, y, cell); }  // '3'
        case 0x34u: { write_pixel(x, y, cell); }  // '4'
        case 0x35u: { write_pixel(x, y, cell); }  // '5'
        case 0x36u: { write_pixel(x, y, cell); }  // '6'
        case 0x37u: { write_pixel(x, y, cell); }  // '7'
        case 0x38u: { write_pixel(x, y, cell); }  // '8'
        case 0x39u: { write_pixel(x, y, cell); }  // '9'

        default: {
            // Unknown opcode, just pass through
            write_pixel(x, y, cell);
        }
    }
}

// ============================================================================
// FORMULA MODE — Legacy stack-based formula evaluation
// ============================================================================

const STACK_SIZE: u32 = 16u;

const OP_PUSH_X: u32 = 0x01u;
const OP_PUSH_Y: u32 = 0x02u;
const OP_PUSH_T: u32 = 0x03u;
const OP_PUSH_CONST: u32 = 0x04u;
const OP_ADD: u32 = 0x10u;
const OP_SUB: u32 = 0x11u;
const OP_MUL: u32 = 0x12u;
const OP_DIV: u32 = 0x13u;
const OP_SIN: u32 = 0x20u;
const OP_COS: u32 = 0x21u;
const OP_SQRT: u32 = 0x23u;
const OP_FLOOR: u32 = 0x25u;
const OP_NOISE: u32 = 0x30u;
const OP_RGB: u32 = 0xF0u;
const OP_HSV: u32 = 0xF1u;

fn hash(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise2D(x: f32, y: f32) -> f32 {
    let i = floor(vec2<f32>(x, y));
    let f = fract(vec2<f32>(x, y));
    let u = f * f * (3.0 - 2.0 * f);
    
    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn hsvToRgb(h: f32, s: f32, v: f32) -> vec3<f32> {
    let i = floor(h * 6.0);
    let f = h * 6.0 - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    
    let im = i32(i) % 6;
    
    switch im {
        case 0: { return vec3<f32>(v, t, p); }
        case 1: { return vec3<f32>(q, v, p); }
        case 2: { return vec3<f32>(p, v, t); }
        case 3: { return vec3<f32>(p, q, v); }
        case 4: { return vec3<f32>(t, p, v); }
        case 5: { return vec3<f32>(v, p, q); }
        default: { return vec3<f32>(v, t, p); }
    }
}

fn execute_formula(px: u32, py: u32) -> Pixel {
    let x = f32(px) / f32(config.width);
    let y = f32(py) / f32(config.height);
    let t = config.time;
    
    var stack: array<f32, STACK_SIZE>;
    var sp: u32 = 0u;
    
    var r: f32 = 0.0;
    var g: f32 = 0.0;
    var b: f32 = 0.0;
    
    var pc: u32 = 0u;
    let bytecode_len = 64u;  // TODO: pass as config
    
    while (pc < bytecode_len) {
        let op = bytecode[pc];
        pc += 1u;
        
        if (op == 0xFFFFFFFFu) { break; }  // End marker
        
        switch op {
            case OP_PUSH_X: { stack[sp] = x; sp += 1u; }
            case OP_PUSH_Y: { stack[sp] = y; sp += 1u; }
            case OP_PUSH_T: { stack[sp] = t; sp += 1u; }
            case OP_PUSH_CONST: {
                let idx = bytecode[pc];
                pc += 1u;
                stack[sp] = constants[idx];
                sp += 1u;
            }
            case OP_ADD: {
                sp -= 1u; let b_ = stack[sp];
                sp -= 1u; let a_ = stack[sp];
                stack[sp] = a_ + b_; sp += 1u;
            }
            case OP_SUB: {
                sp -= 1u; let b_ = stack[sp];
                sp -= 1u; let a_ = stack[sp];
                stack[sp] = a_ - b_; sp += 1u;
            }
            case OP_MUL: {
                sp -= 1u; let b_ = stack[sp];
                sp -= 1u; let a_ = stack[sp];
                stack[sp] = a_ * b_; sp += 1u;
            }
            case OP_DIV: {
                sp -= 1u; let b_ = stack[sp];
                sp -= 1u; let a_ = stack[sp];
                stack[sp] = select(a_ / b_, 0.0, abs(b_) < 0.0001); sp += 1u;
            }
            case OP_SIN: { sp -= 1u; stack[sp] = sin(stack[sp]); sp += 1u; }
            case OP_COS: { sp -= 1u; stack[sp] = cos(stack[sp]); sp += 1u; }
            case OP_SQRT: { sp -= 1u; stack[sp] = sqrt(stack[sp]); sp += 1u; }
            case OP_FLOOR: { sp -= 1u; stack[sp] = floor(stack[sp]); sp += 1u; }
            case OP_NOISE: {
                sp -= 1u; let ny = stack[sp];
                sp -= 1u; let nx = stack[sp];
                stack[sp] = noise2D(nx, ny); sp += 1u;
            }
            case OP_RGB: {
                sp -= 1u; b = stack[sp];
                sp -= 1u; g = stack[sp];
                sp -= 1u; r = stack[sp];
            }
            case OP_HSV: {
                sp -= 1u; let v = stack[sp];
                sp -= 1u; let s = stack[sp];
                sp -= 1u; let h = stack[sp];
                let rgb = hsvToRgb(h, s, v);
                r = rgb.x; g = rgb.y; b = rgb.z;
            }
            default: {}
        }
    }
    
    var result: Pixel;
    result.r = u32(clamp(r, 0.0, 1.0) * 255.0);
    result.g = u32(clamp(g, 0.0, 1.0) * 255.0);
    result.b = u32(clamp(b, 0.0, 1.0) * 255.0);
    result.a = 255u;
    return result;
}

// ============================================================================
// MAIN COMPUTE SHADER
// ============================================================================

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let px = global_id.x;
    let py = global_id.y;
    
    if (px >= config.width || py >= config.height) {
        return;
    }
    
    // NEURAL PIPE: Freeze execution while LLM is processing
    // stats[0] = 0 (READY): execute normally
    // stats[0] = 1 (REQUEST): GPU requested, waiting for host
    // stats[0] = 2 (WRITING): Host writing response
    if (stats[0] != 0u) {
        // Frozen - just copy input to output unchanged
        let idx = py * config.width + px;
        buffer_out[idx] = buffer_in[idx];
        return;
    }
    
    let idx = py * config.width + px;
    
    // Clear output buffer (start empty)
    buffer_out[idx] = empty_pixel();
    
    if (config.mode == 0u) {
        // Agent mode
        let cell = buffer_in[idx];
        if (cell.a != TYPE_EMPTY) {
            execute_agent(px, py, cell);
        }
    } else {
        // Formula mode
        buffer_out[idx] = execute_formula(px, py);
    }
}
