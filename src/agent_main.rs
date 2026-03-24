// Pixel Agent GPU Runner
// Double-buffered, self-propagating pixels with /dev/fb0 output

use wgpu::*;
use wgpu::util::DeviceExt;
use image::{ImageBuffer, Rgba};
use std::time::{Duration, Instant};
use std::fs::OpenOptions;
use std::io;
use memmap2::MmapMut;

// Agent opcodes
const OP_NOP: u32 = 0x00;
const OP_IDLE: u32 = 0x01;
const OP_MOVE_RIGHT: u32 = 0x02;
const OP_MOVE_LEFT: u32 = 0x03;
const OP_MOVE_UP: u32 = 0x04;
const OP_MOVE_DOWN: u32 = 0x05;
const OP_REPLICATE: u32 = 0x06;
const OP_INFECT: u32 = 0x07;

// Sensing opcodes
const OP_READ_N: u32 = 0x08;
const OP_READ_S: u32 = 0x09;
const OP_READ_E: u32 = 0x0A;
const OP_READ_W: u32 = 0x0B;

// Conditional opcodes
const OP_IF_RED: u32 = 0x10;
const OP_IF_GREEN: u32 = 0x11;
const OP_IF_EMPTY: u32 = 0x12;
const OP_IF_AGENT: u32 = 0x13;

// Signal opcodes
const OP_EMIT_SIGNAL: u32 = 0x20;
const OP_SLEEP: u32 = 0x21;

// Logic opcodes
const OP_AND: u32 = 0x30;
const OP_XOR: u32 = 0x31;
const OP_RANDOM: u32 = 0x40;

// Portal opcodes (cross-zone signal teleportation)
const OP_PORTAL_IN: u32 = 0x50;   // Teleport signal: g=target_x, b=target_y
const OP_PORTAL_OUT: u32 = 0x51;  // Receive teleported signal
const OP_PORTAL_BIDIR: u32 = 0x52; // Bidirectional portal

// Agent types
const TYPE_EMPTY: u32 = 0;
const TYPE_AGENT: u32 = 254;
const TYPE_CODE: u32 = 255;

// Formula opcodes
const OP_PUSH_X: u32 = 0x01;
const OP_PUSH_Y: u32 = 0x02;
const OP_PUSH_T: u32 = 0x03;
const OP_PUSH_CONST: u32 = 0x04;
const OP_ADD: u32 = 0x10;
const OP_SUB: u32 = 0x11;
const OP_MUL: u32 = 0x12;
const OP_DIV: u32 = 0x13;
const OP_SIN: u32 = 0x20;
const OP_COS: u32 = 0x21;
const OP_SQRT: u32 = 0x23;
const OP_FLOOR: u32 = 0x25;
const OP_NOISE: u32 = 0x30;
const OP_RGB: u32 = 0xF0;
const OP_HSV: u32 = 0xF1;
const OP_END: u32 = 0xFFFFFFFF;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Pixel {
    r: u32,
    g: u32,
    b: u32,
    a: u32,
}

impl Pixel {
    fn empty() -> Self {
        Self { r: 0, g: 0, b: 0, a: TYPE_EMPTY as u32 }
    }
    
    fn agent(opcode: u32, red: u32, green: u32, blue: u32) -> Self {
        Self { r: opcode, g: red, b: green, a: (TYPE_AGENT as u32) }
    }
    
    fn to_rgba(&self) -> [u8; 4] {
        let (r, g, b) = if self.a == TYPE_AGENT as u32 {
            // Agent: use g, b for color (r is opcode)
            (self.g, self.b, 200)  // Bright blue tint
        } else if self.a == TYPE_CODE as u32 {
            // Code: bright magenta
            (255, 0, 255)
        } else {
            // Color pixel: r=red, g=green, b=blue
            (self.r, self.g, self.b)
        };
        [r.clamp(0, 255) as u8, g.clamp(0, 255) as u8, b.clamp(0, 255) as u8, 255]
    }
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

pub struct PixelUniverse {
    device: Device,
    queue: Queue,
    pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
    
    // Double buffers
    buffer_a: Buffer,
    buffer_b: Buffer,
    staging_buffer: Buffer,
    
    // Formula bytecode
    bytecode_buffer: Buffer,
    constants_buffer: Buffer,
    
    // State
    width: u32,
    height: u32,
    frame: u32,
    mode: u32,  // 0=agent, 1=formula
    
    // Framebuffer (optional)
    fb_path: Option<String>,
    
    // Shared memory for external control
    shared_mem: Option<MmapMut>,
    shared_mem_path: String,
}

impl PixelUniverse {
    pub async fn new(width: u32, height: u32, mode: u32, fb_path: Option<String>) -> Result<Self, Box<dyn std::error::Error>> {
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });
        
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
                label: Some("Pixel Universe GPU"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
            }, None)
            .await?;
        
        // Load shader
        let shader_source = include_str!("../pixel-agent-shader.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Pixel Agent Shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });
        
        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Bind Group Layout"),
            entries: &[
                // Buffer A (input)
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Buffer B (output)
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Bytecode
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Constants
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
                // Config
                BindGroupLayoutEntry {
                    binding: 4,
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
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Pixel Agent Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });
        
        // Create buffers
        let pixel_count = (width * height) as u64;
        let buffer_size = pixel_count * std::mem::size_of::<Pixel>() as u64;
        
        let buffer_a = device.create_buffer(&BufferDescriptor {
            label: Some("Buffer A"),
            size: buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        
        let buffer_b = device.create_buffer(&BufferDescriptor {
            label: Some("Buffer B"),
            size: buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        
        let staging_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Staging Buffer"),
            size: buffer_size,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        // Create bytecode and constants buffers (empty for now)
        let bytecode_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Bytecode Buffer"),
            size: 256 * 4,  // 256 instructions
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        let constants_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Constants Buffer"),
            size: 64 * 4,  // 64 constants
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        // Create shared memory for external control
        let shared_mem_path = "/tmp/pixel-universe.mem".to_string();
        let buffer_size = (width * height) as u64 * std::mem::size_of::<Pixel>() as u64;
        
        // Create or open shared memory file
        let shared_mem = {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&shared_mem_path);
            
            match file {
                Ok(f) => {
                    // Set file size
                    f.set_len(buffer_size).ok();
                    unsafe { MmapMut::map_mut(&f).ok() }
                }
                Err(_) => None,
            }
        };
        
        Ok(Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
            buffer_a,
            buffer_b,
            staging_buffer,
            bytecode_buffer,
            constants_buffer,
            width,
            height,
            frame: 0,
            mode,
            fb_path,
            shared_mem,
            shared_mem_path,
        })
    }
    
    /// Initialize buffer A with pixels
    pub fn init_buffer(&self, pixels: &[Pixel]) {
        let data: Vec<u8> = pixels.iter()
            .flat_map(|p| bytemuck::bytes_of(p).to_vec())
            .collect();
        
        self.queue.write_buffer(&self.buffer_a, 0, &data);
    }
    
    /// Set formula bytecode
    pub fn set_bytecode(&self, bytecode: &[u32], constants: &[f32]) {
        let bytecode_data: Vec<u8> = bytecode.iter()
            .flat_map(|op| op.to_le_bytes())
            .collect();
        
        let constants_data: Vec<u8> = constants.iter()
            .flat_map(|c| c.to_le_bytes())
            .collect();
        
        self.queue.write_buffer(&self.bytecode_buffer, 0, &bytecode_data);
        self.queue.write_buffer(&self.constants_buffer, 0, &constants_data);
    }
    
    /// Execute one frame
    pub fn step(&mut self, time: f32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        // Read from shared memory (injector control)
        if let Some(ref mut mmap) = self.shared_mem {
            // Read injected pixels from shared memory
            let data = &mmap[..];
            
            // Check for any non-zero pixels and write them to buffer A
            let pixel_count = (self.width * self.height) as usize;
            let mut injected = Vec::with_capacity(pixel_count);
            
            for i in 0..pixel_count {
                let offset = i * 16;
                if offset + 16 <= data.len() {
                    let r = u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]);
                    let g = u32::from_le_bytes([data[offset+4], data[offset+5], data[offset+6], data[offset+7]]);
                    let b = u32::from_le_bytes([data[offset+8], data[offset+9], data[offset+10], data[offset+11]]);
                    let a = u32::from_le_bytes([data[offset+12], data[offset+13], data[offset+14], data[offset+15]]);
                    
                    if a > 0 {
                        // This pixel was injected, add to update
                        injected.push((i, Pixel { r, g, b, a }));
                    }
                }
            }
            
            // Write injected pixels to GPU
            if !injected.is_empty() {
                let pixel_size = std::mem::size_of::<Pixel>() as u64;
                for (idx, pixel) in injected {
                    let offset = (idx as u64) * pixel_size;
                    self.queue.write_buffer(&self.buffer_a, offset, bytemuck::bytes_of(&pixel));
                }
            }
        }
        
        let config = Config {
            width: self.width,
            height: self.height,
            time,
            frame: self.frame,
            mode: self.mode,
        };
        
        let config_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Config Buffer"),
            contents: bytemuck::bytes_of(&config),
            usage: BufferUsages::UNIFORM,
        });
        
        // Create bind group (A → B)
        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: self.buffer_a.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: self.buffer_b.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: self.bytecode_buffer.as_entire_binding() },
                BindGroupEntry { binding: 3, resource: self.constants_buffer.as_entire_binding() },
                BindGroupEntry { binding: 4, resource: config_buffer.as_entire_binding() },
            ],
        });
        
        // Execute compute pass
        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Command Encoder"),
        });
        
        {
            let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Compute Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            
            let workgroups_x = (self.width + 15) / 16;
            let workgroups_y = (self.height + 15) / 16;
            compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }
        
        // Copy B to staging
        let buffer_size = (self.width * self.height) as u64 * std::mem::size_of::<Pixel>() as u64;
        encoder.copy_buffer_to_buffer(&self.buffer_b, 0, &self.staging_buffer, 0, buffer_size);
        
        // Swap buffers (A = B for next frame)
        encoder.copy_buffer_to_buffer(&self.buffer_b, 0, &self.buffer_a, 0, buffer_size);
        
        self.queue.submit(std::iter::once(encoder.finish()));
        
        // Read back
        let buffer_slice = self.staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
        self.device.poll(Maintain::Wait);
        rx.recv()??;
        
        let data = buffer_slice.get_mapped_range().to_vec();
        self.staging_buffer.unmap();
        
        self.frame += 1;
        
        // Write current state to shared memory for external tools
        self.write_to_shared_memory(&data);
        
        Ok(data)
    }
    
    /// Write current state to shared memory for external tools (scanner, heatmap)
    fn write_to_shared_memory(&mut self, data: &[u8]) {
        if let Some(ref mut mmap) = self.shared_mem {
            // Write the raw pixel data to shared memory
            if data.len() <= mmap.len() {
                mmap[..data.len()].copy_from_slice(data);
            }
        }
    }
    
    /// Convert raw pixel data to RGBA image
    pub fn to_rgba(&self, data: &[u8]) -> Vec<u8> {
        let pixel_count = (self.width * self.height) as usize;
        let mut rgba = Vec::with_capacity(pixel_count * 4);
        
        // Each pixel is 4 x u32 (16 bytes)
        for i in 0..pixel_count {
            let offset = i * 16;
            if offset + 16 <= data.len() {
                let r = u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]);
                let g = u32::from_le_bytes([data[offset+4], data[offset+5], data[offset+6], data[offset+7]]);
                let b = u32::from_le_bytes([data[offset+8], data[offset+9], data[offset+10], data[offset+11]]);
                let _a = u32::from_le_bytes([data[offset+12], data[offset+13], data[offset+14], data[offset+15]]);
                
                rgba.push(r.clamp(0, 255) as u8);
                rgba.push(g.clamp(0, 255) as u8);
                rgba.push(b.clamp(0, 255) as u8);
                rgba.push(255);
            }
        }
        
        rgba
    }
    
    /// Write to framebuffer
    pub fn write_to_framebuffer(&self, rgba: &[u8]) -> io::Result<()> {
        if let Some(ref path) = self.fb_path {
            // For /dev/fb0, we need to write raw RGB (not RGBA)
            let rgb: Vec<u8> = rgba.chunks(4)
                .flat_map(|chunk| &chunk[..3])
                .copied()
                .collect();
            
            // Write to framebuffer
            let mut file = OpenOptions::new()
                .write(true)
                .open(path)?;
            
            use std::io::Write;
            file.write_all(&rgb)?;
            file.flush()?;
        }
        Ok(())
    }
    
    /// Run animation loop
    pub fn run(&mut self, fps: u32, frames: u32) -> Result<(), Box<dyn std::error::Error>> {
        let frame_time = 1000 / fps as u64;
        let start_total = Instant::now();
        
        for frame in 0..frames {
            let start_frame = Instant::now();
            let time = frame as f32 / fps as f32;
            
            // Execute GPU step
            let raw_data = self.step(time)?;
            let rgba = self.to_rgba(&raw_data);
            
            // Write to framebuffer if available
            if let Err(e) = self.write_to_framebuffer(&rgba) {
                eprintln!("Framebuffer write error: {}", e);
            }
            
            // Save frame as PNG
            if frame % 10 == 0 {
                let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(
                    self.width, self.height, rgba.clone()
                ).ok_or("Failed to create image")?;
                
                let filename = format!("output/frame_{:04}.png", frame);
                img.save(&filename)?;
                println!("Frame {} saved: {} ({:.2}ms)", 
                    frame, filename, start_frame.elapsed().as_secs_f64() * 1000.0);
            }
            
            // Maintain frame rate
            let elapsed = start_frame.elapsed();
            let target = Duration::from_millis(frame_time);
            if elapsed < target {
                std::thread::sleep(target - elapsed);
            }
        }
        
        println!("\nTotal: {:.2}s for {} frames ({:.2} fps avg)",
            start_total.elapsed().as_secs_f64(),
            frames,
            frames as f64 / start_total.elapsed().as_secs_f64());
        
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    println!("=== Pixel Universe - Logic Gates ===\n");
    
    // Check for framebuffer
    let fb_path = if std::path::Path::new("/dev/fb0").exists() {
        println!("Framebuffer: /dev/fb0 detected");
        Some("/dev/fb0".to_string())
    } else {
        println!("Framebuffer: not available (will save PNGs)");
        None
    };
    
    // Create universe
    let width = 480u32;
    let height = 240u32;
    
    println!("Resolution: {}x{} ({} pixels)", width, height, width * height);
    println!("\nInitializing GPU...");
    
    let mut universe = futures::executor::block_on(
        PixelUniverse::new(width, height, 0, fb_path)
    )?;
    
    // Initialize with logic gate pattern
    println!("Building logic circuits...");
    let mut pixels = vec![Pixel::empty(); (width * height) as usize];
    
    // Circuit 1: AND Gate
    // Input A (left wire)
    let input_a_x = 50;
    let input_a_y = 50;
    pixels[(input_a_y * width + input_a_x) as usize] = Pixel::agent(OP_REPLICATE, 255, 0, 0);
    
    // Input B (top wire)
    let input_b_x = 100;
    let input_b_y = 30;
    pixels[(input_b_y * width + input_b_x) as usize] = Pixel::agent(OP_MOVE_DOWN, 0, 255, 0);
    
    // AND gate (center)
    let gate_x = 100;
    let gate_y = 50;
    pixels[(gate_y * width + gate_x) as usize] = Pixel::agent(OP_AND, 255, 255, 255);
    
    // Circuit 2: XOR Gate
    let xor_x = 200;
    let xor_y = 100;
    pixels[(xor_y * width + xor_x) as usize] = Pixel::agent(OP_XOR, 255, 0, 255);
    
    // Inputs for XOR
    pixels[((xor_y - 20) * width + xor_x) as usize] = Pixel::agent(OP_MOVE_DOWN, 0, 255, 0);
    pixels[(xor_y * width + xor_x - 20) as usize] = Pixel::agent(OP_MOVE_RIGHT, 255, 0, 0);
    
    // Circuit 3: Random walkers
    for i in 0..5 {
        let rx = 300 + i * 30;
        let ry = 120;
        pixels[(ry * width + rx) as usize] = Pixel::agent(OP_RANDOM, 255, 255, 0);
    }
    
    // Circuit 4: Signal emitter
    let emitter_x = 400;
    let emitter_y = 180;
    pixels[(emitter_y * width + emitter_x) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 0, 255, 255);
    
    // Circuit 5: Portal Test - Visible cross-zone pattern
    // Clock source at (10, 10) - stays in place, emits signal
    pixels[(10 * width + 10) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 0, 255, 0);  // Green = clock
    
    // Horizontal wire from clock to right edge (foundry zone)
    for x in 11..238 {
        pixels[(10 * width + x) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 100, 100, 0);  // Yellow = wire
    }
    
    // Portal entry at (238, 10) - right edge of foundry
    pixels[(10 * width + 238) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 255, 0, 0);  // Red = portal IN
    
    // Portal exit at architect zone (362, 10) - left edge of architect
    pixels[(10 * width + 362) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 0, 255, 255);  // Cyan = portal OUT
    
    // Wire in architect zone
    for x in 363..470 {
        pixels[(10 * width + x) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 0, 200, 200);  // Cyan = architect wire
    }
    
    // Zone divider lines (visual markers)
    for y in 0..200 {
        pixels[(y * width + 239) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 50, 50, 50);  // Dark = foundry/typist divider
        pixels[(y * width + 359) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 50, 50, 50);  // Dark = typist/architect divider
    }
    
    // Circuit 6: Portal test with actual OP_PORTAL opcodes
    // Clock source in foundry (10, 15)
    pixels[(15 * width + 10) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 0, 255, 0);  // Green clock
    
    // Wire to portal IN at foundry edge (238, 15)
    for x in 11..238 {
        pixels[(15 * width + x) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 100, 100, 0);  // Yellow wire
    }
    
    // Portal IN at (238, 15) - targets architect zone (362, 15)
    // OP_PORTAL_IN: g=target_x, b=target_y
    pixels[(15 * width + 238) as usize] = Pixel::agent(OP_PORTAL_IN, 0, 362, 15);  // Teleport to (362, 15)
    
    // Portal OUT at architect zone (362, 15)
    pixels[(15 * width + 362) as usize] = Pixel::agent(OP_PORTAL_OUT, 0, 0, 0);  // Receiver
    
    // Wire from portal OUT in architect zone
    for x in 363..470 {
        pixels[(15 * width + x) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 0, 200, 200);  // Cyan wire
    }
    
    // Circuit 7: CPU in architect zone (scaled 4x from macro placement)
    // Clock module at (8, 8) in foundry = architect (2, 2) * 4
    // Shows clock oscillator pattern
    for x in 32..56 {
        pixels[(12 * width + x) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 100, 200, 255);
        pixels[(13 * width + x) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 100, 200, 255);
        pixels[(14 * width + x) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 100, 200, 255);
        pixels[(15 * width + x) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 100, 200, 255);
    }
    // Clock loop closure
    pixels[(12 * width + 32) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 0, 255, 0);
    pixels[(12 * width + 55) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 0, 255, 0);
    pixels[(15 * width + 32) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 0, 255, 0);
    pixels[(15 * width + 55) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 0, 255, 0);
    
    // PC module at (8, 32) in foundry = architect (2, 8) * 4
    // Shows 2-bit counter pattern
    for x in 32..128 {
        for y_offset in 0..72 {
            let y = 32 + y_offset;
            if y < 200 {
                pixels[(y * width + x) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 80, 150, 200);
            }
        }
    }
    // XOR gates (red)
    for x_off in &[16, 32, 48, 64] {
        pixels[(48 * width + 32 + x_off) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 255, 100, 100);
        pixels[(49 * width + 32 + x_off) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 255, 100, 100);
        pixels[(50 * width + 32 + x_off) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 255, 100, 100);
        pixels[(51 * width + 32 + x_off) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 255, 100, 100);
    }
    // AND gates (green)
    for x_off in &[20, 40, 60] {
        pixels[(64 * width + 32 + x_off) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 100, 255, 100);
        pixels[(65 * width + 32 + x_off) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 100, 255, 100);
        pixels[(66 * width + 32 + x_off) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 100, 255, 100);
        pixels[(67 * width + 32 + x_off) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 100, 255, 100);
    }
    // Flip-flops (yellow)
    for x_off in &[24, 48, 72, 96] {
        pixels[(80 * width + 32 + x_off) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 255, 255, 100);
        pixels[(81 * width + 32 + x_off) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 255, 255, 100);
        pixels[(82 * width + 32 + x_off) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 255, 255, 100);
        pixels[(83 * width + 32 + x_off) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 255, 255, 100);
    }
    
    // ALU module at (8, 120) in foundry = architect (2, 30) * 4
    // Shows ALU pattern
    for x in 32..120 {
        for y_offset in 0..48 {
            let y = 120 + y_offset;
            if y < 200 {
                pixels[(y * width + x) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 60, 120, 180);
            }
        }
    }
    // ALU XOR gates
    pixels[(140 * width + 48) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 255, 100, 100);
    pixels[(140 * width + 52) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 255, 100, 100);
    pixels[(140 * width + 56) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 255, 100, 100);
    pixels[(140 * width + 60) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 255, 100, 100);
    // ALU AND gates
    pixels[(150 * width + 48) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 100, 255, 100);
    pixels[(150 * width + 52) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 100, 255, 100);
    pixels[(150 * width + 56) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 100, 255, 100);
    pixels[(150 * width + 60) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 100, 255, 100);
    // Output indicator
    for x in 80..120 {
        pixels[(160 * width + x) as usize] = Pixel::agent(OP_EMIT_SIGNAL, 100, 200, 255);
    }
    
    universe.init_buffer(&pixels);
    
    // Create output directory
    std::fs::create_dir_all("output")?;
    
    println!("Running simulation (100 frames at 30 fps)...\n");
    println!("Logic circuits:");
    println!("  - AND gate at ({}, {})", gate_x, gate_y);
    println!("  - XOR gate at ({}, {})", xor_x, xor_y);
    println!("  - Random walkers at x=300-420");
    println!("  - Signal emitter at ({}, {})", emitter_x, emitter_y);
    println!("  - Portal test: clock (10,10) → wire → portal IN (238,10) → portal OUT (362,10)");
    println!("  - Zone dividers at x=239 (foundry/typist) and x=359 (typist/architect)");
    println!();
    
    universe.run(30, 100)?;
    
    println!("\n✓ Done!");
    Ok(())
}
