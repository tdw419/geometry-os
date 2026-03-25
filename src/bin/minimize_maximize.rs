// Minimize/Maximize - Phase 8 Zeta
//
// Collapse wells to taskbar with interpolated physics.
//   - Minimize: Wells lerp to taskbar, agents get "sucked" into tight bead
//   - Maximize: Wells explode back, strength surges briefly
//   - Easing: CubicInOut for smooth transitions
//   - Duration: ~300ms for collapse, ~400ms for expand (with pop)
//
// Visual Effects:
//   - Collapse: Color shifts to tribe color (data compressed)
//   - Expand: Strength surge 1.5x, agents "pop" outward
//   - Friction: Increased during transition to prevent overshooting

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

// Animation constants
const COLLAPSE_DURATION_MS: f32 = 300.0;
const EXPAND_DURATION_MS: f32 = 400.0;
const TASKBAR_Y: f32 = 750.0;  // Bottom of screen
const EXPAND_STRENGTH_SURGE: f32 = 1.5;  // 50% boost during expansion
const TRANSITION_FRICTION: f32 = 0.85;  // Higher friction during transitions

// Message codes
const MSG_YOU_ARE_IT: u32 = 1;
const MSG_CLUSTER: u32 = 3;
const MSG_FLOCK: u32 = 4;

// ============================================================================
// MINIMIZABLE WELL
// ============================================================================

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct Well {
    // Current state
    pos_x: f32,
    pos_y: f32,
    strength: f32,
    selected: f32,
    
    // Animation state
    is_minimized: f32,      // 0.0 = expanded, 1.0 = minimized
    transition_progress: f32, // 0.0 to 1.0 during animation
    _pad0: f32,
    _pad1: f32,
}

impl Well {
    fn new(x: f32, y: f32, strength: f32) -> Self {
        Self {
            pos_x: x,
            pos_y: y,
            strength,
            selected: 0.0,
            is_minimized: 0.0,
            transition_progress: 0.0,
            _pad0: 0.0,
            _pad1: 0.0,
        }
    }
    
    fn zero() -> Self {
        Self {
            pos_x: 0.0,
            pos_y: 0.0,
            strength: 0.0,
            selected: 0.0,
            is_minimized: 0.0,
            transition_progress: 0.0,
            _pad0: 0.0,
            _pad1: 0.0,
        }
    }
    
    fn get_radius(&self) -> f32 {
        (self.strength * 0.4).sqrt()
    }
}

// Saved state for minimize/restore
#[derive(Clone, Debug)]
struct WellSavedState {
    original_x: f32,
    original_y: f32,
    original_strength: f32,
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
// WELL ANIMATOR
// ============================================================================

struct WellAnimator {
    saved_states: Vec<Option<WellSavedState>>,
    animation_starts: Vec<Option<Instant>>,
    target_states: Vec<f32>, // 0.0 = expanded, 1.0 = minimized
}

impl WellAnimator {
    fn new() -> Self {
        Self {
            saved_states: vec![None; MAX_WELLS],
            animation_starts: vec![None; MAX_WELLS],
            target_states: vec![0.0; MAX_WELLS],
        }
    }
    
    fn start_collapse(&mut self, idx: usize, ui_state: &mut UIState) {
        if idx >= ui_state.well_count as usize {
            return;
        }
        
        let well = &ui_state.wells[idx];
        
        // Save current state
        self.saved_states[idx] = Some(WellSavedState {
            original_x: well.pos_x,
            original_y: well.pos_y,
            original_strength: well.strength,
        });
        
        // Start animation
        self.animation_starts[idx] = Some(Instant::now());
        self.target_states[idx] = 1.0;
        
        println!("[MINIMIZE] Well {} collapsing to taskbar", idx);
    }
    
    fn start_expand(&mut self, idx: usize, _ui_state: &mut UIState) {
        if idx >= MAX_WELLS {
            return;
        }
        
        // Start animation
        self.animation_starts[idx] = Some(Instant::now());
        self.target_states[idx] = 0.0;
        
        println!("[MAXIMIZE] Well {} expanding from taskbar", idx);
    }
    
    fn toggle(&mut self, idx: usize, ui_state: &mut UIState) {
        if idx >= ui_state.well_count as usize {
            return;
        }
        
        let well = &ui_state.wells[idx];
        
        if well.is_minimized < 0.5 {
            self.start_collapse(idx, ui_state);
        } else {
            self.start_expand(idx, ui_state);
        }
    }
    
    fn update(&mut self, ui_state: &mut UIState, dt_ms: f32) {
        for i in 0..ui_state.well_count as usize {
            let well = &mut ui_state.wells[i];
            
            let animation_start = match self.animation_starts[i] {
                Some(t) => t,
                None => continue,
            };
            
            let target = self.target_states[i];
            let current = well.is_minimized;
            
            // Check if animation is complete
            if (target - current).abs() < 0.01 {
                well.is_minimized = target;
                well.transition_progress = 0.0;
                self.animation_starts[i] = None;
                continue;
            }
            
            // Calculate progress
            let elapsed = animation_start.elapsed().as_millis() as f32;
            let duration = if target > current {
                COLLAPSE_DURATION_MS
            } else {
                EXPAND_DURATION_MS
            };
            
            let progress = (elapsed / duration).min(1.0);
            well.transition_progress = progress;
            
            // Apply easing (CubicInOut)
            let eased_progress = cubic_in_out(progress);
            
            // Interpolate is_minimized
            well.is_minimized = current + (target - current) * eased_progress;
            
            // Get saved state
            let saved = match &self.saved_states[i] {
                Some(s) => s.clone(),
                None => continue,
            };
            
            // Calculate interpolated position
            let target_x = if target > 0.5 {
                // Collapsing to taskbar
                saved.original_x
            } else {
                // Expanding from taskbar
                saved.original_x
            };
            
            let target_y = if target > 0.5 {
                // Collapsing to taskbar
                TASKBAR_Y
            } else {
                // Expanding from original position
                saved.original_y
            };
            
            let start_x = if current > 0.5 {
                // Was minimized
                saved.original_x
            } else {
                // Was expanded
                saved.original_x
            };
            
            let start_y = if current > 0.5 {
                // Was minimized at taskbar
                TASKBAR_Y
            } else {
                // Was expanded
                saved.original_y
            };
            
            well.pos_x = start_x + (target_x - start_x) * eased_progress;
            well.pos_y = start_y + (target_y - start_y) * eased_progress;
            
            // Apply strength surge during expansion
            if target < 0.5 && progress < 0.5 {
                // Early expansion: surge
                let surge_factor = 1.0 + (EXPAND_STRENGTH_SURGE - 1.0) * (1.0 - progress * 2.0);
                well.strength = saved.original_strength * surge_factor;
            } else {
                well.strength = saved.original_strength;
            }
        }
    }
}

// Cubic in-out easing function
fn cubic_in_out(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
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
        self.apply_animated_gravity(ui_state);
        
        // Tribe-based collective behavior
        if !self.is_it {
            self.collective_behavior(all_agents);
        }
        
        // Determine friction based on transition state
        let mut active_friction = FRICTION;
        for i in 0..ui_state.well_count as usize {
            let well = &ui_state.wells[i];
            if well.transition_progress > 0.01 && well.transition_progress < 0.99 {
                active_friction = active_friction.min(TRANSITION_FRICTION);
            }
        }
        
        // Apply velocity with friction
        self.vel_x *= active_friction;
        self.vel_y *= active_friction;
        
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
    
    fn apply_animated_gravity(&mut self, ui_state: &UIState) {
        let mut total_force_x = 0.0;
        let mut total_force_y = 0.0;
        
        for i in 0..ui_state.well_count as usize {
            let well = &ui_state.wells[i];
            
            let diff_x = well.pos_x - self.pos_x;
            let diff_y = well.pos_y - self.pos_y;
            
            let dist_sq = diff_x * diff_x + diff_y * diff_y + GRAVITY_EPSILON;
            
            // Apply selection bonus
            let bonus = 1.0 + well.selected;
            
            // Minimized wells are weaker (agents shouldn't cluster on taskbar too much)
            let minimized_factor = 1.0 - well.is_minimized * 0.5;
            
            let effective_strength = well.strength * bonus * minimized_factor;
            
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

struct MinimizeRenderer {
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

impl MinimizeRenderer {
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
                label: Some("Minimize/Maximize GPU"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
            }, None)
            .await?;
        
        let shader_source = include_str!("../../minimize_maximize_hud.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Minimize/Maximize HUD Shader"),
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
            label: Some("Minimize/Maximize Bind Group Layout"),
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
            label: Some("Minimize/Maximize Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Minimize/Maximize HUD Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });
        
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Minimize/Maximize Bind Group"),
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
                label: Some("Minimize/Maximize Compute Pass"),
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
    println!("║       MINIMIZE/MAXIMIZE — Phase 8 Zeta                           ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  The Event Horizon: Collapse and explosion physics              ║");
    println!("║                                                                  ║");
    println!("║  Collapse: Wells lerp to taskbar, agents get 'sucked'           ║");
    println!("║  Expand: Wells explode back, strength surges briefly            ║");
    println!("║                                                                  ║");
    println!("║  Mechanics:                                                      ║");
    println!("║    Collapse duration: 300ms                                      ║");
    println!("║    Expand duration: 400ms (with pop)                             ║");
    println!("║    Expand strength surge: 1.5x                                   ║");
    println!("║    Transition friction: 0.85 (prevent overshoot)                ║");
    println!("║                                                                  ║");
    println!("║  Visual:                                                         ║");
    println!("║    Collapse: Color shifts to tribe color                         ║");
    println!("║    Expand: Strength surge causes 'pop' effect                    ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();
    
    let shared = SharedMemory::new();
    let agents: Vec<SwarmAgent> = (0..MAX_AGENTS).map(|id| SwarmAgent::new(id as u32)).collect();
    
    let mut ui_state = UIState::new();
    let mut animator = WellAnimator::new();
    
    // Create 3 wells
    let well0 = ui_state.add_well(400.0, 400.0, 1500.0);
    let well1 = ui_state.add_well(640.0, 400.0, 2000.0);
    let well2 = ui_state.add_well(880.0, 400.0, 1500.0);
    
    println!("[WELLS] Created {} wells:", ui_state.well_count);
    for (i, well) in ui_state.wells.iter().take(ui_state.well_count as usize).enumerate() {
        println!("  Well {}: pos=({:.0}, {:.0}) strength={:.0}", 
            i, well.pos_x, well.pos_y, well.strength);
    }
    println!();
    
    println!("[GPU] Initializing...");
    let rt = tokio::runtime::Runtime::new()?;
    let renderer = rt.block_on(MinimizeRenderer::new())?;
    println!("[GPU] Pipeline ready");
    println!();
    
    let total_frames = 600u32;
    let start_time = Instant::now();
    
    let mut agents = agents;
    let mut last_frame_time = Instant::now();
    
    for frame in 0..total_frames {
        // Calculate delta time
        let now = Instant::now();
        let dt_ms = (now - last_frame_time).as_millis() as f32;
        last_frame_time = now;
        
        // Trigger minimize/maximize at specific frames
        if frame == 100 {
            println!();
            println!("[ANIMATION] Frame 100: Minimizing well 0");
            animator.start_collapse(0, &mut ui_state);
        }
        
        if frame == 200 {
            println!();
            println!("[ANIMATION] Frame 200: Minimizing well 1");
            animator.start_collapse(1, &mut ui_state);
        }
        
        if frame == 300 {
            println!();
            println!("[ANIMATION] Frame 300: Maximizing well 0");
            animator.start_expand(0, &mut ui_state);
        }
        
        if frame == 400 {
            println!();
            println!("[ANIMATION] Frame 400: Minimizing well 2");
            animator.start_collapse(2, &mut ui_state);
        }
        
        if frame == 500 {
            println!();
            println!("[ANIMATION] Frame 500: Maximizing well 1");
            animator.start_expand(1, &mut ui_state);
        }
        
        // Update animations
        animator.update(&mut ui_state, dt_ms);
        
        // Update agents
        let agents_clone = agents.to_vec();
        for agent in agents.iter_mut() {
            agent.update(&ui_state, &shared, &agents_clone);
        }
        
        // Update GPU
        renderer.update(&agents, &ui_state, frame);
        
        // Progress
        if frame % 100 == 0 {
            let states: Vec<String> = ui_state.wells[..ui_state.well_count as usize]
                .iter()
                .map(|w| {
                    if w.is_minimized > 0.5 {
                        format!("MIN({:.0},{:.0})", w.pos_x, w.pos_y)
                    } else {
                        format!("EXP({:.0},{:.0})", w.pos_x, w.pos_y)
                    }
                })
                .collect();
            println!("[FRAME {}/{}] States: {:?}", frame, total_frames, states);
        }
    }
    
    renderer.render_to_file("output/minimize_maximize_demo.png")?;
    
    let elapsed = start_time.elapsed();
    println!();
    println!("Demo complete: {} frames in {:.2}s ({:.1} FPS)",
        total_frames, elapsed.as_secs_f64(), total_frames as f64 / elapsed.as_secs_f64());
    println!("Output: output/minimize_maximize_demo.png");
    
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_demo()
}
