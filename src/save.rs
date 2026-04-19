// save.rs -- Save/load state, PNG screenshot for Geometry OS

use crate::inode_fs;
use crate::vfs;
use crate::vm;
use std::path::Path;

pub fn save_screen_png(path: &str, screen: &[u32]) -> std::io::Result<()> {
    let file = std::fs::File::create(path)?;
    let w = &mut std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(w, 256, 256);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    let mut raw_data = Vec::with_capacity(256 * 256 * 3);
    for pixel in screen {
        raw_data.push((pixel >> 16) as u8); // R
        raw_data.push((pixel >> 8) as u8); // G
        raw_data.push(*pixel as u8); // B
    }
    writer.write_image_data(&raw_data)?;
    Ok(())
}

pub fn save_full_buffer_png(path: &str, buffer: &[u32], w: usize, h: usize) -> std::io::Result<()> {
    let file = std::fs::File::create(path)?;
    let writer = &mut std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, w as u32, h as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    let mut raw_data = Vec::with_capacity(w * h * 3);
    for &pixel in buffer {
        raw_data.push((pixel >> 16) as u8); // R
        raw_data.push((pixel >> 8) as u8); // G
        raw_data.push(pixel as u8); // B
    }
    writer.write_image_data(&raw_data)?;
    Ok(())
}

/// Save full application state (VM + canvas) to a binary file.
/// Format: VM save (see vm.rs) + canvas_len u32 + canvas_buffer + canvas_assembled u8
pub fn save_state(
    path: &str,
    vm: &vm::Vm,
    canvas_buffer: &[u32],
    canvas_assembled: bool,
) -> std::io::Result<()> {
    use std::io::Write;
    // Save VM state first
    vm.save_to_file(Path::new(path))?;
    // Append canvas data
    let mut f = std::fs::OpenOptions::new().append(true).open(path)?;
    let canvas_len = canvas_buffer.len() as u32;
    f.write_all(&canvas_len.to_le_bytes())?;
    for &v in canvas_buffer {
        f.write_all(&v.to_le_bytes())?;
    }
    f.write_all(&[if canvas_assembled { 1 } else { 0 }])?;
    Ok(())
}

/// Load full application state from a binary file.
/// Returns (vm, canvas_buffer, canvas_assembled) on success.
pub fn load_state(path: &str) -> std::io::Result<(vm::Vm, Vec<u32>, bool)> {
    use std::io::Read;
    let mut data = Vec::new();
    let mut f = std::fs::File::open(path)?;
    f.read_to_end(&mut data)?;

    // Read VM portion
    let vm_min = 4 + 4 + 1 + 4 + vm::NUM_REGS * 4 + vm::RAM_SIZE * 4 + vm::SCREEN_SIZE * 4;
    if data.len() < vm_min + 4 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "save file too small for canvas trailer",
        ));
    }

    // Parse VM from the raw bytes (same logic as Vm::load_from_file)
    if &data[0..4] != vm::SAVE_MAGIC {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid magic bytes",
        ));
    }
    let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    if version != vm::SAVE_VERSION {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unsupported save version: {}", version),
        ));
    }

    let mut off = 8usize;
    let halted = data[off] != 0;
    off += 1;
    let pc = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
    off += 4;

    let mut regs = [0u32; vm::NUM_REGS];
    for r in regs.iter_mut() {
        *r = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
        off += 4;
    }
    let mut ram = vec![0u32; vm::RAM_SIZE];
    for v in ram.iter_mut() {
        *v = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
        off += 4;
    }
    let mut screen = vec![0u32; vm::SCREEN_SIZE];
    for v in screen.iter_mut() {
        *v = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
        off += 4;
    }

    let rand_state = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
    off += 4;
    let frame_count = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
    off += 4;

    let vm = vm::Vm {
        ram,
        regs,
        pc,
        screen,
        halted,
        frame_ready: false,
        rand_state,
        frame_count,
            beep: None,
            note: None,
        debug_mode: false,
        access_log: Vec::new(),
        processes: Vec::new(),
        mode: vm::CpuMode::Kernel,
        kernel_stack: Vec::new(),
        allocated_pages: 0b11,
        page_ref_count: {
            let mut rc = [0u32; vm::NUM_RAM_PAGES];
            rc[0] = 1;
            rc[1] = 1;
            rc
        },
        page_cow: 0,
        current_page_dir: None,
        current_vmas: Vec::new(),
        segfault_pid: 0,
        segfault: false,
        vfs: vfs::Vfs::new(),
        inode_fs: inode_fs::InodeFs::new(),
        current_pid: 0,
        sched_tick: 0,
        default_time_slice: vm::DEFAULT_TIME_SLICE,
        yielded: false,
        sleep_frames: 0,
        new_priority: 0,
        pipes: Vec::new(),
        pipe_created: false,
        msg_sender: 0,
        msg_data: [0; vm::MSG_WORDS],
        msg_recv_requested: false,
        env_vars: std::collections::HashMap::new(),
        booted: false,
        shutdown_requested: false,
        step_exit_code: None,
        step_zombie: false,
        hypervisor_active: false,
        hypervisor_config: String::new(),
        hypervisor_mode: vm::HypervisorMode::default(),
        canvas_buffer: vec![0; vm::CANVAS_RAM_SIZE],
        key_buffer: vec![0; 16],
        key_buffer_head: 0,
        key_buffer_tail: 0,
        formulas: Vec::new(),
        formula_dep_index: vec![Vec::new(); vm::CANVAS_RAM_SIZE],
        trace_recording: false,
        trace_buffer: vm::TraceBuffer::new(vm::DEFAULT_TRACE_CAPACITY),
        frame_checkpoints: vm::FrameCheckBuffer::new(vm::DEFAULT_FRAME_CHECK_CAPACITY),
        snapshots: Vec::new(),
    };

    // Parse canvas trailer
    let canvas_len =
        u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]) as usize;
    off += 4;
    if off + canvas_len * 4 + 1 > data.len() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "save file truncated in canvas data",
        ));
    }
    let mut canvas_buffer = vec![0u32; canvas_len];
    for v in canvas_buffer.iter_mut() {
        *v = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
        off += 4;
    }
    let canvas_assembled = data[off] != 0;

    Ok((vm, canvas_buffer, canvas_assembled))
}
