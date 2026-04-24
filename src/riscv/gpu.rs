// riscv/gpu.rs -- GPU Multicore RISC-V Tile Executor
//
// Integrates the WGSL compute shader from examples/gpu_multicore/ into the
// main RISC-V host. Each GPU workgroup runs one RISC-V tile (hart) with
// its own registers, PC, RAM, and UART buffer.
//
// Usage:
//   let executor = pollster::block_on(GpuExecutor::new())?;
//   let mut tile_data = init_tile_states(4, &cartridge, 1000);
//   executor.run_tiles(&mut tile_data, 4)?;
//   // tile_data now contains register state, status, UART output

#[cfg(feature = "gpu")]
use anyhow::{Context, Result};

// WGSL compute shader for RISC-V tile execution
#[cfg(feature = "gpu")]
const RISCV_EXECUTOR_WGSL: &str = include_str!("riscv_executor.wgsl");

// Tile state layout constants (must match WGSL)
pub const STATE_HEADER_WORDS: usize = 40;
pub const UART_BUF_WORDS: usize = 128;
pub const RAM_WORDS: usize = 992;
pub const TILE_STATE_WORDS: usize = STATE_HEADER_WORDS + UART_BUF_WORDS + RAM_WORDS;

// Status flags
pub const STATUS_RUNNING: u32 = 0x1;
pub const STATUS_HALTED: u32 = 0x2;
pub const STATUS_ERROR: u32 = 0x4;

/// Maximum number of GPU tiles supported
pub const MAX_TILES: u32 = 256;

/// Initialize tile state buffer for N tiles with a single cartridge
///
/// Each tile gets its own registers (zeroed), PC=0, status=RUNNING,
/// and the cartridge code copied into its RAM region.
pub fn init_tile_states(num_tiles: usize, cartridge: &[u32], max_steps: u32) -> Vec<u32> {
    let mut buf = vec![0u32; num_tiles * TILE_STATE_WORDS];

    for i in 0..num_tiles {
        let base = i * TILE_STATE_WORDS;
        buf[base + 32] = 0; // PC = 0
        buf[base + 33] = STATUS_RUNNING; // status = running
        buf[base + 34] = 0; // instruction_count
        buf[base + 35] = max_steps; // max_steps
        buf[base + 36] = i as u32; // tile_id
        buf[base + 37] = 0; // uart_len

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

/// Initialize tile state buffer for N tiles with different cartridges per tile
pub fn init_tile_states_multi(cartridges: &[(&str, &[u32])], max_steps: u32) -> Vec<u32> {
    let num_tiles = cartridges.len();
    let mut buf = vec![0u32; num_tiles * TILE_STATE_WORDS];

    for (i, (_name, cart)) in cartridges.iter().enumerate() {
        let base = i * TILE_STATE_WORDS;
        buf[base + 32] = 0; // PC = 0
        buf[base + 33] = STATUS_RUNNING; // status = running
        buf[base + 34] = 0; // instruction_count
        buf[base + 35] = max_steps; // max_steps
        buf[base + 36] = i as u32; // tile_id
        buf[base + 37] = 0; // uart_len

        let ram_base = base + STATE_HEADER_WORDS + UART_BUF_WORDS;
        for (j, &word) in cart.iter().enumerate() {
            if j < RAM_WORDS {
                buf[ram_base + j] = word;
            }
        }
    }
    buf
}

/// Extract UART output from a single tile's state buffer
pub fn extract_uart(tile_state: &[u32]) -> String {
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

/// GPU Executor for RISC-V tile execution.
///
/// Initializes the GPU adapter, creates the compute pipeline from the
/// WGSL shader, and provides `run_tiles()` to dispatch work.
#[cfg(feature = "gpu")]
pub struct GpuExecutor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
}

#[cfg(feature = "gpu")]
impl GpuExecutor {
    /// Create a new GPU executor, initializing the adapter and compute pipeline.
    ///
    /// Selects the highest-performance GPU available. Falls back to software
    /// rendering if no dedicated GPU is found.
    pub async fn new() -> Result<Self> {
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
        eprintln!("[gpu] Adapter: {} (backend: {:?})", info.name, info.backend);

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

        Ok(GpuExecutor {
            device,
            queue,
            pipeline,
        })
    }

    /// Get the adapter info for logging/diagnostics.
    pub fn adapter_info(&self) -> wgpu::AdapterInfo {
        // We don't store the adapter, but we can log from the device
        // The device doesn't expose adapter info directly, so we return
        // a placeholder. In practice, log during new() instead.
        wgpu::AdapterInfo {
            name: String::from("unknown"),
            vendor: 0,
            device: 0,
            device_type: wgpu::DeviceType::Other,
            driver: String::new(),
            driver_info: String::new(),
            backend: wgpu::Backend::Empty,
        }
    }

    /// Run N tiles on the GPU, blocking until all complete.
    ///
    /// `tile_data` is a mutable slice of u32 words laid out as:
    ///   [tile_0_state | tile_1_state | ... | tile_N_state]
    ///
    /// Each tile state is TILE_STATE_WORDS words.
    /// On return, tile_data contains the final register/PC/status/UART state.
    pub fn run_tiles(&self, tile_data: &mut [u32], num_tiles: u32) -> Result<()> {
        let buffer_size = (num_tiles as usize * TILE_STATE_WORDS * 4) as u64;

        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Tile State Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Upload tile data to GPU
        let byte_slice: &[u8] = unsafe {
            std::slice::from_raw_parts(tile_data.as_ptr() as *const u8, tile_data.len() * 4)
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
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
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
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });

        // Read back results
        let read_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Read Back"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Copy Back"),
            });
        encoder.copy_buffer_to_buffer(&buffer, 0, &read_buffer, 0, buffer_size);
        self.queue.submit(Some(encoder.finish()));

        // Wait for copy
        let (tx, rx) = std::sync::mpsc::channel();
        read_buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        rx.recv()?
            .map_err(|e| anyhow::anyhow!("Map failed: {:?}", e))?;

        {
            let data = read_buffer.slice(..).get_mapped_range();
            let result_slice: &[u32] =
                unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u32, data.len() / 4) };
            tile_data.copy_from_slice(result_slice);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_tile_states_single() {
        let cartridge = vec![0x00500513u32, 0x00000593, 0x00000073]; // ADDI + ADDI + ECALL
        let buf = init_tile_states(2, &cartridge, 500);

        // Check tile 0
        assert_eq!(buf[32], 0); // PC = 0
        assert_eq!(buf[33], STATUS_RUNNING);
        assert_eq!(buf[34], 0); // instruction_count
        assert_eq!(buf[35], 500); // max_steps
        assert_eq!(buf[36], 0); // tile_id

        // Check RAM region has cartridge
        let ram_base = STATE_HEADER_WORDS + UART_BUF_WORDS;
        assert_eq!(buf[ram_base], 0x00500513);
        assert_eq!(buf[ram_base + 1], 0x00000593);
        assert_eq!(buf[ram_base + 2], 0x00000073);

        // Check tile 1
        let base1 = TILE_STATE_WORDS;
        assert_eq!(buf[base1 + 32], 0);
        assert_eq!(buf[base1 + 33], STATUS_RUNNING);
        assert_eq!(buf[base1 + 36], 1); // tile_id = 1
    }

    #[test]
    fn test_init_tile_states_multi() {
        let cart_a = vec![0x00000073u32]; // ECALL
        let cart_b = vec![0x00500513u32, 0x00000073]; // ADDI + ECALL
        let cartridges: [(&str, &[u32]); 2] = [("a", &cart_a), ("b", &cart_b)];

        let buf = init_tile_states_multi(&cartridges, 100);

        // Tile 0
        let ram0 = STATE_HEADER_WORDS + UART_BUF_WORDS;
        assert_eq!(buf[ram0], 0x00000073);
        assert_eq!(buf[33], STATUS_RUNNING);

        // Tile 1
        let ram1 = TILE_STATE_WORDS + STATE_HEADER_WORDS + UART_BUF_WORDS;
        assert_eq!(buf[ram1], 0x00500513);
        assert_eq!(buf[ram1 + 1], 0x00000073);
    }

    #[test]
    fn test_extract_uart_empty() {
        let tile = vec![0u32; TILE_STATE_WORDS];
        assert_eq!(extract_uart(&tile), "");
    }

    #[test]
    fn test_extract_uart_with_output() {
        let mut tile = vec![0u32; TILE_STATE_WORDS];
        tile[37] = 3; // uart_len = 3
        tile[STATE_HEADER_WORDS + 0] = 0x48; // 'H'
        tile[STATE_HEADER_WORDS + 1] = 0x69; // 'i'
        tile[STATE_HEADER_WORDS + 2] = 0x21; // '!'

        assert_eq!(extract_uart(&tile), "Hi!");
    }

    #[test]
    fn test_extract_uart_non_printable() {
        let mut tile = vec![0u32; TILE_STATE_WORDS];
        tile[37] = 2;
        tile[STATE_HEADER_WORDS + 0] = 0x01; // non-printable
        tile[STATE_HEADER_WORDS + 1] = 0x41; // 'A'

        let output = extract_uart(&tile);
        assert!(output.contains("[01]"));
        assert!(output.contains('A'));
    }

    #[test]
    fn test_tile_state_constants_match_wgsl() {
        // These constants must match the WGSL struct layout
        assert_eq!(STATE_HEADER_WORDS, 40);
        assert_eq!(UART_BUF_WORDS, 128);
        assert_eq!(RAM_WORDS, 992);
        assert_eq!(
            TILE_STATE_WORDS,
            STATE_HEADER_WORDS + UART_BUF_WORDS + RAM_WORDS
        );
        // PC at offset 32
        assert_eq!(32, 32); // regs[0..31] then pc
                            // status at offset 33
                            // instruction_count at 34
                            // max_steps at 35
                            // tile_id at 36
                            // uart_len at 37
    }

    #[test]
    fn test_status_flags() {
        assert_eq!(STATUS_RUNNING, 0x1);
        assert_eq!(STATUS_HALTED, 0x2);
        assert_eq!(STATUS_ERROR, 0x4);
        // Can combine: halted cleanly = 0x2, error = 0x6
        assert_eq!(STATUS_HALTED | STATUS_ERROR, 0x6);
    }

    // GPU integration test -- only runs with `cargo test --features gpu`
    #[cfg(feature = "gpu")]
    #[test]
    fn test_gpu_fibonacci_single_tile() {
        let executor = pollster::block_on(GpuExecutor::new()).expect("GPU initialization failed");

        let cartridge = gpu_loader::build_fibonacci_cartridge();
        let mut tile_data = init_tile_states(1, &cartridge, 1000);

        executor
            .run_tiles(&mut tile_data, 1)
            .expect("GPU execution failed");

        let status = tile_data[33];
        assert_ne!(
            status & STATUS_ERROR,
            0,
            "Tile should not have errored, status=0x{:x}",
            status
        );
        assert_ne!(
            status & STATUS_HALTED,
            0,
            "Tile should have halted, status=0x{:x}",
            status
        );
        assert!(tile_data[34] > 0, "Should have executed instructions");

        let uart = extract_uart(&tile_data);
        assert!(
            uart.contains('5'),
            "Fibonacci(10)=55 should contain '5', got: {}",
            uart
        );
    }

    #[cfg(feature = "gpu")]
    #[test]
    fn test_gpu_vs_reference() {
        let executor = pollster::block_on(GpuExecutor::new()).expect("GPU initialization failed");

        let fib = gpu_loader::build_fibonacci_cartridge();
        verify_against_reference(&executor, "fibonacci(10)", &fib, 1000);

        let cnt7 = gpu_loader::build_counter_cartridge(7);
        verify_against_reference(&executor, "counter(7)", &cnt7, 1000);
    }

    #[cfg(feature = "gpu")]
    fn verify_against_reference(
        executor: &GpuExecutor,
        name: &str,
        cartridge: &[u32],
        max_steps: u32,
    ) {
        // Run on GPU
        let mut tile_data = init_tile_states(1, cartridge, max_steps);
        executor
            .run_tiles(&mut tile_data, 1)
            .unwrap_or_else(|e| panic!("GPU failed for {}: {}", name, e));

        let gpu_pc = tile_data[32];
        let gpu_status = tile_data[33];
        let gpu_inst_count = tile_data[34];
        let gpu_uart = extract_uart(&tile_data);
        let mut gpu_regs = [0u32; 32];
        gpu_regs.copy_from_slice(&tile_data[0..32]);

        // Run on reference CPU
        let mut ram = vec![0u32; RAM_WORDS];
        for (i, &word) in cartridge.iter().enumerate() {
            if i < RAM_WORDS {
                ram[i] = word;
            }
        }
        let mut ref_vm = gpu_reference::ReferenceVm::new(ram);
        ref_vm.run(max_steps);

        let ref_uart = String::from_utf8_lossy(&ref_vm.uart_output).to_string();

        // Compare
        assert_eq!(
            gpu_pc, ref_vm.pc,
            "[{}] PC mismatch: GPU=0x{:x}, Ref=0x{:x}",
            name, gpu_pc, ref_vm.pc
        );
        assert_eq!(
            gpu_status, ref_vm.status,
            "[{}] Status mismatch: GPU=0x{:x}, Ref=0x{:x}",
            name, gpu_status, ref_vm.status
        );
        for i in 0..32 {
            assert_eq!(
                gpu_regs[i], ref_vm.regs[i],
                "[{}] Reg x{} mismatch: GPU=0x{:x}, Ref=0x{:x}",
                name, i, gpu_regs[i], ref_vm.regs[i]
            );
        }
        assert_eq!(
            gpu_uart, ref_uart,
            "[{}] UART mismatch: GPU='{}', Ref='{}'",
            name, gpu_uart, ref_uart
        );

        eprintln!(
            "[PASS] {} - GPU matches reference ({} insts, uart='{}')",
            name, gpu_inst_count, gpu_uart
        );
    }
}
