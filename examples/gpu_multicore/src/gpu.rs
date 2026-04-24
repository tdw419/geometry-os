//! GPU Executor for RISC-V Tile Execution
//!
//! Shared GPU initialization, buffer management, compute dispatch, and readback.
//! Extracted from duplicated code across multi_tile_ignition and executive_commander.

use anyhow::{Context, Result};

// WGSL compute shader for RISC-V tile execution
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

/// Initialize tile state buffer for N tiles with a single cartridge
pub fn init_tile_states(num_tiles: usize, cartridge: &[u32], max_steps: u32) -> Vec<u32> {
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

/// Initialize tile state buffer for N tiles with different cartridges per tile
pub fn init_tile_states_multi(cartridges: &[(&str, &[u32])], max_steps: u32) -> Vec<u32> {
    let num_tiles = cartridges.len();
    let mut buf = vec![0u32; num_tiles * TILE_STATE_WORDS];

    for (i, (_name, cart)) in cartridges.iter().enumerate() {
        let base = i * TILE_STATE_WORDS;
        buf[base + 32] = 0;              // PC = 0
        buf[base + 33] = STATUS_RUNNING; // status = running
        buf[base + 34] = 0;              // instruction_count
        buf[base + 35] = max_steps;      // max_steps
        buf[base + 36] = i as u32;       // tile_id
        buf[base + 37] = 0;              // uart_len

        let ram_base = base + STATE_HEADER_WORDS + UART_BUF_WORDS;
        for (j, &word) in cart.iter().enumerate() {
            if j < RAM_WORDS {
                buf[ram_base + j] = word;
            }
        }
    }
    buf
}

pub struct GpuExecutor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
}

impl GpuExecutor {
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

    pub fn run_tiles(&self, tile_data: &mut Vec<u32>, num_tiles: u32) -> Result<()> {
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
