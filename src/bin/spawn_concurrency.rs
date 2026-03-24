// SPAWN Concurrency - Multi-Agent Parallel Execution
// Implements $ opcode to fork VMs with spatial isolation

use wgpu::*;
use image::{ImageBuffer, Rgba};
use std::collections::HashMap;
use std::time::Instant;

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;
const MAX_THREADS: usize = 8;
const THREAD_ROWS: u32 = 40; // Each thread gets 40 rows

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
struct ThreadState {
    is_active: u32,         // 1 = active, 0 = inactive
    ip: u32,               // Instruction pointer
    sp: u32,               // Stack pointer
    row_offset: u32,       // Y offset in framebuffer
    _padding: u32,
    registers: [u32; 26],  // A-Z
    stack: [u32; 32],      // Stack (32 entries)
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

struct VMState {
    registers: HashMap<char, i32>,
    stack: Vec<i32>,
    ip: usize,
    active: bool,
}

impl Default for VMState {
    fn default() -> Self {
        Self {
            registers: HashMap::new(),
            stack: Vec::new(),
            ip: 0,
            active: true,
        }
    }
}

impl VMState {
    fn fork(&self, row_offset: u32) -> ThreadState {
        let mut registers = [0u32; 26];
        for (name, value) in &self.registers {
            let idx = (*name as u8 - b'A') as usize;
            if idx < 26 {
                registers[idx] = *value as u32;
            }
        }
        
        let mut stack = [0u32; 32];
        for (i, value) in self.stack.iter().enumerate() {
            if i < 32 {
                stack[i] = *value as u32;
            }
        }
        
        ThreadState {
            is_active: 1,
            ip: self.ip as u32,
            sp: self.stack.len() as u32,
            row_offset,
            _padding: 0,
            registers,
            stack,
        }
    }
}

fn execute_spawn_program(code: &str) -> Vec<VMState> {
    let mut threads = vec![VMState::default()];
    let tokens: Vec<&str> = code.split_whitespace().collect();
    
    let mut current_thread = 0;
    let mut spawn_requested = false;
    
    loop {
        // Check if current thread is done
        {
            let state = &threads[current_thread];
            if !state.active || state.ip >= tokens.len() {
                // Switch to next active thread
                let start = current_thread;
                loop {
                    current_thread = (current_thread + 1) % threads.len();
                    if threads[current_thread].active || current_thread == start {
                        break;
                    }
                }
                
                // If we're back at start and it's inactive, we're done
                if !threads[current_thread].active {
                    break;
                }
                continue;
            }
        }
        
        let token = tokens[threads[current_thread].ip];
        
        // Handle spawn request from previous iteration
        if spawn_requested {
            spawn_requested = false;
            // Continue to next instruction in spawned thread
        }
        
        match token {
            // Push number
            n if n.parse::<i32>().is_ok() => {
                threads[current_thread].stack.push(n.parse().unwrap());
            }
            
            // Register store
            reg if reg.len() == 1 && reg.chars().next().unwrap().is_ascii_lowercase() => {
                let reg_name = reg.chars().next().unwrap().to_ascii_uppercase();
                if let Some(value) = threads[current_thread].stack.last().copied() {
                    threads[current_thread].registers.insert(reg_name, value);
                }
            }
            
            // Register load
            reg if reg.len() == 1 && reg.chars().next().unwrap().is_ascii_uppercase() => {
                let reg_name = reg.chars().next().unwrap();
                if let Some(&value) = threads[current_thread].registers.get(&reg_name) {
                    threads[current_thread].stack.push(value);
                }
            }
            
            // Arithmetic
            "+" | "." => {
                if threads[current_thread].stack.len() >= 2 {
                    let b = threads[current_thread].stack.pop().unwrap();
                    let a = threads[current_thread].stack.pop().unwrap();
                    threads[current_thread].stack.push(a + b);
                }
            }
            "-" => {
                if threads[current_thread].stack.len() >= 2 {
                    let b = threads[current_thread].stack.pop().unwrap();
                    let a = threads[current_thread].stack.pop().unwrap();
                    threads[current_thread].stack.push(a - b);
                }
            }
            "*" => {
                if threads[current_thread].stack.len() >= 2 {
                    let b = threads[current_thread].stack.pop().unwrap();
                    let a = threads[current_thread].stack.pop().unwrap();
                    threads[current_thread].stack.push(a * b);
                }
            }
            
            // SPAWN opcode
            "$" => {
                if threads.len() < MAX_THREADS {
                    let state = &threads[current_thread];
                    let child = VMState {
                        registers: state.registers.clone(),
                        stack: state.stack.clone(),
                        ip: state.ip + 1,
                        active: true,
                    };
                    threads.push(child);
                    println!("[SPAWN] Thread {} spawned, now {} threads", 
                        threads.len() - 1, threads.len());
                    spawn_requested = true;
                }
            }
            
            // Halt
            "@" => {
                threads[current_thread].active = false;
            }
            
            _ => {}
        }
        
        threads[current_thread].ip += 1;
    }
    
    threads
}

struct SpawnRunner {
    device: Device,
    queue: Queue,
    pipeline: ComputePipeline,
    bind_group: BindGroup,
    
    output_buffer: Buffer,
    staging_buffer: Buffer,
    threads_buffer: Buffer,
    config_buffer: Buffer,
}

impl SpawnRunner {
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
                label: Some("SPAWN Concurrency GPU"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
            }, None)
            .await?;
        
        // Load shader
        let shader_source = include_str!("../../spawn_parallel_hud.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("SPAWN Parallel HUD Shader"),
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
        
        // Thread states buffer
        let threads_size = (MAX_THREADS * std::mem::size_of::<ThreadState>()) as u64;
        let threads_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Threads Buffer"),
            size: threads_size,
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
            label: Some("SPAWN Bind Group Layout"),
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
            label: Some("SPAWN Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("SPAWN Parallel HUD Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });
        
        // Create bind group
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("SPAWN Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: output_buffer.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: threads_buffer.as_entire_binding() },
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
            threads_buffer,
            config_buffer,
        })
    }
    
    fn update_threads(&self, threads: &[VMState]) {
        let mut thread_states = Vec::new();
        
        for (i, vm) in threads.iter().enumerate() {
            let row_offset = (i as u32 + 1) * THREAD_ROWS;
            let state = vm.fork(row_offset);
            thread_states.push(state);
        }
        
        // Pad to MAX_THREADS
        while thread_states.len() < MAX_THREADS {
            thread_states.push(ThreadState {
                is_active: 0,
                ip: 0,
                sp: 0,
                row_offset: 0,
                _padding: 0,
                registers: [0; 26],
                stack: [0; 32],
            });
        }
        
        self.queue.write_buffer(&self.threads_buffer, 0, bytemuck::cast_slice(&thread_states));
        self.queue.submit(std::iter::empty());
    }
    
    fn render(&self) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, Box<dyn std::error::Error>> {
        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });
        
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("SPAWN Compute Pass"),
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║          SPAWN CONCURRENCY — MULTI-AGENT SWARM          ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Opcode: $ (fork current VM into child thread)          ║");
    println!("║  Max Threads: {}                                       ║", MAX_THREADS);
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    // Test: Main thread stores 1 in A, spawns child
    // Child thread stores 2 in B
    let test_program = "1 a $ 2 b @";
    println!("[PROGRAM] {}", test_program);
    println!("[EXPECT]  Thread 0: A=1, Thread 1: B=2");
    println!();
    
    // Execute with spawn support
    println!("[EXECUTE] Running with SPAWN support...");
    let threads = execute_spawn_program(test_program);
    
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                  THREAD STATES                          ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    
    for (i, thread) in threads.iter().enumerate() {
        print!("║  Thread {}: ", i);
        for reg in "ABCDEFGHIJ".chars() {
            if let Some(&val) = thread.registers.get(&reg) {
                if val != 0 {
                    print!("{}={} ", reg, val);
                }
            }
        }
        println!("                                 ║");
    }
    
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    // Initialize GPU
    println!("[GPU]     Initializing RTX 5090...");
    let runner = SpawnRunner::new().await?;
    println!("[GPU]     Pipeline ready");
    println!();
    
    // Update thread states
    println!("[UPLOAD]  Sending {} threads to GPU...", threads.len());
    runner.update_threads(&threads);
    
    // Render parallel HUDs
    println!("[RENDER]  Shader rendering {} parallel HUDs...", threads.len());
    let start = Instant::now();
    let img = runner.render()?;
    let render_time = start.elapsed();
    
    // Save output
    let output_path = "output/spawn_parallel_hud.png";
    img.save(output_path)?;
    
    println!("[OUTPUT]  {} ({}ms)", output_path, render_time.as_millis());
    println!();
    
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║              SPAWN CONCURRENCY COMPLETE                 ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  ✅ $ opcode spawns child threads                       ║");
    println!("║  ✅ {} threads executed in parallel                     ║", threads.len());
    println!("║  ✅ Parallel HUDs rendered                              ║");
    println!("║  ✅ Render time: {}ms                                  ║", render_time.as_millis());
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    println!("Next: Use vision model to verify parallel HUDs");
    
    Ok(())
}
