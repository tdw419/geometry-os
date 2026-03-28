use wgpu::*;
use std::fs;

const TILE_SIZE: usize = 4096;
const TEXT_OFFSET: usize = 64;
const REG_OFFSET: usize = 2;
const UART_OUT_OFFSET: usize = 3074;
const MAILBOX_OFFSET: usize = 4000;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║   PHASE 42 — INTER-TILE HANDSHAKE (SEND/RECV)            ║");
    println!("╚══════════════════════════════════════════════════════════╝");

    let num_tiles = 2u32;
    let max_steps = 1000u32;
    let mut tile_data = vec![0u32; num_tiles as usize * TILE_SIZE];

    // Tile 0: SEND 'A' to Tile 1's mailbox slot 5
    // SEND rs2, rs1, rd (rd=slot)
    let prog0 = vec![
        0x00100093, // li x1, 1 (target tile 1)
        0x04100113, // li x2, 0x41 ('A')
        0x002082E0, // SEND x2, x1, slot 5
        0x00000073, // ecall (halt)
    ];

    // Tile 1: RECV from own mailbox slot 5, write to UART
    // RECV rd, rs1, rs2 (rs1=tile, rs2=slot)
    let prog1 = vec![
        0x00100093, // li x1, 1 (source tile = self, but mailbox is shared)
        0x005085e1, // RECV x11, x1, slot 5
        0x00004537, // lui x10, 4 (UART at 0x4000)
        0x00b50023, // sb x11, 0(x10)
        0x00000073, // ecall (halt)
    ];

    // Load programs
    for (i, &insn) in prog0.iter().enumerate() {
        tile_data[TEXT_OFFSET + i] = insn;
    }
    tile_data[0] = 0x1000; // PC for tile 0

    let base1 = TILE_SIZE;
    for (i, &insn) in prog1.iter().enumerate() {
        tile_data[base1 + TEXT_OFFSET + i] = insn;
    }
    tile_data[base1] = 0x1000; // PC for tile 1

    println!("Tile 0 program (sender): {} instructions", prog0.len());
    println!("Tile 1 program (receiver): {} instructions", prog1.len());

    // Setup GPU
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
        label: None,
        source: ShaderSource::Wgsl(fs::read_to_string("riscv-multicore.wgsl")?.into()),
    });

    let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &shader,
        entry_point: "main",
    });

    let storage_buf = device.create_buffer(&BufferDescriptor {
        label: None,
        size: (tile_data.len() * 4) as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    queue.write_buffer(&storage_buf, 0, bytemuck::cast_slice(&tile_data));

    let config_buf = device.create_buffer(&BufferDescriptor {
        label: None,
        size: 16,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&config_buf, 0, bytemuck::cast_slice(&[num_tiles, max_steps, 0u32, 0u32]));

    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            BindGroupEntry { binding: 0, resource: config_buf.as_entire_binding() },
            BindGroupEntry { binding: 1, resource: storage_buf.as_entire_binding() },
        ],
    });

    // Dispatch both tiles simultaneously
    println!("\nDispatching tiles 0-1...");
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor { label: None, timestamp_writes: None });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups((num_tiles + 63) / 64, 1, 1);
    }

    let readback = device.create_buffer(&BufferDescriptor {
        label: None,
        size: (tile_data.len() * 4) as u64,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    encoder.copy_buffer_to_buffer(&storage_buf, 0, &readback, 0, (tile_data.len() * 4) as u64);
    queue.submit(std::iter::once(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(MapMode::Read, move |r| { tx.send(r).unwrap(); });
    device.poll(Maintain::Wait);
    rx.recv()??;

    let data = slice.get_mapped_range();
    let result: &[u32] = bytemuck::cast_slice(&data);

    println!("\nResults:");
    println!("Tile 0: PC=0x{:08X} ticks={}", result[0], result[1]);
    println!("Tile 1: PC=0x{:08X} ticks={}", result[TILE_SIZE], result[TILE_SIZE + 1]);

    let mailbox_5 = result[TILE_SIZE + MAILBOX_OFFSET + 5];
    let uart_0 = result[UART_OUT_OFFSET];
    let uart_1 = result[TILE_SIZE + UART_OUT_OFFSET];

    println!("\nMailbox[5] = 0x{:08X} ({})", mailbox_5, if mailbox_5 >= 0x20 && mailbox_5 < 0x7F { (mailbox_5 as u8) as char } else { '?' });
    println!("Tile 0 UART: 0x{:08X}", uart_0);
    println!("Tile 1 UART: 0x{:08X} ({})", uart_1, if uart_1 >= 0x20 && uart_1 < 0x7F { (uart_1 as u8) as char } else { '?' });
    
    // Debug: show Tile 1 registers
    println!("\nTile 1 registers:");
    println!("  x1 (target): 0x{:08X}", result[TILE_SIZE + 2 + 1]);
    println!("  x10 (UART): 0x{:08X}", result[TILE_SIZE + 2 + 10]);
    println!("  x11 (recv): 0x{:08X} ({})", result[TILE_SIZE + 2 + 11], 
             if result[TILE_SIZE + 2 + 11] >= 0x20 && result[TILE_SIZE + 2 + 11] < 0x7F { 
                 (result[TILE_SIZE + 2 + 11] as u8) as char 
             } else { '?' });
    println!("  UART cursor: {}", result[TILE_SIZE + 3072]);
    
    // Show Tile 1 program (should match what we loaded)
    println!("\nTile 1 program (at TEXT_OFFSET = {}):", TEXT_OFFSET);
    for i in 0..5 {
        println!("  [{:02}] 0x{:08X}", i, result[TILE_SIZE + TEXT_OFFSET + i]);
    }
    
    // Show all UART buffer entries
    println!("\nTile 1 UART buffer (first 10 entries):");
    for i in 0..10 {
        let val = result[TILE_SIZE + 3074 + i];
        if val != 0 {
            println!("  [{}] = 0x{:08X} ({})", i, val, 
                     if val >= 0x20 && val < 0x7F { (val as u8) as char } else { '?' });
        }
    }

    if uart_1 == 0x41 || uart_1 == 0x61 { // 'A' or 'a'
        println!("\n✅ SUCCESS: Tile 1 received 'A' and wrote to UART!");
    } else if mailbox_5 == 0x41 {
        println!("\n⚠️ PARTIAL: Tile 0 wrote to mailbox, but Tile 1 didn't read it to UART");
    } else {
        println!("\n❌ FAIL: No communication detected");
    }

    Ok(())
}
