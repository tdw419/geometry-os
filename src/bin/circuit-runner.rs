// Circuit Runner — The Abstraction Layer
//
// Loads circuit definitions from JSON, compiles to pixel buffer,
// runs on GPU, reads back state as human-readable text.
//
// This replaces raw pixel debugging with:
//   circuit.json → [compile] → GPU → [read back] → state report
//
// Usage:
//   ./target/release/circuit-runner circuits/test-and-gate.json
//   ./target/release/circuit-runner circuits/test-and-gate.json --frames 200

use std::collections::HashMap;

// ===== OPCODES (must match shader) =====
const OP_NOP: u32 = 0x00;
const OP_IDLE: u32 = 0x01;
const OP_EMIT_SIGNAL: u32 = 0x20;
const OP_WIRE: u32 = 0x22;
const OP_CLOCK: u32 = 0x23;
const OP_SIGNAL_SOURCE: u32 = 0x24;
const OP_AND: u32 = 0x30;
const OP_XOR: u32 = 0x31;
const OP_OR: u32 = 0x32;
const OP_NOT: u32 = 0x33;
const OP_PORTAL_IN: u32 = 0x50;
const OP_PORTAL_OUT: u32 = 0x51;

const TYPE_EMPTY: u32 = 0;
const TYPE_AGENT: u32 = 254;

// ===== PIXEL =====
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Pixel {
    r: u32, // opcode
    g: u32, // signal / register A
    b: u32, // register B
    a: u32, // type flag
}

impl Pixel {
    fn empty() -> Self {
        Self { r: 0, g: 0, b: 0, a: TYPE_EMPTY }
    }
    fn agent(opcode: u32, g: u32, b: u32) -> Self {
        Self { r: opcode, g, b, a: TYPE_AGENT }
    }
    fn is_active(&self) -> bool {
        self.a == TYPE_AGENT
    }
    fn signal_high(&self) -> bool {
        self.g > 128
    }
    fn opcode_name(&self) -> &'static str {
        match self.r {
            0x00 => "NOP",
            0x01 => "IDLE",
            0x20 => "EMIT",
            0x22 => "WIRE",
            0x23 => "CLOCK",
            0x24 => "SOURCE",
            0x30 => "AND",
            0x31 => "XOR",
            0x32 => "OR",
            0x33 => "NOT",
            0x50 => "PORTAL_IN",
            0x51 => "PORTAL_OUT",
            _ => "???",
        }
    }
}

// ===== CIRCUIT COMPILER =====
struct CircuitCompiler {
    width: u32,
    height: u32,
    pixels: Vec<Pixel>,
    probes: Vec<Probe>,
    component_names: HashMap<(u32, u32), String>,
}

struct Probe {
    id: String,
    label: String,
    x: u32,
    y: u32,
}

impl CircuitCompiler {
    fn new(width: u32, height: u32) -> Self {
        let size = (width * height) as usize;
        Self {
            width,
            height,
            pixels: vec![Pixel::empty(); size],
            probes: Vec::new(),
            component_names: HashMap::new(),
        }
    }

    fn idx(&self, x: u32, y: u32) -> usize {
        (y * self.width + x) as usize
    }

    fn place(&mut self, x: u32, y: u32, pixel: Pixel, name: &str) {
        if x < self.width && y < self.height {
            let idx = self.idx(x, y);
            self.pixels[idx] = pixel;
            self.component_names.insert((x, y), name.to_string());
        } else {
            eprintln!("WARNING: component '{}' at ({},{}) out of bounds ({}x{})",
                name, x, y, self.width, self.height);
        }
    }

    fn compile_json(&mut self, json: &serde_json::Value) -> Result<(), String> {
        let components = json["components"].as_array()
            .ok_or("Missing 'components' array")?;

        for comp in components {
            let id = comp["id"].as_str().unwrap_or("unnamed");
            let ctype = comp["type"].as_str().ok_or(format!("Component '{}' missing type", id))?;

            match ctype {
                "clock" => {
                    let x = comp["x"].as_u64().ok_or("clock missing x")? as u32;
                    let y = comp["y"].as_u64().ok_or("clock missing y")? as u32;
                    let period = comp["period"].as_u64().unwrap_or(60) as u32;
                    // Clock: r=OP_CLOCK, g=0 (signal output), b=period
                    self.place(x, y, Pixel::agent(OP_CLOCK, 0, period), id);
                    println!("  CLOCK '{}' at ({},{}) period={}", id, x, y, period);
                }
                "source" => {
                    let x = comp["x"].as_u64().ok_or("source missing x")? as u32;
                    let y = comp["y"].as_u64().ok_or("source missing y")? as u32;
                    self.place(x, y, Pixel::agent(OP_SIGNAL_SOURCE, 0, 0), id);
                    println!("  SOURCE '{}' at ({},{})", id, x, y);
                }
                "wire" => {
                    let from = comp["from"].as_array().ok_or("wire missing from")?;
                    let to = comp["to"].as_array().ok_or("wire missing to")?;
                    let fx = from[0].as_u64().ok_or("wire from[0]")? as u32;
                    let fy = from[1].as_u64().ok_or("wire from[1]")? as u32;
                    let tx = to[0].as_u64().ok_or("wire to[0]")? as u32;
                    let ty = to[1].as_u64().ok_or("wire to[1]")? as u32;

                    let dir = comp["direction"].as_str().unwrap_or("east");
                    let wire_op = match dir {
                        "south" => OP_WIRE, // We'll handle direction via layout
                        _ => OP_WIRE,
                    };

                    // Place wire pixels along the path
                    if fy == ty {
                        // Horizontal wire (east)
                        let (start, end) = if fx <= tx { (fx, tx) } else { (tx, fx) };
                        for x in start..=end {
                            let name = format!("{}[{}]", id, x - start);
                            self.place(x, fy, Pixel::agent(wire_op, 0, 0), &name);
                        }
                        println!("  WIRE '{}' horizontal ({},{}) → ({},{}) [{} pixels]",
                            id, fx, fy, tx, ty, end - start + 1);
                    } else if fx == tx {
                        // Vertical wire (south) — wires read from north neighbor
                        // For vertical, we still use WIRE but signal comes from north (y-1)
                        // Need a vertical wire opcode... for now use WIRE which reads west
                        // HACK: use EMIT_SIGNAL for vertical since WIRE only reads west
                        let (start, end) = if fy <= ty { (fy, ty) } else { (ty, fy) };
                        for y in start..=end {
                            let name = format!("{}[{}]", id, y - start);
                            // For vertical wires, we need something that reads north
                            // Let's use a simple approach: vertical wire = reads north neighbor
                            self.place(fx, y, Pixel::agent(OP_WIRE, 0, 1), &name); // b=1 means vertical
                        }
                        println!("  WIRE '{}' vertical ({},{}) → ({},{}) [{} pixels]",
                            id, fx, fy, tx, ty, end - start + 1);
                    } else {
                        return Err(format!("Wire '{}' must be horizontal or vertical", id));
                    }
                }
                "and" => {
                    let x = comp["x"].as_u64().ok_or("and missing x")? as u32;
                    let y = comp["y"].as_u64().ok_or("and missing y")? as u32;
                    self.place(x, y, Pixel::agent(OP_AND, 0, 0), id);
                    println!("  AND '{}' at ({},{})", id, x, y);
                }
                "xor" => {
                    let x = comp["x"].as_u64().ok_or("xor missing x")? as u32;
                    let y = comp["y"].as_u64().ok_or("xor missing y")? as u32;
                    self.place(x, y, Pixel::agent(OP_XOR, 0, 0), id);
                    println!("  XOR '{}' at ({},{})", id, x, y);
                }
                "or" => {
                    let x = comp["x"].as_u64().ok_or("or missing x")? as u32;
                    let y = comp["y"].as_u64().ok_or("or missing y")? as u32;
                    self.place(x, y, Pixel::agent(OP_OR, 0, 0), id);
                    println!("  OR '{}' at ({},{})", id, x, y);
                }
                "not" => {
                    let x = comp["x"].as_u64().ok_or("not missing x")? as u32;
                    let y = comp["y"].as_u64().ok_or("not missing y")? as u32;
                    self.place(x, y, Pixel::agent(OP_NOT, 0, 0), id);
                    println!("  NOT '{}' at ({},{})", id, x, y);
                }
                other => {
                    return Err(format!("Unknown component type: '{}'", other));
                }
            }
        }

        // Compile probes
        if let Some(probes) = json["probes"].as_array() {
            for p in probes {
                self.probes.push(Probe {
                    id: p["id"].as_str().unwrap_or("").to_string(),
                    label: p["label"].as_str().unwrap_or("").to_string(),
                    x: p["x"].as_u64().unwrap_or(0) as u32,
                    y: p["y"].as_u64().unwrap_or(0) as u32,
                });
            }
        }

        Ok(())
    }

    /// Count placed components
    fn component_count(&self) -> usize {
        self.pixels.iter().filter(|p| p.a == TYPE_AGENT).count()
    }
}

// ===== STATE READER =====
struct StateReader {
    width: u32,
    _height: u32,
}

impl StateReader {
    fn new(width: u32, height: u32) -> Self {
        Self { width, _height: height }
    }

    fn read_pixel(&self, data: &[u8], x: u32, y: u32) -> Pixel {
        let idx = (y * self.width + x) as usize;
        let offset = idx * 16;
        if offset + 16 > data.len() {
            return Pixel::empty();
        }
        Pixel {
            r: u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]),
            g: u32::from_le_bytes([data[offset+4], data[offset+5], data[offset+6], data[offset+7]]),
            b: u32::from_le_bytes([data[offset+8], data[offset+9], data[offset+10], data[offset+11]]),
            a: u32::from_le_bytes([data[offset+12], data[offset+13], data[offset+14], data[offset+15]]),
        }
    }

    fn report_probes(&self, data: &[u8], probes: &[Probe], frame: u32) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Frame {:4} │", frame));
        for p in probes {
            let px = self.read_pixel(data, p.x, p.y);
            let signal = if px.signal_high() { "HIGH" } else { " LOW" };
            let active = if px.is_active() { "●" } else { "○" };
            lines.push(format!("  {} {:12} ({:2},{:2}) {:6} │ g={:3} op={} a={}",
                active, p.label, p.x, p.y, signal, px.g, px.opcode_name(), px.a));
        }
        lines.join("\n")
    }

    fn report_hex_region(&self, data: &[u8], x: u32, y: u32, w: u32, h: u32) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Region ({},{}) {}x{}:", x, y, w, h));
        for dy in 0..h {
            let mut row = format!("  y={:3} │", y + dy);
            for dx in 0..w {
                let px = self.read_pixel(data, x + dx, y + dy);
                if px.is_active() {
                    row.push_str(&format!(" {:02x}:{:3}", px.r, px.g));
                } else {
                    row.push_str("   ·   ");
                }
            }
            lines.push(row);
        }
        lines.join("\n")
    }
}

// ===== GPU RUNNER (minimal, reuses existing infrastructure) =====
// We include the PixelUniverse from agent_main but stripped to essentials

use wgpu::*;
use wgpu::util::DeviceExt;
use image::{ImageBuffer, Rgba};

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Config {
    width: u32,
    height: u32,
    time: f32,
    frame: u32,
    mode: u32,
}

struct GpuRunner {
    device: Device,
    queue: Queue,
    pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
    buffer_a: Buffer,
    buffer_b: Buffer,
    staging_buffer: Buffer,
    bytecode_buffer: Buffer,
    constants_buffer: Buffer,
    width: u32,
    height: u32,
    frame: u32,
}

impl GpuRunner {
    async fn new(width: u32, height: u32) -> Result<Self, Box<dyn std::error::Error>> {
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
            .ok_or("No GPU adapter found")?;

        println!("GPU: {}", adapter.get_info().name);

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: Some("Circuit Runner GPU"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
            }, None)
            .await?;

        let shader_source = include_str!("../../pixel-agent-shader.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Pixel Agent Shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry { binding: 0, visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer { ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false, min_binding_size: None }, count: None },
                BindGroupLayoutEntry { binding: 1, visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer { ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false, min_binding_size: None }, count: None },
                BindGroupLayoutEntry { binding: 2, visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer { ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false, min_binding_size: None }, count: None },
                BindGroupLayoutEntry { binding: 3, visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer { ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false, min_binding_size: None }, count: None },
                BindGroupLayoutEntry { binding: 4, visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer { ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false, min_binding_size: None }, count: None },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Circuit Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });

        let buffer_size = (width * height) as u64 * 16; // 16 bytes per pixel

        let buffer_a = device.create_buffer(&BufferDescriptor {
            label: Some("Buffer A"), size: buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false });
        let buffer_b = device.create_buffer(&BufferDescriptor {
            label: Some("Buffer B"), size: buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false });
        let staging_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Staging"), size: buffer_size,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false });
        let bytecode_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Bytecode"), size: 256 * 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false });
        let constants_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Constants"), size: 64 * 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false });

        Ok(Self {
            device, queue, pipeline, bind_group_layout,
            buffer_a, buffer_b, staging_buffer,
            bytecode_buffer, constants_buffer,
            width, height, frame: 0,
        })
    }

    fn load_pixels(&self, pixels: &[Pixel]) {
        let data: Vec<u8> = pixels.iter()
            .flat_map(|p| bytemuck::bytes_of(p).to_vec())
            .collect();
        self.queue.write_buffer(&self.buffer_a, 0, &data);
    }

    fn step(&mut self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let time = self.frame as f32 / 30.0;
        let config = Config {
            width: self.width,
            height: self.height,
            time,
            frame: self.frame,
            mode: 0, // agent mode
        };

        let config_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Config"),
            contents: bytemuck::bytes_of(&config),
            usage: BufferUsages::UNIFORM,
        });

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

        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Encoder"),
        });

        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Compute"), timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups((self.width + 15) / 16, (self.height + 15) / 16, 1);
        }

        let buffer_size = (self.width * self.height) as u64 * 16;
        encoder.copy_buffer_to_buffer(&self.buffer_b, 0, &self.staging_buffer, 0, buffer_size);
        encoder.copy_buffer_to_buffer(&self.buffer_b, 0, &self.buffer_a, 0, buffer_size);

        self.queue.submit(std::iter::once(encoder.finish()));

        let buffer_slice = self.staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(MapMode::Read, move |r| { tx.send(r).unwrap(); });
        self.device.poll(Maintain::Wait);
        rx.recv()??;

        let data = buffer_slice.get_mapped_range().to_vec();
        self.staging_buffer.unmap();

        self.frame += 1;
        Ok(data)
    }

    fn save_png(&self, data: &[u8], filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        let pixel_count = (self.width * self.height) as usize;
        let mut rgba = Vec::with_capacity(pixel_count * 4);

        for i in 0..pixel_count {
            let offset = i * 16;
            if offset + 16 <= data.len() {
                let r = u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]);
                let g = u32::from_le_bytes([data[offset+4], data[offset+5], data[offset+6], data[offset+7]]);
                let b = u32::from_le_bytes([data[offset+8], data[offset+9], data[offset+10], data[offset+11]]);
                let a = u32::from_le_bytes([data[offset+12], data[offset+13], data[offset+14], data[offset+15]]);

                // Color scheme: signal HIGH = bright green, LOW = dark, empty = black
                if a == TYPE_AGENT {
                    let signal = g > 128;
                    match r {
                        0x23 => { // CLOCK
                            if signal { rgba.extend_from_slice(&[255, 255, 0, 255]); }  // Yellow HIGH
                            else { rgba.extend_from_slice(&[80, 80, 0, 255]); }          // Dark yellow LOW
                        }
                        0x22 => { // WIRE
                            if signal { rgba.extend_from_slice(&[0, 200, 255, 255]); }   // Cyan HIGH
                            else { rgba.extend_from_slice(&[0, 40, 60, 255]); }           // Dark cyan LOW
                        }
                        0x30 => { // AND
                            if signal { rgba.extend_from_slice(&[0, 255, 0, 255]); }     // Green HIGH
                            else { rgba.extend_from_slice(&[0, 60, 0, 255]); }            // Dark green LOW
                        }
                        0x31 => { // XOR
                            if signal { rgba.extend_from_slice(&[255, 0, 255, 255]); }   // Magenta HIGH
                            else { rgba.extend_from_slice(&[60, 0, 60, 255]); }            // Dark magenta LOW
                        }
                        0x32 => { // OR
                            if signal { rgba.extend_from_slice(&[255, 128, 0, 255]); }   // Orange HIGH
                            else { rgba.extend_from_slice(&[60, 30, 0, 255]); }
                        }
                        0x33 => { // NOT
                            if signal { rgba.extend_from_slice(&[255, 0, 0, 255]); }     // Red HIGH
                            else { rgba.extend_from_slice(&[60, 0, 0, 255]); }
                        }
                        0x24 => { // SOURCE
                            rgba.extend_from_slice(&[255, 255, 255, 255]);                // White
                        }
                        _ => {
                            // Generic agent
                            let gi = g.clamp(0, 255) as u8;
                            rgba.extend_from_slice(&[gi, gi, 200, 255]);
                        }
                    }
                } else {
                    rgba.extend_from_slice(&[0, 0, 0, 255]); // Empty = black
                }
            }
        }

        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(
            self.width, self.height, rgba
        ).ok_or("Failed to create image")?;
        img.save(filename)?;
        Ok(())
    }
}

// ===== MAIN =====
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: circuit-runner <circuit.json> [--frames N]");
        eprintln!("Example: circuit-runner circuits/test-and-gate.json --frames 120");
        std::process::exit(1);
    }

    let circuit_path = &args[1];
    let num_frames: u32 = args.iter()
        .position(|a| a == "--frames")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(120);

    // Load circuit
    println!("╔══════════════════════════════════════════════════╗");
    println!("║         GEOMETRY OS — Circuit Runner             ║");
    println!("╚══════════════════════════════════════════════════╝\n");

    let json_str = std::fs::read_to_string(circuit_path)?;
    let json: serde_json::Value = serde_json::from_str(&json_str)?;

    let name = json["name"].as_str().unwrap_or("unnamed");
    let desc = json["description"].as_str().unwrap_or("");
    let width = json["width"].as_u64().unwrap_or(480) as u32;
    let height = json["height"].as_u64().unwrap_or(240) as u32;

    println!("Circuit: {}", name);
    println!("Description: {}", desc);
    println!("Grid: {}x{} ({} pixels)\n", width, height, width * height);

    // Compile circuit
    println!("Compiling circuit...");
    let mut compiler = CircuitCompiler::new(width, height);
    compiler.compile_json(&json)?;
    println!("\n  {} active pixels placed\n", compiler.component_count());

    // Pre-hex dump (what we're loading)
    println!("Pre-hex (compiled circuit):");
    let reader = StateReader::new(width, height);
    let pre_data: Vec<u8> = compiler.pixels.iter()
        .flat_map(|p| bytemuck::bytes_of(p).to_vec())
        .collect();

    // Show region around the circuit
    println!("{}\n", reader.report_hex_region(&pre_data, 8, 4, 25, 8));

    // Initialize GPU
    println!("Initializing GPU...");
    let mut gpu = futures::executor::block_on(GpuRunner::new(width, height))?;
    gpu.load_pixels(&compiler.pixels);

    std::fs::create_dir_all("output")?;

    // Run simulation
    println!("\nRunning {} frames...\n", num_frames);
    println!("─────────────────────────────────────────────────────────");

    let mut waveform: Vec<Vec<bool>> = compiler.probes.iter().map(|_| Vec::new()).collect();

    for frame in 0..num_frames {
        let data = gpu.step()?;

        // Record waveform
        for (i, probe) in compiler.probes.iter().enumerate() {
            let px = reader.read_pixel(&data, probe.x, probe.y);
            waveform[i].push(px.signal_high());
        }

        // Print probe state every 10 frames
        if frame % 10 == 0 {
            println!("{}", reader.report_probes(&data, &compiler.probes, frame));

            // Save PNG
            let filename = format!("output/circuit_{:04}.png", frame);
            gpu.save_png(&data, &filename)?;
        }
    }

    // Final state
    let final_data = gpu.step()?;
    println!("\n─────────────────────────────────────────────────────────");
    println!("\nFinal state:");
    println!("{}", reader.report_probes(&final_data, &compiler.probes, num_frames));

    // Post-hex dump
    println!("\nPost-hex (after {} frames):", num_frames);
    println!("{}", reader.report_hex_region(&final_data, 8, 4, 25, 8));

    // Waveform display
    println!("\nWaveform (every 2 frames):");
    println!("─────────────────────────────────────────────────────────");
    for (i, probe) in compiler.probes.iter().enumerate() {
        let wave: String = waveform[i].iter().step_by(2)
            .map(|&high| if high { '█' } else { '░' })
            .collect();
        println!("  {:12} │{}│", probe.label, wave);
    }
    println!("  {:12} │{}│", "frame", (0..waveform[0].len()).step_by(2)
        .map(|f| if f % 10 == 0 { '┊' } else { ' ' })
        .collect::<String>());
    println!("─────────────────────────────────────────────────────────");

    // Save final PNG
    gpu.save_png(&final_data, "output/circuit_final.png")?;
    println!("\nImages saved to output/circuit_*.png");

    Ok(())
}
