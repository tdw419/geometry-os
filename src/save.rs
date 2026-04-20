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
        opcode_histogram: [0; 256],
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
        pixel_write_log: vm::PixelWriteLog::new(vm::DEFAULT_PIXEL_WRITE_CAPACITY),
        tcp_connections: (0..vm::MAX_TCP_CONNECTIONS).map(|_| None).collect(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::Vm;

    #[test]
    fn test_save_screen_png_roundtrip() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_save_screen.png");
        let path_str = path.to_str().unwrap();

        // Create a known screen pattern
        let mut screen = vec![0u32; 256 * 256];
        screen[0] = 0xFF0000; // red pixel at (0,0)
        screen[1] = 0x00FF00; // green pixel at (1,0)
        screen[256] = 0x0000FF; // blue pixel at (0,1)
        screen[257] = 0xFFFFFF; // white pixel at (1,1)

        save_screen_png(path_str, &screen).unwrap();

        // Read back and verify
        let file = std::fs::File::open(&path).unwrap();
        let decoder = png::Decoder::new(std::io::BufReader::new(file));
        let mut reader = decoder.read_info().unwrap();
        assert_eq!(reader.info().width, 256);
        assert_eq!(reader.info().height, 256);

        let mut buf = vec![0u8; 256 * 256 * 3];
        reader.next_frame(&mut buf).unwrap();

        // Check pixel (0,0) = red
        assert_eq!(buf[0], 0xFF, "R channel of pixel (0,0)");
        assert_eq!(buf[1], 0x00, "G channel of pixel (0,0)");
        assert_eq!(buf[2], 0x00, "B channel of pixel (0,0)");

        // Check pixel (1,0) = green
        assert_eq!(buf[3], 0x00, "R channel of pixel (1,0)");
        assert_eq!(buf[4], 0xFF, "G channel of pixel (1,0)");
        assert_eq!(buf[5], 0x00, "B channel of pixel (1,0)");

        // Check pixel (0,1) = blue
        let off = 256 * 3;
        assert_eq!(buf[off], 0x00);
        assert_eq!(buf[off + 1], 0x00);
        assert_eq!(buf[off + 2], 0xFF);

        // Check pixel (1,1) = white
        assert_eq!(buf[off + 3], 0xFF);
        assert_eq!(buf[off + 4], 0xFF);
        assert_eq!(buf[off + 5], 0xFF);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_save_screen_png_all_black() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_save_black.png");
        let path_str = path.to_str().unwrap();

        let screen = vec![0u32; 256 * 256];
        save_screen_png(path_str, &screen).unwrap();

        let file = std::fs::File::open(&path).unwrap();
        let decoder = png::Decoder::new(std::io::BufReader::new(file));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0u8; 256 * 256 * 3];
        reader.next_frame(&mut buf).unwrap();

        assert!(buf.iter().all(|&b| b == 0), "all-black screen should produce all-zero bytes");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_save_screen_png_all_white() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_save_white.png");
        let path_str = path.to_str().unwrap();

        let screen = vec![0xFFFFFFu32; 256 * 256];
        save_screen_png(path_str, &screen).unwrap();

        let file = std::fs::File::open(&path).unwrap();
        let decoder = png::Decoder::new(std::io::BufReader::new(file));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0u8; 256 * 256 * 3];
        reader.next_frame(&mut buf).unwrap();

        assert!(buf.iter().all(|&b| b == 0xFF), "all-white screen should produce all-0xFF bytes");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_save_full_buffer_png_custom_size() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_save_custom.png");
        let path_str = path.to_str().unwrap();

        let w = 64;
        let h = 32;
        let buffer = vec![0x12345678u32; w * h];

        save_full_buffer_png(path_str, &buffer, w, h).unwrap();

        let file = std::fs::File::open(&path).unwrap();
        let decoder = png::Decoder::new(std::io::BufReader::new(file));
        let mut reader = decoder.read_info().unwrap();
        assert_eq!(reader.info().width, w as u32);
        assert_eq!(reader.info().height, h as u32);

        let mut buf = vec![0u8; w * h * 3];
        reader.next_frame(&mut buf).unwrap();

        // save_full_buffer_png extracts RGB from u32 as: R=(pixel>>16), G=(pixel>>8), B=pixel
        // So 0x12345678 -> R=0x34, G=0x56, B=0x78
        assert_eq!(buf[0], 0x34);
        assert_eq!(buf[1], 0x56);
        assert_eq!(buf[2], 0x78);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_save_load_state_roundtrip() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_save_state.bin");
        let path_str = path.to_str().unwrap();

        // Create a VM with known state
        let mut vm = Vm::new();
        vm.pc = 42;
        vm.regs[0] = 100;
        vm.regs[5] = 200;
        vm.ram[100] = 0xDEADBEEF;
        vm.halted = false;
        vm.screen[0] = 0xFF0000;

        let canvas = vec![0x00FF00u32; 1024];
        let canvas_assembled = true;

        save_state(path_str, &vm, &canvas, canvas_assembled).unwrap();

        // Load back
        let (loaded_vm, loaded_canvas, loaded_assembled) = load_state(path_str).unwrap();

        assert_eq!(loaded_vm.pc, 42);
        assert_eq!(loaded_vm.regs[0], 100);
        assert_eq!(loaded_vm.regs[5], 200);
        assert_eq!(loaded_vm.ram[100], 0xDEADBEEF);
        assert!(!loaded_vm.halted);
        assert_eq!(loaded_vm.screen[0], 0xFF0000);
        assert_eq!(loaded_canvas.len(), 1024);
        assert_eq!(loaded_canvas[0], 0x00FF00);
        assert!(loaded_assembled);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_load_state_invalid_magic() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_invalid_magic.bin");
        // Create a file large enough to pass the size check but with wrong magic
        let vm_min = 4 + 4 + 1 + 4 + vm::NUM_REGS * 4 + vm::RAM_SIZE * 4 + vm::SCREEN_SIZE * 4;
        let data_size = vm_min + 4 + 1; // vm_min + canvas_len (4) + canvas_assembled (1)
        let mut data = vec![0u8; data_size];
        data[0] = b'B';
        data[1] = b'A';
        data[2] = b'D';
        data[3] = b'!'; // wrong magic
        std::fs::write(&path, &data).unwrap();

        let result = load_state(path.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid magic"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_load_state_too_small() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_too_small.bin");
        // Write valid magic but way too small
        std::fs::write(
            &path,
            [vm::SAVE_MAGIC[0], vm::SAVE_MAGIC[1], vm::SAVE_MAGIC[2], vm::SAVE_MAGIC[3], 0, 0, 0, 0, 0],
        )
        .unwrap();

        let result = load_state(path.to_str().unwrap());
        assert!(result.is_err());

        let _ = std::fs::remove_file(&path);
    }
}
