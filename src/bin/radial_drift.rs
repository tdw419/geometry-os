// Radial Drift - Self-Organizing Memory Map
//
// Phase 8 Eta: Semantic Defrag
//   - Master gravity well at center
//   - Radial priority: center = active, periphery = archive
//   - Tribe-based color sorting
//   - Drift physics for unused wells

use wgpu::*;
use wgpu::util::DeviceExt;
use image::{ImageBuffer, Rgba};
use std::time::Instant;
use bytemuck::Zeroable;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;
const MAX_AGENTS: usize = 64;
const MAX_WELLS: usize = 16;

// ============================================================================
// STRUCTS
// ============================================================================

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Pixel {
    r: u32, g: u32, b: u32, a: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct AgentGpuState {
    id: u32,
    pos_x: f32, pos_y: f32,
    vel_x: f32, vel_y: f32,
    color: u32,
    tribe: u32,
    is_it: u32,
    message_waiting: u32,
    trail_len: u32,
    collision_count: u32,
    message_count: u32,
    _padding: u32,
    trail: [u32; 32],
    mailbox: [u32; 10],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Well {
    pos_x: f32, pos_y: f32,          // 8 bytes
    strength: f32,                    // 4 bytes
    drift_rate: f32,                  // 4 bytes
    last_access: f32,                 // 4 bytes
    tribe: u32,                       // 4 bytes
    is_active: u32,                   // 4 bytes
    _padding: u32, _padding2: u32, _padding3: u32, _padding4: u32, _padding5: u32,  // 20 bytes = 48 total
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RadialState {
    master_well_x: f32,
    master_well_y: f32,
    core_radius: f32,
    inner_radius: f32,
    drift_enabled: u32,
    color_sorting: u32,
    _padding: u32,
    _padding2: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Config {
    width: u32,
    height: u32,
    time: f32,
    frame: u32,
    mode: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct UIState {
    well_count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    wells: [Well; 16],
    radial: RadialState,
}

struct RadialDriftHud {
    device: Device,
    queue: Queue,
    pipeline: ComputePipeline,
    bind_group: BindGroup,
    output_buffer: Buffer,
    staging_buffer: Buffer,
    frame: u32,
}

impl RadialDriftHud {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let instance = Instance::new(InstanceDescriptor::default());
        
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or("No adapter found")?;
        
        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: Some("Radial Drift HUD"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
            }, None)
            .await?;
        
        // Load shader
        let shader_source = include_str!("../../radial_drift_hud.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Radial Drift Shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });
        
        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Radial Drift Bind Group Layout"),
            entries: &[
                // output (binding 0)
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // agents (binding 1)
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // config (binding 2)
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // ui (binding 3)
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        
        // Create pipeline
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Radial Drift Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Radial Drift Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });
        
        // Create buffers
        let pixel_count = (WIDTH * HEIGHT) as u64;
        let buffer_size = pixel_count * std::mem::size_of::<Pixel>() as u64;
        
        let output_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Output Buffer"),
            size: buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        
        let staging_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Staging Buffer"),
            size: buffer_size,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        // Create agents
        let mut agents = vec![AgentGpuState::zeroed(); MAX_AGENTS];
        for i in 0..MAX_AGENTS {
            let col = (i % 8) as f32;
            let row = (i / 8) as f32;
            
            agents[i] = AgentGpuState {
                id: i as u32,
                pos_x: 100.0 + col * 140.0 + (i as f32 * 13.0 % 30.0),
                pos_y: 150.0 + row * 80.0 + (i as f32 * 17.0 % 20.0),
                vel_x: (i as f32 * 7.0 % 5.0) - 2.5,
                vel_y: (i as f32 * 11.0 % 5.0) - 2.5,
                color: tribe_color(i as u32 % 8),
                tribe: i as u32 % 8,
                is_it: if i == 0 { 1 } else { 0 },
                message_waiting: 0,
                trail_len: 0,
                collision_count: 0,
                message_count: 0,
                _padding: 0,
                trail: [0; 32],
                mailbox: [0; 10],
            };
        }
        
        let agent_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Agent Buffer"),
            contents: bytemuck::cast_slice(&agents),
            usage: BufferUsages::STORAGE,
        });
        
        // Create wells
        let mut wells = [Well::zeroed(); 16];
        
        // Core wells (high priority)
        wells[0] = Well {
            pos_x: 0.35, pos_y: 0.4,
            strength: 0.8,
            drift_rate: 0.001,
            last_access: 0.0,
            tribe: 1,  // Green
            is_active: 1,
            _padding: 0,
            _padding2: 0,
            _padding3: 0,
            _padding4: 0,
            _padding5: 0,
        };
        
        wells[1] = Well {
            pos_x: 0.65, pos_y: 0.4,
            strength: 0.8,
            drift_rate: 0.001,
            last_access: 0.0,
            tribe: 2,  // Blue
            is_active: 1,
            _padding: 0,
            _padding2: 0,
            _padding3: 0,
            _padding4: 0,
            _padding5: 0,
        };
        
        // Inner wells (medium priority)
        wells[2] = Well {
            pos_x: 0.25, pos_y: 0.6,
            strength: 0.6,
            drift_rate: 0.002,
            last_access: -10.0,
            tribe: 0,  // Red
            is_active: 1,
            _padding: 0,
            _padding2: 0,
            _padding3: 0,
            _padding4: 0,
            _padding5: 0,
        };
        
        wells[3] = Well {
            pos_x: 0.75, pos_y: 0.6,
            strength: 0.6,
            drift_rate: 0.002,
            last_access: -15.0,
            tribe: 3,  // Yellow
            is_active: 1,
            _padding: 0,
            _padding2: 0,
            _padding3: 0,
            _padding4: 0,
            _padding5: 0,
        };
        
        // Periphery wells (low priority)
        wells[4] = Well {
            pos_x: 0.1, pos_y: 0.8,
            strength: 0.4,
            drift_rate: 0.003,
            last_access: -30.0,
            tribe: 4,  // Magenta
            is_active: 1,
            _padding: 0,
            _padding2: 0,
            _padding3: 0,
            _padding4: 0,
            _padding5: 0,
        };
        
        wells[5] = Well {
            pos_x: 0.9, pos_y: 0.8,
            strength: 0.4,
            drift_rate: 0.003,
            last_access: -25.0,
            tribe: 5,  // Cyan
            is_active: 1,
            _padding: 0,
            _padding2: 0,
            _padding3: 0,
            _padding4: 0,
            _padding5: 0,
        };
        
        let ui_state = UIState {
            well_count: 6,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
            wells,
            radial: RadialState {
                master_well_x: 0.5,
                master_well_y: 0.5,
                core_radius: 0.15,
                inner_radius: 0.35,
                drift_enabled: 1,
                color_sorting: 1,
                _padding: 0,
                _padding2: 0,
            },
        };
        
        let config = Config {
            width: WIDTH,
            height: HEIGHT,
            time: 0.0,
            frame: 0,
            mode: 0,
        };
        
        let config_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Config Buffer"),
            contents: bytemuck::bytes_of(&config),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        
        let ui_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("UI Buffer"),
            contents: bytemuck::bytes_of(&ui_state),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        
        // Create bind group
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Radial Drift Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: output_buffer.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: agent_buffer.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: config_buffer.as_entire_binding() },
                BindGroupEntry { binding: 3, resource: ui_buffer.as_entire_binding() },
            ],
        });
        
        Ok(Self {
            device,
            queue,
            pipeline,
            bind_group,
            output_buffer,
            staging_buffer,
            frame: 0,
        })
    }
    
    fn render(&mut self) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, Box<dyn std::error::Error>> {
        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });
        
        {
            let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Render Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &self.bind_group, &[]);
            compute_pass.dispatch_workgroups((WIDTH * HEIGHT + 63) / 64, 1, 1);
        }
        
        let buffer_size = (WIDTH * HEIGHT * 16) as u64;
        encoder.copy_buffer_to_buffer(&self.output_buffer, 0, &self.staging_buffer, 0, buffer_size);
        
        self.queue.submit(std::iter::once(encoder.finish()));
        
        // Read output
        let buffer_slice = self.staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(MapMode::Read, move |result| { tx.send(result).unwrap(); });
        self.device.poll(Maintain::Wait);
        rx.recv()??;
        
        let data = buffer_slice.get_mapped_range();
        let pixels: &[Pixel] = bytemuck::cast_slice(&data);
        
        let mut img = ImageBuffer::new(WIDTH, HEIGHT);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            let idx = (y * WIDTH + x) as usize;
            let p = &pixels[idx];
            *pixel = Rgba([p.r as u8, p.g as u8, p.b as u8, p.a as u8]);
        }
        
        drop(data);
        self.staging_buffer.unmap();
        
        self.frame += 1;
        
        Ok(img)
    }
}

fn tribe_color(tribe: u32) -> u32 {
    match tribe % 8 {
        0 => 0xFF4040FF,  // Red
        1 => 0x40FF40FF,  // Green
        2 => 0x4040FFFF,  // Blue
        3 => 0xFFFF40FF,  // Yellow
        4 => 0xFF40FFFF,  // Magenta
        5 => 0x40FFFFFF,  // Cyan
        6 => 0xFF8040FF,  // Orange
        7 => 0x8040FFFF,  // Purple
        _ => 0x808080FF,  // Gray
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║       RADIAL DRIFT — SELF-ORGANIZING MEMORY MAP        ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Phase 8 Eta: Semantic Defrag                           ║");
    println!("║  Master well at center, radial priority zones           ║");
    println!("║  Tribe-based color sorting, drift physics               ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    println!("[GPU] Initializing...");
    let mut hud = RadialDriftHud::new().await?;
    println!("[GPU] Pipeline ready");
    println!();
    
    println!("Architecture:");
    println!("  Master Well: (0.5, 0.5) - center");
    println!("  Core Radius: 0.15 (high priority)");
    println!("  Inner Radius: 0.35 (medium priority)");
    println!("  Periphery: > 0.35 (archive, drifts outward)");
    println!();
    println!("  Wells: 6 active");
    println!("    - 2 Core (Green, Blue)");
    println!("    - 2 Inner (Red, Yellow) - unused, drifting");
    println!("    - 2 Periphery (Magenta, Cyan) - long unused");
    println!();
    println!("  Agents: {} across 8 tribes", MAX_AGENTS);
    println!("  Color Sorting: enabled (tribes sort to their zones)");
    println!("  Drift Physics: enabled (unused wells migrate outward)");
    println!();
    
    println!("[RENDER] Rendering radial drift HUD...");
    let start = Instant::now();
    let img = hud.render()?;
    let render_time = start.elapsed();
    
    // Save output
    std::fs::create_dir_all("output")?;
    let output_path = "output/radial_drift.png";
    img.save(output_path)?;
    
    println!("[OUTPUT] {} ({}ms)", output_path, render_time.as_millis());
    println!();
    
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║              RADIAL DRIFT COMPLETE                      ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  ✅ Master gravity well at center                       ║");
    println!("║  ✅ Radial priority zones visualized                    ║");
    println!("║  ✅ Tribe-based color sorting active                    ║");
    println!("║  ✅ Drift physics for unused wells                      ║");
    println!("║  ✅ ~{}ms render time                                  ║", render_time.as_millis());
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    println!("Next: Add interaction (click to pull well to center)");
    println!("Next: Add visual defrag animation (agents carrying pixels)");
    
    Ok(())
}
