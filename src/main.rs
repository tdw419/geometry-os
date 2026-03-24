// GPU Pixel Formula Runner
// Compiles formulas → bytecode, runs on GPU via wgpu

use wgpu::*;
use wgpu::util::DeviceExt;
use image::{ImageBuffer, Rgba};
use std::time::Instant;

// Bytecode opcodes (must match shader)
const OP_PUSH_X: u32 = 0x01;
const OP_PUSH_Y: u32 = 0x02;
const OP_PUSH_T: u32 = 0x03;
const OP_PUSH_CONST: u32 = 0x04;
const OP_ADD: u32 = 0x10;
const OP_SUB: u32 = 0x11;
const OP_MUL: u32 = 0x12;
const OP_DIV: u32 = 0x13;
const OP_MOD: u32 = 0x14;
const OP_POW: u32 = 0x15;
const OP_SIN: u32 = 0x20;
const OP_COS: u32 = 0x21;
const OP_TAN: u32 = 0x22;
const OP_SQRT: u32 = 0x23;
const OP_ABS: u32 = 0x24;
const OP_FLOOR: u32 = 0x25;
const OP_CEIL: u32 = 0x26;
const OP_FRACT: u32 = 0x27;
const OP_MIN: u32 = 0x28;
const OP_MAX: u32 = 0x29;
const OP_CLAMP: u32 = 0x2A;
const OP_MIX: u32 = 0x2B;
const OP_NOISE: u32 = 0x30;
const OP_RGB: u32 = 0xF0;
const OP_HSV: u32 = 0xF1;

/// Formula compiler - converts text to bytecode
pub struct FormulaCompiler {
    constants: Vec<f32>,
    bytecode: Vec<u32>,
}

impl FormulaCompiler {
    pub fn new() -> Self {
        Self {
            constants: Vec::new(),
            bytecode: Vec::new(),
        }
    }
    
    /// Compile a formula string to bytecode
    /// Examples:
    ///   "x * y" → push x, push y, mul, rgb
    ///   "sin(x * 6.28) * 0.5 + 0.5" → gradient
    ///   "noise(x * 10, y * 10)" → noise pattern
    pub fn compile(&mut self, formula: &str) -> Result<Vec<u32>, String> {
        self.constants.clear();
        self.bytecode.clear();
        
        // Simple recursive descent parser
        self.parse_expr(formula.trim())?;
        
        // Add RGB output if not present
        if !self.bytecode.ends_with(&[OP_RGB]) && !self.bytecode.ends_with(&[OP_HSV]) {
            // Default: use result as grayscale
            self.bytecode.push(OP_PUSH_CONST);
            self.constants.push(1.0); // r = result
            let r_idx = (self.constants.len() - 1) as u32;
            self.bytecode.push(r_idx);
            
            self.bytecode.push(OP_PUSH_CONST);
            self.constants.push(1.0); // g = result
            let g_idx = (self.constants.len() - 1) as u32;
            self.bytecode.push(g_idx);
            
            self.bytecode.push(OP_PUSH_CONST);
            self.constants.push(1.0); // b = result
            let b_idx = (self.constants.len() - 1) as u32;
            self.bytecode.push(b_idx);
            
            self.bytecode.push(OP_MUL);
            self.bytecode.push(OP_MUL);
            self.bytecode.push(OP_RGB);
        }
        
        Ok(self.bytecode.clone())
    }
    
    fn parse_expr(&mut self, expr: &str) -> Result<(), String> {
        let expr = expr.trim();
        
        // Check for functions FIRST (before operator splitting)
        if expr.starts_with("sin(") {
            self.parse_func(OP_SIN, &expr[4..])?;
        } else if expr.starts_with("cos(") {
            self.parse_func(OP_COS, &expr[4..])?;
        } else if expr.starts_with("tan(") {
            self.parse_func(OP_TAN, &expr[4..])?;
        } else if expr.starts_with("sqrt(") {
            self.parse_func(OP_SQRT, &expr[5..])?;
        } else if expr.starts_with("abs(") {
            self.parse_func(OP_ABS, &expr[4..])?;
        } else if expr.starts_with("floor(") {
            self.parse_func(OP_FLOOR, &expr[6..])?;
        } else if expr.starts_with("ceil(") {
            self.parse_func(OP_CEIL, &expr[5..])?;
        } else if expr.starts_with("fract(") {
            self.parse_func(OP_FRACT, &expr[6..])?;
        } else if expr.starts_with("noise(") {
            self.parse_noise(&expr[6..])?;
        } else if expr.starts_with("rgb(") {
            self.parse_rgb(&expr[4..])?;
        } else if expr.starts_with("hsv(") {
            self.parse_hsv(&expr[4..])?;
        } else if expr.starts_with('(') && expr.ends_with(')') {
            // Parenthesized expression - parse inner
            self.parse_expr(&expr[1..expr.len()-1])?;
        } else if expr.contains('+') {
            self.parse_binary_op(OP_ADD, expr, '+')?;
        } else if expr.contains('-') && !expr.starts_with('-') {
            self.parse_binary_op(OP_SUB, expr, '-')?;
        } else if expr.contains('*') {
            self.parse_binary_op(OP_MUL, expr, '*')?;
        } else if expr.contains('/') {
            self.parse_binary_op(OP_DIV, expr, '/')?;
        } else if expr == "x" {
            self.bytecode.push(OP_PUSH_X);
        } else if expr == "y" {
            self.bytecode.push(OP_PUSH_Y);
        } else if expr == "t" {
            self.bytecode.push(OP_PUSH_T);
        } else {
            // Must be a constant
            let val: f32 = expr.parse()
                .map_err(|_| format!("Failed to parse: {}", expr))?;
            self.constants.push(val);
            self.bytecode.push(OP_PUSH_CONST);
            self.bytecode.push((self.constants.len() - 1) as u32);
        }
        
        Ok(())
    }
    
    fn parse_func(&mut self, op: u32, args: &str) -> Result<(), String> {
        // Find matching closing paren (handle nesting)
        let mut depth = 1;
        let mut end = 0;
        for (i, c) in args.chars().enumerate() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        
        if end == 0 {
            return Err("Missing closing paren".to_string());
        }
        
        let inner = &args[..end];
        self.parse_expr(inner)?;
        self.bytecode.push(op);
        Ok(())
    }
    
    fn parse_noise(&mut self, args: &str) -> Result<(), String> {
        // Find matching closing paren
        let mut depth = 1;
        let mut end = 0;
        for (i, c) in args.chars().enumerate() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        
        if end == 0 {
            return Err("Missing closing paren".to_string());
        }
        
        let inner = &args[..end];
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() != 2 {
            return Err("noise() requires 2 arguments".to_string());
        }
        self.parse_expr(parts[0].trim())?;
        self.parse_expr(parts[1].trim())?;
        self.bytecode.push(OP_NOISE);
        Ok(())
    }
    
    fn parse_rgb(&mut self, args: &str) -> Result<(), String> {
        // Find matching closing paren
        let mut depth = 1;
        let mut end = 0;
        for (i, c) in args.chars().enumerate() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        
        if end == 0 {
            return Err("Missing closing paren".to_string());
        }
        
        let inner = &args[..end];
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() != 3 {
            return Err("rgb() requires 3 arguments".to_string());
        }
        self.parse_expr(parts[0].trim())?; // r
        self.parse_expr(parts[1].trim())?; // g
        self.parse_expr(parts[2].trim())?; // b
        self.bytecode.push(OP_RGB);
        Ok(())
    }
    
    fn parse_hsv(&mut self, args: &str) -> Result<(), String> {
        // Find matching closing paren
        let mut depth = 1;
        let mut end = 0;
        for (i, c) in args.chars().enumerate() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        
        if end == 0 {
            return Err("Missing closing paren".to_string());
        }
        
        let inner = &args[..end];
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() != 3 {
            return Err("hsv() requires 3 arguments".to_string());
        }
        self.parse_expr(parts[0].trim())?; // h
        self.parse_expr(parts[1].trim())?; // s
        self.parse_expr(parts[2].trim())?; // v
        self.bytecode.push(OP_HSV);
        Ok(())
    }
    
    fn parse_binary_op(&mut self, op: u32, expr: &str, _sep: char) -> Result<(), String> {
        // Find operator (respecting precedence would need a real parser)
        let pos = expr.find(|c| c == '+' || c == '-' || c == '*' || c == '/')
            .ok_or_else(|| "No operator found".to_string())?;
        
        let left = &expr[..pos];
        let right = &expr[pos+1..];
        
        self.parse_expr(left)?;
        self.parse_expr(right)?;
        self.bytecode.push(op);
        
        Ok(())
    }
    
    pub fn constants(&self) -> &[f32] {
        &self.constants
    }
}

/// GPU Runner - executes bytecode on GPU
pub struct GpuRunner {
    device: Device,
    queue: Queue,
    pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Config {
    width: u32,
    height: u32,
    bytecode_len: u32,
    time: f32,
}

impl GpuRunner {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Setup wgpu
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
                label: Some("Pixel Formula GPU"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
            }, None)
            .await?;
        
        // Load shader
        let shader_source = include_str!("../pixel-formula-shader.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Pixel Formula Shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });
        
        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Bind Group Layout"),
            entries: &[
                // Bytecode buffer
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
                // Constants buffer
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
                // Output buffer
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Config uniform
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
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Pixel Formula Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });
        
        Ok(Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
        })
    }
    
    /// Run formula on GPU, return RGBA image
    pub fn run(&self, bytecode: &[u32], constants: &[f32], width: u32, height: u32, time: f32) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, Box<dyn std::error::Error>> {
        let start = Instant::now();
        
        // Create buffers
        let bytecode_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Bytecode Buffer"),
            contents: bytemuck::cast_slice(bytecode),
            usage: BufferUsages::STORAGE,
        });
        
        let constants_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Constants Buffer"),
            contents: bytemuck::cast_slice(constants),
            usage: BufferUsages::STORAGE,
        });
        
        let output_size = (width * height) as u64 * 4;
        let output_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("Output Buffer"),
            size: output_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        
        let config = Config {
            width,
            height,
            bytecode_len: bytecode.len() as u32,
            time,
        };
        
        let config_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Config Buffer"),
            contents: bytemuck::bytes_of(&config),
            usage: BufferUsages::UNIFORM,
        });
        
        // Create bind group
        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: bytecode_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: constants_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: output_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: config_buffer.as_entire_binding(),
                },
            ],
        });
        
        // Create command encoder
        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Command Encoder"),
        });
        
        // Run compute pass
        {
            let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Compute Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            
            let workgroups_x = (width + 15) / 16;
            let workgroups_y = (height + 15) / 16;
            compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }
        
        // Read back results
        let staging_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("Staging Buffer"),
            size: output_size,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_size);
        
        self.queue.submit(std::iter::once(encoder.finish()));
        
        // Wait for completion
        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
        self.device.poll(Maintain::Wait);
        rx.recv()??;
        
        // Convert to image
        let data = buffer_slice.get_mapped_range().to_vec();
        staging_buffer.unmap();
        
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for chunk in data.chunks(4) {
            let rgba = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let r = (rgba & 0xFF) as u8;
            let g = ((rgba >> 8) & 0xFF) as u8;
            let b = ((rgba >> 16) & 0xFF) as u8;
            let a = ((rgba >> 24) & 0xFF) as u8;
            pixels.extend_from_slice(&[r, g, b, a]);
        }
        
        let elapsed = start.elapsed();
        println!("GPU render: {:.2}ms ({}x{} = {} pixels)", 
            elapsed.as_secs_f64() * 1000.0, width, height, width * height);
        
        ImageBuffer::from_raw(width, height, pixels)
            .ok_or_else(|| "Failed to create image".into())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger
    env_logger::init();
    
    // Example formulas
    let formulas = vec![
        ("Gradient", "x"),
        ("Vertical gradient", "y"),
        ("Diagonal", "x * y"),
        ("Sine wave", "sin(x * 6.28)"),
        ("Radial", "sqrt(x * x + y * y)"),
        ("Checkerboard", "floor(x * 8) + floor(y * 8)"),
    ];
    
    println!("Initializing GPU...");
    let runner = futures::executor::block_on(GpuRunner::new())?;
    
    for (name, formula) in formulas {
        println!("\n=== {} ===", name);
        println!("Formula: {}", formula);
        
        let mut compiler = FormulaCompiler::new();
        let bytecode = compiler.compile(formula)?;
        let constants = compiler.constants().to_vec();
        
        println!("Bytecode: {:?}", bytecode);
        println!("Constants: {:?}", constants);
        
        let image = runner.run(&bytecode, &constants, 480, 240, 0.0)?;
        
        let filename = format!("output/{}.png", name.to_lowercase().replace(' ', "_"));
        image.save(&filename)?;
        println!("Saved: {}", filename);
    }
    
    println!("\n✓ Done!");
    Ok(())
}
