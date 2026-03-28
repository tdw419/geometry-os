// Phase 31: Full GPU Demo - 3000+ FPS Target
//
// Headless GPU demo combining:
// - Spatial physics (signal propagation)
// - Agent simulation (gravity wells)
// - Neural saccade paths (comet rendering)
//
// Target: 3000+ FPS on RTX 5090
//
// Usage:
//   cargo run --release --bin phase31-demo
//
// Output: output/phase31_demo.png

use wgpu::*;
use wgpu::util::{DeviceExt, BufferInitDescriptor};
use image::{ImageBuffer, Rgba};
use std::time::{Instant};
use std::fs;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;
const MAX_AGENTS: usize = 256;
const MAX_WELLS: usize = 16;
const MAX_SACCADES: usize = 64;
const TOTAL_FRAMES: u32 = 1000;

// ============================================================================
// STRUCTS
// ============================================================================

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct AgentGpuState {
    pos_x: f32, pos_y: f32,
    vel_x: f32, vel_y: f32,
    color: u32,
    tribe: u32,
    signal_strength: f32,
    _padding: f32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct Well {
    pos_x: f32, pos_y: f32,
    strength: f32,
    z_index: f32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct SaccadePath {
    start_x: f32, start_y: f32,
    control_x: f32, control_y: f32,
    end_x: f32, end_y: f32,
    similarity: f32,
    effort: f32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct Uniforms {
    time: f32,
    frame: u32,
    width: u32,
    height: u32,
    agent_count: u32,
    well_count: u32,
    saccade_count: u32,
    mode: u32,
}

// ============================================================================
// AGENT (CPU simulation)
// ============================================================================

impl AgentGpuState {
    fn new(id: usize) -> Self {
        Self {
            pos_x: (id as f32 % 32.0) * 40.0 + 20.0,
            pos_y: (id as f32 / 32.0) * 25.0 + 20.0,
            vel_x: 0.0,
            vel_y: 0.0,
            color: 0xFF00FF00,
            tribe: (id % 4) as u32,
            signal_strength: 0.0,
            _padding: 0.0,
        }
    }

    fn update(&mut self, wells: &[Well]) {
        const FRICTION: f32 = 0.95;
        const MAX_FORCE: f32 = 50.0;

        let mut fx = 0.0;
        let mut fy = 0.0;

        for well in wells {
            let dx = well.pos_x - self.pos_x;
            let dy = well.pos_y - self.pos_y;
            let dist_sq = dx * dx + dy * dy + 0.01;
            let dist = dist_sq.sqrt();
            let force = (well.strength / dist_sq).min(MAX_FORCE);
            fx += dx / dist * force;
            fy += dy / dist * force;
        }

        self.vel_x = (self.vel_x + fx) * FRICTION;
        self.vel_y = (self.vel_y + fy) * FRICTION;
        self.pos_x += self.vel_x;
        self.pos_y += self.vel_y;

        // Bounds
        if self.pos_x < 0.0 { self.pos_x = 0.0; self.vel_x *= -0.5; }
        if self.pos_x > WIDTH as f32 { self.pos_x = WIDTH as f32; self.vel_x *= -0.5; }
        if self.pos_y < 0.0 { self.pos_y = 0.0; self.vel_y *= -0.5; }
        if self.pos_y > HEIGHT as f32 { self.pos_y = HEIGHT as f32; self.vel_y *= -0.5; }

        // Decay signal
        self.signal_strength *= 0.98;
    }
}

// ============================================================================
// RENDERER
// ============================================================================

struct Phase31Renderer {
    device: Device,
    queue: Queue,
    pipeline: ComputePipeline,
    bind_group: BindGroup,
    agent_buffer: Buffer,
    well_buffer: Buffer,
    saccade_buffer: Buffer,
    output_buffer: Buffer,
    uniform_buffer: Buffer,
    staging_buffer: Buffer,
}

impl Phase31Renderer {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let instance = Instance::new(InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                ..Default::default()
            })
            .await
            .ok_or("Failed to find adapter")?;

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: Some("Phase 31 GPU"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
            }, None)
            .await?;

        // Create buffers
        let agent_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Agent Buffer"),
            contents: bytemuck::cast_slice(&vec![AgentGpuState::default(); MAX_AGENTS]),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        let well_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Well Buffer"),
            contents: bytemuck::cast_slice(&vec![Well::default(); MAX_WELLS]),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        let saccade_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Saccade Buffer"),
            contents: bytemuck::cast_slice(&vec![SaccadePath::default(); MAX_SACCADES]),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        let output_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Output Buffer"),
            size: (WIDTH * HEIGHT * 4) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Uniform Buffer"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let staging_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Staging Buffer"),
            size: (WIDTH * HEIGHT * 4) as u64,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Load shader
        let shader_source = include_str!("../../phase31_demo.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Phase 31 Shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry { binding: 0, visibility: ShaderStages::COMPUTE, ty: BindingType::Buffer { ty: BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                BindGroupLayoutEntry { binding: 1, visibility: ShaderStages::COMPUTE, ty: BindingType::Buffer { ty: BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                BindGroupLayoutEntry { binding: 2, visibility: ShaderStages::COMPUTE, ty: BindingType::Buffer { ty: BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                BindGroupLayoutEntry { binding: 3, visibility: ShaderStages::COMPUTE, ty: BindingType::Buffer { ty: BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                BindGroupLayoutEntry { binding: 4, visibility: ShaderStages::COMPUTE, ty: BindingType::Buffer { ty: BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
            ],
        });

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: output_buffer.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: agent_buffer.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: well_buffer.as_entire_binding() },
                BindGroupEntry { binding: 3, resource: saccade_buffer.as_entire_binding() },
                BindGroupEntry { binding: 4, resource: uniform_buffer.as_entire_binding() },
            ],
        });

        // Create pipeline
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Compute Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "compute_main",
        });

        Ok(Self {
            device,
            queue,
            pipeline,
            bind_group,
            agent_buffer,
            well_buffer,
            saccade_buffer,
            output_buffer,
            uniform_buffer,
            staging_buffer,
        })
    }

    fn update(&mut self, agents: &[AgentGpuState], wells: &[Well], saccades: &[SaccadePath], frame: u32) {
        let uniforms = Uniforms {
            time: frame as f32 / 60.0,
            frame,
            width: WIDTH,
            height: HEIGHT,
            agent_count: agents.len() as u32,
            well_count: wells.len() as u32,
            saccade_count: saccades.len() as u32,
            mode: 2,  // Combined
        };

        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
        self.queue.write_buffer(&self.agent_buffer, 0, bytemuck::cast_slice(agents));
        self.queue.write_buffer(&self.well_buffer, 0, bytemuck::cast_slice(wells));
        self.queue.write_buffer(&self.saccade_buffer, 0, bytemuck::cast_slice(saccades));
    }

    fn render(&mut self) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, Box<dyn std::error::Error>> {
        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Encoder"),
        });

        // Compute pass
        {
            let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Compute Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &self.bind_group, &[]);
            compute_pass.dispatch_workgroups(WIDTH / 8, HEIGHT / 8, 1);
        }

        // Copy to staging
        encoder.copy_buffer_to_buffer(&self.output_buffer, 0, &self.staging_buffer, 0, (WIDTH * HEIGHT * 4) as u64);

        self.queue.submit(std::iter::once(encoder.finish()));

        // Read back
        let buffer_slice = self.staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(MapMode::Read, move |result| { tx.send(result).unwrap(); });
        self.device.poll(Maintain::Wait);
        rx.recv()??;

        let data = buffer_slice.get_mapped_range().to_vec();
        self.staging_buffer.unmap();

        // Convert to image
        let mut img = ImageBuffer::new(WIDTH, HEIGHT);
        for (i, pixel) in data.chunks_exact(4).enumerate() {
            let x = (i % WIDTH as usize) as u32;
            let y = (i / WIDTH as usize) as u32;
            img.put_pixel(x, y, Rgba([pixel[0], pixel[1], pixel[2], pixel[3]]));
        }

        Ok(img)
    }
}

impl Default for AgentGpuState {
    fn default() -> Self {
        Self {
            pos_x: 0.0, pos_y: 0.0,
            vel_x: 0.0, vel_y: 0.0,
            color: 0,
            tribe: 0,
            signal_strength: 0.0,
            _padding: 0.0,
        }
    }
}

impl Default for Well {
    fn default() -> Self {
        Self {
            pos_x: 0.0, pos_y: 0.0,
            strength: 0.0,
            z_index: 0.0,
        }
    }
}

impl Default for SaccadePath {
    fn default() -> Self {
        Self {
            start_x: 0.0, start_y: 0.0,
            control_x: 0.0, control_y: 0.0,
            end_x: 0.0, end_y: 0.0,
            similarity: 0.0,
            effort: 0.0,
        }
    }
}

// ============================================================================
// MAIN
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("════════════════════════════════════════════════════════════");
    println!(" Phase 31: Full GPU Demo - 3000+ FPS Target");
    println!("════════════════════════════════════════════════════════════");
    println!();
    println!("Agents: {}", MAX_AGENTS);
    println!("Resolution: {}x{}", WIDTH, HEIGHT);
    println!("Frames: {}", TOTAL_FRAMES);
    println!();

    // Initialize agents
    let mut agents: Vec<AgentGpuState> = (0..MAX_AGENTS).map(AgentGpuState::new).collect();

    // Initialize wells
    let mut wells = vec![
        Well { pos_x: WIDTH as f32 / 2.0, pos_y: HEIGHT as f32 / 2.0, strength: 2000.0, z_index: 1.0 },
        Well { pos_x: 300.0, pos_y: 400.0, strength: 1000.0, z_index: 2.0 },
        Well { pos_x: 980.0, pos_y: 400.0, strength: 1000.0, z_index: 3.0 },
    ];

    // Initialize saccades (neural paths)
    let saccades: Vec<SaccadePath> = (0..20).map(|i| {
        let start_x = (i as f32 % 5.0) * 200.0 + 100.0;
        let start_y = (i as f32 / 5.0) * 150.0 + 100.0;
        let end_x = WIDTH as f32 - start_x;
        let end_y = HEIGHT as f32 - start_y;
        SaccadePath {
            start_x, start_y,
            control_x: WIDTH as f32 / 2.0 + (i as f32 - 10.0) * 30.0,
            control_y: HEIGHT as f32 / 2.0 + (i as f32 - 10.0) * 20.0,
            end_x, end_y,
            similarity: 0.5 + (i as f32 % 5.0) * 0.1,
            effort: 0.3 + (i as f32 % 3.0) * 0.2,
        }
    }).collect();

    println!("[GPU] Initializing...");
    let rt = tokio::runtime::Runtime::new()?;
    let mut renderer = rt.block_on(Phase31Renderer::new())?;
    println!("[GPU] Pipeline ready");
    println!();

    println!("[SIM] Running {} frames...", TOTAL_FRAMES);
    let start_time = Instant::now();

    for frame in 0..TOTAL_FRAMES {
        // Animate wells (orbit)
        let t = frame as f32 * 0.02;
        wells[1].pos_x = 300.0 + (t * 2.0).sin() * 100.0;
        wells[1].pos_y = 400.0 + (t * 2.0).cos() * 50.0;
        wells[2].pos_x = 980.0 + (t * 2.0 + std::f32::consts::PI).sin() * 100.0;
        wells[2].pos_y = 400.0 + (t * 2.0 + std::f32::consts::PI).cos() * 50.0;

        // Update agents
        for agent in &mut agents {
            agent.update(&wells);
        }

        // Update GPU
        renderer.update(&agents, &wells, &saccades, frame);

        // Render
        let _img = renderer.render()?;

        // Progress
        if frame % 100 == 0 {
            let elapsed = start_time.elapsed().as_secs_f64();
            let fps = frame as f64 / elapsed;
            println!("[FRAME {}/{}] FPS: {:.0}", frame, TOTAL_FRAMES, fps);
        }
    }

    let total_time = start_time.elapsed();
    let fps = TOTAL_FRAMES as f64 / total_time.as_secs_f64();

    // Save final frame
    renderer.update(&agents, &wells, &saccades, TOTAL_FRAMES);
    let final_img = renderer.render()?;
    fs::create_dir_all("output")?;
    final_img.save("output/phase31_demo.png")?;

    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║              PHASE 31 DEMO COMPLETE                              ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  ✅ {} agents simulated                                          ║", MAX_AGENTS);
    println!("║  ✅ {} gravity wells (animated)                                  ║", wells.len());
    println!("║  ✅ {} neural saccade paths                                      ║", saccades.len());
    println!("║  ✅ Combined mode (Spatial + Neural)                             ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  Performance:                                                    ║");
    println!("║    Total frames: {}                                              ║", TOTAL_FRAMES);
    println!("║    Total time:   {:.2}s                                          ║", total_time.as_secs_f64());
    println!("║    FPS:          {:.0}                                           ║", fps);
    if fps >= 3000.0 {
        println!("║    TARGET MET:   ✅ 3000+ FPS                                    ║");
    } else {
        println!("║    TARGET:       ⏳ {:.0} FPS to go                             ║", 3000.0 - fps);
    }
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  Output: output/phase31_demo.png                                 ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");

    Ok(())
}
