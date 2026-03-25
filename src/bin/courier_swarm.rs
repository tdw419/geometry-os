// Courier Swarm — Semantic Transport
//
// Phase 8 Theta: The Logic Bridge
//   - Agents SENSE pixel colors
//   - PICKUP matching data
//   - TRANSPORT to target zones
//   - DROP and return
//
// This is the "Defrag" moment — agents actively sorting data.

use wgpu::*;
use wgpu::util::DeviceExt;
mod glyph_atlas;
use glyph_atlas::GlyphAtlas;

use image::{ImageBuffer, Rgba};
use std::time::Instant;

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
    cargo: f32,  // Register 5: 0.0 = Empty, 1.0 = Green, 2.0 = Blue
    distance_traveled: f32,  // Register 6: Energy metric
    trail: [u32; 30],
    mailbox: [u32; 10],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Well {
    pos_x: f32, pos_y: f32,
    strength: f32,
    drift_rate: f32,
    last_access: f32,
    tribe: u32,
    is_active: u32,
    _padding: u32, _padding2: u32, _padding3: u32, _padding4: u32, _padding5: u32,
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
    _padding: u32, _padding2: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Config {
    width: u32, height: u32,
    time: f32,
    frame: u32,
    mode: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct UIState {
    well_count: u32,
    _pad0: u32, _pad1: u32, _pad2: u32,
    wells: [Well; 16],
    radial: RadialState,
}

// ============================================================================
// AGENT SIMULATION (CPU-side with Courier Logic)
// ============================================================================

#[derive(Debug, Clone)]
struct CourierAgent {
    id: usize,
    pos_x: f32, pos_y: f32,
    vel_x: f32, vel_y: f32,
    tribe: u32,
    color: u32,
    cargo: f32,  // 0.0 = Empty, 1.0 = Green, 2.0 = Blue
    cargo_glyph: u32,  // Glyph ID to drop (0-8)
    distance_traveled: f32,
    deliveries: u32,
}

impl CourierAgent {
    fn new(id: usize) -> Self {
        let col = (id % 8) as f32;
        let row = (id / 8) as f32;
        let tribe = id as u32 % 8;

        // Assign glyph based on tribe
        let cargo_glyph = match tribe {
            0 | 1 => 1,  // File
            2 | 3 => 2,  // Folder
            4 | 5 => 3,  // Exec
            6 | 7 => 4,  // Data
            _ => 0,
        };

        Self {
            id,
            pos_x: 100.0 + col * 140.0 + (id as f32 * 13.0 % 30.0),
            pos_y: 150.0 + row * 80.0 + (id as f32 * 17.0 % 20.0),
            vel_x: (id as f32 * 7.0 % 5.0) - 2.5,
            vel_y: (id as f32 * 11.0 % 5.0) - 2.5,
            tribe,
            color: tribe_color(tribe),
            cargo: 0.0,
            cargo_glyph,
            distance_traveled: 0.0,
            deliveries: 0,
        }
    }
    
    /// SENSE: Read pixel at current position (from CPU data field)
    fn sense_data(&self, data_field: &[(f32, f32, f32, f32)], x: usize, y: usize) -> (f32, f32, f32) {
        if x < WIDTH as usize && y < HEIGHT as usize {
            let idx = y * WIDTH as usize + x;
            let (r, g, b, _a) = data_field[idx];
            (r, g, b)
        } else {
            (0.0, 0.0, 0.0)
        }
    }
    
    /// SENSE: Scan for data in a small radius around the agent
    fn sense_and_pickup(&mut self, data_field: &mut [(f32, f32, f32, f32)]) {
        if self.cargo > 0.0 {
            return;  // Already carrying
        }
        
        let scan_radius = 5i32;
        let px = self.pos_x as i32;
        let py = self.pos_y as i32;
        
        for dy in -scan_radius..=scan_radius {
            for dx in -scan_radius..=scan_radius {
                let x = (px + dx) as usize;
                let y = (py + dy) as usize;
                
                if x >= WIDTH as usize || y >= HEIGHT as usize || y < 100 {
                    continue;
                }
                
                let idx = y * WIDTH as usize + x;
                let (r, g, b, _a) = data_field[idx];
                
                // Tribe 0-3: Pick up Green (Core data)
                // Tribe 4-7: Pick up Blue (Archive data)
                let pickup = if self.tribe < 4 {
                    g > 0.5 && r < 0.3 && b < 0.3
                } else {
                    b > 0.5 && r < 0.3 && g < 0.3
                };
                
                if pickup {
                    self.cargo = if self.tribe < 4 { 1.0 } else { 2.0 };
                    self.vel_x *= 0.7;  // Momentum hit from weight
                    self.vel_y *= 0.7;
                    // Clear the picked-up pixel
                    data_field[idx] = (0.1, 0.1, 0.15, 1.0);
                    return;  // Stop scanning
                }
            }
        }
    }
    
    /// PUNCH GLYPH: Drop a 3x3 pattern instead of single pixel
    fn punch_glyph(&self, x: usize, y: usize, glyph_id: u32, data_field: &mut [(f32, f32, f32, f32)]) {
        let pixels = GlyphAtlas::get_pixels(glyph_id);

        for (dx, dy) in pixels {
            let px = (x as i32 + dx) as usize;
            let py = (y as i32 + dy) as usize;

            if px < WIDTH as usize && py < HEIGHT as usize && py >= 100 {
                let idx = py * WIDTH as usize + px;

                // Color based on cargo type
                let color = if self.cargo == 1.0 {
                    (0.2, 1.0, 0.2, 1.0)  // Green (Core data)
                } else {
                    (0.2, 0.2, 1.0, 1.0)  // Blue (Archive data)
                };

                data_field[idx] = color;
            }
        }
    }

    /// COURIER LOGIC: SENSE → PICKUP → TRANSPORT → DROP
    fn courier_update(&mut self, data_field: &mut [(f32, f32, f32, f32)]) {
        // Try to pick up data if empty
        self.sense_and_pickup(data_field);
        
        if self.cargo > 0.0 {
            // DELIVERY MODE: Transport to target zone
            let target = if self.tribe < 4 {
                // Green tribe → Center (Core)
                (WIDTH as f32 / 2.0, HEIGHT as f32 / 2.0)
            } else {
                // Blue tribe → Periphery (Archive)
                let angle = (self.id as f32) * 0.785;  // 45° intervals
                let radius = WIDTH as f32 * 0.35;
                (WIDTH as f32 / 2.0 + angle.cos() * radius,
                 HEIGHT as f32 / 2.0 + angle.sin() * radius)
            };
            
            let dx = target.0 - self.pos_x;
            let dy = target.1 - self.pos_y;
            let dist = (dx * dx + dy * dy).sqrt();
            
            // Steering force toward target
            if dist > 1.0 {
                self.vel_x += (dx / dist) * 0.5;
                self.vel_y += (dy / dist) * 0.5;
            }
            
            // DROP LOGIC: If close to target, deposit cargo as glyph
            if dist < 50.0 {
                let drop_x = self.pos_x as usize;
                let drop_y = self.pos_y as usize;

                // PUNCH the glyph pattern
                self.punch_glyph(drop_x, drop_y, self.cargo_glyph, data_field);

                self.cargo = 0.0;  // Empty cargo bay
                self.deliveries += 1;
                self.vel_x *= 1.5;  // Speed boost after delivery
                self.vel_y *= 1.5;
            }
        }
    }
    
    /// Apply physics and movement with Elastic Membrane border
    fn physics_update(&mut self) {
        // Friction
        self.vel_x *= 0.95;
        self.vel_y *= 0.95;
        
        // Clamp velocity
        let max_vel = 5.0;
        self.vel_x = self.vel_x.clamp(-max_vel, max_vel);
        self.vel_y = self.vel_y.clamp(-max_vel, max_vel);
        
        // Apply velocity
        self.pos_x += self.vel_x;
        self.pos_y += self.vel_y;
        
        // Track distance
        self.distance_traveled += (self.vel_x * self.vel_x + self.vel_y * self.vel_y).sqrt();
        
        // ELASTIC MEMBRANE: Keep agents around the Middle Pixel
        let center_x = WIDTH as f32 / 2.0;
        let center_y = HEIGHT as f32 / 2.0;
        let border_radius = WIDTH as f32 * 0.35;  // 35% of screen
        
        let dx = self.pos_x - center_x;
        let dy = self.pos_y - center_y;
        let dist_from_center = (dx * dx + dy * dy).sqrt();
        
        if dist_from_center > border_radius {
            // Elastic snap-back force
            let penetration = dist_from_center - border_radius;
            let snap_strength = penetration * 0.05;  // Proportional to how far past
            
            // Normalize direction toward center
            let dir_x = -dx / dist_from_center;
            let dir_y = -dy / dist_from_center;
            
            // Apply snap-back
            self.vel_x += dir_x * snap_strength;
            self.vel_y += dir_y * snap_strength;
        }
        
        // Hard boundary (fallback)
        if self.pos_x < 10.0 || self.pos_x > WIDTH as f32 - 10.0 {
            self.vel_x = -self.vel_x;
            self.pos_x = self.pos_x.clamp(10.0, WIDTH as f32 - 10.0);
        }
        if self.pos_y < 100.0 || self.pos_y > HEIGHT as f32 - 10.0 {
            self.vel_y = -self.vel_y;
            self.pos_y = self.pos_y.clamp(100.0, HEIGHT as f32 - 10.0);
        }
    }
    
    fn to_gpu_state(&self) -> AgentGpuState {
        AgentGpuState {
            id: self.id as u32,
            pos_x: self.pos_x,
            pos_y: self.pos_y,
            vel_x: self.vel_x,
            vel_y: self.vel_y,
            color: self.color,
            tribe: self.tribe,
            is_it: 0,
            message_waiting: 0,
            trail_len: 0,
            collision_count: 0,
            message_count: self.deliveries,
            cargo: self.cargo,
            distance_traveled: self.distance_traveled,
            trail: [0; 30],
            mailbox: [0; 10],
        }
    }
}

// ============================================================================
// RENDERER
// ============================================================================

struct CourierRenderer {
    device: Device,
    queue: Queue,
    pipeline: ComputePipeline,
    bind_group: BindGroup,
    output_buffer: Buffer,
    staging_buffer: Buffer,
    agent_buffer: Buffer,
    config_buffer: Buffer,
    ui_buffer: Buffer,
    frame: u32,
}

impl CourierRenderer {
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
                label: Some("Courier Renderer"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
            }, None)
            .await?;
        
        // Load shader
        let shader_source = include_str!("../../radial_drift_hud.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Courier Shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });
        
        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Courier Bind Group Layout"),
            entries: &[
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
        
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Courier Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Courier Pipeline"),
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
        
        let agent_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Agent Buffer"),
            size: (MAX_AGENTS * std::mem::size_of::<AgentGpuState>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        let config_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Config Buffer"),
            size: std::mem::size_of::<Config>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        let ui_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("UI Buffer"),
            size: std::mem::size_of::<UIState>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Courier Bind Group"),
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
            agent_buffer,
            config_buffer,
            ui_buffer,
            frame: 0,
        })
    }
    
    fn update_agents(&self, agents: &[CourierAgent]) {
        let gpu_states: Vec<AgentGpuState> = agents.iter().map(|a| a.to_gpu_state()).collect();
        self.queue.write_buffer(&self.agent_buffer, 0, bytemuck::cast_slice(&gpu_states));
    }
    
    fn update_config(&self, time: f32, frame: u32) {
        let config = Config {
            width: WIDTH,
            height: HEIGHT,
            time,
            frame,
            mode: 0,
        };
        self.queue.write_buffer(&self.config_buffer, 0, bytemuck::bytes_of(&config));
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
    
    fn read_framebuffer(&mut self) -> Result<Vec<Pixel>, Box<dyn std::error::Error>> {
        // Render first
        let _ = self.render()?;
        
        // Read back
        let buffer_slice = self.staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(MapMode::Read, move |result| { tx.send(result).unwrap(); });
        self.device.poll(Maintain::Wait);
        rx.recv()??;
        
        let data = buffer_slice.get_mapped_range();
        let pixels: Vec<Pixel> = bytemuck::cast_slice(&data).to_vec();
        
        drop(data);
        self.staging_buffer.unmap();
        
        Ok(pixels)
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

// ============================================================================
// MAIN
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║          COURIER SWARM — Semantic Transport              ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Phase 8 Theta: The Logic Bridge                         ║");
    println!("║  SENSE → PICKUP → TRANSPORT → DROP                       ║");
    println!("║  Agents actively sorting data to zones                   ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    println!("[INIT] Initializing GPU...");
    let mut renderer = CourierRenderer::new().await?;
    println!("[INIT] ✓ GPU ready");
    
    // Create agents
    let mut agents: Vec<CourierAgent> = (0..MAX_AGENTS)
        .map(|i| CourierAgent::new(i))
        .collect();
    
    // Create data field (RGBA as f32 for easier manipulation)
    let mut data_field = vec![(0.1f32, 0.1f32, 0.15f32, 1.0f32); (WIDTH * HEIGHT) as usize];
    
    // Scatter some Green and Blue data pixels
    let mut scattered = 0;
    for i in 0..500 {
        let x = ((i * 37 + 13) % WIDTH as usize) as usize;
        let y = (((i * 53 + 17) % 600) + 100) as usize;
        let idx = y * WIDTH as usize + x;
        if idx < data_field.len() {
            if i % 2 == 0 {
                data_field[idx] = (0.2, 1.0, 0.2, 1.0);  // Green (Core data)
            } else {
                data_field[idx] = (0.2, 0.2, 1.0, 1.0);  // Blue (Archive data)
            }
            scattered += 1;
        }
    }
    
    println!("[INIT] {} agents ready", MAX_AGENTS);
    println!("[INIT] {} data pixels scattered", scattered);
    println!();
    
    // Simulation loop
    let steps = 1000;
    let start = Instant::now();
    
    for step in 0..steps {
        // Update each agent with courier logic
        for agent in &mut agents {
            agent.courier_update(&mut data_field);
            agent.physics_update();
        }
        
        // Update GPU
        renderer.update_agents(&agents);
        renderer.update_config(start.elapsed().as_secs_f32(), step as u32);
        
        // Render GPU HUD
        let img = renderer.render()?;
        
        // Composite: overlay data field onto GPU output
        let mut final_img = img;
        for y in 0..HEIGHT as usize {
            for x in 0..WIDTH as usize {
                let idx = y * WIDTH as usize + x;
                let (r, g, b, a) = data_field[idx];
                
                // Only overlay if there's data (not background)
                if r > 0.15 || g > 0.15 || b > 0.2 {
                    let pixel = final_img.get_pixel_mut(x as u32, y as u32);
                    // Blend with existing
                    pixel[0] = ((pixel[0] as f32 * 0.5) + (r * 255.0 * 0.5)) as u8;
                    pixel[1] = ((pixel[1] as f32 * 0.5) + (g * 255.0 * 0.5)) as u8;
                    pixel[2] = ((pixel[2] as f32 * 0.5) + (b * 255.0 * 0.5)) as u8;
                }
            }
        }
        
        // Save periodic frames
        if step % 100 == 0 {
            let path = format!("output/courier_frame_{:04}.png", step);
            final_img.save(&path)?;
            
            // Calculate metrics
            let total_deliveries: u32 = agents.iter().map(|a| a.deliveries).sum();
            let carrying: usize = agents.iter().filter(|a| a.cargo > 0.0).count();
            let avg_distance: f32 = agents.iter().map(|a| a.distance_traveled).sum::<f32>() / MAX_AGENTS as f32;
            
            // Count remaining data pixels
            let green_count = data_field.iter().filter(|(r, g, b, _)| *g > 0.5 && *r < 0.3).count();
            let blue_count = data_field.iter().filter(|(r, g, b, _)| *b > 0.5 && *r < 0.3).count();
            
            println!("[{:4}] Deliveries: {:3} | Carrying: {:2} | Green: {:3} | Blue: {:3} | Dist: {:.0}", 
                step, total_deliveries, carrying, green_count, blue_count, avg_distance);
        }
    }
    
    // Final output
    let img = renderer.render()?;
    let output_path = "output/courier_swarm.png";
    img.save(output_path)?;
    
    let elapsed = start.elapsed();
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║              COURIER SWARM COMPLETE                      ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Steps: {:5}                                          ║", steps);
    println!("║  Time: {:?}", elapsed);
    println!("║  Output: {}", output_path);
    println!("╚══════════════════════════════════════════════════════════╝");
    
    Ok(())
}
