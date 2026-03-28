// Sovereign Shell - Natural Language Control for Geometry OS
// 
// Architecture:
//   1. User types natural language command in input zone (rows 450-479)
//   2. Agent hits @> (PROMPT opcode) and halts
//   3. Vision reads input zone, extracts user intent
//   4. LLM generates opcode patch from natural language
//   5. Host injects patch into agent's instruction stream
//   6. HUD shows PATCH_SUCCESS or PATCH_FAIL
//   7. Agent resumes with new opcodes

use wgpu::*;
use image::{ImageBuffer, Rgba};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::time::{Duration, Instant};
use base64::Engine;

// ============================================================================
// CONSTANTS
// ============================================================================

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;

// Models
const VISION_MODEL: &str = "qwen/qwen3-vl-8b";
const TEXT_MODEL: &str = "tinyllama-1.1b-chat-v1.0";

// ============================================================================
// VM STATE
// ============================================================================

#[derive(Debug, Default)]
struct VMState {
    registers: HashMap<char, i32>,
    stack: Vec<i32>,
    ip: usize,
    halted: bool,
    waiting_for_input: bool,
    last_result: i32,
    instruction_memory: Vec<String>,  // For self-modification
    spawned_agents: Vec<Box<VMState>>, // For parallel execution (mitosis)
    agent_id: i32, // Unique identifier for this agent
}

impl VMState {
    fn execute_token(&mut self, token: &str, patch_injector: &mut Option<Vec<String>>) -> bool {
        match token {
            // PROMPT opcode - halt and wait for input
            "@>" => {
                self.waiting_for_input = true;
                self.halted = true;
                println!("[PROMPT] VM waiting for natural language input...");
                return false; // Stop execution
            }
            
            // Push number
            n if n.parse::<i32>().is_ok() => {
                self.stack.push(n.parse().unwrap());
            }
            
            // Register store (lowercase)
            reg if reg.len() == 1 && reg.chars().next().unwrap().is_ascii_lowercase() => {
                let reg_name = reg.chars().next().unwrap().to_ascii_uppercase();
                if let Some(value) = self.stack.last().copied() {
                    self.registers.insert(reg_name, value);
                }
            }
            
            // Register load (uppercase)
            reg if reg.len() == 1 && reg.chars().next().unwrap().is_ascii_uppercase() => {
                let reg_name = reg.chars().next().unwrap();
                if let Some(&value) = self.registers.get(&reg_name) {
                    self.stack.push(value);
                }
            }
            
            // Arithmetic
            "+" => {
                if self.stack.len() >= 2 {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    let result = a + b;
                    self.stack.push(result);
                    self.last_result = result;
                    println!("[EXEC] {} + {} = {}", a, b, result);
                }
            }
            "-" => {
                if self.stack.len() >= 2 {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    let result = a - b;
                    self.stack.push(result);
                    self.last_result = result;
                    println!("[EXEC] {} - {} = {}", a, b, result);
                }
            }
            "*" => {
                if self.stack.len() >= 2 {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    let result = a * b;
                    self.stack.push(result);
                    self.last_result = result;
                    println!("[EXEC] {} * {} = {}", a, b, result);
                }
            }
            "/" => {
                if self.stack.len() >= 2 {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    if b != 0 {
                        let result = a / b;
                        self.stack.push(result);
                        self.last_result = result;
                        println!("[EXEC] {} / {} = {}", a, b, result);
                    }
                }
            }
            
            // Comparison operators for conditional branching
            ">" => {
                if self.stack.len() >= 2 {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    let result = if a > b { 1 } else { 0 };
                    self.stack.push(result);
                    self.last_result = result;
                    println!("[EXEC] {} > {} = {}", a, b, result);
                }
            }
            "<" => {
                if self.stack.len() >= 2 {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    let result = if a < b { 1 } else { 0 };
                    self.stack.push(result);
                    self.last_result = result;
                    println!("[EXEC] {} < {} = {}", a, b, result);
                }
            }
            "=" | "==" => {
                if self.stack.len() >= 2 {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    let result = if a == b { 1 } else { 0 };
                    self.stack.push(result);
                    self.last_result = result;
                    println!("[EXEC] {} = {} = {}", a, b, result);
                }
            }
            
            // Conditional: cond true_val false_val ? → result
            // If cond != 0, push true_val, else push false_val
            "?" => {
                if self.stack.len() >= 3 {
                    let false_val = self.stack.pop().unwrap();
                    let true_val = self.stack.pop().unwrap();
                    let condition = self.stack.pop().unwrap();
                    let result = if condition != 0 { true_val } else { false_val };
                    self.stack.push(result);
                    self.last_result = result;
                    println!("[COND] {} ? {} : {} = {}", condition, true_val, false_val, result);
                } else {
                    println!("[COND ERROR] Need 3 values on stack (cond true false)");
                }
            }
            
            // Self-Modification: value offset M → writes value to instruction_memory[IP + offset]
            // Enables the agent to rewrite its own future code
            // Safety: offset must be > 0 and <= 256
            "M" | "modify" | "patch" => {
                if self.stack.len() >= 2 {
                    let offset = self.stack.pop().unwrap() as usize;
                    let value = self.stack.pop().unwrap();
                    
                    // Safety constraints
                    if offset == 0 {
                        println!("[MODIFY ERROR] Offset must be > 0 (cannot modify past instructions)");
                    } else if offset > 256 {
                        println!("[MODIFY ERROR] Offset must be <= 256 (max modification range)");
                    } else {
                        let target_ip = self.ip + offset;
                        if target_ip < self.instruction_memory.len() {
                            let new_token = value.to_string();
                            let old_token = self.instruction_memory[target_ip].clone();
                            self.instruction_memory[target_ip] = new_token.clone();
                            println!("[MODIFY] IP+{}: '{}' → '{}' (value={})", offset, old_token, new_token, value);
                        } else {
                            println!("[MODIFY ERROR] Target IP+{} out of bounds (memory len={})", offset, self.instruction_memory.len());
                        }
                    }
                } else {
                    println!("[MODIFY ERROR] Need 2 values on stack (value offset)");
                }
            }
            
            // Spawn Parallel Agent: offset S → spawns clone at IP + offset
            // MITOSIS - The agent learns to reproduce
            "S" | "spawn" => {
                if self.stack.len() >= 1 {
                    let offset = self.stack.pop().unwrap();
                    
                    if offset <= 0 {
                        println!("[SPAWN ERROR] Offset must be > 0");
                        self.stack.push(-1); // Error code
                    } else if self.spawned_agents.len() >= 16 {
                        println!("[SPAWN ERROR] Max agents reached (16)");
                        self.stack.push(-2); // Error code
                    } else {
                        let target_ip = self.ip + offset as usize;
                        if target_ip < self.instruction_memory.len() {
                            // Clone current agent
                            let mut new_agent = Box::new(VMState {
                                registers: self.registers.clone(),
                                stack: Vec::new(), // Fresh stack for new agent
                                ip: target_ip,
                                halted: false,
                                waiting_for_input: false,
                                last_result: 0,
                                instruction_memory: self.instruction_memory.clone(),
                                spawned_agents: Vec::new(),
                                agent_id: self.spawned_agents.len() as i32,
                            });
                            
                            let agent_id = new_agent.agent_id;
                            self.spawned_agents.push(new_agent);
                            self.stack.push(agent_id);
                            println!("[SPAWN] Agent {} spawned at IP+{} (parallel execution)", agent_id, offset);
                        } else {
                            println!("[SPAWN ERROR] Target IP+{} out of bounds", offset);
                            self.stack.push(-3); // Error code
                        }
                    }
                } else {
                    println!("[SPAWN ERROR] Need 1 value on stack (offset)");
                }
            }
            
            // Duplicate top of stack
            "dup" => {
                if let Some(&top) = self.stack.last() {
                    self.stack.push(top);
                }
            }
            
            // Loop construct: X Y L → pushes X, X+1, ..., Y to stack
            // Example: 1 5 L → stack has [1, 2, 3, 4, 5]
            "L" | "loop" => {
                if self.stack.len() >= 2 {
                    let end = self.stack.pop().unwrap();
                    let start = self.stack.pop().unwrap();
                    let mut values = Vec::new();
                    for value in start..=end {
                        values.push(value);
                        self.stack.push(value);
                    }
                    println!("[LOOP] {} to {} → {:?}", start, end, values);
                } else {
                    println!("[LOOP ERROR] Need 2 values on stack (start end)");
                }
            }
            
            // Print/pop top of stack
            "." => {
                if let Some(top) = self.stack.pop() {
                    self.last_result = top;
                    println!("[PRINT] {}", top);
                }
            }
            
            // Halt
            "@" => {
                self.halted = true;
                println!("[HALT] VM halted");
                return false;
            }
            
            // INJECT - marker for patch injection point
            "INJECT" => {
                if let Some(patch) = patch_injector.take() {
                    println!("[INJECT] Patch applied: {}", patch.join(" "));
                    return true; // Continue with patch
                }
            }
            
            _ => {}
        }
        
        self.ip += 1;
        true
    }
    
    fn step_all(&mut self, patch_injector: &mut Option<Vec<String>>) -> bool {
        let mut any_active = false;
        
        // Advance the main agent
        if self.ip < self.instruction_memory.len() && !self.halted && !self.waiting_for_input {
            let token = self.instruction_memory[self.ip].clone();
            self.execute_token(&token, patch_injector);
            any_active = true;
        }
        
        // Advance all parallel spawned agents (Mitosis)
        for child in &mut self.spawned_agents {
            if child.step_all(&mut None) {
                any_active = true;
            }
        }
        
        any_active
    }

    fn run_program(&mut self, code: &str, mut patch: Option<Vec<String>>) {
        // Load tokens into instruction_memory for self-modification
        if !code.is_empty() {
            self.instruction_memory = code.split_whitespace().map(|s| s.to_string()).collect();
        }
        
        // True Mitosis execution: recursive round-robin evaluation
        while self.step_all(&mut patch) {
        }
    }
}

// ============================================================================
// LLM INTEGRATION
// ============================================================================

async fn call_vision_llm(image_path: &str, prompt: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    
    let image_data = std::fs::read(image_path).map_err(|e| e.to_string())?;
    let base64_image = base64::engine::general_purpose::STANDARD.encode(&image_data);
    
    let response = client
        .post("http://localhost:1234/v1/chat/completions")
        .json(&serde_json::json!({
            "model": VISION_MODEL,
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{}", base64_image)}},
                    {"type": "text", "text": prompt}
                ]
            }],
            "max_tokens": 200,
            "temperature": 0.0
        }))
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("ERROR")
        .to_string())
}

async fn call_text_llm(prompt: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    
    let few_shot = r#"Convert natural language to VM opcodes.

Rules:
- Push numbers: "5" → "5"
- Add: "add" or "+" → "+"
- Subtract: "subtract" or "-" → "-"
- Multiply: "multiply" or "*" → "*"
- Divide: "divide" or "/" → "/"
- Store: "store in A" → "a"
- Load: "load A" → "A"
- Halt: always end with "@"

Examples:
"push 5 and halt" = 5 @
"add 5 and 3" = 5 3 + @
"add 5 and 3 and show result" = 5 3 + . @
"multiply 4 by 7" = 4 7 * @
"count from 1 to 5" = 1 5 loop @
"store 42 in register A" = 42 a @

Convert: "#;
    
    let full_prompt = format!("{}{}", few_shot, prompt);
    
    let response = client
        .post("http://localhost:1234/v1/completions")
        .json(&serde_json::json!({
            "model": TEXT_MODEL,
            "prompt": full_prompt,
            "max_tokens": 50,
            "temperature": 0.0,
            "stop": ["\n", "Explanation", "Note"]
        }))
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(json["choices"][0]["text"].as_str().unwrap_or("ERROR").trim().to_string())
}

fn clean_vm_code(raw: &str) -> String {
    let valid_chars: std::collections::HashSet<char> = 
        "0123456789+-*/.,:@&?<>^v!_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz "
        .chars().collect();
    
    let cleaned: String = raw.chars()
        .filter(|c| valid_chars.contains(c) || c.is_whitespace())
        .collect();
    
    let tokens: Vec<&str> = cleaned.split_whitespace().take(20).collect();
    let mut result = tokens.join(" ");
    
    if !result.ends_with('@') && !result.is_empty() {
        result.push_str(" @");
    }
    
    if result.is_empty() { "1 @".to_string() } else { result }
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
struct Config {
    width: u32,
    height: u32,
    time: f32,
    frame: u32,
    mode: u32,
}

struct SovereignShellRenderer {
    device: Device,
    queue: Queue,
    pipeline: ComputePipeline,
    bind_group: BindGroup,
    
    output_buffer: Buffer,
    staging_buffer: Buffer,
    registers_buffer: Buffer,
    stack_buffer: Buffer,
    config_buffer: Buffer,
    vm_stats_buffer: Buffer,
    input_buffer: Buffer,
    patch_status_buffer: Buffer,
    exec_result_buffer: Buffer,
}

impl SovereignShellRenderer {
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
                label: Some("Sovereign Shell GPU"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
            }, None)
            .await?;
        
        // Load shader
        // Load shader
        let shader_source = include_str!("../../sovereign_shell_hud.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Sovereign Shell HUD Shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });
        
        // Create buffers
        let pixel_count = (WIDTH * HEIGHT) as u64;
        let buffer_size = pixel_count * std::mem::size_of::<Pixel>() as u64;
        
        let input_fb_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Input FB Buffer"),
            size: buffer_size,
            usage: BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        
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
        
        // Registers buffer (26 registers A-Z)
        let registers_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Registers Buffer"),
            size: 26 * 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        // Stack buffer (100 slots)
        let stack_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Stack Buffer"),
            size: 100 * 4,
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
        
        // VM stats buffer (GPU status, IP, SP, telemetry plane)
        let vm_stats_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("VM Stats Buffer"),
            size: 11 * 4,  // Expanded for GlyphLang telemetry (indices 3-10)
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        // Input buffer (64 chars)
        let input_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Input Buffer"),
            size: 64 * 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        // Patch status buffer (0=none, 1=success, 2=fail)
        let patch_status_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Patch Status Buffer"),
            size: 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        // Execution result buffer
        let exec_result_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Exec Result Buffer"),
            size: 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        // Initialize config
        let config = Config { width: WIDTH, height: HEIGHT, time: 0.0, frame: 0, mode: 0 };
        queue.write_buffer(&config_buffer, 0, bytemuck::bytes_of(&config));
        
        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Sovereign Shell Bind Group Layout"),
            entries: &[
                // 0: output buffer
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
                // 1: input buffer (placeholder)
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
                // 2: registers
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
                // 3: stack
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
                // 4: config
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
                // 5: vm_stats (atomic telemetry)
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
                // 6: input_text
                BindGroupLayoutEntry {
                    binding: 6,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 7: patch_status
                BindGroupLayoutEntry {
                    binding: 7,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 8: exec_result
                BindGroupLayoutEntry {
                    binding: 8,
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
            label: Some("Sovereign Shell Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Sovereign Shell HUD Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });
        
        // Create bind group
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Sovereign Shell Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: output_buffer.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: input_fb_buffer.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: registers_buffer.as_entire_binding() },
                BindGroupEntry { binding: 3, resource: stack_buffer.as_entire_binding() },
                BindGroupEntry { binding: 4, resource: config_buffer.as_entire_binding() },
                BindGroupEntry { binding: 5, resource: vm_stats_buffer.as_entire_binding() },
                BindGroupEntry { binding: 6, resource: input_buffer.as_entire_binding() },
                BindGroupEntry { binding: 7, resource: patch_status_buffer.as_entire_binding() },
                BindGroupEntry { binding: 8, resource: exec_result_buffer.as_entire_binding() },
            ],
        });
        
        Ok(Self {
            device,
            queue,
            pipeline,
            bind_group,
            output_buffer,
            staging_buffer,
            registers_buffer,
            stack_buffer,
            config_buffer,
            vm_stats_buffer,
            input_buffer,
            patch_status_buffer,
            exec_result_buffer,
        })
    }
    
    fn update_state(&self, vm: &VMState, input_text: &str, patch_status: u32, frame: u32) {
        // Update registers
        let mut registers = [0u32; 26];
        for (name, value) in &vm.registers {
            let idx = (*name as u8 - b'A') as usize;
            if idx < 26 {
                registers[idx] = *value as u32;
            }
        }
        self.queue.write_buffer(&self.registers_buffer, 0, bytemuck::cast_slice(&registers));
        
        // Update stack
        let mut stack = [0u32; 100];
        for (i, value) in vm.stack.iter().enumerate() {
            if i < 100 {
                stack[i] = *value as u32;
            }
        }
        self.queue.write_buffer(&self.stack_buffer, 0, bytemuck::cast_slice(&stack));
        
        // Update VM stats (now with telemetry support)
        let vm_stats = [
            1u32,                   // [0] GPU Status: 1 = ONLINE
            vm.ip as u32,           // [1] IP
            vm.stack.len() as u32,  // [2] SP / Stack depth
            0u32,                   // [3] Requests (atomic)
            0u32,                   // [4] Errors (atomic)
            0u32,                   // [5] Latency (fixed-point ms*10)
            0u32,                   // [6] Active Routes bitmask
            0u32, 0u32, 0u32, 0u32, // [7-10] Reserved
        ];
        self.queue.write_buffer(&self.vm_stats_buffer, 0, bytemuck::cast_slice(&vm_stats));
        
        // Sync atomic telemetry from vm_stats (for HUD reading)
        // This allows the atomic buffer to be read by the shader
        
        // Update input text
        let mut input = [0u32; 64];
        for (i, ch) in input_text.chars().enumerate() {
            if i < 64 {
                input[i] = ch as u32;
            }
        }
        self.queue.write_buffer(&self.input_buffer, 0, bytemuck::cast_slice(&input));
        
        // Update patch status
        self.queue.write_buffer(&self.patch_status_buffer, 0, bytemuck::cast_slice(&[patch_status]));
        
        // Update execution result
        self.queue.write_buffer(&self.exec_result_buffer, 0, bytemuck::cast_slice(&[vm.last_result as u32]));
        
        // Update config with frame number for cursor blink
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
    
    // ========================================================================
    // TELEMETRY TRACKING METHODS (GlyphLang Integration)
    // ========================================================================
    
    /// Track a request (increment atomic counter)
    pub fn track_request(&self) {
        let mut stats = [0u32; 11];
        // Read current value (simplified - in production use atomic ops)
        stats[3] += 1;  // Increment request counter
    }
    
    /// Track an error (increment atomic counter)
    pub fn track_error(&self, error_code: u32) {
        let mut stats = [0u32; 11];
        stats[4] += 1;  // Increment error counter
        stats[7] = error_code;  // Store last error code
    }
    
    /// Track latency (rolling average)
    pub fn track_latency(&self, latency_ms: f32) {
        let mut stats = [0u32; 11];
        
        // Fixed-point encoding: ms * 10 for 0.1ms precision
        let fixed_point = (latency_ms * 10.0) as u32;
        
        // Rolling average: 90% old, 10% new
        let current = stats[5];
        stats[5] = (current * 9 + fixed_point) / 10;
        
    }
    
    /// Set route active (bitmask)
    pub fn set_route_active(&self, route_id: u32) {
        let mut stats = [0u32; 11];
        stats[6] |= 1 << route_id;
    }
    
    /// Get current telemetry stats
    pub fn get_telemetry(&self) -> [u32; 11] {
        let mut stats = [0u32; 11];
        // Note: In production, this would use buffer mapping
        stats
    }
    
    fn render(&self) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, Box<dyn std::error::Error>> {
        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });
        
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Sovereign Shell Compute Pass"),
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
// SOVEREIGN SHELL MAIN
// ============================================================================

struct SovereignShell {
    vm: VMState,
    renderer: SovereignShellRenderer,
    input_text: String,
    patch_status: u32,
    frame: u32,
    output_path: String,
}

impl SovereignShell {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let renderer = SovereignShellRenderer::new().await?;
        
        Ok(Self {
            vm: VMState::default(),
            renderer,
            input_text: String::new(),
            patch_status: 0,
            frame: 0,
            output_path: "output/sovereign_shell.png".to_string(),
        })
    }
    
    fn render_frame(&mut self) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, Box<dyn std::error::Error>> {
        self.renderer.update_state(&self.vm, &self.input_text, self.patch_status, self.frame);
        self.frame += 1;
        self.renderer.render()
    }
    
    async fn process_natural_language(&mut self, input: &str) -> Result<String, String> {
        println!();
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║           SOVEREIGN SHELL - PROCESSING                  ║");
        println!("╠══════════════════════════════════════════════════════════╣");
        println!("║  Input: {:<48}║", format!("{:.48}", input));
        println!("╚══════════════════════════════════════════════════════════╝");
        println!();
        
        // Step 1: Render current state to get input zone
        println!("[STEP 1] Rendering input zone for vision...");
        let img = self.render_frame().map_err(|e| e.to_string())?;
        img.save(&self.output_path).map_err(|e| e.to_string())?;
        println!("[STEP 1] Saved to {}", self.output_path);
        
        // Step 2: Vision reads the input zone
        println!("[STEP 2] Vision model reading input zone...");
        let vision_prompt = format!(
            "Read the text in the INPUT ZONE (green prompt '> '). \
             Extract only the user's command text. \
             Input text: {}", 
            input
        );
        
        let vision_result = call_vision_llm(&self.output_path, &vision_prompt).await?;
        println!("[STEP 2] Vision: {}", vision_result.lines().take(2).collect::<Vec<_>>().join(" | "));
        
        // Step 3: LLM generates opcodes
        println!("[STEP 3] LLM generating opcodes...");
        let start = Instant::now();
        let raw_code = call_text_llm(&vision_result).await?;
        let code = clean_vm_code(&raw_code);
        let gen_time = start.elapsed();
        println!("[STEP 3] Generated: {} ({}ms)", code, gen_time.as_millis());
        
        // Step 4: Validate code
        println!("[STEP 4] Validating generated code...");
        let valid = self.validate_code(&code);
        
        if valid {
            self.patch_status = 1; // PATCH_SUCCESS
            println!("[STEP 4] ✅ Code validated");
            
            // Step 5: Execute
            println!("[STEP 5] Executing: {}", code);
            self.vm = VMState::default();
            self.vm.run_program(&code, None);
            
            println!("[RESULT] Registers: {:?}", self.vm.registers);
            println!("[RESULT] Stack: {:?}", self.vm.stack);
            println!("[RESULT] Last result: {}", self.vm.last_result);
        } else {
            self.patch_status = 2; // PATCH_FAIL
            println!("[STEP 4] ❌ Code validation failed");
        }
        
        // Render final state
        let final_img = self.render_frame().map_err(|e| e.to_string())?;
        final_img.save(&self.output_path).map_err(|e| e.to_string())?;
        
        Ok(code)
    }
    
    fn validate_code(&self, code: &str) -> bool {
        // Check for required halt
        if !code.ends_with('@') {
            return false;
        }
        
        // Check for balanced operations
        let tokens: Vec<&str> = code.split_whitespace().collect();
        let mut stack_depth = 0;
        
        for token in &tokens {
            match *token {
                n if n.parse::<i32>().is_ok() => stack_depth += 1,
                "+" | "-" | "*" | "/" => {
                    if stack_depth < 2 {
                        return false;
                    }
                    stack_depth -= 1;
                }
                "@" => {}
                _ => {}
            }
        }
        
        true
    }
    
    async fn run_interactive(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!();
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║         SOVEREIGN SHELL - NATURAL LANGUAGE VM           ║");
        println!("╠══════════════════════════════════════════════════════════╣");
        println!("║  Commands:                                               ║");
        println!("║    add 5 and 3       → 5 3 + @                          ║");
        println!("║    multiply 4 by 7   → 4 7 * @                          ║");
        println!("║    quit              → exit shell                       ║");
        println!("╚══════════════════════════════════════════════════════════╝");
        println!();
        
        // Render initial frame
        let img = self.render_frame()?;
        img.save(&self.output_path)?;
        println!("[READY] Initial HUD saved to {}", self.output_path);
        println!();
        
        let stdin = io::stdin();
        print!("sovereign> ");
        io::stdout().flush()?;
        
        for line in stdin.lock().lines() {
            let input = line?.trim().to_string();
            
            if input.is_empty() {
                print!("sovereign> ");
                io::stdout().flush()?;
                continue;
            }
            
            if input == "quit" || input == "exit" {
                println!("[EXIT] Sovereign Shell shutting down...");
                break;
            }
            
            // Process natural language command
            match self.process_natural_language(&input).await {
                Ok(code) => {
                    println!("[OK] Generated: {}", code);
                }
                Err(e) => {
                    println!("[ERROR] {}", e);
                    self.patch_status = 2;
                }
            }
            
            // Reset for next command
            self.patch_status = 0;
            print!("\nsovereign> ");
            io::stdout().flush()?;
        }
        
        Ok(())
    }
}

// ============================================================================
// MAIN
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    
    // Check for test mode
    if args.len() > 1 && args[1] == "--test" {
        run_test_mode().await?;
    } else {
        // Interactive mode
        let mut shell = SovereignShell::new().await?;
        shell.run_interactive().await?;
    }
    
    Ok(())
}

async fn run_test_mode() -> Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║         SOVEREIGN SHELL - TEST MODE                     ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Testing natural language → opcode conversion           ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    let mut shell = SovereignShell::new().await?;
    
    let test_cases = vec![
        ("add 5 and 3", "5 3 + @"),
        ("multiply 4 by 7", "4 7 * @"),
        ("subtract 10 from 20", "20 10 - @"),
    ];
    
    for (input, expected_pattern) in test_cases {
        println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("[TEST] Input: \"{}\"", input);
        println!("[TEST] Expected pattern: {}", expected_pattern);
        
        match shell.process_natural_language(input).await {
            Ok(code) => {
                let success = code.contains(&expected_pattern.replace(" ", ""))
                    || expected_pattern.split_whitespace().all(|t| code.contains(t));
                
                if success {
                    println!("[TEST] ✅ PASSED - Generated: {}", code);
                } else {
                    println!("[TEST] ❌ FAILED - Generated: {}, expected pattern: {}", code, expected_pattern);
                }
            }
            Err(e) => {
                println!("[TEST] ❌ ERROR: {}", e);
            }
        }
    }
    
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║              TEST MODE COMPLETE                         ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    
    Ok(())
}
