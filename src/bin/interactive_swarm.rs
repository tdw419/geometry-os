// Interactive Swarm - Phase 8 Beta
//
// Transform gravity wells into user-controlled handles.
// When you click, the CPU checks which well is closest.
// If within threshold, that well becomes "latched" to the mouse.
//
// Architecture:
//   - Hit Detection: CPU-side distance check (30px threshold)
//   - Latching: selected_well: Option<usize> in Rust state
//   - Input Mapping: winit event loop (MouseInput, CursorMoved)
//   - Visual Feedback: Latched well glows brighter in shader
//
// "The Living Window" - grab a well, drag the swarm, watch the comet tail.

use wgpu::*;
use winit::{
    event::{Event, WindowEvent, ElementState, MouseButton},
    event_loop::EventLoop,
    window::WindowBuilder,
};
use image::{ImageBuffer, Rgba};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Instant, Duration};
use std::fs;
use std::cell::RefCell;
use std::rc::Rc;
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

// Hit detection
const HIT_THRESHOLD_PX: f32 = 30.0;  // Click within 30px to latch

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
    selected: f32,  // NEW: 1.0 if latched, 0.0 otherwise (for visual feedback)
}

impl Well {
    fn new(x: f32, y: f32, strength: f32) -> Self {
        Self {
            pos_x: x,
            pos_y: y,
            strength,
            selected: 0.0,
        }
    }
    
    fn zero() -> Self {
        Self {
            pos_x: 0.0,
            pos_y: 0.0,
            strength: 0.0,
            selected: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct UIState {
    well_count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
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
    
    fn set_selected(&mut self, idx: usize, selected: bool) {
        if idx < MAX_WELLS {
            self.wells[idx].selected = if selected { 1.0 } else { 0.0 };
        }
    }
    
    fn clear_all_selected(&mut self) {
        for well in &mut self.wells {
            well.selected = 0.0;
        }
    }
    
    /// Find the nearest well to a point, returning (index, distance)
    fn find_nearest_well(&self, x: f32, y: f32) -> Option<(usize, f32)> {
        let mut nearest: Option<(usize, f32)> = None;
        
        for i in 0..self.well_count as usize {
            let well = &self.wells[i];
            let dx = well.pos_x - x;
            let dy = well.pos_y - y;
            let dist = (dx * dx + dy * dy).sqrt();
            
            match nearest {
                None => nearest = Some((i, dist)),
                Some((_, prev_dist)) if dist < prev_dist => {
                    nearest = Some((i, dist));
                }
                _ => {}
            }
        }
        
        nearest
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
            // Selected wells have 2x strength for visual emphasis
            let effective_strength = well.strength * (1.0 + well.selected);
            let force_mag = (effective_strength / dist_sq).min(MAX_FORCE);
            
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

struct InteractiveRenderer {
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

impl InteractiveRenderer {
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
                label: Some("Interactive Swarm GPU"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
            }, None)
            .await?;
        
        // Load shader (use interactive version)
        let shader_source = include_str!("../../interactive_swarm_hud.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Interactive Swarm HUD Shader"),
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
        
        // Create bind group layout with 4 bindings
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Interactive Swarm Bind Group Layout"),
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
        
        // Create pipeline
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Interactive Swarm Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Interactive Swarm HUD Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });
        
        // Create bind group
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Interactive Swarm Bind Group"),
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
    
    fn render_to_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });
        
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Interactive Swarm Compute Pass"),
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
        
        fs::create_dir_all("output")?;
        img.save(path)?;
        
        Ok(())
    }
}

// ============================================================================
// INTERACTIVE APPLICATION
// ============================================================================

struct InteractiveApp {
    agents: Vec<SwarmAgent>,
    shared: SharedMemory,
    ui_state: UIState,
    selected_well: Option<usize>,
    cursor_pos: (f32, f32),
    frame: u32,
}

impl InteractiveApp {
    fn new() -> Self {
        let shared = SharedMemory::new();
        let agents: Vec<SwarmAgent> = (0..MAX_AGENTS).map(|id| SwarmAgent::new(id as u32)).collect();
        
        let mut ui_state = UIState::new();
        
        // Add initial wells
        ui_state.add_well(300.0, 400.0, 1000.0);  // Left
        ui_state.add_well(640.0, 300.0, 500.0);   // Center
        ui_state.add_well(980.0, 400.0, 1000.0);  // Right
        
        Self {
            agents,
            shared,
            ui_state,
            selected_well: None,
            cursor_pos: (0.0, 0.0),
            frame: 0,
        }
    }
    
    fn handle_mouse_press(&mut self, x: f32, y: f32) {
        // Find nearest well within hit threshold
        if let Some((idx, dist)) = self.ui_state.find_nearest_well(x, y) {
            if dist < HIT_THRESHOLD_PX {
                // Clear any previous selection
                self.ui_state.clear_all_selected();
                
                // Latch this well
                self.ui_state.set_selected(idx, true);
                self.selected_well = Some(idx);
                
                println!("[LATCH] Well {} selected at ({:.0}, {:.0})", idx, x, y);
            }
        }
    }
    
    fn handle_mouse_release(&mut self) {
        if let Some(idx) = self.selected_well {
            println!("[RELEASE] Well {} released at ({:.0}, {:.0})", 
                idx, self.ui_state.wells[idx].pos_x, self.ui_state.wells[idx].pos_y);
        }
        
        self.ui_state.clear_all_selected();
        self.selected_well = None;
    }
    
    fn handle_cursor_move(&mut self, x: f32, y: f32) {
        self.cursor_pos = (x, y);
        
        // If a well is latched, move it
        if let Some(idx) = self.selected_well {
            self.ui_state.update_well(idx, x, y);
        }
    }
    
    fn update(&mut self) {
        // Update agents
        let agents_clone = self.agents.to_vec();
        for agent in self.agents.iter_mut() {
            agent.update(&self.ui_state, &self.shared, &agents_clone);
        }
        
        self.frame += 1;
    }
}

// ============================================================================
// MAIN: INTERACTIVE WINDOW
// ============================================================================

fn run_interactive() -> Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║        INTERACTIVE SWARM — Phase 8 Beta                         ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  The Living Window: Drag wells, watch the swarm follow          ║");
    println!("║                                                                  ║");
    println!("║  Controls:                                                       ║");
    println!("║    Click + Drag: Move gravity wells                              ║");
    println!("║    ESC: Exit                                                     ║");
    println!("║                                                                  ║");
    println!("║  Hit Threshold: 30px                                             ║");
    println!("║  Latched wells glow brighter (2x strength)                       ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();
    
    // Initialize app
    let app = Rc::new(RefCell::new(InteractiveApp::new()));
    
    {
        let app_ref = app.borrow();
        println!("[INIT] Created {} gravity wells:", app_ref.ui_state.well_count);
        for (i, well) in app_ref.ui_state.wells.iter().take(app_ref.ui_state.well_count as usize).enumerate() {
            println!("  Well {}: pos=({:.0}, {:.0}) strength={:.0}", 
                i, well.pos_x, well.pos_y, well.strength);
        }
    }
    println!();
    
    // Initialize GPU
    println!("[GPU] Initializing...");
    let rt = tokio::runtime::Runtime::new()?;
    let renderer = Rc::new(RefCell::new(rt.block_on(InteractiveRenderer::new())?));
    println!("[GPU] Pipeline ready");
    println!();
    
    // Initialize winit
    let event_loop = EventLoop::new()?;
    let window = Rc::new(WindowBuilder::new()
        .with_title("Interactive Swarm — Phase 8 Beta")
        .with_inner_size(winit::dpi::LogicalSize::new(WIDTH, HEIGHT))
        .build(&event_loop)?);
    
    println!("[WINDOW] Created {}x{} window", WIDTH, HEIGHT);
    println!();
    println!("[READY] Click on a well and drag to move it!");
    println!();
    
    let start_time = Instant::now();
    let last_frame = Rc::new(RefCell::new(Instant::now()));
    let target_frame_time = Duration::from_micros(16_667); // ~60 FPS
    
    // Clone for the closure
    let app_clone = app.clone();
    let renderer_clone = renderer.clone();
    let window_clone = window.clone();
    let last_frame_clone = last_frame.clone();
    
    event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    elwt.exit();
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if event.state == ElementState::Pressed {
                        use winit::keyboard::{KeyCode, PhysicalKey};
                        if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                            elwt.exit();
                        }
                    }
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    if button == MouseButton::Left {
                        let mut app = app_clone.borrow_mut();
                        match state {
                            ElementState::Pressed => {
                                let (x, y) = app.cursor_pos;
                                app.handle_mouse_press(x, y);
                            }
                            ElementState::Released => {
                                app.handle_mouse_release();
                            }
                        }
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let mut app = app_clone.borrow_mut();
                    let x = position.x as f32;
                    let y = position.y as f32;
                    app.handle_cursor_move(x, y);
                }
                _ => {}
            }
            Event::AboutToWait => {
                // Frame rate limiting
                let now = Instant::now();
                let elapsed = now - *last_frame_clone.borrow();
                
                if elapsed < target_frame_time {
                    let remaining = target_frame_time - elapsed;
                    std::thread::sleep(remaining / 2);
                }
                
                *last_frame_clone.borrow_mut() = now;
                
                // Update simulation
                app_clone.borrow_mut().update();
                
                // Update GPU
                {
                    let app = app_clone.borrow();
                    let renderer = renderer_clone.borrow();
                    renderer.update(&app.agents, &app.ui_state, app.frame);
                }
                
                // Request redraw
                window_clone.request_redraw();
            }
            Event::WindowEvent { event: WindowEvent::RedrawRequested, .. } => {
                // For compute-only rendering, nothing to do here
                // In a full implementation, we'd present a texture
            }
            _ => {}
        }
    })?;
    
    // Final stats
    let elapsed = start_time.elapsed();
    let app = app.borrow();
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║            INTERACTIVE SWARM SESSION ENDED                      ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  Frames: {}                                                      ║", app.frame);
    println!("║  Time:   {:.2}s                                                 ║", elapsed.as_secs_f64());
    println!("║  FPS:    {:.1}                                                  ║", app.frame as f64 / elapsed.as_secs_f64());
    println!("╚══════════════════════════════════════════════════════════════════╝");
    
    // Save final frame to file
    renderer.borrow().render_to_file("output/interactive_swarm_final.png")?;
    println!("[OUTPUT] Final frame saved to output/interactive_swarm_final.png");
    
    Ok(())
}

// ============================================================================
// DEMO MODE (Non-interactive, for testing)
// ============================================================================

fn run_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("[DEMO] Running non-interactive demo...");
    
    let mut app = InteractiveApp::new();
    
    println!("[GPU] Initializing...");
    let rt = tokio::runtime::Runtime::new()?;
    let renderer = rt.block_on(InteractiveRenderer::new())?;
    println!("[GPU] Pipeline ready");
    
    let total_frames = 300u32;
    let start_time = Instant::now();
    
    for frame in 0..total_frames {
        // Simulate dragging well 0 in a circle
        let t = frame as f32 * 0.02;
        app.ui_state.update_well(0, 
            300.0 + t.sin() * 150.0,
            400.0 + t.cos() * 100.0
        );
        
        // Simulate dragging well 2 in opposite direction
        app.ui_state.update_well(2,
            980.0 + (t + std::f32::consts::PI).sin() * 150.0,
            400.0 + (t + std::f32::consts::PI).cos() * 100.0
        );
        
        app.update();
        renderer.update(&app.agents, &app.ui_state, frame);
        
        if frame % 50 == 0 {
            println!("[FRAME {}/{}] Well 0: ({:.0}, {:.0})", 
                frame, total_frames, 
                app.ui_state.wells[0].pos_x, 
                app.ui_state.wells[0].pos_y);
        }
    }
    
    renderer.render_to_file("output/interactive_swarm_demo.png")?;
    
    let elapsed = start_time.elapsed();
    println!();
    println!("Demo complete: {} frames in {:.2}s ({:.1} FPS)",
        total_frames, elapsed.as_secs_f64(), total_frames as f64 / elapsed.as_secs_f64());
    println!("Output: output/interactive_swarm_demo.png");
    
    Ok(())
}

// ============================================================================
// MAIN
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() > 1 && args[1] == "--demo" {
        run_demo()
    } else {
        run_interactive()
    }
}
