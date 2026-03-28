// Multi-Tile Ignition Test (High Performance Batch Edition)
//
// Dispatches N independent RISC-V tiles on the GPU simultaneously.
// Supports batching multiple dispatches per GPU submission to reduce CPU overhead.

use wgpu::*;
use std::fs;
use std::time::Instant;

// Must match riscv-multicore.wgsl
const TILE_SIZE: usize = 4096;  // u32 words per tile
const REG_OFFSET: usize = 2;
const TEXT_OFFSET: usize = 64;
const UART_CURSOR_OFFSET: usize = 3072;
const UART_OUT_OFFSET: usize = 3074;

fn program_write_tile_id() -> Vec<u32> {
    vec![
        0x00004337,  // lui x6, 0x4
        0x00c2d393,  // srli x7, x5, 12
        0x00f3f393,  // andi x7, x7, 0xF
        0x03000e13,  // li x28, 48
        0x01c383b3,  // add x7, x7, x28
        0x00730023,  // sb x7, 0(x6)
        0x0082d393,  // srli x7, x5, 8
        0x00f3f393,  // andi x7, x7, 0xF
        0x01c383b3,  // add x7, x7, x28
        0x00730023,  // sb x7, 0(x6)
        0x0042d393,  // srli x7, x5, 4
        0x00f3f393,  // andi x7, x7, 0xF
        0x01c383b3,  // add x7, x7, x28
        0x00730023,  // sb x7, 0(x6)
        0x00f2f393,  // andi x7, x5, 0xF
        0x01c383b3,  // add x7, x7, x28
        0x00730023,  // sb x7, 0(x6)
        0x00a00393,  // li x7, 10
        0x00730023,  // sb x7, 0(x6)
        0x00000073,  // ecall
    ]
}

fn program_fibonacci() -> Vec<u32> {
    vec![
        0x00128493, 0x01400e13, 0x01c4d463, 0x0080006f, 0x01400493, 
        0x00000313, 0x00100393, 0x00000413, 0x00004537, 0x03000593,
        0x00f37e13, 0x01c58eb3, 0x01d50023, 0x00730f33, 0x00038313,
        0x000f0393, 0x00140413, 0xfe9442e3, 0x00a00e13, 0x01c50023,
        0x00000073,
    ]
}

fn program_stress() -> Vec<u32> {
    vec![
        0x00000293,  // li x5, 0
        0x00128293,  // addi x5, x5, 1
        0xff9ff06f,  // j -4
    ]
}

fn read_tile_uart(words: &[u32], tile_id: u32) -> String {
    let base = tile_id as usize * TILE_SIZE;
    let cursor = words[base + UART_CURSOR_OFFSET] as usize;
    let mut s = String::new();
    for i in 0..cursor.min(1022) {
        let ch = (words[base + UART_OUT_OFFSET + i] & 0xFF) as u8;
        if ch >= 0x20 && ch <= 0x7E { s.push(ch as char); }
        else if ch == 0x0A { s.push('\n'); }
    }
    s
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut num_tiles = 100u32;
    let mut max_steps = 128u32;
    let mut batch_size = 1u32;
    let mut prog_type = "id".to_string();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--tiles" => { num_tiles = args[i+1].parse()?; i += 2; }
            "--steps" => { max_steps = args[i+1].parse()?; i += 2; }
            "--batch" => { batch_size = args[i+1].parse()?; i += 2; }
            "-p" | "--program" => { prog_type = args[i+1].clone(); i += 2; }
            _ => { i += 1; }
        }
    }

    let program = match prog_type.as_str() {
        "fib" => program_fibonacci(),
        "stress" => program_stress(),
        _ => program_write_tile_id(),
    };

    println!("Tiles: {}  Steps: {}  Batch: {}  Program: {}", num_tiles, max_steps, batch_size, prog_type);

    let instance = Instance::new(InstanceDescriptor::default());
    let adapter = instance.request_adapter(&RequestAdapterOptions::default()).await.unwrap();
    let (device, queue) = adapter.request_device(&DeviceDescriptor {
        required_limits: Limits {
            max_storage_buffer_binding_size: 256 * 1024 * 1024,
            max_buffer_size: 256 * 1024 * 1024,
            ..Limits::default()
        }, ..Default::default()
    }, None).await.unwrap();

    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: None, source: ShaderSource::Wgsl(fs::read_to_string("riscv-multicore.wgsl")?.into()),
    });

    let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: None, layout: None, module: &shader, entry_point: "main",
    });

    let total_words = num_tiles as usize * TILE_SIZE;
    let buf_size = (total_words * 4) as u64;
    let mut tile_data = vec![0u32; total_words];

    for tid in 0..num_tiles {
        let base = tid as usize * TILE_SIZE;
        tile_data[base] = 0x1000;
        tile_data[base + REG_OFFSET + 2] = 0x4000;
        tile_data[base + REG_OFFSET + 5] = tid;
        for (j, &insn) in program.iter().enumerate() {
            tile_data[base + TEXT_OFFSET + j] = insn;
        }
    }

    let config_buf = device.create_buffer(&BufferDescriptor {
        label: None, size: 16, usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST, mapped_at_creation: false,
    });
    queue.write_buffer(&config_buf, 0, bytemuck::cast_slice(&[num_tiles, max_steps, 0u32, 0u32]));

    let tile_buf = device.create_buffer(&BufferDescriptor {
        label: None, size: buf_size, usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC, mapped_at_creation: false,
    });
    queue.write_buffer(&tile_buf, 0, bytemuck::cast_slice(&tile_data));

    let readback = device.create_buffer(&BufferDescriptor {
        label: None, size: buf_size, usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST, mapped_at_creation: false,
    });

    let bgl = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: None, layout: &bgl,
        entries: &[
            BindGroupEntry { binding: 0, resource: config_buf.as_entire_binding() },
            BindGroupEntry { binding: 1, resource: tile_buf.as_entire_binding() },
        ],
    });

    println!("Dispatching...");
    let t_start = Instant::now();
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());
    for _ in 0..batch_size {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups((num_tiles + 63) / 64, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&tile_buf, 0, &readback, 0, buf_size);
    queue.submit(std::iter::once(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(MapMode::Read, move |r| { tx.send(r).unwrap(); });
    device.poll(Maintain::Wait);
    rx.recv()??;

    let duration = t_start.elapsed();
    let data = slice.get_mapped_range();
    let result: &[u32] = bytemuck::cast_slice(&data);

    let mut total_insns = 0u64;
    for tid in 0..num_tiles {
        total_insns += result[tid as usize * TILE_SIZE + 1] as u64;
    }

    println!("Done in {:.2}ms", duration.as_secs_f64() * 1000.0);
    println!("Total instructions: {}", total_insns);
    println!("Throughput: {:.2} MIPS", (total_insns as f64 / duration.as_secs_f64()) / 1_000_000.0);

    let show = num_tiles.min(5) as usize;
    for tid in 0..show {
        let uart = read_tile_uart(result, tid as u32);
        println!("  Tile {:4}: PC=0x{:08X} ticks={} UART: {}", tid, result[tid * TILE_SIZE], result[tid * TILE_SIZE + 1], uart.trim());
    }

    Ok(())
}
