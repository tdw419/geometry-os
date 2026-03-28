// ============================================================================
// ATOMIC TELEMETRY MODULE - Thread-Safe Metrics for 1000+ VM Instances
// ============================================================================
// Architecture:
//   - GlyphLang services write metrics via atomic ops
//   - HUD shader reads non-atomically (acceptable race for display)
//   - Zero-copy integration with Rust host
//
// vm_stats Layout:
//   [0] GPU Status (1=ONLINE, 0=OFFLINE)
//   [1] IP (instruction pointer)
//   [2] SP (stack pointer)
//   [3] Request Counter (atomic)
//   [4] Error Counter (atomic)
//   [5] Rolling Latency (ms * 10, fixed-point)
//   [6] Active Route Bitmask
//   [7-10] Reserved
// ============================================================================

struct AtomicTelemetry {
    stats: array<atomic<u32>, 11>,
}

// Global atomic telemetry buffer
@group(0) @binding(10) var<storage, read_write> atomic_telemetry: AtomicTelemetry;

// Error codes (GlyphLang standard)
const ERR_UNKNOWN_OP: u32 = 0x01u;
const ERR_STACK_UNDERFLOW: u32 = 0x02u;
const ERR_STACK_OVERFLOW: u32 = 0x03u;
const ERR_INVALID_ROUTE: u32 = 0x04u;
const ERR_DB_ERROR: u32 = 0x05u;
const ERR_AUTH_FAILED: u32 = 0x06u;
const ERR_TIMEOUT: u32 = 0x07u;
const ERR_RATE_LIMITED: u32 = 0x08u;

// Route IDs (bitmask positions)
const ROUTE_DB: u32 = 0u;
const ROUTE_AUTH: u32 = 1u;
const ROUTE_CACHE: u32 = 2u;
const ROUTE_API: u32 = 3u;
const ROUTE_FILE: u32 = 4u;
const ROUTE_MATH: u32 = 5u;

// ============================================================================
// ATOMIC TELEMETRY FUNCTIONS
// ============================================================================

// Increment request counter (vm_stats[3])
fn track_request() {
    atomicAdd(&atomic_telemetry.stats[3], 1u);
}

// Increment error counter (vm_stats[4])
fn track_error() {
    atomicAdd(&atomic_telemetry.stats[4], 1u);
}

// Track specific error code (stores in reserved slot [7])
fn track_error_code(code: u32) {
    atomicStore(&atomic_telemetry.stats[7], code);
}

// Update execution state (IP, SP) - high frequency
fn update_execution_state(ip: u32, sp: u32) {
    atomicStore(&atomic_telemetry.stats[1], ip);
    atomicStore(&atomic_telemetry.stats[2], sp);
}

// Set GPU online status
fn set_gpu_online() {
    atomicStore(&atomic_telemetry.stats[0], 1u);
}

// Set GPU offline status
fn set_gpu_offline() {
    atomicStore(&atomic_telemetry.stats[0], 0u);
}

// ============================================================================
// Rolling Average Latency (Atomic Compare-Exchange Loop)
// ============================================================================
// This prevents flickering and handles concurrent updates correctly

fn track_latency(latency_ms: f32) {
    // Fixed-point encoding: ms * 10 for 0.1ms precision
    let fixed_point = u32(latency_ms * 10.0);
    
    // Atomic compare-exchange loop for rolling average
    // 90% old, 10% new: new = (old * 9 + sample) / 10
    var done = false;
    var attempts = 0u;
    let max_attempts = 10u;  // Prevent infinite loop
    
    loop {
        if (done || attempts >= max_attempts) { break; }
        
        let current = atomicLoad(&atomic_telemetry.stats[5]);
        let updated = (current * 9u + fixed_point) / 10u;
        
        // Try to update; if another thread modified it between load and store, retry
        let old_val = atomicCompareExchangeWeak(&atomic_telemetry.stats[5], current, updated);
        if (old_val == current) {
            done = true;
        }
        attempts += 1u;
    }
}

// Simple atomic latency update (no rolling average)
// Use for one-shot latency tracking
fn track_latency_simple(latency_ms: f32) {
    let fixed_point = u32(latency_ms * 10.0);
    atomicStore(&atomic_telemetry.stats[5], fixed_point);
}

// ============================================================================
// Route Bitmask Management
// ============================================================================

// Mark route as active (sets bit)
fn set_route_active(route_id: u32) {
    let mask = 1u << route_id;
    atomicOr(&atomic_telemetry.stats[6], mask);
}

// Mark route as inactive (clears bit)
fn set_route_inactive(route_id: u32) {
    let mask = !(1u << route_id);
    atomicAnd(&atomic_telemetry.stats[6], mask);
}

// Check if route is active
fn is_route_active(route_id: u32) -> bool {
    let mask = 1u << route_id;
    let routes = atomicLoad(&atomic_telemetry.stats[6]);
    return (routes & mask) != 0u;
}

// Clear all routes
fn clear_all_routes() {
    atomicStore(&atomic_telemetry.stats[6], 0u);
}

// ============================================================================
// Read Functions for HUD (Non-Atomic, Slight Race Acceptable)
// ============================================================================

fn read_request_count() -> u32 {
    return atomicLoad(&atomic_telemetry.stats[3]);
}

fn read_error_count() -> u32 {
    return atomicLoad(&atomic_telemetry.stats[4]);
}

fn read_latency_ms() -> f32 {
    let fixed_point = atomicLoad(&atomic_telemetry.stats[5]);
    return f32(fixed_point) / 10.0;
}

fn read_active_routes() -> u32 {
    return atomicLoad(&atomic_telemetry.stats[6]);
}

fn read_active_route_count() -> u32 {
    let routes = atomicLoad(&atomic_telemetry.stats[6]);
    return count_bits(routes);
}

// Count set bits
fn count_bits(bits: u32) -> u32 {
    var count = 0u;
    var n = bits;
    loop {
        if (n == 0u) { break; }
        count += n & 1u;
        n >>= 1u;
    }
    return count;
}

// ============================================================================
// VM Integration Hooks
// ============================================================================
// Drop these into your VM dispatch loop:
//   - Top of Loop: update_execution_state(pc, sp);
//   - On @ Opcode: track_request();
//   - On Error: track_error(); track_error_code(code);
//   - On Route Entry: set_route_active(ROUTE_*); track_latency(ms);
// ============================================================================

// Example VM dispatch integration:
// 
// fn dispatch_opcode(opcode: u32, pc: u32, sp: u32) {
//     // Update execution state every cycle
//     update_execution_state(pc, sp);
//     
//     if (opcode == 0x40) {  // @ (PROMPT/entry point)
//         track_request();
//     } else if (opcode == 0xFF) {  // Unknown opcode
//         track_error();
//         track_error_code(ERR_UNKNOWN_OP);
//     }
// }
