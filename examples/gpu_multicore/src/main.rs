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

use anyhow::{Context, Result};
use std::time::Instant;

mod reference;

// WGSL compute shader for RISC-V tile execution
const RISCV_EXECUTOR_WGSL: &str = include_str!("riscv_executor.wgsl");

// Tile state layout constants (must match WGSL)
const STATE_HEADER_WORDS: usize = 40;
const UART_BUF_WORDS: usize = 128;
const RAM_WORDS: usize = 992;
const TILE_STATE_WORDS: usize = STATE_HEADER_WORDS + UART_BUF_WORDS + RAM_WORDS;

// Status flags
const STATUS_RUNNING: u32 = 0x1;
const STATUS_HALTED: u32 = 0x2;
const STATUS_ERROR: u32 = 0x4;

/// Build a fibonacci RISC-V cartridge
/// Computes fib(10) = 55 and writes result + ASCII to UART
fn build_fibonacci_cartridge() -> Vec<u32> {
    let mut code = Vec::new();
    // 0: ADDI a0, x0, 10    (n = 10)
    code.push(0x00500513);
    // 4: ADDI a1, x0, 0     (fib_prev = 0)
    code.push(0x00000593);
    // 8: ADDI a2, x0, 1     (fib_curr = 1)
    code.push(0x00100613);
    // 12: ADDI a3, x0, 0    (counter = 0)
    code.push(0x00000693);
    // 16: BEQ a3, a0, done  (if counter == n, done) offset = +24 bytes (6 instructions)
    code.push(encode_btype(0, 13, 10, 24));
    // 20: ADD a4, a2, a1    (temp = fib_curr + fib_prev)
    code.push(0x00b60733);
    // 24: ADDI a1, a2, 0    (fib_prev = old fib_curr)
    code.push(0x00060593);
    // 28: ADDI a2, a4, 0    (fib_curr = temp)
    code.push(0x00070613);
    // 32: ADDI a3, a3, 1    (counter++)
    code.push(0x00168693);
    // 36: JAL x0, -20       (back to BEQ at addr 16)
    code.push(0xfedff06f);
    // 40: LUI a5, 0x10000   (UART base)
    code.push(0x100007b7);
    // 44: SW a2, 0(a5)      (write fib result to UART)
    code.push(0x00c7a023);
    // 48: ADDI a5, x0, 0x35 (= '5' ASCII for fib(10)=55)
    code.push(0x03500793);
    // 52: LUI a7, 0x10000
    code.push(0x100008b7);
    // 56: SW a5, 0(a7)      (write '5')
    code.push(0x00f8a023);
    // 58: SW a5, 0(a7)      (write '5' again)
    code.push(0x00f8a023);
    // 60: ECALL (halt)
    code.push(0x00000073);
    code
}

/// Build a counter cartridge - counts 0..N, writes each value to UART
fn build_counter_cartridge(n: u32) -> Vec<u32> {
    let mut code = Vec::new();
    // 0: ADDI a0, x0, N     (limit)
    code.push(0x00000513 | ((n & 0xFFF) << 20));
    // 4: ADDI a1, x0, 0     (counter = 0)
    code.push(0x00000593);
    // 8: LUI a2, 0x10000    (UART base)
    code.push(0x10000637);
    // 12: SW a1, 0(a2)      (write counter to UART)  <- loop
    code.push(0x00b62023);
    // 16: ADDI a1, a1, 1    (counter++)
    code.push(0x00158593);
    // 20: BEQ a1, a0, done  (if counter == limit, done) offset = +8 bytes
    code.push(encode_btype(0, 11, 10, 8));
    // 24: JAL x0, -16       (back to loop at 12)
    code.push(0xff1ff06f);
    // 28: ECALL (halt)
    code.push(0x00000073);
    code
}

/// Encode a B-type (branch) instruction
fn encode_btype(funct3: u32, rs1: u32, rs2: u32, offset_bytes: i32) -> u32 {
    let imm = offset_bytes as u32;
    let imm12 = (imm >> 12) & 0x1;
    let imm10_5 = (imm >> 5) & 0x3F;
    let imm4_1 = (imm >> 1) & 0xF;
    let imm11 = (imm >> 11) & 0x1;
    
    0x63 | (funct3 << 12) | (rs1 << 15) | (rs2 << 20)
        | (imm4_1 << 8) | (imm10_5 << 25) | (imm11 << 7) | (imm12 << 31)
}

/// Initialize tile state buffer for N tiles
fn init_tile_states(num_tiles: usize, cartridge: &[u32], max_steps: u32) -> Vec<u32> {
    let mut buf = vec![0u32; num_tiles * TILE_STATE_WORDS];
    
    for i in 0..num_tiles {
        let base = i * TILE_STATE_WORDS;
        buf[base + 32] = 0;              // PC = 0
        buf[base + 33] = STATUS_RUNNING; // status = running
        buf[base + 34] = 0;              // instruction_count
        buf[base + 35] = max_steps;      // max_steps
        buf[base + 36] = i as u32;       // tile_id
        buf[base + 37] = 0;              // uart_len
        
        // Copy cartridge code into RAM region
        let ram_base = base + STATE_HEADER_WORDS + UART_BUF_WORDS;
        for (j, &word) in cartridge.iter().enumerate() {
            if j < RAM_WORDS {
                buf[ram_base + j] = word;
            }
        }
    }
    buf
}

/// Extract UART output from tile state
fn extract_uart(tile_state: &[u32]) -> String {
    let uart_len = tile_state[37] as usize;
    let mut s = String::new();
    let uart_base = STATE_HEADER_WORDS;
    for i in 0..uart_len.min(UART_BUF_WORDS) {
        let byte = tile_state[uart_base + i] & 0xFF;
        if byte >= 0x20 && byte < 0x7F {
            s.push(byte as u8 as char);
        } else if byte != 0 {
            s.push_str(&format!("[{:02x}]", byte));
        }
    }
    s
}

struct GpuExecutor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
}

impl GpuExecutor {
    async fn new() -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .context("No GPU adapter found")?;
        
        let info = adapter.get_info();
        println!("GPU: {} (backend: {:?})", info.name, info.backend);
        println!("Vendor: {:#x}, Device: {:#x}", info.vendor, info.device);
        
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("RISC-V Tile Executor"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            })
            .await
            .context("Failed to get GPU device")?;
        
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("RISC-V Executor"),
            source: wgpu::ShaderSource::Wgsl(RISCV_EXECUTOR_WGSL.into()),
        });
        
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("RISC-V Compute Pipeline"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        
        Ok(GpuExecutor { device, queue, pipeline })
    }
    
    fn run_tiles(&self, tile_data: &mut Vec<u32>, num_tiles: u32) -> Result<()> {
        let buffer_size = (num_tiles as usize * TILE_STATE_WORDS * 4) as u64;
        
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Tile State Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        
        // Upload tile data to GPU
        let byte_slice: &[u8] = unsafe {
            std::slice::from_raw_parts(
                tile_data.as_ptr() as *const u8,
                tile_data.len() * 4,
            )
        };
        self.queue.write_buffer(&buffer, 0, byte_slice);
        
        // Create bind group
        let bind_group_layout = self.pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Tile Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        
        // Dispatch compute
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("RISC-V Dispatch"),
        });
        
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("RISC-V Execution"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(num_tiles, 1, 1);
        }
        
        self.queue.submit(Some(encoder.finish()));
        let _ = self.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        
        // Read back results
        let read_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Read Back"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Copy Back"),
        });
        encoder.copy_buffer_to_buffer(&buffer, 0, &read_buffer, 0, buffer_size);
        self.queue.submit(Some(encoder.finish()));
        
        // Wait for copy
        let (tx, rx) = std::sync::mpsc::channel();
        read_buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        let _ = self.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        rx.recv()?.map_err(|e| anyhow::anyhow!("Map failed: {:?}", e))?;
        
        {
            let data = read_buffer.slice(..).get_mapped_range();
            let result_slice: &[u32] = unsafe {
                std::slice::from_raw_parts(data.as_ptr() as *const u32, data.len() / 4)
            };
            tile_data.copy_from_slice(result_slice);
        }
        
        Ok(())
    }
}

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
    
    let num_tiles = cartridges.len();
    let mut tile_data = vec![0u32; num_tiles * TILE_STATE_WORDS];
    
    for (i, (_name, cart)) in cartridges.iter().enumerate() {
        let base = i * TILE_STATE_WORDS;
        tile_data[base + 32] = 0;              // PC = 0
        tile_data[base + 33] = STATUS_RUNNING;
        tile_data[base + 34] = 0;
        tile_data[base + 35] = 2000;           // max_steps
        tile_data[base + 36] = i as u32;
        tile_data[base + 37] = 0;
        
        let ram_base = base + STATE_HEADER_WORDS + UART_BUF_WORDS;
        for (j, &word) in cart.iter().enumerate() {
            if j < RAM_WORDS {
                tile_data[ram_base + j] = word;
            }
        }
    }
    
    let start = Instant::now();
    executor.run_tiles(&mut tile_data, num_tiles as u32)?;
    let elapsed = start.elapsed();
    
    println!("Execution time: {:?}", elapsed);
    println!();
    
    let mut all_ok = true;
    for (i, (name, _)) in cartridges.iter().enumerate() {
        let base = i * TILE_STATE_WORDS;
        let status = tile_data[base + 33];
        let inst_count = tile_data[base + 34];
        let uart = extract_uart(&tile_data[base..base + TILE_STATE_WORDS]);
        
        let ok = (status & STATUS_HALTED) != 0 && (status & STATUS_ERROR) == 0;
        let marker = if ok { "PASS" } else { "FAIL" };
        
        println!("  Tile {} [{}]: {} insts={:5} uart={}",
                 i, name, marker, inst_count, uart);
        
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
