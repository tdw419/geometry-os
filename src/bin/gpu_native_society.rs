use wgpu::*;
use std::time::Instant;
use image::{ImageBuffer, Rgba};

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;
const AGENT_COUNT: usize = 64;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct AgentState {
    pc: u32, sp: u32, pos_x: u32, pos_y: u32, vel_x: i32, vel_y: i32,
    color: u32, is_it: u32, halted: u32, step_count: u32, flags: u32, _padding: u32,
    registers: [u32; 16], stack: [u32; 16],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Config { width: u32, height: u32, time: f32, frame: u32 }

async fn read_buffer<T: bytemuck::Pod>(device: &Device, queue: &Queue, buffer: &Buffer, size: u64) -> Vec<T> {
    let staging = device.create_buffer(&BufferDescriptor {
        label: None, size, usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST, mapped_at_creation: false
    });
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
    encoder.copy_buffer_to_buffer(buffer, 0, &staging, 0, size);
    queue.submit(Some(encoder.finish()));
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(MapMode::Read, move |res| tx.send(res).unwrap());
    device.poll(Maintain::Wait);
    rx.recv().unwrap().unwrap();
    let data = slice.get_mapped_range();
    let result = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    staging.unmap();
    result
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Phase 7 Gamma: The 64-Agent Collective...");

    let instance = Instance::new(InstanceDescriptor::default());
    let adapter = instance.request_adapter(&RequestAdapterOptions::default()).await.unwrap();
    let (device, queue) = adapter.request_device(&DeviceDescriptor::default(), None).await.unwrap();

    let shader_source = std::fs::read_to_string("gpu_native_society.wgsl")?;
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("GPU Society Shader"),
        source: ShaderSource::Wgsl(shader_source.into())
    });

    let mut initial_agents = [AgentState {
        pc: 0, sp: 0, pos_x: 320, pos_y: 240, vel_x: 0, vel_y: 0, color: 0xFFFFFFFF,
        is_it: 0, halted: 0, step_count: 0, flags: 0, _padding: 0,
        registers: [0; 16], stack: [0; 16]
    }; AGENT_COUNT];

    for i in 0..AGENT_COUNT {
        initial_agents[i].pos_x = 320 + ((i % 8) as u32) * 10;
        initial_agents[i].pos_y = 240 + ((i / 8) as u32) * 10;
        initial_agents[i].registers[10] = (i % 2) as u32;
        initial_agents[i].registers[0] = if i % 2 == 0 { 0x00FF00u32 } else { 0xFF0000u32 }; 
    }

    let mut bytecode = vec![0u32; 1024];
    bytecode[0] = 0x0000000A; 
    bytecode[1] = 0x00010102; 
    bytecode[2] = 0x00010202; 
    bytecode[3] = 0x00010B02; 
    bytecode[4] = 0x000B0A10; 
    bytecode[5] = 0x0002010B; 
    bytecode[6] = 0x0000000D; 
    bytecode[7] = 0x00000007; 

    let agents_buffer = device.create_buffer(&BufferDescriptor { label: None, size: (AGENT_COUNT * 176) as u64, usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST, mapped_at_creation: false });
    queue.write_buffer(&agents_buffer, 0, bytemuck::cast_slice(&initial_agents));
    let bytecode_buffer = device.create_buffer(&BufferDescriptor { label: None, size: (bytecode.len() * 4) as u64, usage: BufferUsages::STORAGE | BufferUsages::COPY_DST, mapped_at_creation: false });
    queue.write_buffer(&bytecode_buffer, 0, bytemuck::cast_slice(&bytecode));
    let fb_buffer = device.create_buffer(&BufferDescriptor { label: None, size: (WIDTH * HEIGHT * 4) as u64, usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST, mapped_at_creation: false });
    let mailbox_buffer = device.create_buffer(&BufferDescriptor { label: None, size: (AGENT_COUNT * 4) as u64, usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST, mapped_at_creation: false });
    let config_buffer = device.create_buffer(&BufferDescriptor { label: None, size: 16, usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST, mapped_at_creation: false });

    let bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor { label: None, entries: &[
        BindGroupLayoutEntry { binding: 0, visibility: ShaderStages::COMPUTE, ty: BindingType::Buffer { ty: BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None },
        BindGroupLayoutEntry { binding: 1, visibility: ShaderStages::COMPUTE, ty: BindingType::Buffer { ty: BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
        BindGroupLayoutEntry { binding: 2, visibility: ShaderStages::COMPUTE, ty: BindingType::Buffer { ty: BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None },
        BindGroupLayoutEntry { binding: 3, visibility: ShaderStages::COMPUTE, ty: BindingType::Buffer { ty: BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None },
        BindGroupLayoutEntry { binding: 4, visibility: ShaderStages::COMPUTE, ty: BindingType::Buffer { ty: BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
    ]});
    let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor { label: None, layout: Some(&device.create_pipeline_layout(&PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&bgl], push_constant_ranges: &[] })), module: &shader, entry_point: "main" });
    let bg = device.create_bind_group(&BindGroupDescriptor { label: None, layout: &bgl, entries: &[
        BindGroupEntry { binding: 0, resource: agents_buffer.as_entire_binding() },
        BindGroupEntry { binding: 1, resource: bytecode_buffer.as_entire_binding() },
        BindGroupEntry { binding: 2, resource: fb_buffer.as_entire_binding() },
        BindGroupEntry { binding: 3, resource: mailbox_buffer.as_entire_binding() },
        BindGroupEntry { binding: 4, resource: config_buffer.as_entire_binding() },
    ]});

    println!("🏃 Running 64-Agent Collective (2000 steps)...");
    let start = Instant::now();
    for frame in 0..2000 {
        queue.write_buffer(&config_buffer, 0, bytemuck::bytes_of(&Config { width: WIDTH, height: HEIGHT, time: 0.0, frame }));
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
        { let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor { label: None, timestamp_writes: None }); cpass.set_pipeline(&pipeline); cpass.set_bind_group(0, &bg, &[]); cpass.dispatch_workgroups(1, 1, 1); }
        queue.submit(Some(encoder.finish()));
    }
    device.poll(Maintain::Wait);
    println!("✅ Done in {:?}", start.elapsed());

    let fb: Vec<u32> = read_buffer(&device, &queue, &fb_buffer, (WIDTH * HEIGHT * 4) as u64).await;
    let mut out_img = ImageBuffer::new(WIDTH, HEIGHT);
    for (i, &val) in fb.iter().enumerate() {
        let x = i as u32 % WIDTH; let y = i as u32 / WIDTH;
        out_img.put_pixel(x, y, Rgba([ (val & 0xFF) as u8, ((val >> 8) & 0xFF) as u8, ((val >> 16) & 0xFF) as u8, 255 ]));
    }
    out_img.save("output/64_agent_collective.png")?;
    println!("💾 Collective state saved to output/64_agent_collective.png");

    Ok(())
}
