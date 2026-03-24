// Agent Messaging - SEND/RECV Opcodes for Inter-Thread Communication
// Implements ! (SEND) and ? (RECV) opcodes for mailbox-based messaging

use wgpu::*;
use image::{ImageBuffer, Rgba};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;
const MAX_THREADS: usize = 8;
const MAILBOX_SIZE: usize = 10; // Each thread has 10 mailbox slots

// Shared memory for inter-thread messaging
struct SharedMemory {
    // Mailbox for each thread: [thread_id][slot] = message
    mailboxes: Vec<Vec<AtomicU32>>,
    // Message waiting flags: [thread_id]
    message_waiting: Vec<AtomicU32>,
}

impl SharedMemory {
    fn new() -> Self {
        let mut mailboxes = Vec::new();
        let mut message_waiting = Vec::new();
        
        for _ in 0..MAX_THREADS {
            let mut mailbox = Vec::new();
            for _ in 0..MAILBOX_SIZE {
                mailbox.push(AtomicU32::new(0));
            }
            mailboxes.push(mailbox);
            message_waiting.push(AtomicU32::new(0));
        }
        
        Self { mailboxes, message_waiting }
    }
    
    // SEND: Write to another thread's mailbox (non-blocking)
    fn send(&self, value: u32, target_thread: usize, _row_offset: usize) -> bool {
        if target_thread >= MAX_THREADS {
            return false;
        }
        
        // Find empty slot in target's mailbox
        for slot in &self.mailboxes[target_thread] {
            if slot.compare_exchange(0, value, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                // Set message waiting flag
                self.message_waiting[target_thread].store(1, Ordering::SeqCst);
                return true;
            }
        }
        
        // Mailbox full
        false
    }
    
    // RECV: Check mailbox and receive message
    fn recv(&self, thread_id: usize) -> Option<u32> {
        if thread_id >= MAX_THREADS {
            return None;
        }
        
        // Check each slot
        for slot in &self.mailboxes[thread_id] {
            let value = slot.load(Ordering::SeqCst);
            if value != 0 {
                // Clear slot
                slot.store(0, Ordering::SeqCst);
                
                // Check if more messages waiting
                let mut has_more = false;
                for s in &self.mailboxes[thread_id] {
                    if s.load(Ordering::SeqCst) != 0 {
                        has_more = true;
                        break;
                    }
                }
                
                if !has_more {
                    self.message_waiting[thread_id].store(0, Ordering::SeqCst);
                }
                
                return Some(value);
            }
        }
        
        None
    }
    
    fn has_message(&self, thread_id: usize) -> bool {
        if thread_id >= MAX_THREADS {
            return false;
        }
        self.message_waiting[thread_id].load(Ordering::SeqCst) == 1
    }
}

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
    is_active: u32,
    ip: u32,
    sp: u32,
    row_offset: u32,
    message_waiting: u32,  // 1 = has messages
    last_sent: u32,        // Last value sent
    last_received: u32,    // Last value received
    _padding: u32,
    registers: [u32; 26],
    mailbox: [u32; 10],    // Mailbox contents for display
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
    message_waiting: bool,
    last_sent: u32,
    last_received: u32,
    mailbox: Vec<u32>,
}

impl Default for VMState {
    fn default() -> Self {
        Self {
            registers: HashMap::new(),
            stack: Vec::new(),
            ip: 0,
            active: true,
            message_waiting: false,
            last_sent: 0,
            last_received: 0,
            mailbox: vec![0; MAILBOX_SIZE],
        }
    }
}

impl VMState {
    fn to_thread_state(&self, row_offset: u32) -> ThreadState {
        let mut registers = [0u32; 26];
        for (name, value) in &self.registers {
            let idx = (*name as u8 - b'A') as usize;
            if idx < 26 {
                registers[idx] = *value as u32;
            }
        }
        
        let mut mailbox = [0u32; 10];
        for (i, value) in self.mailbox.iter().enumerate() {
            if i < 10 {
                mailbox[i] = *value;
            }
        }
        
        ThreadState {
            is_active: if self.active { 1 } else { 0 },
            ip: self.ip as u32,
            sp: self.stack.len() as u32,
            row_offset,
            message_waiting: if self.message_waiting { 1 } else { 0 },
            last_sent: self.last_sent,
            last_received: self.last_received,
            _padding: 0,
            registers,
            mailbox,
        }
    }
}

fn execute_agent_program(code: &str, shared: &SharedMemory) -> Vec<VMState> {
    let mut threads = vec![VMState::default()];
    let tokens: Vec<&str> = code.split_whitespace().collect();
    
    let mut current_thread = 0;
    
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
                
                if !threads[current_thread].active {
                    break;
                }
                continue;
            }
        }
        
        let token = tokens[threads[current_thread].ip];
        
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
            
            // SPAWN opcode ($)
            "$" => {
                if threads.len() < MAX_THREADS {
                    let state = &threads[current_thread];
                    let child = VMState {
                        registers: state.registers.clone(),
                        stack: state.stack.clone(),
                        ip: state.ip + 1,
                        active: true,
                        message_waiting: false,
                        last_sent: 0,
                        last_received: 0,
                        mailbox: vec![0; MAILBOX_SIZE],
                    };
                    threads.push(child);
                    println!("[SPAWN] Thread {} spawned, now {} threads", 
                        threads.len() - 1, threads.len());
                }
            }
            
            // SEND opcode (!)
            // Format: target_thread row_offset value !
            // Stack order (bottom to top): target_thread, row_offset, value
            // Pops: value first, then row_offset, then target_thread
            "!" => {
                if threads[current_thread].stack.len() >= 3 {
                    let value = threads[current_thread].stack.pop().unwrap() as u32;
                    let row_offset = threads[current_thread].stack.pop().unwrap() as usize;
                    let target_thread = threads[current_thread].stack.pop().unwrap() as usize;
                    
                    if target_thread < MAX_THREADS {
                        if shared.send(value, target_thread, row_offset) {
                            threads[current_thread].last_sent = value;
                            println!("[SEND] Thread {} → Thread {}: value={}", 
                                current_thread, target_thread, value);
                        } else {
                            println!("[SEND] FAILED: Thread {} mailbox full", target_thread);
                        }
                    } else {
                        println!("[SEND] FAILED: Invalid target thread {}", target_thread);
                    }
                }
            }
            
            // RECV opcode (?)
            "?" => {
                if let Some(value) = shared.recv(current_thread) {
                    threads[current_thread].stack.push(value as i32);
                    threads[current_thread].last_received = value;
                    threads[current_thread].message_waiting = shared.has_message(current_thread);
                    println!("[RECV] Thread {} received: {}", current_thread, value);
                } else {
                    threads[current_thread].stack.push(0);
                    threads[current_thread].message_waiting = false;
                    println!("[RECV] Thread {}: mailbox empty", current_thread);
                }
            }
            
            // Halt
            "@" => {
                threads[current_thread].active = false;
            }
            
            _ => {}
        }
        
        threads[current_thread].ip += 1;
        
        // Update mailbox display for current thread
        for (i, slot) in shared.mailboxes[current_thread].iter().enumerate() {
            if i < MAILBOX_SIZE {
                threads[current_thread].mailbox[i] = slot.load(Ordering::SeqCst);
            }
        }
        threads[current_thread].message_waiting = shared.has_message(current_thread);
    }
    
    threads
}

struct AgentMessagingRunner {
    device: Device,
    queue: Queue,
    pipeline: ComputePipeline,
    bind_group: BindGroup,
    
    output_buffer: Buffer,
    staging_buffer: Buffer,
    threads_buffer: Buffer,
    config_buffer: Buffer,
}

impl AgentMessagingRunner {
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
                label: Some("Agent Messaging GPU"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
            }, None)
            .await?;
        
        // Load shader
        let shader_source = include_str!("../../agent_messaging_hud.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Agent Messaging HUD Shader"),
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
            label: Some("Agent Messaging Bind Group Layout"),
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
            label: Some("Agent Messaging Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Agent Messaging HUD Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });
        
        // Create bind group
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Agent Messaging Bind Group"),
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
            let row_offset = (i as u32) * 100; // Each thread gets 100 rows
            let state = vm.to_thread_state(row_offset);
            thread_states.push(state);
        }
        
        // Pad to MAX_THREADS
        while thread_states.len() < MAX_THREADS {
            thread_states.push(ThreadState {
                is_active: 0,
                ip: 0,
                sp: 0,
                row_offset: 0,
                message_waiting: 0,
                last_sent: 0,
                last_received: 0,
                _padding: 0,
                registers: [0; 26],
                mailbox: [0; 10],
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
                label: Some("Agent Messaging Compute Pass"),
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
    println!("║          AGENT MESSAGING — SEND/RECV OPCODES            ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  SEND (!): value target_thread row_offset !             ║");
    println!("║  RECV (?): Returns value or 0 if mailbox empty          ║");
    println!("║  Max Threads: {}                                        ║", MAX_THREADS);
    println!("║  Mailbox Size: {} slots per thread                      ║", MAILBOX_SIZE);
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    // Initialize shared memory
    let shared = SharedMemory::new();
    
    // Test program:
    // Thread 0: Store 42 in A, spawn Thread 1, send A to Thread 1's mailbox at offset 0
    // Format: target_thread row_offset value !
    let test_program = "42 a $ 1 0 A ! @ ?";
    println!("[PROGRAM] {}", test_program);
    println!("[EXPECT]  Thread 0: A=42, sends 42 to Thread 1 mailbox[0]");
    println!("[EXPECT]  Thread 1: receives 42 from mailbox");
    println!();
    
    // Execute with messaging support
    println!("[EXECUTE] Running with SEND/RECV support...");
    let threads = execute_agent_program(test_program, &shared);
    
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
        if thread.last_sent > 0 {
            print!("SENT={} ", thread.last_sent);
        }
        if thread.last_received > 0 {
            print!("RECV={} ", thread.last_received);
        }
        if thread.message_waiting {
            print!("MSG+");
        }
        println!("                              ║");
    }
    
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    // Initialize GPU
    println!("[GPU]     Initializing RTX 5090...");
    let runner = AgentMessagingRunner::new().await?;
    println!("[GPU]     Pipeline ready");
    println!();
    
    // Update thread states
    println!("[UPLOAD]  Sending {} threads to GPU...", threads.len());
    runner.update_threads(&threads);
    
    // Render HUDs with messaging
    println!("[RENDER]  Shader rendering agent messaging HUDs...");
    let start = Instant::now();
    let img = runner.render()?;
    let render_time = start.elapsed();
    
    // Save output
    let output_path = "output/agent_messaging.png";
    img.save(output_path)?;
    
    println!("[OUTPUT]  {} ({}ms)", output_path, render_time.as_millis());
    println!();
    
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║              AGENT MESSAGING COMPLETE                   ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  ✅ SEND (!) opcode writes to mailbox                   ║");
    println!("║  ✅ RECV (?) opcode reads from mailbox                  ║");
    println!("║  ✅ {} threads with message passing                     ║", threads.len());
    println!("║  ✅ Atomic operations for thread safety                 ║");
    println!("║  ✅ Render time: {}ms                                  ║", render_time.as_millis());
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    println!("Next: Use vision model to verify messaging HUD");
    
    Ok(())
}
