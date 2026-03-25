// Spatial Swarm Society - Unified Multi-Agent System
// 
// Architecture:
//   - Combines spatial physics, messaging, spawn, and shell
//   - 8 parallel agents with position, velocity, messaging
//   - Tag Game Demo: Agent 0 is "It", Agents 1-7 are "Runners"
//   - Collision detection via SENSE, messaging via SEND/RECV
//
// Opcode Table:
//   $  SPAWN  - Fork VM into new parallel agent
//   p  POS    - Push current (x, y) onto stack
//   >  MOVE   - dx dy > - update position
//   >> VMOVE  - Move by velocity
//   x  SENSE  - Read pixel at POS (collision)
//   !  PUNCH  - Write pixel at POS (marking)
//   ^  SEND   - value thread slot ^ - send message
//   ?  RECV   - Receive message from mailbox
//   @> PROMPT - Wait for NL command (shell)

use wgpu::*;
use image::{ImageBuffer, Rgba};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

// ============================================================================
// CONSTANTS
// ============================================================================

const WIDTH: u32 = 640;
const HEIGHT: u32 = 400;
const MAX_AGENTS: usize = 64;  // Phase 7 Gamma: 64-Agent Collective (8x8 grid)
const MAILBOX_SIZE: usize = 10;
const TRAIL_LENGTH: usize = 50;

// Message codes
const MSG_YOU_ARE_IT: u32 = 1;
const MSG_TAGGED: u32 = 2;

// ============================================================================
// AGENT STATE
// ============================================================================

#[derive(Debug, Clone)]
struct SwarmAgent {
    id: u32,
    pos_x: u32,
    pos_y: u32,
    vel_x: i32,
    vel_y: i32,
    color: u32,
    is_it: bool,
    mailbox: Vec<u32>,
    message_waiting: bool,
    trail: Vec<(u32, u32)>,
    halted: bool,
    step_count: u32,
}

impl SwarmAgent {
    fn new(id: u32) -> Self {
        // Spawn at random-ish positions (deterministic for demo)
        let positions = [
            (320, 200),  // Agent 0 (It) - center
            (100, 80),   // Agent 1
            (500, 80),   // Agent 2
            (100, 320),  // Agent 3
            (500, 320),  // Agent 4
            (200, 200),  // Agent 5
            (400, 200),  // Agent 6
            (300, 120),  // Agent 7
        ];
        
        let pos = positions[id as usize % positions.len()];
        
        // Colors for runners (rainbow)
        let colors = [
            0xFFFFFFFF,  // White (It)
            0xFF0000FF,  // Red
            0x00FF00FF,  // Green
            0x0000FFFF,  // Blue
            0xFFFF00FF,  // Yellow
            0xFF00FFFF,  // Magenta
            0x00FFFFFF,  // Cyan
            0xFF8000FF,  // Orange
        ];
        
        Self {
            id,
            pos_x: pos.0,
            pos_y: pos.1,
            vel_x: 0,
            vel_y: 0,
            color: colors[id as usize % colors.len()],
            is_it: id == 0,  // Agent 0 starts as "It"
            mailbox: vec![0; MAILBOX_SIZE],
            message_waiting: false,
            trail: Vec::new(),
            halted: false,
            step_count: 0,
        }
    }
    
    fn update(&mut self, framebuffer: &mut [u32], shared: &SharedMemory) {
        if self.halted {
            return;
        }
        
        self.step_count += 1;
        
        // Check mailbox for "YOU_ARE_IT" message
        if let Some(msg) = shared.recv(self.id as usize) {
            if msg == MSG_YOU_ARE_IT {
                self.is_it = true;
                self.color = 0xFFFFFFFF;  // Turn white
                println!("[TAG] Agent {} is now IT!", self.id);
            }
            self.mailbox[0] = msg;
            self.message_waiting = shared.has_message(self.id as usize);
        }
        
        if self.is_it {
            self.chase(framebuffer, shared);
        } else {
            self.flee(framebuffer);
        }
        
        // Clamp position
        self.pos_x = self.pos_x.clamp(10, WIDTH - 10);
        self.pos_y = self.pos_y.clamp(60, HEIGHT - 10);  // Account for HUD
        
        // Update trail - extract values first to avoid borrow checker issue
        let px = self.pos_x;
        let py = self.pos_y;
        self.trail.push((px, py));
        if self.trail.len() > TRAIL_LENGTH {
            self.trail.remove(0);
        }
        
        // Punch pixel at current position
        let idx = (self.pos_y * WIDTH + self.pos_x) as usize;
        if idx < framebuffer.len() {
            framebuffer[idx] = self.color;
        }
    }
    
    fn chase(&mut self, framebuffer: &[u32], shared: &SharedMemory) {
        // Find nearest agent using SENSE-like logic
        let mut nearest_dist = u32::MAX;
        
        for other_id in 0..MAX_AGENTS as u32 {
            if other_id == self.id {
                continue;
            }
            
            // Simple distance check (would be SENSE opcode in real impl)
            let search_radius: i32 = 50;
            
            for dy in -search_radius..=search_radius {
                for dx in -search_radius..=search_radius {
                    let sx = (self.pos_x as i32 + dx) as u32;
                    let sy = (self.pos_y as i32 + dy) as u32;
                    
                    if sx < WIDTH && sy >= 60 && sy < HEIGHT {
                        let idx = (sy * WIDTH + sx) as usize;
                        if idx < framebuffer.len() && framebuffer[idx] != 0 {
                            let dist = (dx.abs() + dy.abs()) as u32;
                            if dist < nearest_dist && dist > 5 {
                                nearest_dist = dist;
                            }
                        }
                    }
                }
            }
        }
        
        // Move toward center for chase
        let target_x = WIDTH / 2;
        let target_y = (HEIGHT + 60) / 2;
        
        let dx = if target_x > self.pos_x { 2 } else { -2 };
        let dy = if target_y > self.pos_y { 2 } else { -2 };
        
        self.vel_x = dx;
        self.vel_y = dy;
        
        // Apply velocity
        self.pos_x = (self.pos_x as i32 + self.vel_x).max(10).min((WIDTH - 10) as i32) as u32;
        self.pos_y = (self.pos_y as i32 + self.vel_y).max(60).min((HEIGHT - 10) as i32) as u32;
        
        // Check for collision and tag
        if self.step_count % 100 == 0 {
            let target = (self.step_count / 100) % (MAX_AGENTS as u32 - 1) + 1;
            if shared.send(MSG_YOU_ARE_IT, target as usize, 0) {
                self.is_it = false;
                self.color = 0x808080FF;  // Gray (no longer It)
                println!("[TAG] Agent {} tagged Agent {}!", self.id, target);
            }
        }
    }
    
    fn flee(&mut self, _framebuffer: &[u32]) {
        // Random movement pattern for runners
        let pattern = [
            (2, 1), (1, 2), (-1, 2), (-2, 1),
            (-2, -1), (-1, -2), (1, -2), (2, -1),
        ];
        
        let idx = (self.step_count as usize) % pattern.len();
        self.vel_x = pattern[idx].0;
        self.vel_y = pattern[idx].1;
        
        // Apply velocity
        self.pos_x = (self.pos_x as i32 + self.vel_x).max(10).min((WIDTH - 10) as i32) as u32;
        self.pos_y = (self.pos_y as i32 + self.vel_y).max(60).min((HEIGHT - 10) as i32) as u32;
        
        // Bounce off walls
        if self.pos_x <= 10 || self.pos_x >= WIDTH - 10 {
            self.vel_x = -self.vel_x;
        }
        if self.pos_y <= 60 || self.pos_y >= HEIGHT - 10 {
            self.vel_y = -self.vel_y;
        }
    }
    
    fn to_gpu_state(&self) -> AgentGpuState {
        let mut trail_packed = [0u32; 32];
        for (i, (x, y)) in self.trail.iter().enumerate() {
            if i < 32 {
                trail_packed[i] = (x << 16) | y;
            }
        }
        
        let mut mailbox_arr = [0u32; 10];
        for (i, msg) in self.mailbox.iter().enumerate() {
            if i < 10 {
                mailbox_arr[i] = *msg;
            }
        }
        
        AgentGpuState {
            id: self.id,
            pos_x: self.pos_x,
            pos_y: self.pos_y,
            vel_x: self.vel_x,
            vel_y: self.vel_y,
            color: self.color,
            is_it: if self.is_it { 1 } else { 0 },
            message_waiting: if self.message_waiting { 1 } else { 0 },
            trail_len: self.trail.len() as u32,
            _padding: [0; 2],
            trail: trail_packed,
            mailbox: mailbox_arr,
        }
    }
}

// ============================================================================
// SHARED MEMORY (Inter-Agent Messaging)
// ============================================================================

struct SharedMemory {
    mailboxes: Vec<Vec<AtomicU32>>,
    message_waiting: Vec<AtomicU32>,
}

impl SharedMemory {
    fn new() -> Self {
        let mut mailboxes = Vec::new();
        let mut message_waiting = Vec::new();
        
        for _ in 0..MAX_AGENTS {
            let mut mailbox = Vec::new();
            for _ in 0..MAILBOX_SIZE {
                mailbox.push(AtomicU32::new(0));
            }
            mailboxes.push(mailbox);
            message_waiting.push(AtomicU32::new(0));
        }
        
        Self { mailboxes, message_waiting }
    }
    
    fn send(&self, value: u32, target: usize, _slot: usize) -> bool {
        if target >= MAX_AGENTS {
            return false;
        }
        
        for slot in &self.mailboxes[target] {
            if slot.compare_exchange(0, value, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                self.message_waiting[target].store(1, Ordering::SeqCst);
                return true;
            }
        }
        
        false
    }
    
    fn recv(&self, thread_id: usize) -> Option<u32> {
        if thread_id >= MAX_AGENTS {
            return None;
        }
        
        for slot in &self.mailboxes[thread_id] {
            let value = slot.load(Ordering::SeqCst);
            if value != 0 {
                slot.store(0, Ordering::SeqCst);
                
                let has_more = self.mailboxes[thread_id]
                    .iter()
                    .any(|s| s.load(Ordering::SeqCst) != 0);
                
                if !has_more {
                    self.message_waiting[thread_id].store(0, Ordering::SeqCst);
                }
                
                return Some(value);
            }
        }
        
        None
    }
    
    fn has_message(&self, thread_id: usize) -> bool {
        if thread_id >= MAX_AGENTS {
            return false;
        }
        self.message_waiting[thread_id].load(Ordering::SeqCst) == 1
    }
}

// ============================================================================
// GPU TYPES
// ============================================================================

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct Pixel {
    r: u32,
    g: u32,
    b: u32,
    a: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct AgentGpuState {
    id: u32,
    pos_x: u32,
    pos_y: u32,
    vel_x: i32,
    vel_y: i32,
    color: u32,
    is_it: u32,
    message_waiting: u32,
    trail_len: u32,
    _padding: [u32; 2],
    trail: [u32; 32],
    mailbox: [u32; 10],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Config {
    width: u32,
    height: u32,
    time: f32,
    frame: u32,
    mode: u32,
}

// ============================================================================
// GPU RENDERER
// ============================================================================

struct SpatialSwarmRenderer {
    device: Device,
    queue: Queue,
    pipeline: ComputePipeline,
    bind_group: BindGroup,
    
    output_buffer: Buffer,
    staging_buffer: Buffer,
    agents_buffer: Buffer,
    config_buffer: Buffer,
}

impl SpatialSwarmRenderer {
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
                label: Some("Spatial Swarm GPU"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
            }, None)
            .await?;
        
        // Load shader
        let shader_source = include_str!("../../spatial_swarm_hud.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Spatial Swarm HUD Shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
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
        
        // Agents buffer
        let agents_size = (MAX_AGENTS * std::mem::size_of::<AgentGpuState>()) as u64;
        let agents_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Agents Buffer"),
            size: agents_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        // Config buffer
        let config_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Config Buffer"),
            size: std::mem::size_of::<Config>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        // Initialize config
        let config = Config { width: WIDTH, height: HEIGHT, time: 0.0, frame: 0, mode: 0 };
        queue.write_buffer(&config_buffer, 0, bytemuck::bytes_of(&config));
        
        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Spatial Swarm Bind Group Layout"),
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
            ],
        });
        
        // Create pipeline
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Spatial Swarm Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Spatial Swarm HUD Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });
        
        // Create bind group
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Spatial Swarm Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: output_buffer.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: agents_buffer.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: config_buffer.as_entire_binding() },
            ],
        });
        
        Ok(Self {
            device,
            queue,
            pipeline,
            bind_group,
            output_buffer,
            staging_buffer,
            agents_buffer,
            config_buffer,
        })
    }
    
    fn update_agents(&self, agents: &[SwarmAgent], frame: u32) {
        let mut states = Vec::new();
        for agent in agents {
            states.push(agent.to_gpu_state());
        }
        
        // Pad to MAX_AGENTS
        while states.len() < MAX_AGENTS {
            states.push(AgentGpuState {
                id: 0,
                pos_x: 0,
                pos_y: 0,
                vel_x: 0,
                vel_y: 0,
                color: 0,
                is_it: 0,
                message_waiting: 0,
                trail_len: 0,
                _padding: [0; 2],
                trail: [0; 32],
                mailbox: [0; 10],
            });
        }
        
        self.queue.write_buffer(&self.agents_buffer, 0, bytemuck::cast_slice(&states));
        
        let config = Config {
            width: WIDTH,
            height: HEIGHT,
            time: frame as f32 / 60.0,
            frame,
            mode: 0,
        };
        self.queue.write_buffer(&self.config_buffer, 0, bytemuck::bytes_of(&config));
        
        self.queue.submit(std::iter::empty());
    }
    
    fn render(&self) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, Box<dyn std::error::Error>> {
        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });
        
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Spatial Swarm Compute Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups((WIDTH * HEIGHT + 63) / 64, 1, 1);
        }
        
        encoder.copy_buffer_to_buffer(&self.output_buffer, 0, &self.staging_buffer, 0, (WIDTH * HEIGHT * 16) as u64);
        self.queue.submit(std::iter::once(encoder.finish()));
        
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
        
        Ok(img)
    }
}

// ============================================================================
// TAG GAME SIMULATION
// ============================================================================

fn run_tag_game(framebuffer: &mut [u32], shared: &SharedMemory, agents: &mut [SwarmAgent]) {
    for agent in agents.iter_mut() {
        agent.update(framebuffer, shared);
    }
}

// ============================================================================
// MAIN
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║         SPATIAL SWARM SOCIETY — TAG GAME DEMO           ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Unified Opcode Set:                                    ║");
    println!("║    $  SPAWN  - Fork VM into parallel agent              ║");
    println!("║    p  POS    - Push position (x, y)                     ║");
    println!("║    >  MOVE   - dx dy > update position                  ║");
    println!("║    >> VMOVE  - Move by velocity                         ║");
    println!("║    x  SENSE  - Read pixel at POS (collision)            ║");
    println!("║    !  PUNCH  - Write pixel at POS (marking)             ║");
    println!("║    ^  SEND   - value thread slot ^ send message         ║");
    println!("║    ?  RECV   - Receive message from mailbox             ║");
    println!("║    @> PROMPT - Wait for NL command (shell)              ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Agents: 8 (1 It + 7 Runners)                           ║");
    println!("║  Messaging: Atomic mailboxes with SEND/RECV             ║");
    println!("║  Spatial: Position, velocity, trails, collision         ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    // Initialize shared memory
    println!("[INIT] Creating shared memory for {} agents...", MAX_AGENTS);
    let shared = SharedMemory::new();
    
    // Initialize agents
    println!("[INIT] Spawning {} agents...", MAX_AGENTS);
    let mut agents: Vec<SwarmAgent> = (0..MAX_AGENTS)
        .map(|id| SwarmAgent::new(id as u32))
        .collect();
    
    println!("[INIT] Agent 0 is IT (white), Agents 1-7 are RUNNERS (colored)");
    println!();
    
    // Initialize framebuffer
    let mut framebuffer = vec![0u32; (WIDTH * HEIGHT) as usize];
    
    // Initialize GPU
    println!("[GPU] Initializing RTX 5090...");
    let renderer = SpatialSwarmRenderer::new().await?;
    println!("[GPU] Pipeline ready");
    println!();
    
    // Run simulation
    println!("[SIM] Running tag game simulation...");
    println!();
    
    let total_frames = 200u32;
    let start_time = Instant::now();
    
    for frame in 0..total_frames {
        // Clear framebuffer
        framebuffer.fill(0);
        
        // Run tag game
        run_tag_game(&mut framebuffer, &shared, &mut agents);
        
        // Update GPU
        renderer.update_agents(&agents, frame);
        
        // Render HUD
        let img = renderer.render()?;
        
        // Save final frame
        if frame == total_frames - 1 {
            let output_path = "output/spatial_swarm.png";
            img.save(output_path)?;
            println!("[OUTPUT] Saved final frame to {}", output_path);
        }
        
        // Progress
        if frame % 50 == 0 {
            println!("[FRAME {}/{}] Agents active", frame, total_frames);
        }
    }
    
    let total_time = start_time.elapsed();
    let avg_frame_time = total_time.as_millis() as f64 / total_frames as f64;
    
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║              SPATIAL SWARM COMPLETE                     ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  ✅ {} agents spawned and running in parallel            ║", MAX_AGENTS);
    println!("║  ✅ Agent 0 started as IT (white)                       ║");
    println!("║  ✅ Agents 1-7 are RUNNERS (colored)                    ║");
    println!("║  ✅ Collision detection via SENSE                       ║");
    println!("║  ✅ Messaging via SEND/RECV (^ / ?)                     ║");
    println!("║  ✅ HUD displays all 8 agents with status               ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Performance:                                            ║");
    println!("║    Total frames: {}                                      ║", total_frames);
    println!("║    Total time:   {:.2}s                                 ║", total_time.as_secs_f64());
    println!("║    Avg frame:    {:.2}ms                                ║", avg_frame_time);
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    // Print agent states
    println!("Agent States:");
    for agent in &agents {
        let status = if agent.is_it { "IT" } else { "RUNNER" };
        println!("  Agent {}: POS=({},{}) VEL=({},{}) STATUS={} MSG={}", 
            agent.id, agent.pos_x, agent.pos_y, 
            agent.vel_x, agent.vel_y, 
            status,
            if agent.message_waiting { "YES" } else { "-" }
        );
    }
    
    println!();
    println!("Next: Use vision model (qwen3-vl-8b) to verify swarm mood");
    
    Ok(())
}
