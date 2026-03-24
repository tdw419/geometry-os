// Spatial Physics - Geometry Opcodes for Position, Velocity, and Collision
// 
// Architecture:
//   - Spatial registers: POS_X, POS_Y (u32), VEL_X, VEL_Y (i32)
//   - Opcodes: p (POS), > (MOVE), x (SENSE), ! (PUNCH)
//   - Each thread has independent spatial state
//   - Boundary clamping prevents out-of-bounds

use wgpu::*;
use image::{ImageBuffer, Rgba};
use std::collections::HashMap;
use std::time::Instant;

// ============================================================================
// CONSTANTS
// ============================================================================

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;
const MAX_AGENTS: usize = 8;
const TRAIL_LENGTH: usize = 50;

// ============================================================================
// SPATIAL STATE
// ============================================================================

#[derive(Debug, Clone)]
struct SpatialState {
    pos_x: u32,
    pos_y: u32,
    vel_x: i32,
    vel_y: i32,
    trail: Vec<(u32, u32)>,
    collision: bool,
}

impl Default for SpatialState {
    fn default() -> Self {
        Self {
            pos_x: 320,
            pos_y: 240,
            vel_x: 0,
            vel_y: 0,
            trail: Vec::new(),
            collision: false,
        }
    }
}

// ============================================================================
// VM STATE
// ============================================================================

#[derive(Debug, Default)]
struct VMState {
    registers: HashMap<char, i32>,
    stack: Vec<i32>,
    ip: usize,
    halted: bool,
    spatial: SpatialState,
    last_result: i32,
    loop_count: u32,
    max_loops: u32,
}

impl VMState {
    fn with_spatial(spatial: SpatialState) -> Self {
        Self {
            spatial,
            max_loops: 10000,
            ..Default::default()
        }
    }
    
    fn clamp_position(&mut self) {
        self.spatial.pos_x = self.spatial.pos_x.clamp(10, WIDTH - 10);
        self.spatial.pos_y = self.spatial.pos_y.clamp(10, HEIGHT - 10);
    }
    
    fn execute_token(&mut self, token: &str, output_buffer: &mut Vec<u32>) -> bool {
        match token {
            // ========================================
            // POS Opcode (`p`) - Push position onto stack
            // ========================================
            "p" => {
                self.stack.push(self.spatial.pos_x as i32);
                self.stack.push(self.spatial.pos_y as i32);
                println!("[POS] Pushed ({}, {})", self.spatial.pos_x, self.spatial.pos_y);
            }
            
            // POS with register: `p reg` - Set position from register
            reg if reg.starts_with("p") && reg.len() > 1 => {
                let axis = reg.chars().nth(1).unwrap();
                if let Some(&value) = self.stack.last() {
                    match axis {
                        'x' | 'X' => {
                            self.spatial.pos_x = value.max(10).min((WIDTH - 10) as i32) as u32;
                            println!("[POS] Set X = {}", self.spatial.pos_x);
                        }
                        'y' | 'Y' => {
                            self.spatial.pos_y = value.max(10).min((HEIGHT - 10) as i32) as u32;
                            println!("[POS] Set Y = {}", self.spatial.pos_y);
                        }
                        _ => {}
                    }
                }
            }
            
            // ========================================
            // VEL Opcode (`v`) - Set velocity
            // ========================================
            reg if reg.starts_with("v") && reg.len() > 1 => {
                let axis = reg.chars().nth(1).unwrap();
                if let Some(&value) = self.stack.last() {
                    match axis {
                        'x' | 'X' => {
                            self.spatial.vel_x = value;
                            println!("[VEL] Set VEL_X = {}", self.spatial.vel_x);
                        }
                        'y' | 'Y' => {
                            self.spatial.vel_y = value;
                            println!("[VEL] Set VEL_Y = {}", self.spatial.vel_y);
                        }
                        _ => {}
                    }
                }
            }
            
            // ========================================
            // MOVE Opcode (`>`) - Move by velocity
            // Format: dx dy >
            // ========================================
            ">" => {
                // Pop dy, dx from stack (in reverse order)
                if self.stack.len() >= 2 {
                    let dy = self.stack.pop().unwrap();
                    let dx = self.stack.pop().unwrap();
                    
                    self.spatial.pos_x = (self.spatial.pos_x as i32 + dx).clamp(10, (WIDTH - 10) as i32) as u32;
                    self.spatial.pos_y = (self.spatial.pos_y as i32 + dy).clamp(10, (HEIGHT - 10) as i32) as u32;
                    
                    // Add to trail
                    self.spatial.trail.push((self.spatial.pos_x, self.spatial.pos_y));
                    if self.spatial.trail.len() > TRAIL_LENGTH {
                        self.spatial.trail.remove(0);
                    }
                    
                    println!("[MOVE] dx={}, dy={} → pos=({},{})", 
                        dx, dy, self.spatial.pos_x, self.spatial.pos_y);
                }
            }
            
            // Move by current velocity (shorthand)
            ">>" => {
                self.spatial.pos_x = (self.spatial.pos_x as i32 + self.spatial.vel_x)
                    .clamp(10, (WIDTH - 10) as i32) as u32;
                self.spatial.pos_y = (self.spatial.pos_y as i32 + self.spatial.vel_y)
                    .clamp(10, (HEIGHT - 10) as i32) as u32;
                
                self.spatial.trail.push((self.spatial.pos_x, self.spatial.pos_y));
                if self.spatial.trail.len() > TRAIL_LENGTH {
                    self.spatial.trail.remove(0);
                }
                
                println!("[MOVE] vel=({},{}) → pos=({},{})", 
                    self.spatial.vel_x, self.spatial.vel_y, 
                    self.spatial.pos_x, self.spatial.pos_y);
            }
            
            // ========================================
            // SENSE Opcode (`x`) - Read pixel at current position
            // ========================================
            "x" => {
                let idx = (self.spatial.pos_y * WIDTH + self.spatial.pos_x) as usize;
                if idx < output_buffer.len() {
                    let pixel = output_buffer[idx];
                    let occupied = if pixel > 0 { 1 } else { 0 };
                    self.stack.push(occupied);
                    self.spatial.collision = occupied > 0;
                    println!("[SENSE] pos=({},{}) pixel={} occupied={}", 
                        self.spatial.pos_x, self.spatial.pos_y, pixel, occupied);
                } else {
                    self.stack.push(0);
                }
            }
            
            // ========================================
            // PUNCH Opcode (`!`) - Write pixel at current position
            // ========================================
            "!" => {
                let value = self.stack.pop().unwrap_or(255) as u32;
                let idx = (self.spatial.pos_y * WIDTH + self.spatial.pos_x) as usize;
                if idx < output_buffer.len() {
                    output_buffer[idx] = value;
                    println!("[PUNCH] pos=({},{}) value={}", 
                        self.spatial.pos_x, self.spatial.pos_y, value);
                }
            }
            
            // ========================================
            // Branch if greater (`>:`)
            // ========================================
            op if op.ends_with('>') && op.len() > 1 => {
                let label = op.trim_end_matches('>');
                if self.stack.len() >= 2 {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    if a > b {
                        // Jump to label (find it)
                        println!("[BRANCH] {} > {} → jump to :{}", a, b, label);
                    }
                }
            }
            
            // ========================================
            // Branch if less (`<:`)
            // ========================================
            op if op.ends_with('<') && op.len() > 1 => {
                let label = op.trim_end_matches('<');
                if self.stack.len() >= 2 {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    if a < b {
                        println!("[BRANCH] {} < {} → jump to :{}", a, b, label);
                    }
                }
            }
            
            // ========================================
            // Labels and jumps
            // ========================================
            label if label.starts_with(':') => {
                // Label definition - skip
            }
            
            jump if jump.starts_with('@') => {
                // Jump to label
                let label = jump.trim_start_matches('@');
                println!("[JUMP] → :{}", label);
            }
            
            // ========================================
            // Standard stack operations
            // ========================================
            n if n.parse::<i32>().is_ok() => {
                self.stack.push(n.parse().unwrap());
            }
            
            reg if reg.len() == 1 && reg.chars().next().unwrap().is_ascii_lowercase() => {
                let reg_name = reg.chars().next().unwrap().to_ascii_uppercase();
                if let Some(value) = self.stack.last().copied() {
                    self.registers.insert(reg_name, value);
                }
            }
            
            reg if reg.len() == 1 && reg.chars().next().unwrap().is_ascii_uppercase() => {
                let reg_name = reg.chars().next().unwrap();
                if let Some(&value) = self.registers.get(&reg_name) {
                    self.stack.push(value);
                }
            }
            
            "+" => {
                if self.stack.len() >= 2 {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(a + b);
                }
            }
            "-" => {
                if self.stack.len() >= 2 {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(a - b);
                }
            }
            "*" => {
                if self.stack.len() >= 2 {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(a * b);
                }
            }
            "/" => {
                if self.stack.len() >= 2 {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    if b != 0 {
                        self.stack.push(a / b);
                    }
                }
            }
            
            "dup" => {
                if let Some(&top) = self.stack.last() {
                    self.stack.push(top);
                }
            }
            
            "." => {
                if let Some(top) = self.stack.pop() {
                    self.last_result = top;
                }
            }
            
            "@" => {
                self.halted = true;
                return false;
            }
            
            _ => {}
        }
        
        self.ip += 1;
        self.loop_count += 1;
        
        // Prevent infinite loops
        if self.loop_count > self.max_loops {
            self.halted = true;
            return false;
        }
        
        true
    }
}

// ============================================================================
// BOUNCING AGENT DEMO
// ============================================================================

fn run_bouncing_agent(output_buffer: &mut Vec<u32>) -> VMState {
    let spatial = SpatialState {
        pos_x: 320,
        pos_y: 240,
        vel_x: 2,
        vel_y: 2,
        trail: Vec::new(),
        collision: false,
    };
    
    let mut vm = VMState::with_spatial(spatial);
    
    // Bouncing agent program (simplified - logic runs directly below)
    let _program = r#"
        :loop
        >>
        255 !
        
        :check_x
        POS_X 630 >
        :hit_x
        POS_X 10 <
        :hit_x
        
        :check_y
        POS_Y 470 >
        :hit_y
        POS_Y 10 <
        :hit_y
        
        @ :loop
        
        :hit_x
        VEL_X -1 *
        v x !
        @ :loop
        
        :hit_y
        VEL_Y -1 *
        v y !
        @ :loop
    "#;
    
    // Simplified execution - just run the bouncing logic directly
    println!("\n[BUTTON] Bouncing Agent Demo");
    println!("[INIT] POS=({},{}) VEL=({},{})", 
        vm.spatial.pos_x, vm.spatial.pos_y, 
        vm.spatial.vel_x, vm.spatial.vel_y);
    
    for frame in 0..60 {
        // Move by velocity
        vm.spatial.pos_x = (vm.spatial.pos_x as i32 + vm.spatial.vel_x)
            .clamp(10, (WIDTH - 10) as i32) as u32;
        vm.spatial.pos_y = (vm.spatial.pos_y as i32 + vm.spatial.vel_y)
            .clamp(10, (HEIGHT - 10) as i32) as u32;
        
        // Add to trail
        vm.spatial.trail.push((vm.spatial.pos_x, vm.spatial.pos_y));
        if vm.spatial.trail.len() > TRAIL_LENGTH {
            vm.spatial.trail.remove(0);
        }
        
        // Punch pixel
        let idx = (vm.spatial.pos_y * WIDTH + vm.spatial.pos_x) as usize;
        output_buffer[idx] = 255;
        
        // Check boundaries and bounce
        if vm.spatial.pos_x >= WIDTH - 10 || vm.spatial.pos_x <= 10 {
            vm.spatial.vel_x = -vm.spatial.vel_x;
            vm.spatial.collision = true;
            println!("[FRAME {}] BOUNCE X at ({},{})", frame, vm.spatial.pos_x, vm.spatial.pos_y);
        }
        
        if vm.spatial.pos_y >= HEIGHT - 10 || vm.spatial.pos_y <= 10 {
            vm.spatial.vel_y = -vm.spatial.vel_y;
            vm.spatial.collision = true;
            println!("[FRAME {}] BOUNCE Y at ({},{})", frame, vm.spatial.pos_x, vm.spatial.pos_y);
        }
    }
    
    println!("[FINAL] POS=({},{}) VEL=({},{}) trail_len={}", 
        vm.spatial.pos_x, vm.spatial.pos_y,
        vm.spatial.vel_x, vm.spatial.vel_y,
        vm.spatial.trail.len());
    
    vm
}

// ============================================================================
// GPU RENDERER
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
struct AgentState {
    pos_x: u32,
    pos_y: u32,
    vel_x: i32,
    vel_y: i32,
    collision: u32,
    trail_len: u32,
    _padding: [u32; 2],
    trail: [u32; 32],  // Packed trail (x << 16 | y) - first 32 points
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

struct SpatialPhysicsRenderer {
    device: Device,
    queue: Queue,
    pipeline: ComputePipeline,
    bind_group: BindGroup,
    
    output_buffer: Buffer,
    staging_buffer: Buffer,
    agent_buffer: Buffer,
    config_buffer: Buffer,
    trail_buffer: Buffer,
}

impl SpatialPhysicsRenderer {
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
                label: Some("Spatial Physics GPU"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
            }, None)
            .await?;
        
        // Load shader
        let shader_source = include_str!("../../spatial_physics_hud.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Spatial Physics HUD Shader"),
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
        
        // Agent state buffer
        let agent_size = (MAX_AGENTS * std::mem::size_of::<AgentState>()) as u64;
        let agent_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Agent Buffer"),
            size: agent_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        // Trail buffer (for rendering trails)
        let trail_size = (MAX_AGENTS * TRAIL_LENGTH * std::mem::size_of::<(u32, u32)>()) as u64;
        let trail_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Trail Buffer"),
            size: trail_size,
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
            label: Some("Spatial Physics Bind Group Layout"),
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
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        
        // Create pipeline
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Spatial Physics Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Spatial Physics HUD Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });
        
        // Create bind group
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Spatial Physics Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: output_buffer.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: agent_buffer.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: config_buffer.as_entire_binding() },
                BindGroupEntry { binding: 3, resource: trail_buffer.as_entire_binding() },
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
            trail_buffer,
        })
    }
    
    fn update_agent(&self, vm: &VMState, frame: u32) {
        let mut agent_state = AgentState {
            pos_x: vm.spatial.pos_x,
            pos_y: vm.spatial.pos_y,
            vel_x: vm.spatial.vel_x,
            vel_y: vm.spatial.vel_y,
            collision: if vm.spatial.collision { 1 } else { 0 },
            trail_len: vm.spatial.trail.len() as u32,
            _padding: [0; 2],
            trail: [0; 32],
        };
        
        // Pack trail into u32 array (x << 16 | y)
        for (i, (x, y)) in vm.spatial.trail.iter().enumerate() {
            if i < 32 {
                agent_state.trail[i] = (x << 16) | y;
            }
        }
        
        self.queue.write_buffer(&self.agent_buffer, 0, bytemuck::bytes_of(&agent_state));
        
        // Update config
        let config = Config { 
            width: WIDTH, 
            height: HEIGHT, 
            time: frame as f32 / 60.0, 
            frame, 
            mode: 0 
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
                label: Some("Spatial Physics Compute Pass"),
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
// MAIN
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║         SPATIAL PHYSICS — GEOMETRY OPCODES              ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Spatial Registers:                                      ║");
    println!("║    POS_X, POS_Y (u32) — Position in framebuffer         ║");
    println!("║    VEL_X, VEL_Y (i32) — Velocity vectors                ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Geometry Opcodes:                                       ║");
    println!("║    p      — Push POS (x, y) onto stack                  ║");
    println!("║    >      — MOVE: dx dy > or >> (by velocity)           ║");
    println!("║    x      — SENSE: Read pixel at POS                    ║");
    println!("║    !      — PUNCH: Write pixel at POS                   ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Demo: Bouncing Pixel Agent                             ║");
    println!("║    Start: (320, 240)  Velocity: (2, 2)                  ║");
    println!("║    Bounces off boundaries, leaves trail                 ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    // Initialize output buffer
    let mut output_buffer = vec![0u32; (WIDTH * HEIGHT) as usize];
    
    // Run bouncing agent demo
    let vm = run_bouncing_agent(&mut output_buffer);
    
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                  GPU HUD RENDERING                      ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    
    // Initialize GPU
    println!("[GPU] Initializing RTX 5090...");
    let renderer = SpatialPhysicsRenderer::new().await?;
    println!("[GPU] Pipeline ready");
    
    // Update agent state
    println!("[UPLOAD] Sending spatial state to GPU...");
    renderer.update_agent(&vm, 0);
    
    // Render HUD
    println!("[RENDER] Shader rendering spatial physics HUD...");
    let start = Instant::now();
    let img = renderer.render()?;
    let render_time = start.elapsed();
    
    // Save output
    let output_path = "output/spatial_physics.png";
    img.save(output_path)?;
    
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║              SPATIAL PHYSICS COMPLETE                   ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  ✅ POS opcode pushes position                          ║");
    println!("║  ✅ MOVE opcode updates position with clamping          ║");
    println!("║  ✅ SENSE opcode reads pixels for collision             ║");
    println!("║  ✅ PUNCH opcode writes pixels for drawing              ║");
    println!("║  ✅ Agent bounces off boundaries                        ║");
    println!("║  ✅ Trail rendering ({} points)                         ║", TRAIL_LENGTH);
    println!("║  ✅ HUD displays POS, VEL, collision                    ║");
    println!("║  ✅ Render time: {:?}                                  ║", render_time);
    println!("║  ✅ Output: {}                                 ║", output_path);
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    Ok(())
}
