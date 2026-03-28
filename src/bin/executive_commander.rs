
use wgpu::*;
use std::env;
use std::fs;

const TILE_SIZE: usize = 4096;
const REG_OFFSET: usize = 2;
const TEXT_OFFSET: usize = 64;
const RODATA_OFFSET: usize = 1024;
const RAM_OFFSET: usize = 2048;
const UART_OUT_OFFSET: usize = 3074;

const ADDR_TEXT: u32 = 0x1000;
const ADDR_RODATA: u32 = 0x2000;
const ADDR_RAM: u32 = 0x3000;

const MAILBOX_CMD: u32 = 0x3000;
const MAILBOX_STATUS: u32 = 0x3004;
const CMD_TARGET: u32 = 0x3008;
const CMD_PAYLOAD: u32 = 0x300C;

const CMD_PING: u32 = 0x01;
const STATUS_PENDING: u32 = 1;

fn addr_to_word_idx(addr: u32) -> usize {
    if addr >= 0x4000 { return UART_OUT_OFFSET; }
    if addr >= ADDR_RAM { return RAM_OFFSET + ((addr - ADDR_RAM) / 4) as usize; }
    if addr >= ADDR_RODATA { return RODATA_OFFSET + ((addr - ADDR_RODATA) / 4) as usize; }
    if addr >= ADDR_TEXT { return TEXT_OFFSET + ((addr - ADDR_TEXT) / 4) as usize; }
    0
}

fn load_elf_into_tile(elf_bytes: &[u8], tile: &mut [u32]) {
    let e_phoff = u32::from_le_bytes(elf_bytes[28..32].try_into().unwrap()) as usize;
    let e_phentsize = u16::from_le_bytes(elf_bytes[42..44].try_into().unwrap()) as usize;
    let e_phnum = u16::from_le_bytes(elf_bytes[44..46].try_into().unwrap()) as usize;
    for i in 0..e_phnum {
        let ph_start = e_phoff + i * e_phentsize;
        let p_type = u32::from_le_bytes(elf_bytes[ph_start..ph_start+4].try_into().unwrap());
        let p_offset = u32::from_le_bytes(elf_bytes[ph_start+4..ph_start+8].try_into().unwrap()) as usize;
        let p_vaddr = u32::from_le_bytes(elf_bytes[ph_start+8..ph_start+12].try_into().unwrap());
        let p_filesz = u32::from_le_bytes(elf_bytes[ph_start+16..ph_start+20].try_into().unwrap()) as usize;
        if p_type != 1 { continue; }
        let segment_data = &elf_bytes[p_offset..p_offset + p_filesz];
        for (j, chunk) in segment_data.chunks(4).enumerate() {
            let addr = p_vaddr + (j as u32) * 4;
            let mut word = 0u32;
            for (k, &b) in chunk.iter().enumerate() { word |= (b as u32) << (k * 8); }
            let idx = addr_to_word_idx(addr);
            if idx < TILE_SIZE { tile[idx] = word; }
        }
    }
}

fn read_uart_output(tile: &[u32]) -> String {
    let mut output = String::new();
    for i in UART_OUT_OFFSET..4096 {
        let pixel = tile[i];
        if pixel == 0 { continue; }
        let ch = (pixel & 0xFF) as u8;
        if ch >= 0x20 && ch <= 0x7E { output.push(ch as char); }
        else if ch == 0x0A { output.push('\n'); }
    }
    output
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let mut cmd_name = "boot".to_string();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--cmd" { cmd_name = args[i+1].clone(); i += 2; }
        else { i += 1; }
    }

    let elf_path = "riscv-cartridges/executive_commander/target/riscv32im-unknown-none-elf/release/executive-commander";
    let elf_bytes = fs::read(elf_path)?;
    let mut tile_data = vec![0u32; TILE_SIZE];
    tile_data[0] = ADDR_TEXT;
    tile_data[REG_OFFSET + 2] = 0x3FFC;
    load_elf_into_tile(&elf_bytes, &mut tile_data);

    let instance = Instance::new(InstanceDescriptor::default());
    let adapter = instance.request_adapter(&RequestAdapterOptions::default()).await.unwrap();
    let (device, queue) = adapter.request_device(&DeviceDescriptor::default(), None).await.unwrap();
    let shader = device.create_shader_module(ShaderModuleDescriptor { label: None, source: ShaderSource::Wgsl(fs::read_to_string("riscv-multicore.wgsl")?.into()) });
    let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor { label: None, layout: None, module: &shader, entry_point: "main" });

    let storage_buf = device.create_buffer(&BufferDescriptor { label: None, size: (TILE_SIZE*4) as u64, usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC, mapped_at_creation: false });
    queue.write_buffer(&storage_buf, 0, bytemuck::cast_slice(&tile_data));
    let readback_buf = device.create_buffer(&BufferDescriptor { label: None, size: (TILE_SIZE*4) as u64, usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST, mapped_at_creation: false });
    let config_buf = device.create_buffer(&BufferDescriptor { label: None, size: 16, usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST, mapped_at_creation: false });
    queue.write_buffer(&config_buf, 0, bytemuck::cast_slice(&[1u32, 1u32, 1000u32, 0u32]));

    let bind_group = device.create_bind_group(&BindGroupDescriptor { label: None, layout: &pipeline.get_bind_group_layout(0), entries: &[
        BindGroupEntry { binding: 0, resource: config_buf.as_entire_binding() },
        BindGroupEntry { binding: 1, resource: storage_buf.as_entire_binding() },
    ]});

    for frame in 0..500 {
        if cmd_name == "ping" && frame == 200 {
            let mut update = vec![0u32; 4];
            update[0] = CMD_PING; update[1] = STATUS_PENDING;
            queue.write_buffer(&storage_buf, (RAM_OFFSET * 4) as u64, bytemuck::cast_slice(&update));
        }
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor::default());
        { let mut pass = enc.begin_compute_pass(&ComputePassDescriptor::default()); pass.set_pipeline(&pipeline); pass.set_bind_group(0, &bind_group, &[]); pass.dispatch_workgroups(1, 1, 1); }
        enc.copy_buffer_to_buffer(&storage_buf, 0, &readback_buf, 0, (TILE_SIZE*4) as u64);
        queue.submit(std::iter::once(enc.finish()));
        let slice = readback_buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(MapMode::Read, move |r| { tx.send(r).unwrap(); });
        device.poll(Maintain::Wait);
        rx.recv()??;
        let words: Vec<u32> = bytemuck::cast_slice(&slice.get_mapped_range()).to_vec();
        readback_buf.unmap();
        
        let output = read_uart_output(&words);
        if !output.is_empty() { 
            println!("UART:\n{}", output); 
            if output.contains("PONG") { break; }
        }
        if words[0] == 0xFFFFFFFF { println!("Halted."); break; }
    }
    Ok(())
}
