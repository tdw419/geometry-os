//! Neural Pipe Runner
//! ===================
//! GPU runner with LLM integration.
//!
//! Flow:
//! 1. GPU shader sets stats[0] = NEURAL_PIPE_REQUEST when OP_GENERATE is hit
//! 2. Host polls stats buffer after each frame
//! 3. Host reads prompt zone (rows 10-19), calls LM Studio
//! 4. Host writes response zone (rows 20-39)
//! 5. Host clears stats[0], VM resumes

use wgpu::*;
use wgpu::util::DeviceExt;
use image::{ImageBuffer, Rgba};
use std::time::{Duration, Instant};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;

// Neural Pipe constants
const NEURAL_PIPE_READY: u32 = 0;     // Host done, GPU can execute
const NEURAL_PIPE_REQUEST: u32 = 1;   // GPU requesting, Host starts LLM
const NEURAL_PIPE_WRITING: u32 = 2;   // Host writing response, GPU frozen

const PROMPT_START_ROW: u32 = 10;
const PROMPT_END_ROW: u32 = 19;
const RESPONSE_START_ROW: u32 = 20;
const RESPONSE_END_ROW: u32 = 39;

// Recursive Optimization config
const SELF_MUTATE: bool = true;   // 🔥 SINGULARITY MODE ENABLED 🔥
const SOURCE_ROW: u32 = 40;       // Row containing seed code to optimize

// Opcodes
const OP_GENERATE: u32 = 0x53;
const OP_JMP_RESPONSE: u32 = 0x56;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Pixel {
    r: u32,
    g: u32,
    b: u32,
    a: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct StatsBuffer {
    pub neural_pipe_signal: u32,  // 0 = idle, 1 = request pending
    pub prompt_start: u32,        // Row where prompt starts
    pub response_start: u32,      // Row where response should be written
    pub _padding: u32,
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

pub struct NeuralPipeRunner {
    device: Device,
    queue: Queue,
    pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
    
    // Double buffers
    buffer_a: Buffer,
    buffer_b: Buffer,
    staging_buffer: Buffer,
    stats_buffer: Buffer,
    stats_staging: Buffer,
    
    // State
    width: u32,
    height: u32,
    frame: u32,
    
    // Stats (shared with async LLM caller)
    stats: Arc<RwLock<StatsBuffer>>,
}

impl NeuralPipeRunner {
    pub async fn new(width: u32, height: u32) -> Result<Self, Box<dyn std::error::Error>> {
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
                label: Some("Neural Pipe GPU"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
            }, None)
            .await?;
        
        // Load shader
        let shader_source = include_str!("../../pixel-agent-shader.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Pixel Agent Shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });
        
        // Bind group layout (same as agent_main.rs but with stats buffer)
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Neural Pipe Bind Group Layout"),
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
                // Bytecode (unused but required)
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
                // Constants (unused but required)
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
                // Stats buffer (binding 5)
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        
        // Create pipeline
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Neural Pipe Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Neural Pipe Pipeline"),
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
            label: Some("Pixel Staging Buffer"),
            size: buffer_size,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        // Stats buffer - start FROZEN (REQUEST state) so GPU doesn't execute before LLM responds
        let stats_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Stats Buffer"),
            size: std::mem::size_of::<StatsBuffer>() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        
        // Initialize stats to REQUEST state (frozen)
        let initial_stats = StatsBuffer {
            neural_pipe_signal: NEURAL_PIPE_REQUEST,  // Start frozen!
            prompt_start: PROMPT_START_ROW,
            response_start: RESPONSE_START_ROW,
            _padding: 0,
        };
        queue.write_buffer(&stats_buffer, 0, bytemuck::bytes_of(&initial_stats));
        
        let stats_staging = device.create_buffer(&BufferDescriptor {
            label: Some("Stats Staging Buffer"),
            size: std::mem::size_of::<StatsBuffer>() as u64,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        // Dummy bytecode/constants
        let bytecode_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Bytecode Buffer"),
            size: 256 * 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        let constants_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Constants Buffer"),
            size: 64 * 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        let stats = Arc::new(RwLock::new(StatsBuffer {
            neural_pipe_signal: 0,
            prompt_start: PROMPT_START_ROW,
            response_start: RESPONSE_START_ROW,
            _padding: 0,
        }));
        
        Ok(Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
            buffer_a,
            buffer_b,
            staging_buffer,
            stats_buffer,
            stats_staging,
            width,
            height,
            frame: 0,
            stats,
        })
    }
    
    /// Initialize buffer with pixels
    pub fn init_buffer(&self, pixels: &[Pixel]) {
        let data: Vec<u8> = pixels.iter()
            .flat_map(|p| bytemuck::bytes_of(p).to_vec())
            .collect();
        self.queue.write_buffer(&self.buffer_a, 0, &data);
    }
    
    /// Inject text into prompt zone
    pub fn inject_prompt(&self, text: &str) {
        let chars: Vec<char> = text.chars().collect();
        let mut idx = 0;
        
        // Write prompt to rows 10-18 (leave row 19 for JMP_RESPONSE)
        for y in PROMPT_START_ROW..(RESPONSE_START_ROW - 1) {
            for x in 0..self.width {
                if idx < chars.len() as usize {
                    let pixel_idx = (y * self.width + x) as u64;
                    let offset = pixel_idx * std::mem::size_of::<Pixel>() as u64;
                    
                    let ch = chars[idx] as u32;
                    let pixel = Pixel {
                        r: ch,           // ASCII in red channel
                        g: 100,          // Prompt zone marker
                        b: 200,
                        a: 254,          // TYPE_AGENT
                    };
                    
                    self.queue.write_buffer(&self.buffer_a, offset, bytemuck::bytes_of(&pixel));
                    idx += 1;
                }
            }
        }
        
        // Add JMP_RESPONSE at row 19 (just before response zone at row 20)
        let jmp_y = RESPONSE_START_ROW - 1;  // Row 19
        let jmp_x = 0u32;
        let jmp_idx = (jmp_y * self.width + jmp_x) as u64;
        let jmp_offset = jmp_idx * std::mem::size_of::<Pixel>() as u64;
        
        let jmp_pixel = Pixel {
            r: OP_JMP_RESPONSE,
            g: 0,
            b: 0,
            a: 254,  // TYPE_AGENT
        };
        
        self.queue.write_buffer(&self.buffer_a, jmp_offset, bytemuck::bytes_of(&jmp_pixel));
        println!("[OK] JMP_RESPONSE (0x56) injected at row {}, col 0", jmp_y);
    }
    
    /// Execute one frame and return pixel data + stats
    pub fn step(&mut self, time: f32) -> Result<(Vec<u8>, StatsBuffer), Box<dyn std::error::Error>> {
        let config = Config {
            width: self.width,
            height: self.height,
            time,
            frame: self.frame,
            mode: 0,  // Agent mode
        };
        
        let config_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Config Buffer"),
            contents: bytemuck::bytes_of(&config),
            usage: BufferUsages::UNIFORM,
        });
        
        // Create bind group
        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Neural Pipe Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: self.buffer_a.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: self.buffer_b.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: self.buffer_a.as_entire_binding() }, // Dummy bytecode
                BindGroupEntry { binding: 3, resource: self.buffer_a.as_entire_binding() }, // Dummy constants
                BindGroupEntry { binding: 4, resource: config_buffer.as_entire_binding() },
                BindGroupEntry { binding: 5, resource: self.stats_buffer.as_entire_binding() },
            ],
        });
        
        // Execute compute pass
        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Command Encoder"),
        });
        
        {
            let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Neural Pipe Compute Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            
            let workgroups_x = (self.width + 15) / 16;
            let workgroups_y = (self.height + 15) / 16;
            compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }
        
        let buffer_size = (self.width * self.height) as u64 * std::mem::size_of::<Pixel>() as u64;
        let stats_size = std::mem::size_of::<StatsBuffer>() as u64;
        
        // Copy outputs
        encoder.copy_buffer_to_buffer(&self.buffer_b, 0, &self.staging_buffer, 0, buffer_size);
        encoder.copy_buffer_to_buffer(&self.stats_buffer, 0, &self.stats_staging, 0, stats_size);
        encoder.copy_buffer_to_buffer(&self.buffer_b, 0, &self.buffer_a, 0, buffer_size);
        
        self.queue.submit(std::iter::once(encoder.finish()));
        
        // Read pixel data
        let pixel_slice = self.staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        pixel_slice.map_async(MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
        self.device.poll(Maintain::Wait);
        rx.recv()??;
        let pixel_data = pixel_slice.get_mapped_range().to_vec();
        self.staging_buffer.unmap();
        
        // Read stats
        let stats_slice = self.stats_staging.slice(..);
        let (tx2, rx2) = std::sync::mpsc::channel();
        stats_slice.map_async(MapMode::Read, move |result| {
            tx2.send(result).unwrap();
        });
        self.device.poll(Maintain::Wait);
        rx2.recv()??;
        let stats_data = stats_slice.get_mapped_range().to_vec();
        self.stats_staging.unmap();
        
        let stats: StatsBuffer = *bytemuck::from_bytes(&stats_data);
        
        self.frame += 1;
        
        Ok((pixel_data, stats))
    }
    
    /// Write response text into response zone
    pub fn write_response(&self, text: &str) {
        let chars: Vec<char> = text.chars().collect();
        let mut idx = 0;
        
        for y in RESPONSE_START_ROW..=RESPONSE_END_ROW {
            for x in 0..self.width {
                if idx < chars.len() as usize {
                    let pixel_idx = (y * self.width + x) as u64;
                    let offset = pixel_idx * std::mem::size_of::<Pixel>() as u64;
                    
                    let ch = chars[idx] as u32;
                    let pixel = Pixel {
                        r: ch,           // ASCII in red channel (brightness)
                        g: ch,           // Also in green
                        b: ch,           // Also in blue (white text)
                        a: 1,            // Non-agent, non-code = renders as RGB
                    };
                    
                    self.queue.write_buffer(&self.buffer_a, offset, bytemuck::bytes_of(&pixel));
                    idx += 1;
                }
            }
        }
    }
    
    /// Clear the neural pipe signal (allow GPU to resume)
    pub fn clear_signal(&self) {
        let signal: u32 = NEURAL_PIPE_READY;
        self.queue.write_buffer(&self.stats_buffer, 0, &signal.to_le_bytes());
    }
    
    /// Remove the OP_GENERATE trigger pixel (prevent infinite loop)
    pub fn clear_trigger(&self) {
        // Clear the trigger at end of prompt row
        let trigger_y = PROMPT_END_ROW;
        let trigger_x = self.width - 1;
        let trigger_idx = (trigger_y * self.width + trigger_x) as u64;
        let trigger_offset = trigger_idx * std::mem::size_of::<Pixel>() as u64;
        
        // Replace with NOP
        let nop = Pixel { r: 0, g: 0, b: 0, a: 0 };
        self.queue.write_buffer(&self.buffer_a, trigger_offset, bytemuck::bytes_of(&nop));
    }
    
    /// Self-mutation: Copy optimized code from response zone back to source
    /// This makes optimizations permanent
    pub fn mutate_source(&self, optimized_code: &str) {
        let chars: Vec<char> = optimized_code.chars().collect();
        let mut idx = 0;
        
        println!("[MUTATE] Writing optimized code to source row {}...", SOURCE_ROW);
        
        for x in 0..self.width {
            let pixel_idx = (SOURCE_ROW * self.width + x) as u64;
            let offset = pixel_idx * std::mem::size_of::<Pixel>() as u64;
            
            if idx < chars.len() as usize {
                let ch = chars[idx] as u32;
                let pixel = Pixel {
                    r: ch,
                    g: ch,
                    b: ch,
                    a: 1,
                };
                self.queue.write_buffer(&self.buffer_a, offset, bytemuck::bytes_of(&pixel));
                idx += 1;
            } else {
                // Clear rest of row
                let nop = Pixel { r: 0, g: 0, b: 0, a: 0 };
                self.queue.write_buffer(&self.buffer_a, offset, bytemuck::bytes_of(&nop));
            }
        }
        
        println!("[MUTATE] Source row {} updated with {} chars", SOURCE_ROW, chars.len());
    }
    
    /// Convert pixel data to RGBA image
    pub fn to_rgba(&self, data: &[u8]) -> Vec<u8> {
        let pixel_count = (self.width * self.height) as usize;
        let mut rgba = Vec::with_capacity(pixel_count * 4);
        
        for i in 0..pixel_count {
            let offset = i * 16;
            let pixel: &Pixel = bytemuck::from_bytes(&data[offset..offset+16]);
            let [r, g, b, a] = pixel.to_rgba();
            rgba.extend_from_slice(&[r, g, b, a]);
        }
        
        rgba
    }
    
    pub fn save_frame(&self, data: &[u8], path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let rgba = self.to_rgba(data);
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(
            self.width,
            self.height,
            rgba,
        ).ok_or("Failed to create image")?;
        
        img.save(path)?;
        Ok(())
    }
}

impl Pixel {
    fn to_rgba(&self) -> [u8; 4] {
        if self.a == 254 {
            // Agent: use g, b for color
            [self.g.clamp(0, 255) as u8, self.b.clamp(0, 255) as u8, 200, 255]
        } else {
            [self.r.clamp(0, 255) as u8, self.g.clamp(0, 255) as u8, self.b.clamp(0, 255) as u8, 255]
        }
    }
}

// ============================================================================
// LLM Client
// ============================================================================

async fn call_llm(prompt: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    
    // Few-shot completion prompt for clean opcode output
    let few_shot_prefix = "VM Code (opcodes only):\n\
        Push 5, halt = 5 @\n\
        Push 1, push 2, add, print, halt = 1 2 + . @\n\
        Push 10, store x, halt = 10 x ! @\n\
        Load x, print, halt = X . @\n\
        Fibonacci 10 = 0a 1b 10i A B + : . b a I 1 - i I ? < @\n";
    
    let full_prompt = format!("{}{} =", few_shot_prefix, prompt);
    
    let response = client
        .post("http://localhost:1234/v1/completions")
        .json(&serde_json::json!({
            "model": "qwen3.5-27b",
            "prompt": full_prompt,
            "max_tokens": 50,
            "temperature": 0.1,
            "stop": ["\n", "Explanation", "Note", "This", "**", "1.", "2.", "3.", "Thinking", "Process"]
        }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    
    Ok(json["choices"][0]["text"]
        .as_str()
        .unwrap_or("ERROR: No response")
        .to_string())
}

/// Clean LLM response to only valid VM opcodes
/// Strips conversational filler and ensures proper termination
/// Bootstrap Guardian: Reject empty/junk responses
/// Handles Qwen 3.5 "Thinking Process" sections
fn clean_vm_code(raw: &str) -> String {
    let mut cleaned = raw.to_string();
    
    // Strip markdown bold/italic
    cleaned = cleaned.replace("**", "");
    cleaned = cleaned.replace("__", "");
    cleaned = cleaned.replace("*", "");
    cleaned = cleaned.replace("_", "");
    
    // Strip numbered lists (1. 2. 3. etc)
    let re = regex::Regex::new(r"\d+\.\s*").unwrap();
    cleaned = re.replace_all(&cleaned, "").to_string();
    
    // Strip "Thinking Process:" sections from reasoning models
    if let Some(tp_pos) = cleaned.find("Thinking Process:") {
        let after_tp = &cleaned[tp_pos..];
        let end_tp = after_tp.find("\n\n")
            .or_else(|| after_tp.find("Output:"))
            .or_else(|| after_tp.find("Result:"))
            .unwrap_or(after_tp.len().min(500));  // Cap thinking section
        let thinking_section = &after_tp[..end_tp];
        cleaned = cleaned.replace(thinking_section, "");
    }
    
    // Strip "Analyze" sections
    if let Some(a_pos) = cleaned.find("Analyze") {
        let end = cleaned[a_pos..].find("\n").unwrap_or(cleaned.len() - a_pos);
        cleaned = cleaned.replace(&cleaned[a_pos..a_pos+end.min(100)], "");
    }
    
    // Try to extract "Output:" pattern first (common in few-shot prompts)
    if let Some(output_pos) = cleaned.find("Output:") {
        let after_output = &cleaned[output_pos + 7..];
        let end_pos = after_output.find("\n\n").unwrap_or(after_output.len());
        return clean_tokens(&after_output[..end_pos]);
    }
    
    // Strip common conversational patterns
    let patterns = [
        "Geometry OS Kernel", "Constraint:", "Role:", "Valid Tokens:",
        "Output only", "VM Code Examples", "Thinking", "Process"
    ];
    for pattern in patterns.iter() {
        if cleaned.contains(pattern) {
            cleaned = cleaned.replace(pattern, "");
        }
    }
    
    clean_tokens(&cleaned)
}

fn clean_tokens(text: &str) -> String {
    // Valid ASCII VM characters
    // Numbers: 0-9
    // Math: + - * / 
    // Stack: . , : 
    // Control: @ (halt) & (jmp_response) ? (conditional)
    // Registers: A-Z a-z
    // Spatial: > < ^ v
    // Pixel: ! _
    // Whitespace: space
    
    let valid_chars: std::collections::HashSet<char> = 
        "0123456789+-*/.,:@&?<>^v!_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz "
        .chars().collect();
    
    let mut cleaned: String = text.chars()
        .filter(|c| valid_chars.contains(c) || c.is_whitespace())
        .collect();
    
    // Collapse whitespace
    cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    
    // Take only first 20 valid tokens
    let tokens: Vec<&str> = cleaned.split_whitespace().take(20).collect();
    
    if tokens.len() >= 2 {
        cleaned = tokens.join(" ");
    }
    
    // Ensure HALT at end
    if !cleaned.ends_with('@') {
        cleaned.push_str(" @");
    }
    
    // Bootstrap Guardian: Reject if too short
    if cleaned.len() < 3 {
        return "1 @".to_string();  // Minimal valid program
    }
    
    // Limit length to fit in response zone
    let max_chars = ((RESPONSE_END_ROW - RESPONSE_START_ROW + 1) * 256) as usize;
    if cleaned.len() > max_chars {
        // Truncate at last complete instruction
        let truncate_at = cleaned[..max_chars].rfind(' ').unwrap_or(max_chars);
        cleaned = cleaned[..truncate_at].to_string();
        if !cleaned.ends_with('@') {
            cleaned.push_str(" @");
        }
    }
    
    cleaned
}

// ============================================================================
// Main Loop
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), String> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║         NEURAL PIPE RUNNER - LLM in Framebuffer          ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Prompt Zone:  Rows 10-19                                ║");
    println!("║  Response Zone: Rows 20-39                               ║");
    println!("║  LM Studio:     localhost:1234                           ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    let width = 256u32;
    let height = 256u32;
    
    let mut runner = NeuralPipeRunner::new(width, height).await.map_err(|e| e.to_string())?;
    println!("[OK] GPU runner initialized ({}x{})", width, height);
    
    // Initialize with empty pixels
    let pixels: Vec<Pixel> = (0..width*height)
        .map(|_| Pixel { r: 0, g: 0, b: 0, a: 0 })
        .collect();
    runner.init_buffer(&pixels);
    
    // For testing: inject known-good VM code directly into response zone
    // This simulates what the LLM *should* generate
    let test_code = "1 2 + . @";  // Push 1, push 2, add, print, halt
    
    // The prompt that will be sent to LLM - Autogenic Evolution Seed
    let test_prompt = "Fibonacci 10";
    
    // First write the test code
    runner.write_response(test_code);
    println!("[TEST] Pre-injected test code at rows 20+: '{}'", test_code);
    
    // Then inject prompt (which adds JMP_RESPONSE at row 19, just before response zone)
    runner.inject_prompt(test_prompt);
    println!("[OK] Injected prompt: '{}'", test_prompt);
    
    let output_dir = std::path::Path::new("output");
    std::fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
    
    let start = Instant::now();
    let llm_response: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));
    let llm_pending: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    
    println!("[OK] Starting main loop (Ctrl+C to stop)");
    println!();
    
    loop {
        let time = start.elapsed().as_secs_f32();
        let (pixel_data, stats) = runner.step(time).map_err(|e| e.to_string())?;
        
        // Check for LLM request
        if stats.neural_pipe_signal == NEURAL_PIPE_REQUEST {
            // Check if we already have a response
            let response_guard = llm_response.read().await;
            if response_guard.is_none() && !llm_pending.load(Ordering::SeqCst) {
                drop(response_guard);
                llm_pending.store(true, Ordering::SeqCst);
                println!("[GEN] OP_GENERATE detected! Calling LLM...");
                
                // Spawn LLM call
                let prompt = test_prompt.to_string();
                let llm_response_clone = llm_response.clone();
                let llm_pending_clone = llm_pending.clone();
                tokio::spawn(async move {
                    match call_llm(&prompt).await {
                        Ok(response) => {
                            // Clean the response to valid VM opcodes
                            let cleaned = clean_vm_code(&response);
                            println!("[GEN] Raw response ({} chars)", response.len());
                            println!("[GEN] Cleaned VM code ({} chars)", cleaned.len());
                            
                            // Safe preview
                            let preview_len = cleaned.len().min(60);
                            let preview = if preview_len < cleaned.len() {
                                let mut end = preview_len;
                                while end > 0 && !cleaned.is_char_boundary(end) {
                                    end -= 1;
                                }
                                &cleaned[..end]
                            } else {
                                &cleaned
                            };
                            println!("[GEN] Code: {}...", preview);
                            
                            let mut guard = llm_response_clone.write().await;
                            *guard = Some(cleaned);
                        }
                        Err(e) => {
                            eprintln!("[GEN] Error: {}", e);
                            llm_pending_clone.store(false, Ordering::SeqCst);
                        }
                    }
                });
            }
        }
        
        // Check if LLM response is ready, write to framebuffer
        {
            let response_guard = llm_response.read().await;
            if let Some(ref response) = *response_guard {
                if llm_pending.load(Ordering::SeqCst) {
                    // Clone response before dropping guard
                    let response_clone = response.clone();
                    drop(response_guard);
                    
                    // Write response to framebuffer
                    runner.write_response(&response_clone);
                    println!("[GEN] Response written to rows 20-39!");
                    
                    // Self-mutation: Copy optimized code back to source
                    if SELF_MUTATE {
                        runner.mutate_source(&response_clone);
                    }
                    
                    // Safe preview (handle UTF-8 boundaries)
                    let preview_len = response_clone.len().min(100);
                    let preview = if preview_len < response_clone.len() {
                        // Find safe boundary
                        let mut end = preview_len;
                        while end > 0 && !response_clone.is_char_boundary(end) {
                            end -= 1;
                        }
                        &response_clone[..end]
                    } else {
                        &response_clone
                    };
                    println!("[GEN] Preview: {}...", preview);
                    
                    // Remove trigger pixel (prevent infinite loop)
                    runner.clear_trigger();
                    
                    // Clear the signal - GPU resumes execution!
                    runner.clear_signal();
                    println!("[GEN] Signal cleared - GPU unfrozen! (one-shot complete)");
                    
                    // Clear pending flag (response now in FB)
                    llm_pending.store(false, Ordering::SeqCst);
                    
                    // Mark response as consumed
                    let mut guard = llm_response.write().await;
                    *guard = None;
                }
            }
        }
        
        // Save frame every 10 frames
        if runner.frame % 10 == 0 {
            let frame_path = format!("output/neural_pipe_{:04}.png", runner.frame);
            runner.save_frame(&pixel_data, &frame_path).map_err(|e| e.to_string())?;
            
            // Check for OP_GENERATE in pixel data
            let has_generate = pixel_data.chunks(16).any(|chunk| {
                let r = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                r == OP_GENERATE
            });
            
            if has_generate && runner.frame % 100 == 0 {
                println!("Frame {}: OP_GENERATE active", runner.frame);
            }
        }
        
        // Cap at 60 FPS
        tokio::time::sleep(Duration::from_millis(16)).await;
    }
}
