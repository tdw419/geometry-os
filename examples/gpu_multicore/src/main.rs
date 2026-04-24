//! GPU Multicore RISC-V Tile Executor
//!
//! Issue #4: Test multicore RISC-V on actual GPU
//!
//! Two test modes:
//!   1. multi_tile_ignition - Spawns N RISC-V tiles on GPU, runs a fibonacci cartridge
//!   2. executive_commander - Orchestrator assigns different programs to tiles, collects results
//!
//! Usage:
//!   cargo run --release --                    # multi_tile_ignition with 10 tiles (default)
//!   cargo run --release -- ignition 50        # multi_tile_ignition with 50 tiles
//!   cargo run --release -- commander          # executive_commander mode
//!   cargo run --release -- bench              # benchmark at 1, 10, 100, 256 tiles

use anyhow::Result;
use std::time::Instant;

mod gpu;
mod loader;
mod reference;
mod uart;

use gpu::{GpuExecutor, STATUS_ERROR, STATUS_HALTED, TILE_STATE_WORDS, init_tile_states, init_tile_states_multi};
use loader::{build_counter_cartridge, build_fibonacci_cartridge};
use uart::extract_uart;

/// Multi-Tile Ignition Test
fn multi_tile_ignition(executor: &GpuExecutor, num_tiles: u32) -> Result<()> {
    println!("=== MULTI-TILE IGNITION TEST ===");
    println!("Tiles: {}", num_tiles);
    println!("Cartridge: fibonacci (14 instructions)");
    println!();

    let cartridge = build_fibonacci_cartridge();
    println!("Cartridge: {} instructions ({} bytes)", cartridge.len(), cartridge.len() * 4);

    let mut tile_data = init_tile_states(num_tiles as usize, &cartridge, 1000);

    let start = Instant::now();
    executor.run_tiles(&mut tile_data, num_tiles)?;
    let elapsed = start.elapsed();

    println!("Execution time: {:?}", elapsed);
    println!();

    // Analyze results
    let mut halted_count = 0u32;
    let mut error_count = 0u32;
    let mut total_instructions = 0u64;

    for i in 0..num_tiles as usize {
        let base = i * TILE_STATE_WORDS;
        let status = tile_data[base + 33];
        let inst_count = tile_data[base + 34];
        let uart_output = extract_uart(&tile_data[base..base + TILE_STATE_WORDS]);

        total_instructions += inst_count as u64;

        let is_ok = (status & STATUS_HALTED) != 0 && (status & STATUS_ERROR) == 0;
        if is_ok { halted_count += 1; } else { error_count += 1; }

        if i < 3 || i == num_tiles as usize - 1 {
            println!("  Tile {:3}: status=0x{:x} insts={:5} uart={}",
                     i, status, inst_count, uart_output);
        } else if i == 3 {
            println!("  ... ({} more tiles) ...", num_tiles - 4);
        }
    }

    println!();
    println!("Results:");
    println!("  Halted cleanly: {}/{}", halted_count, num_tiles);
    println!("  Errors:         {}/{}", error_count, num_tiles);
    println!("  Total instructions: {}", total_instructions);
    if elapsed.as_secs_f64() > 0.0 {
        println!("  Throughput: {:.0} instructions/sec", total_instructions as f64 / elapsed.as_secs_f64());
    }
    println!("  Per-tile avg: {:.1} instructions", total_instructions as f64 / num_tiles as f64);

    if halted_count == num_tiles && error_count == 0 {
        println!("  [PASS] All tiles halted cleanly");
    } else {
        println!("  [WARN] Some tiles had errors");
    }

    Ok(())
}

/// Executive Commander Test
fn executive_commander(executor: &GpuExecutor) -> Result<()> {
    println!("=== EXECUTIVE COMMANDER TEST ===");
    println!("4 tiles, different programs");
    println!();

    let fib = build_fibonacci_cartridge();
    let cnt5 = build_counter_cartridge(5);
    let cnt3 = build_counter_cartridge(3);
    let cnt7 = build_counter_cartridge(7);

    let cartridges: [(&str, &[u32]); 4] = [
        ("fibonacci(10)", &fib),
        ("counter(5)", &cnt5),
        ("counter(3)", &cnt3),
        ("counter(7)", &cnt7),
    ];

    // Use shared init_tile_states_multi instead of manual per-tile init
    let mut tile_data = init_tile_states_multi(&cartridges, 2000);

    let num_tiles = cartridges.len() as u32;
    let start = Instant::now();
    executor.run_tiles(&mut tile_data, num_tiles)?;
    let elapsed = start.elapsed();

    println!("Execution time: {:?}", elapsed);
    println!();

    let mut all_ok = true;
    for (i, (name, _)) in cartridges.iter().enumerate() {
        let base = i * TILE_STATE_WORDS;
        let status = tile_data[base + 33];
        let inst_count = tile_data[base + 34];
        let uart_out = extract_uart(&tile_data[base..base + TILE_STATE_WORDS]);

        let ok = (status & STATUS_HALTED) != 0 && (status & STATUS_ERROR) == 0;
        let marker = if ok { "PASS" } else { "FAIL" };

        println!("  Tile {} [{}]: {} insts={:5} uart={}",
                 i, name, marker, inst_count, uart_out);

        if !ok { all_ok = false; }
    }

    println!();
    if all_ok {
        println!("  [PASS] Commander: all {} tiles completed successfully", num_tiles);
    } else {
        println!("  [WARN] Commander: some tiles failed");
    }

    Ok(())
}

/// Benchmark at different tile counts
fn benchmark(executor: &GpuExecutor) -> Result<()> {
    println!("=== GPU THROUGHPUT BENCHMARK ===");
    println!();

    let cartridge = build_fibonacci_cartridge();
    let tile_counts: [u32; 6] = [1, 4, 16, 64, 128, 256];

    println!("{:>6} {:>12} {:>12} {:>18} {:>10}", "Tiles", "Time", "Total Insts", "Insts/sec", "Status");
    println!("{}", "-".repeat(65));

    for num_tiles in &tile_counts {
        let num_tiles = *num_tiles;
        let mut tile_data = init_tile_states(num_tiles as usize, &cartridge, 1000);

        let start = Instant::now();
        match executor.run_tiles(&mut tile_data, num_tiles) {
            Ok(()) => {
                let elapsed = start.elapsed();
                let total_insts: u64 = (0..num_tiles as usize)
                    .map(|i| tile_data[i * TILE_STATE_WORDS + 34] as u64)
                    .sum();
                let throughput = if elapsed.as_secs_f64() > 0.0 {
                    total_insts as f64 / elapsed.as_secs_f64()
                } else {
                    0.0
                };

                let all_ok = (0..num_tiles as usize)
                    .all(|i| (tile_data[i * TILE_STATE_WORDS + 33] & STATUS_ERROR) == 0);

                let status = if all_ok { "OK" } else { "ERR" };

                println!("{:>6} {:>10.3}ms {:>12} {:>18.0} {:>10}",
                         num_tiles, elapsed.as_secs_f64() * 1000.0, total_insts, throughput, status);
            }
            Err(e) => {
                let err_str = e.to_string();
                let truncated = &err_str[..err_str.len().min(20)];
                println!("{:>6} {:>12} {:>12} {:>18} {:>10}",
                         num_tiles, "FAIL", "-", "-", truncated);
            }
        }
    }

    Ok(())
}

/// Verify a cartridge by running it on both GPU and reference CPU interpreter
fn verify_cartridge(executor: &GpuExecutor, name: &str, cartridge: &[u32], max_steps: u32) -> Result<()> {
    use gpu::RAM_WORDS;
    println!("--- Verifying {} ---", name);

    // 1. Run on GPU (1 tile)
    let mut tile_data = init_tile_states(1, cartridge, max_steps);
    executor.run_tiles(&mut tile_data, 1)?;

    let gpu_status = tile_data[33];
    let gpu_inst_count = tile_data[34];
    let gpu_pc = tile_data[32];
    let gpu_uart = extract_uart(&tile_data[..TILE_STATE_WORDS]);
    let mut gpu_regs = [0u32; 32];
    gpu_regs.copy_from_slice(&tile_data[0..32]);

    // 2. Run on Reference (CPU)
    let mut ram = vec![0u32; RAM_WORDS];
    for (i, &word) in cartridge.iter().enumerate() {
        if i < RAM_WORDS {
            ram[i] = word;
        }
    }
    let mut ref_vm = reference::ReferenceVm::new(ram);
    ref_vm.run(max_steps);

    let ref_uart = String::from_utf8_lossy(&ref_vm.uart_output).to_string();

    // 3. Compare results
    println!("  Reference: insts={} status=0x{:x} pc=0x{:x} uart=\"{}\"",
             ref_vm.instruction_count, ref_vm.status, ref_vm.pc, ref_uart);
    println!("  GPU      : insts={} status=0x{:x} pc=0x{:x} uart=\"{}\"",
             gpu_inst_count, gpu_status, gpu_pc, gpu_uart);

    let mut mismatch = false;
    if gpu_pc != ref_vm.pc {
        println!("  [ERR] PC mismatch: GPU=0x{:x}, Ref=0x{:x}", gpu_pc, ref_vm.pc);
        mismatch = true;
    }
    if gpu_status != ref_vm.status {
        println!("  [ERR] Status mismatch: GPU=0x{:x}, Ref=0x{:x}", gpu_status, ref_vm.status);
        mismatch = true;
    }
    for i in 0..32 {
        if gpu_regs[i] != ref_vm.regs[i] {
            println!("  [ERR] Reg x{} mismatch: GPU=0x{:x}, Ref=0x{:x}", i, gpu_regs[i], ref_vm.regs[i]);
            mismatch = true;
        }
    }

    if !mismatch {
        println!("  [PASS] GPU matches reference exactly.");
    } else {
        println!("  [FAIL] Mismatch detected!");
    }
    println!();

    if mismatch {
        anyhow::bail!("Verification failed for {}", name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_vs_reference() -> Result<()> {
        let executor = pollster::block_on(GpuExecutor::new())?;

        let fib = build_fibonacci_cartridge();
        verify_cartridge(&executor, "fibonacci(10)", &fib, 1000)?;

        let cnt7 = build_counter_cartridge(7);
        verify_cartridge(&executor, "counter(7)", &cnt7, 1000)?;

        Ok(())
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("ignition");

    println!("GPU Multicore RISC-V Tile Executor");
    println!("===================================");
    println!();

    // Initialize GPU
    println!("Initializing GPU...");
    let executor = pollster::block_on(GpuExecutor::new())?;
    println!("GPU initialized successfully!");
    println!();

    match mode {
        "ignition" => {
            let num_tiles = args.get(2)
                .and_then(|s| s.parse().ok())
                .unwrap_or(10);
            multi_tile_ignition(&executor, num_tiles)?;
        }
        "commander" => {
            executive_commander(&executor)?;
        }
        "bench" => {
            benchmark(&executor)?;
        }
        "verify" => {
            let fib = build_fibonacci_cartridge();
            verify_cartridge(&executor, "fibonacci(10)", &fib, 1000)?;

            let cnt7 = build_counter_cartridge(7);
            verify_cartridge(&executor, "counter(7)", &cnt7, 1000)?;
        }
        _ => {
            println!("Unknown mode: {}", mode);
            println!("Usage: gpu_multicore [ignition|commander|bench|verify] [num_tiles]");
            println!();
            println!("Modes:");
            println!("  ignition [N]  - Run N tiles with fibonacci (default: 10)");
            println!("  commander     - Run 4 tiles with different programs");
            println!("  bench         - Benchmark at 1/4/16/64/128/256 tiles");
            println!("  verify        - Verify GPU against CPU reference interpreter");
        }
    }

    Ok(())
}
