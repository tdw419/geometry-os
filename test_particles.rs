// Quick test: manually increment vm_stats to trigger particle spawns
use std::process::Command;

fn main() {
    println!("🧬 Testing Telemetry Bridge Particles");
    println!("======================================");
    println!();
    println!("To test particles, we need to:");
    println!("1. Run sovereign-shell in background");
    println!("2. Write to vm_stats buffer via shared memory");
    println!("3. Watch for white pulses in rows 410-419");
    println!();
    println!("Current limitation: vm_stats is GPU buffer, not directly writable from CPU.");
    println!("Next step: Add a Rust method to increment vm_stats[3] (request counter)");
    println!();
    println!("Alternative: Modify check_telemetry_pulses() to spawn on frame % 60 == 0");
}
