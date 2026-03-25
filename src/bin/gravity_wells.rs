// Gravity Wells - Phase 8 Alpha
// 
// Transforms the 64-agent swarm into a functional UI substrate.
// Wells act as attraction points - dragging a window moves the well,
// and agents cluster around it.
//
// Architecture:
//   - Well Storage: Uniform Buffer (16 wells max, 260 bytes total)
//   - Falloff: Inverse Square (F = strength / dist^2)
//   - Resolution: Vector Summation (agents exist between windows)
//
// Physics Constants:
//   GRAVITY_EPSILON = 0.01  (avoid div/0)
//   FRICTION = 0.95         (prevent orbital slingshots)
//   MAX_FORCE = 50.0        (cap inverse square spike)

use wgpu::*;
use image::{ImageBuffer, Rgba};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;
use std::fs;
use serde::{Serialize, Deserialize};

// ============================================================================
// CONSTANTS
// ============================================================================

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;
const MAX_AGENTS: usize = 64;
const MAX_WELLS: usize = 16;
const MAILBOX_SIZE: usize = 10;
const TRAIL_LENGTH: usize = 50;

// Physics constants
const GRAVITY_EPSILON: f32 = 0.01;
const FRICTION: f32 = 0.95;
const MAX_FORCE: f32 = 50.0;

// Message codes
const MSG_YOU_ARE_IT: u32 = 1;
const MSG_CLUSTER: u32 = 3;
const MSG_FLOCK: u32 = 4;

// ============================================================================
// GRAVITY WELL
// ============================================================================

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct Well {
    pos_x: f32,
    pos_y: f32,
    strength: f32,
    _padding: f32,
}

impl Well {
    fn new(x: f32, y: f32, strength: f32) -> Self {
        Self {
            pos_x: x,
            pos_y: y,
            strength,
            _padding: 0.0,
        }
    }
    
    fn zero() -> Self {
        Self {
            pos_x: 0.0,
            pos_y: 0.0,
            strength: 0.0,
            _padding: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct UIState {
    well_count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,  // 4 u32s = 16 bytes, properly aligned
    wells: [Well; MAX_WELLS],
}

impl UIState {
    fn new() -> Self {
        Self {
            well_count: 0,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
            wells: [Well::zero(); MAX_WELLS],
        }
    }
    
    fn add_well(&mut self, x: f32, y: f32, strength: f32) -> usize {
        if self.well_count as usize >= MAX_WELLS {
            return usize::MAX;
        }
        
        let idx = self.well_count as usize;
        self.wells[idx] = Well::new(x, y, strength);
        self.well_count += 1;
        idx
    }
    
    fn update_well(&mut self, idx: usize, x: f32, y: f32) {
        if idx < MAX_WELLS {
            self.wells[idx].pos_x = x;
            self.wells[idx].pos_y = y;
        }
    }
    
    fn remove_well(&mut self, idx: usize) {
        if idx < MAX_WELLS && idx < self.well_count as usize {
            // Shift remaining wells down
            for i in idx..(MAX_WELLS - 1) {
                self.wells[i] = self.wells[i + 1];
            }
            self.wells[MAX_WELLS - 1] = Well::zero();
            self.well_count = self.well_count.saturating_sub(1);
        }
    }
}

// ============================================================================
// AGENT STATE
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SwarmAgent {
    id: u32,
    pos_x: f32,
    pos_y: f32,
    vel_x: f32,
    vel_y: f32,
    color: u32,
    tribe: u32,
    is_it: bool,
    mailbox: Vec<u32>,
    message_waiting: bool,
    trail: Vec<(f32, f32)>,
    halted: bool,
    step_count: u32,
    collision_count: u32,
    message_count: u32,
}

impl SwarmAgent {
    fn new(id: u32) -> Self {
        let col = id % 8;
        let row = id / 8;
        
        let spacing_x = (WIDTH - 100) as f32 / 8.0;
        let spacing_y = (HEIGHT - 200) as f32 / 8.0;
        
        let base_x = 50.0 + col as f32 * spacing_x;
        let base_y = 100.0 + row as f32 * spacing_y;
        
        let offset_x = ((id * 17) % 30) as f32;
        let offset_y = ((id * 23) % 30) as f32;
        
        let tribe_colors = [
            0xFF4040FF, 0x40FF40FF, 0x4040FFFF, 0xFFFF40FF,
            0xFF40FFFF, 0x40FFFFFF, 0xFF8040FF, 0x8040FFFF,
        ];
        
        let tribe = id % 8;
        
        let vel_patterns = [
            (2.0, 1.0), (1.0, 2.0), (-1.0, 2.0), (-2.0, 1.0),
            (-2.0, -1.0), (-1.0, -2.0), (1.0, -2.0), (2.0, -1.0),
        ];
        let (vx, vy) = vel_patterns[tribe as usize];
        
        Self {
            id,
            pos_x: base_x + offset_x,
            pos_y: base_y + offset_y,
            vel_x: vx,
            vel_y: vy,
            color: tribe_colors[tribe as usize],
            tribe,
            is_it: id == 0,
            mailbox: vec![0; MAILBOX_SIZE],
            message_waiting: false,
            trail: Vec::new(),
            halted: false,
            step_count: 0,
            collision_count: 0,
            message_count: 0,
        }
    }
    
    fn update(&mut self, ui_state: &UIState, shared: &SharedMemory, all_agents: &[SwarmAgent]) {
        if self.halted {
            return;
        }
        
        self.step_count += 1;
        
        // Check mailbox
        if let Some(msg) = shared.recv(self.id as usize) {
            self.message_count += 1;
            match msg {
                MSG_YOU_ARE_IT => {
                    self.is_it = true;
                    self.color = 0xFFFFFFFF;
                }
                MSG_CLUSTER => self.move_to_tribe_center(all_agents),
                MSG_FLOCK => self.flock_behavior(all_agents),
                _ => {}
            }
            self.mailbox[0] = msg;
            self.message_waiting = shared.has_message(self.id as usize);
        }
        
        // Apply gravity well forces
        self.apply_gravity_wells(ui_state);
        
        // Tribe-based collective behavior
        if !self.is_it {
            self.collective_behavior(all_agents);
        }
        
        // Apply velocity with friction
        self.vel_x *= FRICTION;
        self.vel_y *= FRICTION;
        
        // Update position
        self.pos_x += self.vel_x;
        self.pos_y += self.vel_y;
        
        // Clamp position
        self.pos_x = self.pos_x.clamp(10.0, (WIDTH - 10) as f32);
        self.pos_y = self.pos_y.clamp(100.0, (HEIGHT - 10) as f32);
        
        // Bounce off walls
        if self.pos_x <= 10.0 || self.pos_x >= (WIDTH - 10) as f32 {
            self.vel_x = -self.vel_x;
            self.collision_count += 1;
        }
        if self.pos_y <= 100.0 || self.pos_y >= (HEIGHT - 10) as f32 {
            self.vel_y = -self.vel_y;
            self.collision_count += 1;
        }
        
        // Update trail
        self.trail.push((self.pos_x, self.pos_y));
        if self.trail.len() > TRAIL_LENGTH {
            self.trail.remove(0);
        }
    }
    
    fn apply_gravity_wells(&mut self, ui_state: &UIState) {
        let mut total_force_x = 0.0;
        let mut total_force_y = 0.0;
        
        for i in 0..ui_state.well_count as usize {
            let well = &ui_state.wells[i];
            
            let diff_x = well.pos_x - self.pos_x;
            let diff_y = well.pos_y - self.pos_y;
            
            let dist_sq = diff_x * diff_x + diff_y * diff_y + GRAVITY_EPSILON;
            
            // Inverse square attraction with force cap
            let force_mag = (well.strength / dist_sq).min(MAX_FORCE);
            
            let dist = dist_sq.sqrt();
            total_force_x += (diff_x / dist) * force_mag;
            total_force_y += (diff_y / dist) * force_mag;
        }
        
        // Integrate into velocity
        self.vel_x += total_force_x * 0.016;  // ~60fps delta
        self.vel_y += total_force_y * 0.016;
    }
    
    fn collective_behavior(&mut self, all_agents: &[SwarmAgent]) {
        let phase = (self.step_count / 200) % 3;
        
        match phase {
            0 => self.flock_behavior(all_agents),
            1 => self.move_to_tribe_center(all_agents),
            2 => self.swarm_behavior(all_agents),
            _ => {}
        }
    }
    
    fn flock_behavior(&mut self, all_agents: &[SwarmAgent]) {
        let mut avg_vel_x = 0.0;
        let mut avg_vel_y = 0.0;
        let mut count = 0;
        
        for other in all_agents {
            if other.id != self.id && other.tribe == self.tribe {
                let dx = (other.pos_x - self.pos_x).abs();
                let dy = (other.pos_y - self.pos_y).abs();
                
                if dx < 100.0 && dy < 100.0 {
                    avg_vel_x += other.vel_x;
                    avg_vel_y += other.vel_y;
                    count += 1;
                }
            }
        }
        
        if count > 0 {
            avg_vel_x /= count as f32;
            avg_vel_y /= count as f32;
            
            self.vel_x = (self.vel_x + avg_vel_x) / 2.0;
            self.vel_y = (self.vel_y + avg_vel_y) / 2.0;
        }
        
        if self.vel_x.abs() < 0.5 { self.vel_x = 0.5 * self.vel_x.signum().max(1.0); }
        if self.vel_y.abs() < 0.5 { self.vel_y = 0.5 * self.vel_y.signum().max(1.0); }
    }
    
    fn move_to_tribe_center(&mut self, all_agents: &[SwarmAgent]) {
        let mut center_x = 0.0;
        let mut center_y = 0.0;
        let mut count = 0;
        
        for other in all_agents {
            if other.tribe == self.tribe {
                center_x += other.pos_x;
                center_y += other.pos_y;
                count += 1;
            }
        }
        
        if count > 0 {
            center_x /= count as f32;
            center_y /= count as f32;
            
            let dx = center_x - self.pos_x;
            let dy = center_y - self.pos_y;
            
            self.vel_x += dx.signum() * 0.5;
            self.vel_y += dy.signum() * 0.5;
        }
    }
    
    fn swarm_behavior(&mut self, _all_agents: &[SwarmAgent]) {
        let global_center_x = WIDTH as f32 / 2.0;
        let global_center_y = (HEIGHT + 100) as f32 / 2.0;
        
        let dx = self.pos_x - global_center_x;
        let dy = self.pos_y - global_center_y;
        
        self.vel_x += -dy.signum() * 0.5;
        self.vel_y += dx.signum() * 0.5;
        
        self.vel_x += (self.tribe as f32 - 4.0) / 2.0;
    }
    
    fn to_gpu_state(&self) -> AgentGpuState {
        let mut trail_packed = [0u32; 32];
        for (i, (x, y)) in self.trail.iter().enumerate() {
            if i < 32 {
                trail_packed[i] = ((*x as u32) << 16) | (*y as u32);
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
            tribe: self.tribe,
            is_it: if self.is_it { 1 } else { 0 },
            message_waiting: if self.message_waiting { 1 } else { 0 },
            trail_len: self.trail.len() as u32,
            collision_count: self.collision_count,
            message_count: self.message_count,
            _padding: [0; 1],
            trail: trail_packed,
            mailbox: mailbox_arr,
        }
    }
}

// ============================================================================
// SHARED MEMORY
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
    pos_x: f32,
    pos_y: f32,
    vel_x: f32,
    vel_y: f32,
    color: u32,
    tribe: u32,
    is_it: u32,
    message_waiting: u32,
    trail_len: u32,
    collision_count: u32,
    message_count: u32,
    _padding: [u32; 1],
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

struct GravityWellsRenderer {
    device: Device,
    queue: Queue,
    pipeline: ComputePipeline,
    bind_group: BindGroup,
    
    output_buffer: Buffer,
    staging_buffer: Buffer,
    agents_buffer: Buffer,
    config_buffer: Buffer,
    ui_state_buffer: Buffer,
}

impl GravityWellsRenderer {
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
                label: Some("Gravity Wells GPU"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
            }, None)
            .await?;
        
        // Load shader (will create separately)
        let shader_source = include_str!("../../gravity_wells_hud.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Gravity Wells HUD Shader"),
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
        
        // UI State buffer (gravity wells)
        let ui_state_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("UI State Buffer"),
            size: std::mem::size_of::<UIState>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        // Initialize buffers
        let config = Config { width: WIDTH, height: HEIGHT, time: 0.0, frame: 0, mode: 0 };
        queue.write_buffer(&config_buffer, 0, bytemuck::bytes_of(&config));
        
        let ui_state = UIState::new();
        queue.write_buffer(&ui_state_buffer, 0, bytemuck::bytes_of(&ui_state));
        
        // Create bind group layout with 4 bindings (added UIState)
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Gravity Wells Bind Group Layout"),
            entries: &[
                // Binding 0: Output buffer
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
                // Binding 1: Agents buffer
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
                // Binding 2: Config buffer
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
                // Binding 3: UI State buffer (gravity wells)
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
            label: Some("Gravity Wells Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Gravity Wells HUD Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });
        
        // Create bind group
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Gravity Wells Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: output_buffer.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: agents_buffer.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: config_buffer.as_entire_binding() },
                BindGroupEntry { binding: 3, resource: ui_state_buffer.as_entire_binding() },
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
            ui_state_buffer,
        })
    }
    
    fn update(&self, agents: &[SwarmAgent], ui_state: &UIState, frame: u32) {
        // Update agents
        let mut states = Vec::new();
        for agent in agents {
            states.push(agent.to_gpu_state());
        }
        
        while states.len() < MAX_AGENTS {
            states.push(AgentGpuState {
                id: 0, pos_x: 0.0, pos_y: 0.0, vel_x: 0.0, vel_y: 0.0,
                color: 0, tribe: 0, is_it: 0, message_waiting: 0,
                trail_len: 0, collision_count: 0, message_count: 0,
                _padding: [0; 1], trail: [0; 32], mailbox: [0; 10],
            });
        }
        
        self.queue.write_buffer(&self.agents_buffer, 0, bytemuck::cast_slice(&states));
        
        // Update config
        let config = Config {
            width: WIDTH,
            height: HEIGHT,
            time: frame as f32 / 60.0,
            frame,
            mode: 0,
        };
        self.queue.write_buffer(&self.config_buffer, 0, bytemuck::bytes_of(&config));
        
        // Update UI state (gravity wells)
        self.queue.write_buffer(&self.ui_state_buffer, 0, bytemuck::bytes_of(ui_state));
        
        self.queue.submit(std::iter::empty());
    }
    
    fn render(&self) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, Box<dyn std::error::Error>> {
        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });
        
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Gravity Wells Compute Pass"),
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
// DEMO: MOVING GRAVITY WELLS
// ============================================================================

fn run_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║         GRAVITY WELLS — Phase 8 Alpha                           ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  Transforming swarm into functional UI substrate                 ║");
    println!("║                                                                  ║");
    println!("║  Architecture:                                                   ║");
    println!("║    Well Storage: Uniform Buffer (16 max, 260 bytes)              ║");
    println!("║    Falloff: Inverse Square (F = strength / dist^2)               ║");
    println!("║    Resolution: Vector Summation                                  ║");
    println!("║                                                                  ║");
    println!("║  Physics Constants:                                              ║");
    println!("║    GRAVITY_EPSILON = 0.01  (avoid div/0)                         ║");
    println!("║    FRICTION = 0.95         (prevent slingshots)                  ║");
    println!("║    MAX_FORCE = 50.0        (cap inverse square spike)            ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();
    
    // Initialize
    let shared = SharedMemory::new();
    let mut agents: Vec<SwarmAgent> = (0..MAX_AGENTS).map(|id| SwarmAgent::new(id as u32)).collect();
    
    // Initialize UI state with demo wells
    let mut ui_state = UIState::new();
    
    // Add 3 wells at different positions with varying strengths
    let well1 = ui_state.add_well(300.0, 400.0, 1000.0);  // Strong well (left)
    let well2 = ui_state.add_well(640.0, 300.0, 500.0);   // Medium well (center)
    let well3 = ui_state.add_well(980.0, 400.0, 1000.0);  // Strong well (right)
    
    println!("[INIT] Created {} gravity wells:", ui_state.well_count);
    for (i, well) in ui_state.wells.iter().take(ui_state.well_count as usize).enumerate() {
        println!("  Well {}: pos=({:.0}, {:.0}) strength={:.0}", 
            i, well.pos_x, well.pos_y, well.strength);
    }
    println!();
    
    // Initialize GPU
    println!("[GPU] Initializing...");
    let rt = tokio::runtime::Runtime::new()?;
    let renderer = rt.block_on(GravityWellsRenderer::new())?;
    println!("[GPU] Pipeline ready");
    println!();
    
    // Run simulation
    println!("[SIM] Running gravity well demo...");
    println!("[SIM] Wells will orbit to demonstrate agent following");
    println!();
    
    let total_frames = 300u32;
    let start_time = Instant::now();
    
    for frame in 0..total_frames {
        // Animate wells (orbit around center)
        let t = frame as f32 * 0.02;
        
        // Well 1: orbit left
        ui_state.update_well(well1, 
            300.0 + (t * 2.0).sin() * 100.0,
            400.0 + (t * 2.0).cos() * 50.0
        );
        
        // Well 2: stationary center
        
        // Well 3: orbit right
        ui_state.update_well(well3,
            980.0 + (t * 2.0 + std::f32::consts::PI).sin() * 100.0,
            400.0 + (t * 2.0 + std::f32::consts::PI).cos() * 50.0
        );
        
        // Update agents
        let agents_clone = agents.to_vec();
        for agent in agents.iter_mut() {
            agent.update(&ui_state, &shared, &agents_clone);
        }
        
        // Update GPU
        renderer.update(&agents, &ui_state, frame);
        
        // Render
        let img = renderer.render()?;
        
        // Save final frame
        if frame == total_frames - 1 {
            fs::create_dir_all("output")?;
            img.save("output/gravity_wells_demo.png")?;
            println!("[OUTPUT] Saved to output/gravity_wells_demo.png");
        }
        
        // Progress
        if frame % 50 == 0 {
            // Calculate average distance to nearest well
            let mut total_dist = 0.0;
            for agent in &agents {
                let mut min_dist = f32::MAX;
                for i in 0..ui_state.well_count as usize {
                    let well = &ui_state.wells[i];
                    let dx = agent.pos_x - well.pos_x;
                    let dy = agent.pos_y - well.pos_y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    min_dist = min_dist.min(dist);
                }
                total_dist += min_dist;
            }
            let avg_dist = total_dist / agents.len() as f32;
            
            println!("[FRAME {}/{}] Avg dist to nearest well: {:.1}px", 
                frame, total_frames, avg_dist);
        }
    }
    
    let total_time = start_time.elapsed();
    
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║              GRAVITY WELLS DEMO COMPLETE                        ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  ✅ {} gravity wells created                                     ║", ui_state.well_count);
    println!("║  ✅ 64 agents attracted to moving wells                          ║");
    println!("║  ✅ Inverse square falloff with force capping                    ║");
    println!("║  ✅ Vector summation for liminal agent positions                 ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  Performance:                                                    ║");
    println!("║    Total frames: {}                                              ║", total_frames);
    println!("║    Total time:   {:.2}s                                         ║", total_time.as_secs_f64());
    println!("║    Avg frame:    {:.2}ms                                        ║", total_time.as_millis() as f64 / total_frames as f64);
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();
    println!("Next: Add winit event loop for interactive drag-and-drop");
    
    Ok(())
}

// ============================================================================
// MAIN
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_demo()
}
