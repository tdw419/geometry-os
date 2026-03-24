// GPU-VM: Programs run POWERED BY the framebuffer
//
// The GPU shader reads pixel values as instructions and executes them.
// The CPU does NOTHING except upload initial pixels and read back results.
// All computation happens on the GPU, reading from the pixel buffer.
//
// Proof: pixel data IS the computation.

use wgpu::*;
use wgpu::util::DeviceExt;

const TYPE_AGENT: u32 = 254;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Pixel { r: u32, g: u32, b: u32, a: u32 }

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Config { width: u32, height: u32, time: f32, frame: u32, mode: u32 }

impl Pixel {
    fn empty() -> Self { Self { r: 0, g: 0, b: 0, a: 0 } }
    fn inst(op: u32, g: u32, b: u32, a: u32) -> Self { Self { r: op, g, b, a } }
}

// VM opcodes (must match shader)
const VM_NOP: u32    = 0x00;
const VM_HALT: u32   = 0xFF;
const VM_LOAD: u32   = 0x01;
const VM_ADD: u32    = 0x02;
const VM_SUB: u32    = 0x03;
const VM_CMP: u32    = 0x0D;
const VM_MOV: u32    = 0x0E;
const VM_JMP: u32    = 0x10;
const VM_JLT: u32    = 0x14;
const VM_PRINT: u32  = 0x40;
const VM_PRINTI: u32 = 0x41;

fn assemble(source: &str) -> Vec<Pixel> {
    let mut insts = Vec::new();
    let mut labels: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    // Pass 1: labels
    let mut addr = 0;
    for line in source.lines() {
        let line = line.split(';').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        if line.ends_with(':') {
            labels.insert(line.trim_end_matches(':').to_string(), addr);
        } else { addr += 1; }
    }

    // Pass 2: assemble
    for line in source.lines() {
        let line = line.split(';').next().unwrap_or("").trim();
        if line.is_empty() || line.ends_with(':') { continue; }

        let parts: Vec<&str> = line.split_whitespace().collect();
        let op = parts[0].to_uppercase();
        let joined = parts[1..].join(" ");
        let args: Vec<String> = joined.split(',').map(|s| s.trim().to_string()).collect();

        let reg = |s: &str| -> u32 {
            let s = s.trim().to_lowercase();
            if s.starts_with('r') { s[1..].parse::<u32>().unwrap_or(0) & 0xF }
            else { 0 }
        };
        let imm = |s: &str| -> u32 {
            let s = s.trim();
            if s.starts_with("0x") { u32::from_str_radix(&s[2..], 16).unwrap_or(0) }
            else { s.parse::<u32>().unwrap_or(0) }
        };
        let label_addr = |s: &str| -> u32 {
            labels.get(s.trim()).copied().unwrap_or(0) as u32
        };

        let px = match op.as_str() {
            "NOP" => Pixel::inst(VM_NOP, 0, 0, 0),
            "HALT" => Pixel::inst(VM_HALT, 0, 0, 0),
            "LOAD" => {
                let d = reg(&args[0]);
                let v = imm(args.get(1).map(|s| s.as_str()).unwrap_or("0"));
                Pixel::inst(VM_LOAD, v & 0xFF, (v >> 8) & 0xFF, d)
            }
            "ADD" => Pixel::inst(VM_ADD, reg(&args[1]), reg(&args[2]), reg(&args[0])),
            "SUB" => Pixel::inst(VM_SUB, reg(&args[1]), reg(&args[2]), reg(&args[0])),
            "MOV" => Pixel::inst(VM_MOV, reg(&args[1]), 0, reg(&args[0])),
            "CMP" => Pixel::inst(VM_CMP, reg(&args[0]), reg(&args[1]), 0),
            "JMP" => { let a = label_addr(&args[0]); Pixel::inst(VM_JMP, a & 0xFF, (a >> 8) & 0xFF, 0) }
            "JLT" => { let a = label_addr(&args[0]); Pixel::inst(VM_JLT, a & 0xFF, (a >> 8) & 0xFF, 0) }
            "PRINT" => Pixel::inst(VM_PRINT, reg(&args[0]), 0, 0),
            "PRINTI" => Pixel::inst(VM_PRINTI, reg(&args[0]), 0, 0),
            _ => { eprintln!("Unknown: {}", op); Pixel::inst(VM_NOP, 0, 0, 0) }
        };
        insts.push(px);
    }
    insts
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let demo = std::env::args().nth(1).unwrap_or("count".to_string());

    let source = match demo.as_str() {
        "hello" => r#"
LOAD r0, 72   ; H
PRINT r0
LOAD r0, 101  ; e
PRINT r0
LOAD r0, 108  ; l
PRINT r0
PRINT r0      ; l
LOAD r0, 111  ; o
PRINT r0
LOAD r0, 32   ; space
PRINT r0
LOAD r0, 87   ; W
PRINT r0
LOAD r0, 111  ; o
PRINT r0
LOAD r0, 114  ; r
PRINT r0
LOAD r0, 108  ; l
PRINT r0
LOAD r0, 100  ; d
PRINT r0
HALT
"#,
        "count" => r#"
LOAD r0, 0    ; counter
LOAD r1, 21   ; limit
LOAD r2, 1    ; increment
loop:
PRINTI r0
LOAD r3, 32   ; space
PRINT r3
ADD r0, r0, r2
CMP r0, r1
JLT loop
HALT
"#,
        "fib" => r#"
LOAD r0, 0    ; a = 0
LOAD r1, 1    ; b = 1
LOAD r4, 10   ; limit
LOAD r5, 0    ; counter
loop:
PRINTI r0
LOAD r3, 32
PRINT r3
MOV r2, r1    ; temp = b
ADD r1, r0, r1 ; b = a + b
MOV r0, r2    ; a = temp
LOAD r3, 1
ADD r5, r5, r3
CMP r5, r4
JLT loop
HALT
"#,
        _ => {
            if std::path::Path::new(&demo).exists() {
                &std::fs::read_to_string(&demo)?
            } else {
                eprintln!("Usage: gpu-vm [hello|count|fib|file.asm]");
                std::process::exit(1);
            }
        }
    };

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║     GPU-VM: Computation Powered by the Framebuffer      ║");
    println!("║                                                          ║");
    println!("║  The CPU uploads pixels. The GPU executes them.          ║");
    println!("║  Pixel data IS the computation.                          ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    // Assemble program into pixels
    let program = assemble(source);
    println!("Program: {} instructions (pixels)", program.len());

    // Show program as pixels
    println!("\n── Program (each pixel = 1 instruction) ──");
    for (i, px) in program.iter().enumerate() {
        let name = match px.r {
            0x00 => "NOP", 0x01 => "LOAD", 0x02 => "ADD", 0x03 => "SUB",
            0x0D => "CMP", 0x0E => "MOV", 0x10 => "JMP", 0x14 => "JLT",
            0x40 => "PRINT", 0x41 => "PRINTI", 0xFF => "HALT", _ => "???",
        };
        println!("  {:3}: pixel({:02x},{:02x},{:02x},{:02x}) = {} g={} b={} a={}",
            i, px.r, px.g, px.b, px.a, name, px.g, px.b, px.a);
    }

    // Setup GPU
    let width: u32 = 480;
    let height: u32 = 240;

    let instance = Instance::new(InstanceDescriptor { backends: Backends::all(), ..Default::default() });
    let adapter = futures::executor::block_on(
        instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            compatible_surface: None, force_fallback_adapter: false,
        })
    ).ok_or("No GPU")?;

    println!("\nGPU: {}", adapter.get_info().name);

    let (device, queue) = futures::executor::block_on(
        adapter.request_device(&DeviceDescriptor {
            label: Some("GPU-VM"), required_features: Features::empty(),
            required_limits: Limits::default(),
        }, None)
    )?;

    // Load the GPU-VM shader (not the old pixel-agent shader)
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("GPU-VM Shader"),
        source: ShaderSource::Wgsl(include_str!("../../gpu-vm-shader.wgsl").into()),
    });

    // Create pipeline
    let bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("BGL"),
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

    let pl = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("PL"), bind_group_layouts: &[&bgl], push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("GPU-VM Pipeline"), layout: Some(&pl), module: &shader, entry_point: "main",
    });

    let buf_size = (width * height) as u64 * 16;
    let buffer_a = device.create_buffer(&BufferDescriptor {
        label: Some("A"), size: buf_size,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
        mapped_at_creation: false });
    let buffer_b = device.create_buffer(&BufferDescriptor {
        label: Some("B"), size: buf_size,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
        mapped_at_creation: false });
    let staging = device.create_buffer(&BufferDescriptor {
        label: Some("Staging"), size: buf_size,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false });
    let bc_buf = device.create_buffer(&BufferDescriptor {
        label: Some("BC"), size: 256 * 4,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST, mapped_at_creation: false });
    let const_buf = device.create_buffer(&BufferDescriptor {
        label: Some("C"), size: 64 * 4,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST, mapped_at_creation: false });

    // === BUILD THE FRAMEBUFFER (pixels = program + state) ===
    let mut fb = vec![Pixel::empty(); (width * height) as usize];

    // Pixel (0,0) = CPU state: r=running, g=PC_low, b=PC_high, a=flags
    fb[0] = Pixel::inst(0, 0, 0, 0); // PC=0, flags=0, running

    // Pixels (1..16, 0) = registers (start at 0)
    for i in 1..=16 {
        fb[i] = Pixel::inst(0x01, 0, 0, TYPE_AGENT);
    }

    // Pixel (17, 0) = output cursor
    fb[17] = Pixel::inst(0x02, 0, 0, TYPE_AGENT);

    // Program memory starts at row 2
    for (i, inst) in program.iter().enumerate() {
        let x = i % width as usize;
        let y = 2 + i / width as usize;
        fb[y * width as usize + x] = *inst;
    }

    // Upload to GPU
    let data: Vec<u8> = fb.iter().flat_map(|p| bytemuck::bytes_of(p).to_vec()).collect();
    queue.write_buffer(&buffer_a, 0, &data);

    println!("\n── Executing on GPU ──");
    println!("CPU does NOTHING. GPU reads pixels, executes instructions, writes pixels.\n");

    // Run for enough frames to complete the program
    // Each frame = 1 instruction
    let max_frames = (program.len() as u32 + 50) * 3; // Extra for loops

    for frame in 0..max_frames {
        let config = Config { width, height, time: frame as f32 / 30.0, frame, mode: 0 };
        let config_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cfg"), contents: bytemuck::bytes_of(&config), usage: BufferUsages::UNIFORM,
        });

        let bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("BG"), layout: &bgl,
            entries: &[
                BindGroupEntry { binding: 0, resource: buffer_a.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: buffer_b.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: bc_buf.as_entire_binding() },
                BindGroupEntry { binding: 3, resource: const_buf.as_entire_binding() },
                BindGroupEntry { binding: 4, resource: config_buf.as_entire_binding() },
            ],
        });

        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Enc") });
        { let mut pass = enc.begin_compute_pass(&ComputePassDescriptor { label: Some("Pass"), timestamp_writes: None });
          pass.set_pipeline(&pipeline);
          pass.set_bind_group(0, &bg, &[]);
          pass.dispatch_workgroups((width + 15) / 16, (height + 15) / 16, 1);
        }
        enc.copy_buffer_to_buffer(&buffer_b, 0, &buffer_a, 0, buf_size);
        enc.copy_buffer_to_buffer(&buffer_b, 0, &staging, 0, buf_size);
        queue.submit(std::iter::once(enc.finish()));

        // Read back to check if halted
        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(MapMode::Read, move |r| { tx.send(r).unwrap(); });
        device.poll(Maintain::Wait);
        rx.recv()??;
        let raw = slice.get_mapped_range().to_vec();
        staging.unmap();

        // Check CPU pixel (0,0)
        let cpu_r = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
        let cpu_g = u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]);
        let cpu_b = u32::from_le_bytes([raw[8], raw[9], raw[10], raw[11]]);
        let pc = cpu_g | (cpu_b << 8);

        if cpu_r == VM_HALT {
            println!("GPU halted after {} frames (instructions)", frame + 1);
            println!("Final PC: {}\n", pc);

            // Read output area (last 10 rows)
            println!("── Output (from framebuffer pixels) ──");
            let out_start = (height - 10) as usize;
            for row in 0..10usize {
                let y = out_start + row;
                let mut line = String::new();
                let mut has_content = false;
                for x in 0..width as usize {
                    let offset = (y * width as usize + x) * 16;
                    let ch = u32::from_le_bytes([raw[offset], raw[offset+1], raw[offset+2], raw[offset+3]]);
                    let a = u32::from_le_bytes([raw[offset+12], raw[offset+13], raw[offset+14], raw[offset+15]]);
                    if a == TYPE_AGENT && ch >= 32 && ch < 127 {
                        line.push(ch as u8 as char);
                        has_content = true;
                    } else if has_content {
                        break;
                    }
                }
                if has_content {
                    println!("  {}", line);
                }
            }

            // Read registers
            println!("\n── Registers (from framebuffer pixels) ──");
            for i in 0..16u32 {
                let offset = ((i + 1) * 16) as usize;
                let g = u32::from_le_bytes([raw[offset+4], raw[offset+5], raw[offset+6], raw[offset+7]]);
                let b = u32::from_le_bytes([raw[offset+8], raw[offset+9], raw[offset+10], raw[offset+11]]);
                let val = g | (b << 8);
                if val != 0 {
                    print!("  r{}={}", i, val);
                }
            }
            println!("\n");

            println!("══════════════════════════════════════════════════════════");
            println!("  PROVEN: The GPU read pixel values as instructions,");
            println!("  executed them, and wrote results back as pixels.");
            println!("  The CPU did ZERO computation. Only uploaded and read back.");
            println!("  Pixel data IS the computation.");
            println!("══════════════════════════════════════════════════════════");
            break;
        }
    }

    Ok(())
}
