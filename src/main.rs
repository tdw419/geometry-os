// main.rs -- Geometry OS Canvas Text Surface
//
// The canvas grid IS a text editor. Type assembly, press F8 to assemble,
// press F5 to run. Each keystroke writes a colored pixel glyph.
//
// Build: cargo run
// Test:  cargo test

mod assembler;
mod audio;
mod canvas;
mod cli;
mod font;
mod hermes;
mod inode_fs;
mod keys;
#[allow(dead_code)]
mod pixel;
mod preprocessor;
mod qemu;
mod render;
mod save;
mod vfs;
#[allow(dead_code)]
mod vision;
mod vm;

use qemu::QemuBridge;

use minifb::{Key, KeyRepeat, Window, WindowOptions};
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

use audio::play_beep;
use canvas::*;
use cli::cli_main;
use hermes::{run_build_canvas, run_hermes_canvas};
use keys::{key_to_ascii, key_to_ascii_shifted};
use render::*;
use save::{load_state, save_full_buffer_png, save_screen_png, save_state};

// ── Memory map ───────────────────────────────────────────────────
const KEYS_BITMASK_PORT: usize = 0xFFB;
const NET_PORT: usize = 0xFFC;
#[allow(dead_code)]
const TICKS_PORT: usize = 0xFFE;
#[allow(dead_code)]
const KEY_PORT: usize = 0xFFF;

// ── Save file ───────────────────────────────────────────────────
const SAVE_FILE: &str = "geometry_os.sav";

// ── Terminal mode ──────────────────────────────────────────────────
#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Terminal,
    Editor,
}

/// Auto-decode .rts.png pixel paths in a QEMU config string.
/// Replaces kernel=/initrd=.rts.png with temp file paths.
fn resolve_qemu_pixel_paths(config: &str) -> String {
    let mut result = config.to_string();
    for key in &["kernel", "initrd", "dtb", "drive"] {
        if let Some(start) = result.find(&format!("{}=", key)) {
            let val_start = start + key.len() + 1;
            let val_end = result[val_start..]
                .find(' ')
                .map(|i| val_start + i)
                .unwrap_or(result.len());
            let value = &result[val_start..val_end];

            if value.to_lowercase().ends_with(".rts.png") {
                if let Ok(temp_path) = geometry_os::pixel::decode_rts_to_temp(value) {
                    result.replace_range(val_start..val_end, &temp_path);
                }
            }
        }
    }
    result
}

fn main() {
    // ── CLI mode: headless geo> prompt on stdin/stdout ─────────────
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "--cli" {
        cli_main(&args[2..]);
        return;
    }

    // Networking setup
    let mut local_port = 9000;
    let mut remote_port = 9001;
    let mut boot_mode = false;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--local-port" && i + 1 < args.len() {
            local_port = args[i + 1].parse().unwrap_or(9000);
            i += 2;
        } else if args[i] == "--remote-port" && i + 1 < args.len() {
            remote_port = args[i + 1].parse().unwrap_or(9001);
            i += 2;
        } else if args[i] == "--boot" {
            boot_mode = true;
            i += 1;
        } else {
            i += 1;
        }
    }
    let socket = std::net::UdpSocket::bind(format!("127.0.0.1:{}", local_port)).ok();
    if let Some(ref s) = socket {
        let _ = s.set_nonblocking(true);
    }

    let mut window = Window::new(
        "Geometry OS -- Canvas Text Surface",
        WIDTH,
        HEIGHT,
        WindowOptions {
            resize: false,
            ..Default::default()
        },
    )
    .expect("Failed to create window. Ensure a display is available.");

    window.set_target_fps(60);

    let mut buffer: Vec<u32> = vec![0; WIDTH * HEIGHT];

    // ── State ────────────────────────────────────────────────────
    let mut vm = vm::Vm::new();
    let mut is_running = false;
    let mut canvas_assembled = false;
    let mut breakpoints: HashSet<u32> = HashSet::new();
    let mut hit_breakpoint = false;
    let mut recording = false;
    let mut frame_id = 0;

    // Visual Debugger state
    let mut ram_intensity = vec![0.0f32; vm::RAM_SIZE];
    let mut ram_kind = vec![vm::MemAccessKind::Read; vm::RAM_SIZE];
    let mut pc_history: VecDeque<u32> = VecDeque::with_capacity(64);
    let mut ram_view_base: usize = 0x2000;

    // Cursor position on canvas (logical coordinates, can exceed visible area)
    let mut cursor_row: usize;
    let mut cursor_col: usize;

    // Scroll offset: which logical row is at the top of the visible window
    let mut scroll_offset: usize;

    // Canvas backing buffer (separate from VM RAM to allow > 32 rows
    // without overlapping bytecode at 0x1000)
    let mut canvas_buffer: Vec<u32> = vec![0; CANVAS_MAX_ROWS * CANVAS_COLS];

    // Status bar message
    let mut status_msg = String::from("[TERM: type commands, Enter=run]");

    // Last loaded file (for Ctrl+F8 reload)
    let mut loaded_file: Option<PathBuf> = None;

    // If --boot flag, perform boot sequence: load init.asm as PID 1
    if boot_mode {
        match vm.boot() {
            Ok(pid) => {
                status_msg = format!("[BOOT] init started as PID {}", pid);
                is_running = true;
            }
            Err(e) => {
                status_msg = format!("[BOOT FAILED] {}", e);
            }
        }
    }

    // ── Mode state ──────────────────────────────────────────────
    let mut mode = Mode::Terminal;
    // In terminal mode, track which row the prompt is on
    let mut term_prompt_row: usize;
    // The "output row" for terminal -- where next line goes
    let mut term_output_row: usize;

    // ── QEMU bridge state ───────────────────────────────────────
    let mut qemu_bridge: Option<QemuBridge> = None;
    let mut qemu_active: bool = false;
    let mut qemu_exited: bool = false;

    // Boot: write welcome banner + first prompt into canvas
    {
        term_output_row = write_line_to_canvas(&mut canvas_buffer, 0, "Geometry OS v1.0.0");
        term_output_row = write_line_to_canvas(
            &mut canvas_buffer,
            term_output_row,
            "40 opcodes | 32 regs | 256x256",
        );
        term_output_row = write_line_to_canvas(
            &mut canvas_buffer,
            term_output_row,
            "Type 'help' for commands.",
        );
        term_output_row = write_line_to_canvas(&mut canvas_buffer, term_output_row, "");
        term_prompt_row = term_output_row;
        let _ = write_line_to_canvas(&mut canvas_buffer, term_output_row, "geo> ");
        // Position cursor after "geo> "
        cursor_row = term_prompt_row;
        cursor_col = 5; // after "geo> "
        scroll_offset = 0;
    }

    // File input mode (Ctrl+F8 activates this)
    let mut file_input_mode = false;
    let mut file_input_buf = String::new();
    let mut file_completions: Vec<String> = Vec::new();
    let mut file_completion_idx: usize = 0;

    // Load file from command-line argument at startup
    if let Some(path_str) = std::env::args().nth(1) {
        let path = PathBuf::from(&path_str);
        if let Ok(source) = std::fs::read_to_string(&path) {
            load_source_to_canvas(
                &mut canvas_buffer,
                &source,
                &mut cursor_row,
                &mut cursor_col,
            );
            scroll_offset = 0;
            status_msg = format!("[loaded: {}]", path.display());
            loaded_file = Some(path);
        } else {
            status_msg = format!("[error: could not read {}]", path_str);
        }
    }

    // Restore saved state on startup (only if no command-line arg)
    if std::env::args().nth(1).is_none() {
        if let Ok((saved_vm, saved_canvas, saved_assembled)) = load_state(SAVE_FILE) {
            vm = saved_vm;
            canvas_buffer = saved_canvas;
            canvas_assembled = saved_assembled;
            status_msg = String::from("[state restored from geometry_os.sav]");
        }
    }

    // ── Unix socket command channel (for AI/remote control) ─────
    let cmd_sock_path = "/tmp/geo_cmd.sock";
    let _ = std::fs::remove_file(cmd_sock_path);
    let cmd_listener = std::os::unix::net::UnixListener::bind(cmd_sock_path).ok();
    if let Some(ref l) = cmd_listener {
        l.set_nonblocking(true).ok();
    }

    // ── Main loop ────────────────────────────────────────────────
    let mut should_quit = false;
    while window.is_open() && !should_quit {
        // ── Handle input ─────────────────────────────────────────
        if is_running {
            let mut mask: u32 = 0;
            if window.is_key_down(Key::Up) || window.is_key_down(Key::W) {
                mask |= 1 << 0;
            }
            if window.is_key_down(Key::Down) || window.is_key_down(Key::S) {
                mask |= 1 << 1;
            }
            if window.is_key_down(Key::Left) || window.is_key_down(Key::A) {
                mask |= 1 << 2;
            }
            if window.is_key_down(Key::Right) || window.is_key_down(Key::D) {
                mask |= 1 << 3;
            }
            if window.is_key_down(Key::Space) {
                mask |= 1 << 4;
            }
            if window.is_key_down(Key::Enter) {
                mask |= 1 << 5;
            }
            vm.ram[KEYS_BITMASK_PORT] = mask;

            // ── Networking ───────────────────────────────────────
            if let Some(ref s) = socket {
                let val = vm.ram[NET_PORT];
                if val != 0 {
                    // VM wrote something, send it
                    let _ = s.send_to(&val.to_le_bytes(), format!("127.0.0.1:{}", remote_port));
                    vm.ram[NET_PORT] = 0; // clear after send
                } else {
                    // Port is empty, try to receive
                    let mut buf = [0u8; 4];
                    if let Ok((amt, _src)) = s.recv_from(&mut buf) {
                        if amt == 4 {
                            vm.ram[NET_PORT] = u32::from_le_bytes(buf);
                        }
                    }
                }
            }
        }

        for key in window.get_keys_pressed(KeyRepeat::No) {
            if is_running {
                // Runtime: send keys to VM key ring buffer
                if let Some(ch) = key_to_ascii(key) {
                    vm.push_key(ch as u32);
                }
                continue;
            }

            // ── QEMU exited: any key returns to terminal ──────────────
            if qemu_exited {
                qemu_exited = false;
                // Restore normal canvas
                canvas_buffer.fill(0);
                term_output_row = write_line_to_canvas(&mut canvas_buffer, 0, "geo> ");
                term_prompt_row = 0;
                cursor_row = 0;
                cursor_col = 5;
                scroll_offset = 0;
                status_msg = String::from("[TERM: type commands, Enter=run]");
                continue;
            }

            // ── QEMU mode: forward all keys to QEMU stdin ────────────
            if qemu_active {
                if key == Key::Escape {
                    // Exit QEMU mode
                    if let Some(ref mut bridge) = qemu_bridge {
                        let _ = bridge.kill();
                    }
                    qemu_bridge = None;
                    qemu_active = false;
                    status_msg = String::from("[QEMU] Exited");
                    // Restore normal canvas
                    canvas_buffer.fill(0);
                    term_output_row = write_line_to_canvas(&mut canvas_buffer, 0, "geo> ");
                    term_prompt_row = 0;
                    cursor_row = 0;
                    cursor_col = 5;
                    scroll_offset = 0;
                    continue;
                }
                // Forward printable characters and Enter to QEMU
                if let Some(ref mut bridge) = qemu_bridge {
                    let shift =
                        window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
                    match key {
                        Key::Enter => {
                            let _ = bridge.write_bytes(b"\n");
                        }
                        Key::Backspace => {
                            let _ = bridge.write_bytes(b"\x08");
                        }
                        Key::Tab => {
                            let _ = bridge.write_bytes(b"\t");
                        }
                        Key::Up => {
                            let _ = bridge.write_bytes(b"\x1b[A");
                        }
                        Key::Down => {
                            let _ = bridge.write_bytes(b"\x1b[B");
                        }
                        Key::Right => {
                            let _ = bridge.write_bytes(b"\x1b[C");
                        }
                        Key::Left => {
                            let _ = bridge.write_bytes(b"\x1b[D");
                        }
                        _ => {
                            if let Some(ch) = key_to_ascii_shifted(key, shift) {
                                let _ = bridge.write_bytes(&[ch]);
                            }
                        }
                    }
                }
                continue;
            }

            // Escape: in editor mode, switch back to terminal. In terminal, quit.
            if key == Key::Escape {
                if mode == Mode::Editor {
                    mode = Mode::Terminal;
                    status_msg = String::from("[TERM: type commands, Enter=run]");
                    // Set cursor to after the last "geo> " prompt
                    cursor_row = term_prompt_row;
                    cursor_col = 5;
                    ensure_cursor_visible(&cursor_row, &mut scroll_offset);
                    continue;
                } else {
                    should_quit = true;
                    break;
                }
            }

            // File input mode: Ctrl+F8 activates, handles typing a path
            if file_input_mode {
                match key {
                    Key::Escape => {
                        file_input_mode = false;
                        file_input_buf.clear();
                        status_msg =
                            String::from("[TEXT mode: type assembly, F8=assemble, F5=run]");
                    }
                    Key::Enter => {
                        // Attempt to load the file
                        let path = Path::new(&file_input_buf);
                        if let Ok(source) = std::fs::read_to_string(path) {
                            load_source_to_canvas(
                                &mut canvas_buffer,
                                &source,
                                &mut cursor_row,
                                &mut cursor_col,
                            );
                            scroll_offset = 0;
                            loaded_file = Some(path.to_path_buf());
                            status_msg = format!("[loaded: {}]", file_input_buf);
                        } else {
                            status_msg = format!("[error: cannot read {}]", file_input_buf);
                        }
                        file_input_mode = false;
                        file_input_buf.clear();
                    }
                    Key::Backspace => {
                        file_input_buf.pop();
                        status_msg = format!(
                            "[load file: {} | Tab=complete, Enter=load, Esc=cancel]",
                            file_input_buf
                        );
                    }
                    Key::Tab => {
                        // Cycle through completions from programs/*.asm
                        if !file_completions.is_empty() {
                            file_completion_idx =
                                (file_completion_idx + 1) % file_completions.len();
                            file_input_buf = file_completions[file_completion_idx].clone();
                            status_msg = format!(
                                "[load file: {} | Tab=complete, Enter=load, Esc=cancel]",
                                file_input_buf
                            );
                        }
                    }
                    _ => {
                        // Type characters into the path buffer
                        let shift = window.is_key_down(Key::LeftShift)
                            || window.is_key_down(Key::RightShift);
                        if let Some(ch) = key_to_ascii_shifted(key, shift) {
                            file_input_buf.push(ch as char);
                            // Reset completion index when user types manually
                            file_completion_idx = 0;
                            status_msg = format!(
                                "[load file: {} | Tab=complete, Enter=load, Esc=cancel]",
                                file_input_buf
                            );
                        }
                    }
                }
                continue;
            }

            // ── Mode-aware key handling ───────────────────────────
            if mode == Mode::Terminal {
                // Terminal mode: type into prompt line, Enter = execute
                match key {
                    Key::Enter => {
                        // Read command text from prompt row (skip "geo> " prefix)
                        let raw = read_canvas_line(&canvas_buffer, term_prompt_row);
                        let cmd = raw.strip_prefix("geo> ").unwrap_or(&raw);
                        let cmd = cmd.trim();

                        // ── QEMU command interception ────────────────────
                        if cmd.starts_with("qemu") {
                            let parts: Vec<&str> = cmd.split_whitespace().collect();
                            let subcmd = parts.get(1).copied().unwrap_or("");
                            match subcmd {
                                "boot" => {
                                    if parts.len() < 3 {
                                        term_output_row = write_line_to_canvas(
                                            &mut canvas_buffer,
                                            term_output_row,
                                            "Usage: qemu boot <config>",
                                        );
                                        term_output_row = write_line_to_canvas(
                                            &mut canvas_buffer,
                                            term_output_row,
                                            "  e.g. qemu boot arch=riscv64 kernel=Image ram=256M",
                                        );
                                        term_output_row = write_line_to_canvas(
                                            &mut canvas_buffer,
                                            term_output_row,
                                            "geo> ",
                                        );
                                        ensure_scroll(term_output_row, &mut scroll_offset);
                                        continue;
                                    }
                                    // Kill any existing QEMU first
                                    if let Some(ref mut bridge) = qemu_bridge {
                                        let _ = bridge.kill();
                                    }
                                    qemu_bridge = None;
                                    qemu_active = false;

                                    let mut config_str = parts[2..].join(" ");
                                    // Auto-decode .rts.png files to temp files
                                    config_str = resolve_qemu_pixel_paths(&config_str);
                                    match QemuBridge::spawn(&config_str) {
                                        Ok(bridge) => {
                                            // Clear canvas for QEMU terminal output
                                            canvas_buffer.fill(0);
                                            scroll_offset = 0;
                                            qemu_active = true;
                                            qemu_bridge = Some(bridge);
                                            status_msg = String::from(
                                                "[QEMU] Running -- Esc to exit, type to send",
                                            );
                                            continue;
                                        }
                                        Err(e) => {
                                            term_output_row = write_line_to_canvas(
                                                &mut canvas_buffer,
                                                term_output_row,
                                                &format!("[qemu] Error: {}", e),
                                            );
                                            term_output_row = write_line_to_canvas(
                                                &mut canvas_buffer,
                                                term_output_row,
                                                "geo> ",
                                            );
                                            ensure_scroll(term_output_row, &mut scroll_offset);
                                            continue;
                                        }
                                    }
                                }
                                "kill" => {
                                    if let Some(ref mut bridge) = qemu_bridge {
                                        let _ = bridge.kill();
                                        qemu_bridge = None;
                                        qemu_active = false;
                                        status_msg = String::from("[QEMU] Killed");
                                        // Restore normal canvas
                                        canvas_buffer.fill(0);
                                        term_output_row =
                                            write_line_to_canvas(&mut canvas_buffer, 0, "geo> ");
                                        term_prompt_row = 0;
                                        cursor_row = 0;
                                        cursor_col = 5;
                                        scroll_offset = 0;
                                    } else {
                                        status_msg = String::from("[QEMU] Not running");
                                    }
                                    continue;
                                }
                                "status" => {
                                    if let Some(ref mut bridge) = qemu_bridge {
                                        if bridge.is_alive() {
                                            status_msg = String::from("[QEMU] Running");
                                        } else {
                                            status_msg = String::from("[QEMU] Exited");
                                            qemu_bridge = None;
                                        }
                                    } else {
                                        status_msg = String::from("[QEMU] Not running");
                                    }
                                    continue;
                                }
                                _ => {
                                    term_output_row = write_line_to_canvas(
                                        &mut canvas_buffer,
                                        term_output_row,
                                        "Usage: qemu <boot|kill|status>",
                                    );
                                    term_output_row = write_line_to_canvas(
                                        &mut canvas_buffer,
                                        term_output_row,
                                        "geo> ",
                                    );
                                    ensure_scroll(term_output_row, &mut scroll_offset);
                                    continue;
                                }
                            }
                        }

                        // Output goes on the line after the prompt
                        term_output_row = term_prompt_row + 1;

                        let (hermes_prompt, go_edit, quit) = handle_terminal_command(
                            cmd,
                            &mut vm,
                            &mut canvas_buffer,
                            &mut term_output_row,
                            &mut scroll_offset,
                            &mut loaded_file,
                            &mut canvas_assembled,
                            &mut breakpoints,
                        );

                        // Handle hermes/build prompt if returned
                        if let Some(prompt) = hermes_prompt {
                            if let Some(build_prompt) = prompt.strip_prefix("build:") {
                                run_build_canvas(
                                    build_prompt,
                                    &mut vm,
                                    &mut canvas_buffer,
                                    &mut term_output_row,
                                    &mut scroll_offset,
                                    &mut loaded_file,
                                    &mut canvas_assembled,
                                    &mut breakpoints,
                                );
                            } else {
                                let hermes_prompt_str =
                                    prompt.strip_prefix("hermes:").unwrap_or(&prompt);
                                run_hermes_canvas(
                                    hermes_prompt_str,
                                    &mut vm,
                                    &mut canvas_buffer,
                                    &mut term_output_row,
                                    &mut scroll_offset,
                                    &mut loaded_file,
                                    &mut canvas_assembled,
                                    &mut breakpoints,
                                );
                            }
                            term_output_row =
                                write_line_to_canvas(&mut canvas_buffer, term_output_row, "geo> ");
                            ensure_scroll(term_output_row, &mut scroll_offset);
                            term_prompt_row = term_output_row - 1;
                            cursor_row = term_prompt_row;
                            cursor_col = 5;
                            ensure_cursor_visible(&cursor_row, &mut scroll_offset);
                            continue;
                        }

                        if quit {
                            should_quit = true;
                            break;
                        }

                        if go_edit {
                            mode = Mode::Editor;
                            status_msg = String::from(
                                "[EDIT mode: type assembly, F8=assemble, F5=run, Esc=terminal]",
                            );
                            // Position cursor at start of canvas
                            cursor_row = 0;
                            cursor_col = 0;
                            scroll_offset = 0;
                        } else {
                            // Track the new prompt position
                            term_prompt_row = term_output_row - 1; // write_line left us after the "geo> " line
                            cursor_row = term_prompt_row;
                            cursor_col = 5; // after "geo> "
                            ensure_cursor_visible(&cursor_row, &mut scroll_offset);
                            // Update term_output_row for next command
                            // (it's already set past the "geo> " prompt)
                        }
                    }
                    Key::Backspace => {
                        if cursor_col > 5 {
                            cursor_col -= 1;
                            let idx = cursor_row * CANVAS_COLS + cursor_col;
                            canvas_buffer[idx] = 0;
                        }
                    }
                    Key::Up => {
                        if cursor_row > 0 {
                            cursor_row -= 1;
                            ensure_cursor_visible(&cursor_row, &mut scroll_offset);
                        }
                    }
                    Key::Down => {
                        if cursor_row < CANVAS_MAX_ROWS - 1 {
                            cursor_row += 1;
                            ensure_cursor_visible(&cursor_row, &mut scroll_offset);
                        }
                    }
                    _ => {
                        // Type characters into prompt line
                        let shift = window.is_key_down(Key::LeftShift)
                            || window.is_key_down(Key::RightShift);
                        if let Some(ch) = key_to_ascii_shifted(key, shift) {
                            if cursor_col < CANVAS_COLS - 1 {
                                let idx = cursor_row * CANVAS_COLS + cursor_col;
                                canvas_buffer[idx] = ch as u32;
                                cursor_col += 1;
                            }
                        }
                    }
                }
                continue;
            }

            // ── Editor mode: canvas editing (VM paused) ──────────
            match key {
                Key::Enter => {
                    let idx = cursor_row * CANVAS_COLS + cursor_col;
                    canvas_buffer[idx] = '\n' as u32;
                    cursor_col = 0;
                    cursor_row += 1;
                    if cursor_row >= CANVAS_MAX_ROWS {
                        cursor_row = CANVAS_MAX_ROWS - 1;
                    }
                    ensure_cursor_visible(&cursor_row, &mut scroll_offset);
                }
                Key::Space => {
                    let idx = cursor_row * CANVAS_COLS + cursor_col;
                    canvas_buffer[idx] = 0x20;
                    advance_cursor(
                        &mut canvas_buffer,
                        &mut cursor_row,
                        &mut cursor_col,
                        &mut scroll_offset,
                    );
                }
                Key::Backspace => {
                    if cursor_col > 0 {
                        cursor_col -= 1;
                    } else if cursor_row > 0 {
                        cursor_row -= 1;
                        cursor_col = CANVAS_COLS - 1;
                    }
                    let idx = cursor_row * CANVAS_COLS + cursor_col;
                    canvas_buffer[idx] = 0;
                    ensure_cursor_visible(&cursor_row, &mut scroll_offset);
                }
                Key::F5 => {
                    if vm.halted {
                        vm.pc = if canvas_assembled {
                            CANVAS_BYTECODE_ADDR as u32
                        } else {
                            0
                        };
                        vm.halted = false;
                    }
                    hit_breakpoint = false;
                    is_running = !is_running;
                }
                Key::F6 => {
                    // Single-step: execute one instruction when paused
                    if !is_running && !vm.halted && canvas_assembled {
                        hit_breakpoint = false;
                        vm.step();
                        if breakpoints.contains(&vm.pc) {
                            hit_breakpoint = true;
                        }
                        status_msg = format!("[step] PC=0x{:04X}", vm.pc);
                    }
                }
                Key::F8 => {
                    let ctrl =
                        window.is_key_down(Key::LeftCtrl) || window.is_key_down(Key::RightCtrl);
                    if ctrl {
                        // Ctrl+F8: enter file input mode
                        file_input_mode = true;
                        file_input_buf.clear();
                        file_completions = list_asm_files("programs");
                        file_completion_idx = 0;
                        // Pre-populate with last loaded file path if available
                        if let Some(ref path) = loaded_file {
                            file_input_buf = path.to_string_lossy().to_string();
                        }
                        status_msg = format!(
                            "[load file: {} | Tab=complete, Enter=load, Esc=cancel]",
                            file_input_buf
                        );
                    } else {
                        canvas_assemble(
                            &canvas_buffer,
                            &mut vm,
                            &mut canvas_assembled,
                            &mut status_msg,
                        );
                    }
                }
                Key::F7 => {
                    // Save state to file
                    match save_state(SAVE_FILE, &vm, &canvas_buffer, canvas_assembled) {
                        Ok(()) => {
                            let file_size =
                                std::fs::metadata(SAVE_FILE).map(|m| m.len()).unwrap_or(0);
                            status_msg = format!(
                                "[saved: {} ({:.0}KB)]",
                                SAVE_FILE,
                                file_size as f64 / 1024.0
                            );
                        }
                        Err(e) => {
                            status_msg = format!("[save error: {}]", e);
                        }
                    }
                }
                Key::F9 => {
                    // Screenshot: save screen as PNG
                    let png_path = "screenshot.png";
                    match save_screen_png(png_path, &vm.screen) {
                        Ok(()) => {
                            let file_size =
                                std::fs::metadata(png_path).map(|m| m.len()).unwrap_or(0);
                            status_msg = format!(
                                "[screenshot: {} ({:.0}KB)]",
                                png_path,
                                file_size as f64 / 1024.0
                            );
                        }
                        Err(e) => {
                            status_msg = format!("[screenshot error: {}]", e);
                        }
                    }
                }
                Key::F10 => {
                    recording = !recording;
                    if recording {
                        frame_id = 0;
                        let _ = std::fs::create_dir_all("/tmp/geo_frames");
                        status_msg = String::from("[RECORDING STARTED: /tmp/geo_frames/]");
                    } else {
                        status_msg = format!(
                            "[RECORDING STOPPED: {} frames saved. Use ffmpeg to compile GIF]",
                            frame_id
                        );
                    }
                }
                Key::PageUp => {
                    if mode == Mode::Terminal {
                        ram_view_base = ram_view_base.saturating_sub(1024);
                        status_msg = format!(
                            "[RAM Inspector: 0x{:04X}-0x{:04X}]",
                            ram_view_base,
                            ram_view_base + 1023
                        );
                    } else if scroll_offset > 0 {
                        scroll_offset = scroll_offset.saturating_sub(CANVAS_ROWS);
                        let new_cursor = scroll_offset + CANVAS_ROWS / 2;
                        if new_cursor < cursor_row || cursor_row < scroll_offset {
                            cursor_row = new_cursor.min(CANVAS_MAX_ROWS - 1);
                        }
                    }
                }
                Key::PageDown => {
                    if mode == Mode::Terminal {
                        ram_view_base = ram_view_base.saturating_add(1024).min(0xFC00);
                        status_msg = format!(
                            "[RAM Inspector: 0x{:04X}-0x{:04X}]",
                            ram_view_base,
                            ram_view_base + 1023
                        );
                    } else {
                        let max_scroll = CANVAS_MAX_ROWS.saturating_sub(CANVAS_ROWS);
                        if scroll_offset < max_scroll {
                            scroll_offset = (scroll_offset + CANVAS_ROWS).min(max_scroll);
                            let new_cursor = scroll_offset + CANVAS_ROWS / 2;
                            if new_cursor > cursor_row || cursor_row >= scroll_offset + CANVAS_ROWS
                            {
                                cursor_row = new_cursor.min(CANVAS_MAX_ROWS - 1);
                            }
                        }
                    }
                }
                Key::Left => {
                    cursor_col = cursor_col.saturating_sub(1);
                }
                Key::Right => {
                    if cursor_col < CANVAS_COLS - 1 {
                        cursor_col += 1;
                    }
                }
                Key::Up => {
                    cursor_row = cursor_row.saturating_sub(1);
                    ensure_cursor_visible(&cursor_row, &mut scroll_offset);
                }
                Key::Down => {
                    if cursor_row < CANVAS_MAX_ROWS - 1 {
                        cursor_row += 1;
                    }
                    ensure_cursor_visible(&cursor_row, &mut scroll_offset);
                }
                Key::V => {
                    let ctrl =
                        window.is_key_down(Key::LeftCtrl) || window.is_key_down(Key::RightCtrl);
                    if ctrl {
                        match arboard::Clipboard::new() {
                            Ok(mut clipboard) => match clipboard.get_text() {
                                Ok(text) => {
                                    let pasted = paste_text_to_canvas(
                                        &mut canvas_buffer,
                                        &text,
                                        &mut cursor_row,
                                        &mut cursor_col,
                                    );
                                    ensure_cursor_visible(&cursor_row, &mut scroll_offset);
                                    status_msg = format!("[pasted {} chars]", pasted);
                                }
                                Err(e) => {
                                    status_msg = format!("[paste error: {}]", e);
                                }
                            },
                            Err(e) => {
                                status_msg = format!("[clipboard error: {}]", e);
                            }
                        }
                    } else {
                        let shift = window.is_key_down(Key::LeftShift)
                            || window.is_key_down(Key::RightShift);
                        if let Some(ch) = key_to_ascii_shifted(Key::V, shift) {
                            let idx = cursor_row * CANVAS_COLS + cursor_col;
                            canvas_buffer[idx] = ch as u32;
                            advance_cursor(
                                &mut canvas_buffer,
                                &mut cursor_row,
                                &mut cursor_col,
                                &mut scroll_offset,
                            );
                        }
                    }
                }
                _ => {
                    let shift =
                        window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
                    if let Some(ch) = key_to_ascii_shifted(key, shift) {
                        let idx = cursor_row * CANVAS_COLS + cursor_col;
                        canvas_buffer[idx] = ch as u32;
                        advance_cursor(
                            &mut canvas_buffer,
                            &mut cursor_row,
                            &mut cursor_col,
                            &mut scroll_offset,
                        );
                    }
                }
            }
        }

        // ── QEMU output polling ────────────────────────────────────
        if qemu_active {
            if let Some(ref mut bridge) = qemu_bridge {
                bridge.read_output(&mut canvas_buffer);
                if !bridge.is_alive() {
                    qemu_active = false;
                    qemu_exited = true;
                    qemu_bridge = None;
                    status_msg = String::from("[QEMU] Process exited -- press any key");
                }
            }
        }

        // ── VM execution ─────────────────────────────────────────
        if is_running && !vm.halted {
            // Phase 45: Sync canvas buffer TO VM before execution
            vm.canvas_buffer.copy_from_slice(&canvas_buffer);

            // Run until FRAME, breakpoint, halt, or 1M steps (safety cap)
            vm.frame_ready = false;
            for _ in 0..1_000_000 {
                if !vm.step() {
                    is_running = false;
                    break;
                }
                // Step all spawned child processes in lock-step with the primary
                vm.step_all_processes();
                if vm.frame_ready {
                    // FRAME opcode hit: stop here, let the host render this tick
                    break;
                }
                if breakpoints.contains(&vm.pc) {
                    is_running = false;
                    hit_breakpoint = true;
                    status_msg = format!("[BREAK] PC=0x{:04X}", vm.pc);
                    break;
                }
            }

            // Phase 45: Sync canvas buffer FROM VM after execution
            canvas_buffer.copy_from_slice(&vm.canvas_buffer);
        }

        // ── Audio dispatch ───────────────────────────────────────
        if let Some((freq, dur)) = vm.beep.take() {
            play_beep(freq, dur);
        }
        if let Some((wave, freq, dur)) = vm.note.take() {
            audio::play_note(audio::Waveform::from_u32(wave), freq, dur);
        }

        // ── Shutdown check ────────────────────────────────────────
        if vm.shutdown_requested {
            status_msg = "[SHUTDOWN] System halted cleanly.".into();
            is_running = false;
            let _ = status_msg; // suppress unused warning (break follows)
            let _ = is_running;
            break;
        }

        // ── Process Unix socket commands (AI control) ──────────
        if let Some(ref listener) = cmd_listener {
            while let Ok((mut stream, _)) = listener.accept() {
                use std::io::{Read, Write};
                let mut buf = [0u8; 4096];
                let mut response = String::new();
                if let Ok(n) = stream.read(&mut buf) {
                    let cmd = String::from_utf8_lossy(&buf[..n]);
                    for line in cmd.lines() {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.is_empty() {
                            continue;
                        }
                        match parts[0] {
                            "save" => {
                                match save_state(SAVE_FILE, &vm, &canvas_buffer, canvas_assembled) {
                                    Ok(()) => status_msg = "[saved]".into(),
                                    Err(e) => status_msg = format!("[save error: {}]", e),
                                }
                            }
                            "screenshot" => {
                                let path = parts.get(1).copied().unwrap_or("screenshot.png");
                                match save_full_buffer_png(path, &buffer, WIDTH, HEIGHT) {
                                    Ok(()) => status_msg = format!("[screenshot: {}]", path),
                                    Err(e) => status_msg = format!("[screenshot error: {}]", e),
                                }
                            }
                            "canvas" => {
                                let mut out = String::new();
                                for row in 0..CANVAS_MAX_ROWS {
                                    let mut ln = String::new();
                                    for col in 0..CANVAS_COLS {
                                        let val = canvas_buffer[row * CANVAS_COLS + col];
                                        if val > 0 && val < 128 {
                                            ln.push(val as u8 as char);
                                        } else {
                                            ln.push(' ');
                                        }
                                    }
                                    let trimmed = ln.trim_end();
                                    if !trimmed.is_empty() {
                                        out.push_str(&format!("{}|{}\n", row, trimmed));
                                    }
                                }
                                response.push_str(&out);
                            }
                            "assemble" | "asm" => {
                                canvas_assemble(
                                    &canvas_buffer,
                                    &mut vm,
                                    &mut canvas_assembled,
                                    &mut status_msg,
                                );
                                response.push_str(&format!("{}\n", status_msg));
                            }
                            "run" => {
                                if vm.halted {
                                    vm.pc = if canvas_assembled {
                                        CANVAS_BYTECODE_ADDR as u32
                                    } else {
                                        0
                                    };
                                    vm.halted = false;
                                }
                                hit_breakpoint = false;
                                is_running = !is_running;
                            }
                            "type" => {
                                // Type text onto canvas. Use \\n (literal backslash-n)
                                // for newline since socket protocol strips actual newlines.
                                if line.len() > 5 {
                                    let text = line[5..].replace("\\n", "\n");
                                    for ch in text.chars() {
                                        if ch == '\n' {
                                            cursor_col = 0;
                                            cursor_row += 1;
                                            if cursor_row >= CANVAS_MAX_ROWS {
                                                cursor_row = CANVAS_MAX_ROWS - 1;
                                            }
                                        } else if cursor_col < CANVAS_COLS {
                                            canvas_buffer[cursor_row * CANVAS_COLS + cursor_col] =
                                                ch as u32;
                                            cursor_col += 1;
                                            // Auto-wrap at end of line
                                            if cursor_col >= CANVAS_COLS {
                                                cursor_col = 0;
                                                cursor_row += 1;
                                                if cursor_row >= CANVAS_MAX_ROWS {
                                                    cursor_row = CANVAS_MAX_ROWS - 1;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            "clear" => {
                                canvas_buffer.fill(0);
                                cursor_row = 0;
                                cursor_col = 0;
                                scroll_offset = 0;
                                term_output_row = 0;
                            }
                            "status" => {
                                response.push_str(&format!(
                                    "mode={:?} running={} assembled={} pc=0x{:04X} cursor=({},{})\n",
                                    mode, is_running, canvas_assembled, vm.pc, cursor_row, cursor_col
                                ));
                            }
                            "screen" => {
                                let mut out = String::new();
                                for y in 0..256 {
                                    let mut row = String::new();
                                    for x in 0..256 {
                                        let px = vm.screen[y * 256 + x];
                                        row.push_str(&format!("{:06x} ", px));
                                    }
                                    out.push_str(row.trim_end());
                                    out.push('\n');
                                }
                                response.push_str(&out);
                            }
                            "registers" | "regs" => {
                                for i in 0..32 {
                                    response.push_str(&format!("r{:02}={:08X}\n", i, vm.regs[i]));
                                }
                            }
                            "disasm" => {
                                let pc = vm.pc;
                                // Try to decode around PC
                                let bases = [0u32, CANVAS_BYTECODE_ADDR as u32];
                                let mut inst_starts: std::collections::BTreeSet<u32> =
                                    std::collections::BTreeSet::new();
                                for &base in &bases {
                                    if pc >= base && pc < base + 0x1000 {
                                        let mut addr = base;
                                        while addr <= pc + 30 {
                                            if addr as usize >= vm.ram.len() {
                                                break;
                                            }
                                            let op = vm.ram[addr as usize];
                                            if op == 0 && addr > pc + 20 {
                                                break;
                                            }
                                            inst_starts.insert(addr);
                                            let (_, len) = vm.disassemble_at(addr);
                                            if len == 0 {
                                                break;
                                            }
                                            addr += len as u32;
                                        }
                                    }
                                }
                                let mut display_addrs: Vec<u32> = Vec::new();
                                let mut before: Vec<u32> = Vec::new();
                                let mut after: Vec<u32> = Vec::new();
                                for &a in &inst_starts {
                                    if a < pc {
                                        before.push(a);
                                        if before.len() > 4 {
                                            before.remove(0);
                                        }
                                    } else if a == pc { /* skip, add later */
                                    } else {
                                        after.push(a);
                                        if after.len() >= 5 {
                                            break;
                                        }
                                    }
                                }
                                display_addrs.extend_from_slice(&before);
                                if inst_starts.contains(&pc) {
                                    display_addrs.push(pc);
                                }
                                display_addrs.extend_from_slice(&after);
                                for &addr in &display_addrs {
                                    let (mnemonic, _) = vm.disassemble_at(addr);
                                    let marker = if addr == pc { ">" } else { " " };
                                    response.push_str(&format!(
                                        "{}{:04X} {}\n",
                                        marker, addr, mnemonic
                                    ));
                                }
                            }
                            "vmscreen" => {
                                // ASCII art of the 256x256 VM screen (compressed to 64x32)
                                let scale_x = 4;
                                let scale_y = 8;
                                for y in 0..32 {
                                    let mut row = String::new();
                                    for x in 0..64 {
                                        let sx = x * scale_x;
                                        let sy = y * scale_y;
                                        let mut lit = 0u32;
                                        let mut total = 0u32;
                                        for dy in 0..scale_y {
                                            for dx in 0..scale_x {
                                                let py = sy + dy;
                                                let px = sx + dx;
                                                if py < 256 && px < 256 {
                                                    if vm.screen[py * 256 + px] != 0 {
                                                        lit += 1;
                                                    }
                                                    total += 1;
                                                }
                                            }
                                        }
                                        let ratio = if total > 0 {
                                            lit as f32 / total as f32
                                        } else {
                                            0.0
                                        };
                                        let ch = if ratio > 0.75 {
                                            '#'
                                        } else if ratio > 0.50 {
                                            '@'
                                        } else if ratio > 0.25 {
                                            '+'
                                        } else if ratio > 0.05 {
                                            '.'
                                        } else {
                                            ' '
                                        };
                                        row.push(ch);
                                    }
                                    response.push_str(&(row.trim_end().to_string() + "\n"));
                                }
                            }
                            "ram" => {
                                let base = parts
                                    .get(1)
                                    .and_then(|s| {
                                        usize::from_str_radix(s.trim_start_matches("0x"), 16).ok()
                                    })
                                    .unwrap_or(ram_view_base);
                                let rows: usize =
                                    parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(8);
                                for r in 0..rows {
                                    let mut line = format!("{:04X}: ", base + r * 16);
                                    for c in 0..16 {
                                        let addr = base + r * 16 + c;
                                        if addr < vm.ram.len() {
                                            line.push_str(&format!("{:08X} ", vm.ram[addr]));
                                        }
                                    }
                                    response.push_str(&(line.trim_end().to_string() + "\n"));
                                }
                            }
                            "vm_state" | "vmstate" => {
                                // JSON-ish dump of key VM state
                                response.push_str(&format!("pc=0x{:04X}\n", vm.pc));
                                response.push_str(&format!("halted={}\n", vm.halted));
                                response.push_str(&format!("running={}\n", is_running));
                                response.push_str(&format!("assembled={}\n", canvas_assembled));
                                for i in 0..32 {
                                    response.push_str(&format!("r{:02}={:08X}\n", i, vm.regs[i]));
                                }
                            }
                            "dashboard" | "dash" => {
                                // Full ASCII dashboard of the GUI state
                                let state_label = if is_running {
                                    "RUNNING"
                                } else if vm.halted {
                                    "HALTED"
                                } else {
                                    "PAUSED"
                                };

                                response.push_str("╔════════════════════════════════════════════════════════════════╗\n");
                                response.push_str(&format!("║ Geometry OS Dashboard  {}  PC=0x{:04X}                              ║\n", state_label, vm.pc));
                                response.push_str("╠════════════════════════════════════════════════════════════════╣\n");

                                // Registers (4 per row)
                                response.push_str("║ REGS: ");
                                for i in 0..32 {
                                    response.push_str(&format!("r{:02}={:08X} ", i, vm.regs[i]));
                                    if (i + 1) % 4 == 0 {
                                        if i < 31 {
                                            response.push_str("║\n║       ");
                                        } else {
                                            response.push_str("║\n");
                                        }
                                    }
                                }

                                response.push_str("╠════════════════════════════════════════════════════════════════╣\n");

                                // Disassembly
                                let pc = vm.pc;
                                let bases = [0u32, CANVAS_BYTECODE_ADDR as u32];
                                let mut inst_starts2: std::collections::BTreeSet<u32> =
                                    std::collections::BTreeSet::new();
                                for &base in &bases {
                                    if pc >= base && pc < base + 0x1000 {
                                        let mut addr = base;
                                        while addr <= pc + 30 {
                                            if addr as usize >= vm.ram.len() {
                                                break;
                                            }
                                            let op = vm.ram[addr as usize];
                                            if op == 0 && addr > pc + 20 {
                                                break;
                                            }
                                            inst_starts2.insert(addr);
                                            let (_, len) = vm.disassemble_at(addr);
                                            if len == 0 {
                                                break;
                                            }
                                            addr += len as u32;
                                        }
                                    }
                                }
                                let mut da2: Vec<u32> = Vec::new();
                                let mut b2: Vec<u32> = Vec::new();
                                let mut a2: Vec<u32> = Vec::new();
                                for &a in &inst_starts2 {
                                    if a < pc {
                                        b2.push(a);
                                        if b2.len() > 2 {
                                            b2.remove(0);
                                        }
                                    } else if a == pc {
                                    } else {
                                        a2.push(a);
                                        if a2.len() >= 3 {
                                            break;
                                        }
                                    }
                                }
                                da2.extend_from_slice(&b2);
                                if inst_starts2.contains(&pc) {
                                    da2.push(pc);
                                }
                                da2.extend_from_slice(&a2);

                                response.push_str("║ DISASM:\n");
                                for &addr in &da2 {
                                    let (mnemonic, _) = vm.disassemble_at(addr);
                                    let marker = if addr == pc { ">>" } else { "  " };
                                    response.push_str(&format!(
                                        "║ {} {:04X} {}\n",
                                        marker, addr, mnemonic
                                    ));
                                }

                                response.push_str("╠════════════════════════════════════════════════════════════════╣\n");

                                // VM Screen (ASCII art, compact 32x16)
                                response.push_str("║ VM DISPLAY:\n");
                                let sx = 8;
                                let sy = 16;
                                for y in 0..16 {
                                    let mut row = String::new();
                                    for x in 0..32 {
                                        let mut lit = 0u32;
                                        let mut total = 0u32;
                                        for dy in 0..sy {
                                            for dx in 0..sx {
                                                let py = y * sy + dy;
                                                let px = x * sx + dx;
                                                if py < 256 && px < 256 {
                                                    if vm.screen[py * 256 + px] != 0 {
                                                        lit += 1;
                                                    }
                                                    total += 1;
                                                }
                                            }
                                        }
                                        let ratio = if total > 0 {
                                            lit as f32 / total as f32
                                        } else {
                                            0.0
                                        };
                                        row.push(if ratio > 0.5 {
                                            '#'
                                        } else if ratio > 0.1 {
                                            '.'
                                        } else {
                                            ' '
                                        });
                                    }
                                    response.push_str(&format!("║ {}\n", row.trim_end()));
                                }

                                response.push_str("╠════════════════════════════════════════════════════════════════╣\n");
                                response.push_str(&format!("║ {}\n", status_msg));
                                response.push_str("╚════════════════════════════════════════════════════════════════╝\n");
                            }
                            "load" => {
                                // Load an .asm file onto the canvas
                                if let Some(path) = parts.get(1) {
                                    match std::fs::read_to_string(path) {
                                        Ok(source) => {
                                            load_source_to_canvas(
                                                &mut canvas_buffer,
                                                &source,
                                                &mut cursor_row,
                                                &mut cursor_col,
                                            );
                                            scroll_offset = 0;
                                            loaded_file = Some(PathBuf::from(path));
                                            canvas_assembled = false;
                                            status_msg = format!("[loaded: {}]", path);
                                            response.push_str(&format!("[loaded: {}]\n", path));
                                        }
                                        Err(e) => {
                                            response.push_str(&format!("[error: {}]\n", e));
                                        }
                                    }
                                } else {
                                    response.push_str("[usage: load <path>]\n");
                                }
                            }
                            "step" => {
                                // Single-step the VM
                                if !is_running && (!vm.halted || vm.pc > 0) {
                                    vm.step();
                                    response.push_str(&format!("pc=0x{:04X}\n", vm.pc));
                                } else if is_running {
                                    response.push_str("[vm is running, pause first]\n");
                                } else {
                                    response.push_str("[not loaded]\n");
                                }
                            }
                            "halt" => {
                                is_running = false;
                                vm.halted = true;
                                status_msg = "[HALTED]".into();
                            }
                            "loadbin" => {
                                // Load a binary file directly into VM RAM at address 0.
                                // Supports both raw byte format (1 byte per word) and
                                // u32 LE format (4 bytes per word, written by asm_bin).
                                if let Some(path) = parts.get(1) {
                                    match std::fs::read(path) {
                                        Ok(bytes) => {
                                            // Auto-detect: if size is divisible by 4 and large enough,
                                            // treat as u32 LE words (asm_bin output).
                                            // Otherwise treat as raw bytes.
                                            let words: Vec<u32> = if bytes.len() > 4
                                                && bytes.len() % 4 == 0
                                            {
                                                bytes
                                                    .chunks_exact(4)
                                                    .map(|c| {
                                                        u32::from_le_bytes([c[0], c[1], c[2], c[3]])
                                                    })
                                                    .collect()
                                            } else {
                                                bytes.iter().map(|&b| b as u32).collect()
                                            };
                                            let len = words.len().min(vm.ram.len());
                                            vm.ram[..len].copy_from_slice(&words[..len]);
                                            vm.pc = 0;
                                            vm.halted = false;
                                            canvas_assembled = false;
                                            status_msg =
                                                format!("[loaded {} words at 0x0000]", len);
                                            response.push_str(&format!(
                                                "[loaded {} words at 0x0000]\n",
                                                len
                                            ));
                                        }
                                        Err(e) => {
                                            response.push_str(&format!("[error: {}]\n", e));
                                        }
                                    }
                                } else {
                                    response.push_str("[usage: loadbin <path>]\n");
                                }
                            }
                            "help" => {
                                response.push_str("Commands: status, canvas, assemble, run, type <text>, clear, save, screenshot [path], screenshot_b64, canvas_checksum, canvas_diff <hex>, screen, registers, disasm, vmscreen, ram [base] [rows], vm_state, dashboard, load <path>, loadasm <path>, loadbin <path>, step, halt, buildings [radius], desktop_json, launch <app>, player_pos, hypervisor_boot <config>, hypervisor_kill, inject_key <keycode>, inject_mouse <move|click> <x> <y> [button], inject_text <text>, help\n");
                                response.push_str("In 'type' command, use \\n for newlines.\n");
                            }
                            "loadasm" => {
                                // Assemble a .asm file directly into VM RAM at
                                // CANVAS_BYTECODE_ADDR, bypassing the canvas text buffer.
                                if let Some(path) = parts.get(1) {
                                    match std::fs::read_to_string(path) {
                                        Ok(source) => {
                                            let mut pp =
                                                crate::preprocessor::Preprocessor::new();
                                            let preprocessed = pp.preprocess(&source);
                                            match crate::assembler::assemble(
                                                &preprocessed,
                                                crate::render::CANVAS_BYTECODE_ADDR,
                                            ) {
                                                Ok(asm_result) => {
                                                    let ram_len = vm.ram.len();
                                                    let base = crate::render::CANVAS_BYTECODE_ADDR;
                                                    for v in vm.ram
                                                        [base..ram_len.min(base + 8192)]
                                                        .iter_mut()
                                                    {
                                                        *v = 0;
                                                    }
                                                    for (i, &word) in
                                                        asm_result.pixels.iter().enumerate()
                                                    {
                                                        let addr = base + i;
                                                        if addr < ram_len {
                                                            vm.ram[addr] = word;
                                                        }
                                                    }
                                                    vm.pc = base as u32;
                                                    vm.halted = false;
                                                    canvas_assembled = true;
                                                    status_msg = format!(
                                                        "[loadasm OK: {} words at 0x{:04X}]",
                                                        asm_result.pixels.len(),
                                                        base
                                                    );
                                                    response.push_str(&format!(
                                                        "[loaded {} words at 0x{:04X}]\n",
                                                        asm_result.pixels.len(),
                                                        base
                                                    ));
                                                }
                                                Err(e) => {
                                                    response.push_str(&format!(
                                                        "[assembly error: {}]\n",
                                                        e
                                                    ));
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            response.push_str(&format!(
                                                "[error: {}]\n",
                                                e
                                            ));
                                        }
                                    }
                                } else {
                                    response.push_str("[usage: loadasm <path>]\n");
                                }
                            }
                            // ── Phase 84: Building & Desktop Socket Commands ──────
                            "buildings" => {
                                // List buildings from VM RAM (0x7500 table)
                                // Format: id,world_x,world_y,type_color,name per line
                                let radius: i32 =
                                    parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(256);
                                let player_x = vm.ram[0x7808] as i32;
                                let player_y = vm.ram[0x7809] as i32;
                                let bldg_count = vm.ram[0x7580].min(32) as u32;
                                for i in 0..bldg_count {
                                    let base = 0x7500 + (i as usize) * 4;
                                    if base + 3 >= vm.ram.len() {
                                        break;
                                    }
                                    let bx = vm.ram[base] as i32;
                                    let by = vm.ram[base + 1] as i32;
                                    let color = vm.ram[base + 2];
                                    let name_addr = vm.ram[base + 3] as usize;
                                    if radius > 0 {
                                        let dx = (bx - player_x).abs();
                                        let dy = (by - player_y).abs();
                                        if dx > radius || dy > radius {
                                            continue;
                                        }
                                    }
                                    // Read name from RAM
                                    let mut name = String::new();
                                    for j in 0..16 {
                                        if name_addr + j >= vm.ram.len() {
                                            break;
                                        }
                                        let ch = vm.ram[name_addr + j];
                                        if ch == 0 || ch > 127 {
                                            break;
                                        }
                                        name.push(ch as u8 as char);
                                    }
                                    response.push_str(&format!(
                                        "{},{},{},{:06x},{}\n",
                                        i,
                                        bx,
                                        by,
                                        color & 0xFFFFFF,
                                        name
                                    ));
                                }
                            }
                            "desktop_json" => {
                                // Full desktop state as JSON-ish
                                let player_x = vm.ram[0x7808];
                                let player_y = vm.ram[0x7809];
                                let cam_x = vm.ram[0x7800];
                                let cam_y = vm.ram[0x7801];
                                let frame = vm.ram[0x7802];
                                let nearby = vm.ram[0x7588];
                                response.push_str(&format!(
                                    "{{\"player\":{{\"x\":{},\"y\":{}}},\"camera\":{{\"x\":{},\"y\":{}}},\"frame\":{},\"nearby_building\":{},\"buildings\":[",
                                    player_x, player_y, cam_x, cam_y, frame, nearby
                                ));
                                let bldg_count = vm.ram[0x7580].min(32) as u32;
                                for i in 0..bldg_count {
                                    let base = 0x7500 + (i as usize) * 4;
                                    if base + 3 >= vm.ram.len() {
                                        break;
                                    }
                                    let bx = vm.ram[base];
                                    let by = vm.ram[base + 1];
                                    let color = vm.ram[base + 2];
                                    let name_addr = vm.ram[base + 3] as usize;
                                    let mut name = String::new();
                                    for j in 0..16 {
                                        if name_addr + j >= vm.ram.len() {
                                            break;
                                        }
                                        let ch = vm.ram[name_addr + j];
                                        if ch == 0 || ch > 127 {
                                            break;
                                        }
                                        name.push(ch as u8 as char);
                                    }
                                    if i > 0 {
                                        response.push(',');
                                    }
                                    response.push_str(&format!(
                                        "{{\"id\":{},\"x\":{},\"y\":{},\"color\":\"{:06x}\",\"name\":\"{}\"}}",
                                        i, bx, by, color & 0xFFFFFF, name
                                    ));
                                }
                                response.push_str("]}\n");
                            }
                            "launch" => {
                                // Launch an app by name (sets VM state to run the program)
                                let app_name = parts.get(1).copied().unwrap_or("");
                                if app_name.is_empty() {
                                    response.push_str("[error: launch requires app name]\n");
                                } else {
                                    // Find the building with matching name
                                    let mut found = false;
                                    let bldg_count = vm.ram[0x7580].min(32) as u32;
                                    for i in 0..bldg_count {
                                        let base = 0x7500 + (i as usize) * 4;
                                        if base + 3 >= vm.ram.len() {
                                            break;
                                        }
                                        let name_addr = vm.ram[base + 3] as usize;
                                        let mut name = String::new();
                                        for j in 0..16 {
                                            if name_addr + j >= vm.ram.len() {
                                                break;
                                            }
                                            let ch = vm.ram[name_addr + j];
                                            if ch == 0 || ch > 127 {
                                                break;
                                            }
                                            name.push(ch as u8 as char);
                                        }
                                        if name == app_name {
                                            // Load and assemble the program
                                            let prog_path = format!("programs/{}.asm", app_name);
                                            match std::fs::read_to_string(&prog_path) {
                                                Ok(source) => {
                                                    let mut pp =
                                                        crate::preprocessor::Preprocessor::new();
                                                    let preprocessed = pp.preprocess(&source);
                                                    let base_addr =
                                                        crate::render::CANVAS_BYTECODE_ADDR;
                                                    match crate::assembler::assemble(
                                                        &preprocessed,
                                                        base_addr,
                                                    ) {
                                                        Ok(asm_result) => {
                                                            let ram_len = vm.ram.len();
                                                            for v in vm.ram
                                                                [base_addr
                                                                    ..ram_len
                                                                        .min(base_addr + 8192)]
                                                                .iter_mut()
                                                            {
                                                                *v = 0;
                                                            }
                                                            for (idx, &word) in
                                                                asm_result.pixels.iter().enumerate()
                                                            {
                                                                let addr = base_addr + idx;
                                                                if addr < ram_len {
                                                                    vm.ram[addr] = word;
                                                                }
                                                            }
                                                            vm.pc = base_addr as u32;
                                                            vm.halted = false;
                                                            canvas_assembled = true;
                                                            is_running = true;
                                                            hit_breakpoint = false;
                                                            response.push_str(&format!(
                                                                "[launching: {} from building {} ({} words)]\n",
                                                                app_name, i, asm_result.pixels.len()
                                                            ));
                                                        }
                                                        Err(e) => {
                                                            response.push_str(&format!(
                                                                "[assembly error for {}: {}]\n",
                                                                app_name, e
                                                            ));
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    response.push_str(&format!(
                                                        "[no program file for {}: {}]\n",
                                                        app_name, e
                                                    ));
                                                }
                                            }
                                            found = true;
                                            break;
                                        }
                                    }
                                    if !found {
                                        response
                                            .push_str(&format!("[app not found: {}]\n", app_name));
                                    }
                                }
                            }
                            "player_pos" => {
                                let px = vm.ram[0x7808];
                                let py = vm.ram[0x7809];
                                let facing = vm.ram[0x780A];
                                let facing_str = match facing {
                                    0 => "down",
                                    1 => "up",
                                    2 => "left",
                                    3 => "right",
                                    _ => "unknown",
                                };
                                response.push_str(&format!("{},{},{}\n", px, py, facing_str));
                            }
                            "hypervisor_boot" => {
                                // Boot a guest OS via hypervisor
                                // Usage: hypervisor_boot <config> [window_id]
                                // e.g. hypervisor_boot arch=riscv64 kernel=Image ram=256M
                                if parts.len() < 2 {
                                    response.push_str(
                                        "[error: hypervisor_boot requires config string]\n",
                                    );
                                } else {
                                    let config_parts: Vec<&str> = parts[1..]
                                        .iter()
                                        .take_while(|p| !p.starts_with("window="))
                                        .cloned()
                                        .collect();
                                    let config_str = config_parts.join(" ");
                                    // Check for optional window_id
                                    let window_id: u32 = parts
                                        .iter()
                                        .find(|p| p.starts_with("window="))
                                        .and_then(|p| p.split('=').nth(1))
                                        .and_then(|v| v.parse().ok())
                                        .unwrap_or(0);

                                    // Write config string to RAM at 0x2000
                                    let config_bytes: Vec<u32> = config_str
                                        .chars()
                                        .map(|c| c as u32)
                                        .chain(std::iter::once(0))
                                        .collect();
                                    for (i, &b) in config_bytes.iter().enumerate() {
                                        if 0x2000 + i < vm.ram.len() {
                                            vm.ram[0x2000 + i] = b;
                                        }
                                    }

                                    // Set up registers and call HYPERVISOR
                                    vm.regs[10] = 0x2000; // config addr in r10
                                    vm.regs[11] = window_id; // window_id in r11
                                                             // Simulate HYPERVISOR opcode manually
                                    let addr = vm.regs[10] as usize;
                                    let config = {
                                        let mut s = String::new();
                                        let mut i = addr;
                                        while i < vm.ram.len() && vm.ram[i] != 0 {
                                            s.push((vm.ram[i] & 0xFF) as u8 as char);
                                            i += 1;
                                        }
                                        if s.is_empty() {
                                            None
                                        } else {
                                            Some(s)
                                        }
                                    };
                                    match config {
                                        Some(cfg) => {
                                            let has_arch = cfg.split_whitespace().any(|t| {
                                                t.to_lowercase().starts_with("arch=") && t.len() > 5
                                            });
                                            if !has_arch {
                                                response
                                                    .push_str("[error: missing arch= parameter]\n");
                                            } else {
                                                vm.hypervisor_config = cfg.clone();
                                                vm.hypervisor_window_id = window_id;
                                                vm.hypervisor_active = true;
                                                response.push_str(&format!(
                                                    "[hypervisor: booted config='{}' window={} active={}]\n",
                                                    cfg, window_id, vm.hypervisor_active
                                                ));
                                            }
                                        }
                                        None => {
                                            response.push_str("[error: empty config string]\n");
                                        }
                                    }
                                }
                            }
                            "hypervisor_kill" => {
                                // Kill running hypervisor
                                if vm.hypervisor_active {
                                    vm.hypervisor_active = false;
                                    vm.hypervisor_config.clear();
                                    vm.hypervisor_window_id = 0;
                                    response.push_str("[hypervisor: killed]\n");
                                } else {
                                    response.push_str("[hypervisor: not running]\n");
                                }
                            }
                            // Phase 88: AI Vision Bridge socket commands
                            "screenshot_b64" => {
                                let b64 = geometry_os::vision::encode_png_base64(&vm.screen);
                                response.push_str(&b64);
                                response.push('\n');
                            }
                            "canvas_checksum" => {
                                let hash = geometry_os::vision::canvas_checksum(&vm.screen);
                                response.push_str(&format!("{}\n", hash));
                            }
                            "canvas_diff" => {
                                // canvas_diff <checksum_hex>
                                let prev_str = parts.get(1).unwrap_or(&"");
                                let prev_hash =
                                    u32::from_str_radix(prev_str.trim_start_matches("0x"), 16)
                                        .unwrap_or(prev_str.parse::<u32>().unwrap_or(0));
                                let current_hash = geometry_os::vision::canvas_checksum(&vm.screen);
                                if current_hash == prev_hash {
                                    response.push_str("same\n");
                                } else {
                                    response.push_str(&format!("changed: {:08X}\n", current_hash));
                                }
                            }
                            // ── Phase 89: AI Input Injection Socket Commands ──
                            "inject_key" => {
                                // inject_key <keycode> [shift]
                                // Injects a key event into the VM's key buffer
                                if let Some(keycode_str) = parts.get(1) {
                                    let keycode = keycode_str.parse::<u32>().unwrap_or_else(|_| {
                                        // Try single character
                                        let bytes = keycode_str.as_bytes();
                                        if bytes.len() == 1 {
                                            bytes[0] as u32
                                        } else {
                                            0
                                        }
                                    });
                                    let ok = vm.push_key(keycode);
                                    response.push_str(if ok { "ok\n" } else { "buffer_full\n" });
                                } else {
                                    response.push_str("[usage: inject_key <keycode>]\n");
                                }
                            }
                            "inject_mouse" => {
                                // inject_mouse <action> <x> <y> [button]
                                // action: move, click
                                let action = parts.get(1).copied().unwrap_or("");
                                if let (Some(x_str), Some(y_str)) = (parts.get(2), parts.get(3)) {
                                    let x = x_str.parse::<u32>().unwrap_or(0);
                                    let y = y_str.parse::<u32>().unwrap_or(0);
                                    match action {
                                        "move" => {
                                            vm.push_mouse(x, y);
                                            response.push_str("ok\n");
                                        }
                                        "click" => {
                                            let button = parts
                                                .get(4)
                                                .and_then(|s| s.parse::<u32>().ok())
                                                .unwrap_or(2); // default: left click
                                            vm.push_mouse(x, y);
                                            vm.push_mouse_button(button);
                                            response.push_str("ok\n");
                                        }
                                        _ => {
                                            response.push_str("[usage: inject_mouse <move|click> <x> <y> [button]]\n");
                                        }
                                    }
                                } else {
                                    response.push_str(
                                        "[usage: inject_mouse <move|click> <x> <y> [button]]\n",
                                    );
                                }
                            }
                            "inject_text" => {
                                // inject_text <text>
                                // Types each character into the VM's key buffer
                                if line.len() > 12 {
                                    let text = &line[12..]; // skip "inject_text "
                                    let mut count = 0u32;
                                    for ch in text.chars() {
                                        if !vm.push_key(ch as u32) {
                                            break;
                                        }
                                        count += 1;
                                    }
                                    response.push_str(&format!("injected {} chars\n", count));
                                } else {
                                    response.push_str("[usage: inject_text <text>]\n");
                                }
                            }
                            _ => {
                                response.push_str(&format!("[unknown: {}]\n", line));
                            }
                        }
                    }
                }
                if !response.is_empty() {
                    let _ = stream.write_all(response.as_bytes());
                }
            }
        }

        // ── Update Visual Debugger intensities ──────────────────
        // Process new accesses
        for access in &vm.access_log {
            if access.addr < ram_intensity.len() {
                let boost = if access.kind == vm::MemAccessKind::Write {
                    1.5
                } else {
                    1.0
                };
                ram_intensity[access.addr] = boost;
                ram_kind[access.addr] = access.kind;
            }
        }
        // Decay existing intensities (every frame)
        for val in ram_intensity.iter_mut() {
            if *val > 0.01 {
                *val *= 0.75;
            } else {
                *val = 0.0;
            }
        }

        // Track PC for trail
        if is_running {
            pc_history.push_back(vm.pc);
            if pc_history.len() > 64 {
                pc_history.pop_front();
            }
        } else {
            pc_history.clear();
        }

        // ── Render ───────────────────────────────────────────────
        render(
            &mut buffer,
            &vm,
            &canvas_buffer,
            cursor_row,
            cursor_col,
            scroll_offset,
            is_running,
            hit_breakpoint,
            &status_msg,
            &ram_intensity,
            &ram_kind,
            &pc_history,
            ram_view_base,
        );
        if let Err(e) = window.update_with_buffer(&buffer, WIDTH, HEIGHT) {
            eprintln!("Render error: {}. Exiting.", e);
            break;
        }

        if recording {
            let path = format!("/tmp/geo_frames/frame_{:05}.png", frame_id);
            if let Err(e) = save_full_buffer_png(&path, &buffer, WIDTH, HEIGHT) {
                status_msg = format!("[rec error: {}]", e);
                recording = false;
            } else {
                frame_id += 1;
            }
        }
    }
}
