// Z-Order Stacking - Phase 8 Delta
//
// Window Stacking: Competitive gravity dominance.
//   - z_index: Higher = foreground, stronger attraction
//   - Z-Bias: Foreground wells "muffle" background wells
//   - Click to focus: bring_to_front() swaps z_index
//   - Visual feedback: Brighter = foreground, Dim = background
//
// Physics:
//   - Agents calculate attraction for ALL wells
//   - Apply Z-Bias: higher z_index = stronger pull
//   - Overlapping regions: agents "migrate" to foreground
//
// The "Focus" Test:
//   1. Create two overlapping wells (one in front, one behind)
//   2. Click the back well
//   3. Swarm visibly "migrates" as z_index swaps

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
const HIT_THRESHOLD_PX: f32 = 30.0;

// Z-Order constants
const Z_BIAS_STRENGTH: f32 = 1.5;  // Foreground wells are 1.5x stronger
const Z_MUFFLE_RADIUS: f32 = 150.0;  // Wells within this radius are muffled

// Message codes
const MSG_YOU_ARE_IT: u32 = 1;
const MSG_CLUSTER: u32 = 3;
const MSG_FLOCK: u32 = 4;

// ============================================================================
// Z-ORDER WELL
// ============================================================================

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct Well {
    // Position and strength
    pos_x: f32,
    pos_y: f32,
    strength: f32,
    selected: f32,
    
    // Z-Order (16-byte aligned block)
    z_index: f32,        // Higher = foreground
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

impl Well {
    fn new(x: f32, y: f32, strength: f32, z_index: f32) -> Self {
        Self {
            pos_x: x,
            pos_y: y,
            strength,
            selected: 0.0,
            z_index,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        }
    }
    
    fn zero() -> Self {
        Self {
            pos_x: 0.0,
            pos_y: 0.0,
            strength: 0.0,
            selected: 0.0,
            z_index: 0.0,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
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
        let z_index = idx as f32;  // New wells start at current z-level
        self.wells[idx] = Well::new(x, y, strength, z_index);
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
    
    /// Bring a well to the front (highest z_index)
    fn bring_to_front(&mut self, idx: usize) {
        if idx >= self.well_count as usize {
            return;
        }
        
        // Find current max z_index
        let max_z = self.wells[..self.well_count as usize]
            .iter()
            .map(|w| w.z_index)
            .fold(0.0f32, f32::max);
        
        // Set this well to be on top
        self.wells[idx].z_index = max_z + 1.0;
        
        println!("[FOCUS] Well {} brought to front (z={:.0})", idx, self.wells[idx].z_index);
    }
    
    /// Get the z-bias multiplier for a well (foreground = stronger)
    fn get_z_bias(&self, idx: usize) -> f32 {
        if idx >= self.well_count as usize {
            return 1.0;
        }
        
        let well = &self.wells[idx];
        let max_z = self.wells[..self.well_count as usize]
            .iter()
            .map(|w| w.z_index)
            .fold(0.0f32, f32::max);
        
        // Normalize z_index to 0.0-1.0 range
        if max_z > 0.0 {
            let normalized_z = well.z_index / max_z;
            // Foreground (z=1.0) = Z_BIAS_STRENGTH, Background (z=0.0) = 1.0
            1.0 + normalized_z * (Z_BIAS_STRENGTH - 1.0)
        } else {
            1.0
        }
    }
    
    /// Check if a well is "muffled" by a foreground well at a given position
    fn is_muffled(&self, idx: usize, x: f32, y: f32) -> bool {
        if idx >= self.well_count as usize {
            return false;
        }
        
        let well = &self.wells[idx];
        
        // Check if any foreground well is within muffle radius
        for i in 0..self.well_count as usize {
            if i == idx {
                continue;
            }
            
            let other = &self.wells[i];
            
            // Only foreground wells can muffle
            if other.z_index <= well.z_index {
                continue;
            }
            
            let dx = other.pos_x - x;
            let dy = other.pos_y - y;
            let dist = (dx * dx + dy * dy).sqrt();
            
            if dist < Z_MUFFLE_RADIUS {
                return true;
            }
        }
        
        false
    }
    
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
        
        // Apply gravity well forces with z-bias
        self.apply_z_order_gravity(ui_state);
        
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
    
    fn apply_z_order_gravity(&mut self, ui_state: &UIState) {
        let mut total_force_x = 0.0;
        let mut total_force_y = 0.0;
        
        for i in 0..ui_state.well_count as usize {
            let well = &ui_state.wells[i];
            
            let diff_x = well.pos_x - self.pos_x;
            let diff_y = well.pos_y - self.pos_y;
            
            let dist_sq = diff_x * diff_x + diff_y * diff_y + GRAVITY_EPSILON;
            
            // Apply z-bias multiplier
            let z_bias = ui_state.get_z_bias(i);
            
            // Check if muffled by foreground well
            let muffled = ui_state.is_muffled(i, self.pos_x, self.pos_y);
            let muffle_factor = if muffled { 0.3 } else { 1.0 };  // Muffled wells are 30% strength
            
            // Selected wells get bonus
            let selected_bonus = 1.0 + well.selected;
            
            // Final effective strength
            let effective_strength = well.strength * z_bias * muffle_factor * selected_bonus;
            
            let force_mag = (effective_strength / dist_sq).min(MAX_FORCE);
            
            let dist = dist_sq.sqrt();
            total_force_x += (diff_x / dist) * force_mag;
            total_force_y += (diff_y / dist) * force_mag;
        }
        
        // Integrate into velocity
        self.vel_x += total_force_x * 0.016;
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
    r: u32, g: u32, b: u32, a: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
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

struct ZOrderRenderer {
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

impl ZOrderRenderer {
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
                label: Some("Z-Order GPU"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
            }, None)
            .await?;
        
        let shader_source = include_str!("../../z_order_hud.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Z-Order HUD Shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });
        
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
        
        let agents_size = (MAX_AGENTS * std::mem::size_of::<AgentGpuState>()) as u64;
        let agents_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Agents Buffer"),
            size: agents_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        let config_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Config Buffer"),
            size: std::mem::size_of::<Config>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        let ui_state_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("UI State Buffer"),
            size: std::mem::size_of::<UIState>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        let config = Config { width: WIDTH, height: HEIGHT, time: 0.0, frame: 0, mode: 0 };
        queue.write_buffer(&config_buffer, 0, bytemuck::bytes_of(&config));
        
        let ui_state = UIState::new();
        queue.write_buffer(&ui_state_buffer, 0, bytemuck::bytes_of(&ui_state));
        
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Z-Order Bind Group Layout"),
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
            label: Some("Z-Order Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Z-Order HUD Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });
        
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Z-Order Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: output_buffer.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: agents_buffer.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: config_buffer.as_entire_binding() },
                BindGroupEntry { binding: 3, resource: ui_state_buffer.as_entire_binding() },
            ],
        });
        
        Ok(Self {
            device, queue, pipeline, bind_group,
            output_buffer, staging_buffer, agents_buffer,
            config_buffer, ui_state_buffer,
        })
    }
    
    fn update(&self, agents: &[SwarmAgent], ui_state: &UIState, frame: u32) {
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
        
        let config = Config {
            width: WIDTH, height: HEIGHT,
            time: frame as f32 / 60.0, frame, mode: 0,
        };
        self.queue.write_buffer(&self.config_buffer, 0, bytemuck::bytes_of(&config));
        self.queue.write_buffer(&self.ui_state_buffer, 0, bytemuck::bytes_of(ui_state));
        
        self.queue.submit(std::iter::empty());
    }
    
    fn render_to_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });
        
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Z-Order Compute Pass"),
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
// DEMO
// ============================================================================

fn run_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║          Z-ORDER STACKING — Phase 8 Delta                       ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  Competitive Gravity: Foreground wells dominate                 ║");
    println!("║                                                                  ║");
    println!("║  Mechanics:                                                      ║");
    println!("║    z_index: Higher = foreground, stronger attraction            ║");
    println!("║    Z-Bias: Foreground wells are 1.5x stronger                   ║");
    println!("║    Muffle: Background wells near foreground are weakened        ║");
    println!("║                                                                  ║");
    println!("║  Visual:                                                         ║");
    println!("║    Foreground = Brighter, larger                                ║");
    println!("║    Background = Dimmer, ghostly                                 ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();
    
    let shared = SharedMemory::new();
    let agents: Vec<SwarmAgent> = (0..MAX_AGENTS).map(|id| SwarmAgent::new(id as u32)).collect();
    
    let mut ui_state = UIState::new();
    
    // Create two overlapping wells (simulating overlapping windows)
    let well0 = ui_state.add_well(500.0, 400.0, 1500.0);  // Background (z=0)
    let well1 = ui_state.add_well(600.0, 400.0, 1500.0);  // Foreground (z=1)
    let well2 = ui_state.add_well(780.0, 400.0, 1500.0);  // Side well (z=2)
    
    println!("[WELLS] Created {} wells:", ui_state.well_count);
    for (i, well) in ui_state.wells.iter().take(ui_state.well_count as usize).enumerate() {
        println!("  Well {}: pos=({:.0}, {:.0}) strength={:.0} z={:.0}", 
            i, well.pos_x, well.pos_y, well.strength, well.z_index);
    }
    println!();
    
    println!("[GPU] Initializing...");
    let rt = tokio::runtime::Runtime::new()?;
    let renderer = rt.block_on(ZOrderRenderer::new())?;
    println!("[GPU] Pipeline ready");
    println!();
    
    let total_frames = 300u32;
    let start_time = Instant::now();
    
    let mut agents = agents;
    
    for frame in 0..total_frames {
        // Simulate focus switching at frames 100 and 200
        if frame == 100 {
            println!();
            println!("[FOCUS SWITCH] Frame 100: Clicking well 0 (background)");
            ui_state.bring_to_front(0);
        }
        
        if frame == 200 {
            println!();
            println!("[FOCUS SWITCH] Frame 200: Clicking well 1 (middle)");
            ui_state.bring_to_front(1);
        }
        
        // Update agents
        let agents_clone = agents.to_vec();
        for agent in agents.iter_mut() {
            agent.update(&ui_state, &shared, &agents_clone);
        }
        
        // Update GPU
        renderer.update(&agents, &ui_state, frame);
        
        // Progress
        if frame % 50 == 0 {
            // Count agents near each well
            let mut counts = vec![0; ui_state.well_count as usize];
            for agent in &agents {
                for (i, well) in ui_state.wells.iter().take(ui_state.well_count as usize).enumerate() {
                    let dx = agent.pos_x - well.pos_x;
                    let dy = agent.pos_y - well.pos_y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist < 100.0 {
                        counts[i] += 1;
                    }
                }
            }
            
            println!("[FRAME {}/{}] Agents per well: {:?}", frame, total_frames, counts);
        }
    }
    
    renderer.render_to_file("output/z_order_demo.png")?;
    
    let elapsed = start_time.elapsed();
    println!();
    println!("Demo complete: {} frames in {:.2}s ({:.1} FPS)",
        total_frames, elapsed.as_secs_f64(), total_frames as f64 / elapsed.as_secs_f64());
    println!("Output: output/z_order_demo.png");
    
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_demo()
}
