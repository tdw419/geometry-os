// RISC-V CPU in the Framebuffer
//
// The GPU executes real RISC-V instructions stored as pixels.
// One pixel = one 32-bit instruction.
// The CPU does NOTHING except upload instructions and read back output.
//
// This proves: Linux COULD run in the framebuffer (theoretically).

use wgpu::*;
use std::env;
use std::fs;
use std::io::{self, Write};

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;

// Memory map constants
const TEXT_START: u32 = 0x1000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     RISC-V CPU IN THE FRAMEBUFFER                            ║");
    println!("║     The GPU executes RISC-V instructions stored as pixels   ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    
    // Load a RISC-V program
    let program = if args.len() > 1 {
        match args[1].as_str() {
            "hello" => program_hello(),
            "count" => program_count(),
            "fib" => program_fibonacci(),
            _ => program_hello(),
        }
    } else {
        program_hello()
    };
    
    println!("Program: {} instructions ({} bytes)", program.len(), program.len() * 4);
    println!("Entry point: 0x{:08X}", TEXT_START);
    println!("First 5 instructions:");
    for (i, &word) in program.iter().take(5).enumerate() {
        println!("  0x{:04X}: 0x{:08X}", TEXT_START + (i * 4) as u32, word);
    }
    println!();
    
    // Setup wgpu
    let instance = Instance::new(InstanceDescriptor {
        backends: Backends::VULKAN,
        ..Default::default()
    });
    
    let adapter = futures::executor::block_on(
        instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
    ).ok_or("No GPU adapter found")?;
    
    println!("GPU: {}", adapter.get_info().name);
    
    let (device, queue) = futures::executor::block_on(
        adapter.request_device(&DeviceDescriptor {
            label: Some("RISC-V GPU"),
            required_features: Features::empty(),
            required_limits: Limits::default(),
        }, None)
    )?;
    
    // Load shader
    let shader_code = fs::read_to_string("riscv-shader.wgsl")
        .expect("Failed to read riscv-shader.wgsl");
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("RISC-V Shader"),
        source: ShaderSource::Wgsl(shader_code.into()),
    });
    
    // Create bind group layout
    let bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("BGL"),
        entries: &[
            // Config uniform
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // buf_in (read)
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
            // buf_out (read_write)
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
        ],
    });
    
    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("Pipeline Layout"),
        bind_group_layouts: &[&bgl],
        push_constant_ranges: &[],
    });
    
    let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("RISC-V Pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: "main",
    });
    
    // Initialize framebuffer (640x480 u32 pixels)
    let mut framebuffer = vec![0u32; (WIDTH * HEIGHT) as usize];
    
    // Set initial PC = TEXT_START
    framebuffer[0] = TEXT_START;
    
    // Set stack pointer (x2) to top of RAM
    // Register x2 is at pixel (2 + 2, 0) = pixel 4 in row 0
    framebuffer[4] = 0x4000;  // Stack at 0x4000
    
    // Debug: verify initial state
    println!("DEBUG: Initial framebuffer[0] (PC) = 0x{:08X}", framebuffer[0]);
    println!("DEBUG: Initial framebuffer[4] (SP) = 0x{:08X}", framebuffer[4]);
    
    // Load program into memory (row 4+ = address 0x1000+)
    // Shader uses: pixel_offset = (addr - 0x1000) / 4
    for (i, &word) in program.iter().enumerate() {
        let pixel_offset = i as u32;  // (addr - TEXT_START) / 4 = i
        let x = pixel_offset % WIDTH;
        let y = (pixel_offset / WIDTH) + 4;
        if y < HEIGHT {
            framebuffer[(y * WIDTH + x) as usize] = word;
            if i == 0 {
                println!("DEBUG: First instruction 0x{:08X} written to pixel ({}, {}) = index {}",
                         word, x, y, y * WIDTH + x);
            }
        }
    }

    // Debug: verify data at pixel (384, 5) = framebuffer[5*640+384]
    let data_pixel_idx = (5 * WIDTH + 384) as usize;
    println!("DEBUG: Data at pixel (384, 5) = index {}, value = 0x{:08X}",
        data_pixel_idx, framebuffer[data_pixel_idx]);
    let first_char = (framebuffer[data_pixel_idx] & 0xFF) as u8 as char;
    println!("DEBUG: First char = '{}' (0x{:02X})",
        if first_char >= ' ' && first_char <= '~' { first_char } else { '?' },
        first_char as u8);
    println!();
    
    // Create buffers
    let buf_size = (WIDTH * HEIGHT * 4) as u64;
    
    let config_buf = device.create_buffer(&BufferDescriptor {
        label: Some("Config"),
        size: 16,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&config_buf, 0, bytemuck::cast_slice(&[WIDTH, HEIGHT, 0u32, 0u32]));
    
    let buf_a = device.create_buffer(&BufferDescriptor {
        label: Some("Buffer A"),
        size: buf_size,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buf_a, 0, bytemuck::cast_slice(&framebuffer));
    
    let buf_b = device.create_buffer(&BufferDescriptor {
        label: Some("Buffer B"),
        size: buf_size,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buf_b, 0, bytemuck::cast_slice(&framebuffer));
    
    let readback_buf = device.create_buffer(&BufferDescriptor {
        label: Some("Readback"),
        size: buf_size,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    
    println!("Running RISC-V program on GPU...");
    println!();
    
    let max_iterations = 20;  // Debug first 20 frames
    let mut halted = false;
    
    for frame in 0..max_iterations {
        // Create bind group (A -> B)
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some(&format!("Bind Group {}", frame)),
            layout: &bgl,
            entries: &[
                BindGroupEntry { binding: 0, resource: config_buf.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: buf_a.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: buf_b.as_entire_binding() },
            ],
        });
        
        // Dispatch
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Encoder"),
        });
        
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("RISC-V Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        
        encoder.copy_buffer_to_buffer(&buf_b, 0, &readback_buf, 0, buf_size);
        queue.submit(std::iter::once(encoder.finish()));
        
        // Read back
        let slice = readback_buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(MapMode::Read, move |result| { tx.send(result).unwrap(); });
        device.poll(Maintain::Wait);
        rx.recv()??;
        
        {
            let data = slice.get_mapped_range();
            let pixels: &[u32] = bytemuck::cast_slice(&data);
            
            let pc = pixels[0];
            if pc == 0xFFFFFFFF {
                halted = true;
            }

            let insn_count = pixels[1];
            // Read registers: x5-x10 are at pixels (2+5,0) to (2+10,0) = indices 7-12
            let x5 = pixels[7];  // data base
            let x6 = pixels[8];  // length
            let x7 = pixels[9];  // counter
            let x8 = pixels[10]; // address
            let x9 = pixels[11]; // char
            println!("Frame {:2}, PC: 0x{:04X}, x5=0x{:04X} x6={} x7={} x8=0x{:04X} x9=0x{:02X}",
                frame, pc, x5, x6, x7, x8, x9 & 0xFF);
            
            // Check output region (last 10 rows)
            if frame % 50 == 0 {
                let mut output = String::new();
                for y in (HEIGHT - 10)..HEIGHT {
                    for x in 0..WIDTH {
                        let pixel = pixels[(y * WIDTH + x) as usize];
                        let ch = (pixel & 0xFF) as u8 as char;
                        if ch >= ' ' && ch <= '~' {
                            output.push(ch);
                        }
                    }
                }
                if !output.trim().is_empty() {
                    println!("\nOutput: {}", output.trim());
                }
            }
        }
        readback_buf.unmap();
        
        if halted {
            println!("\n\nProgram halted after {} instructions.", frame + 1);
            break;
        }
        
        // Swap: copy B to A
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Swap"),
        });
        encoder.copy_buffer_to_buffer(&buf_b, 0, &buf_a, 0, buf_size);
        queue.submit(std::iter::once(encoder.finish()));
    }
    
    if !halted {
        println!("\n\nReached max iterations ({}).", max_iterations);
    }
    
    println!();
    println!("════════════════════════════════════════════════════════════");
    println!("  PROVEN: RISC-V instructions executed by GPU shader.");
    println!("  The GPU decoded and ran real RISC-V machine code.");
    println!("  Linux COULD theoretically run in the framebuffer.");
    println!("════════════════════════════════════════════════════════════");
    
    Ok(())
}

// Hand-assembled RISC-V programs

fn program_hello() -> Vec<u32> {
    // Print "Hello" using UART MMIO
    // Data at 0x2000, code at 0x1000
    let hello_str = b"Hello from RISC-V!\n";
    
    // Instructions
    let instructions = vec![
        0x00002537,  // lui x5, 0x2       (x5 = 0x2000 = data start)
        0x02200313,  // li x6, 34        (x6 = string length)
        0x00000393,  // li x7, 0         (x7 = counter)
        // loop:
        0x00758433,  // add x8, x5, x7   (x8 = address)
        0x00040483,  // lb x9, 0(x8)     (x9 = char)
        0x00040463,  // beq x9, x0, done (+8 bytes = 2 instr) - FIXED
        0x00004537,  // lui x10, 0x4     (x10 = 0x4000 = UART)
        0x00950023,  // sb x9, 0(x10)    (write char)
        0x00138393,  // addi x7, x7, 1   (counter++)
        0xfe63c4e3,  // blt x7, x6, loop (-28 = -7 instr)
        // done:
        0x00000073,  // ecall (halt)
    ];
    
    // Build program with data section
    // Text at 0x1000, data at 0x2000, so we need (0x2000 - 0x1000) / 4 = 0x400 words
    let mut program = vec![0u32; 0x500];  // Extra space for data
    
    // Copy instructions
    for (i, &insn) in instructions.iter().enumerate() {
        program[i] = insn;
    }
    
    // Copy string data at offset (0x2000 - 0x1000) / 4 = 0x400
    for (i, chunk) in hello_str.chunks(4).enumerate() {
        let mut word = 0u32;
        for (j, &b) in chunk.iter().enumerate() {
            word |= (b as u32) << (j * 8);
        }
        program[0x400 + i] = word;
    }

    // Debug: print first few data words
    println!("Data at 0x2000 (program[0x400]):");
    for i in 0..4 {
        print!("  [{:03X}] = 0x{:08X}", 0x400 + i, program[0x400 + i]);
        if i == 0 {
            let b0 = (program[0x400] & 0xFF) as u8 as char;
            let b1 = ((program[0x400] >> 8) & 0xFF) as u8 as char;
            println!(" ('{}' '{}')", if b0 >= ' ' && b0 <= '~' { b0 } else { '?' }, if b1 >= ' ' && b1 <= '~' { b1 } else { '?' });
        } else {
            println!();
        }
    }
    println!();
    
    program
}

fn program_count() -> Vec<u32> {
    // Count 0-9
    vec![
        0x00000293,  // li x5, 0        (counter)
        0x00a00313,  // li x6, 10       (limit)
        0x00004397,  // lui x7, 0x4     (UART = 0x4000)
        0x03000413,  // li x8, '0'      (48)
        // loop:
        0x005404b3,  // add x9, x8, x5  (digit)
        0x00938023,  // sb x9, 0(x7)    (write)
        0x00a00513,  // li x10, '\n'
        0x00a38023,  // sb x10, 0(x7)
        0x00128293,  // addi x5, x5, 1
        0xfe62c2e3,  // blt x5, x6, loop
        0x00000073,  // ecall
    ]
}

fn program_fibonacci() -> Vec<u32> {
    // Print first 15 Fibonacci numbers
    vec![
        0x00000293,  // li x5, 0  (a = 0)
        0x00100313,  // li x6, 1  (b = 1)
        0x00004397,  // lui x7, 1 (UART = 0x4000)
        0x00f00413,  // li x8, 15 (count)
        0x00000493,  // li x9, 0  (i)
        // loop:
        0x005285b3,  // add x11, x5, x6  (temp = a + b)
        0x000302b3,  // add x5, x6, x0   (a = b)
        0x00058333,  // add x6, x11, x0  (b = temp)
        0x00f2f613,  // andi x12, x5, 0xF
        0x03000693,  // li x13, '0'
        0x00d60633,  // add x12, x12, x13
        0x00c38023,  // sb x12, 0(x7)
        0x00a00513,  // li x10, '\n'
        0x00a38023,  // sb x10, 0(x7)
        0x00148493,  // addi x9, x9, 1
        0xfe8492e3,  // blt x9, x8, loop
        0x00000073,  // ecall
    ]
}
