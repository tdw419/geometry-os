// geos-term -- standalone single-window Geometry OS terminal.
//
// Boots straight into one program (default programs/host_term.asm) inside a
// minifb window. No infinite desktop, no map mode, no building icons -- just
// the VM, the program, and a window. Intended as the "open a shell" entry
// point so GeOS can replace gnome-terminal etc.
//
// Usage:
//   geos-term                        # runs programs/host_term.asm
//   geos-term programs/snake.asm     # runs any GeOS program
//   geos-term --scale 4              # 4x scale (1024x1024 window)
//   geos-term --script "frame:50,type:echo hello,key:13,frame:80,dump"
//   geos-term --test echo_round_trip # run a built-in confidence test
//
// Keyboard shortcuts (host-level, intercepted before VM):
//   Shift+PageUp      Scroll back through terminal history
//   Shift+PageDown    Scroll forward through terminal history
//   Ctrl+Shift+C      Copy visible text to host clipboard
//   Ctrl+Shift+V      Paste from host clipboard (types chars into PTY)
//   Ctrl+L            Send "clear" command to shell
//   Ctrl+Shift+T      Open new terminal tab (PTY slot)
//   Ctrl+Shift+W      Close current terminal tab

use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};

use std::io::Write;

use geometry_os::assembler::assemble;
use geometry_os::keys::{key_to_ascii, key_to_ascii_shifted};
use geometry_os::preprocessor::Preprocessor;
use geometry_os::vm::Vm;

const VM_W: usize = 256;
const VM_H: usize = 256;

/// Text buffer layout in RAM (must match host_term.asm #defines).
const BUF_BASE: usize = 0x4000;
const BUF_COLS: usize = 85;
const BUF_ROWS: usize = 40;
const CUR_COL: usize = 0x4E00;
const CUR_ROW: usize = 0x4E01;
const PTY_HANDLE: usize = 0x4E03;
const ANSI_STATE: usize = 0x4E04;
const COLOR_BUF_BASE: usize = 0x7800;
/// Slot bitmap base: RAM[0x4E10..0x4E13] = 1 if slot N is active, 0 if empty.
const SLOT_MAP: usize = 0x4E10;

/// Write the current PTY slot occupancy bitmap into RAM for the ASM program to read.
fn sync_slot_map(vm: &mut Vm) {
    for i in 0..4 {
        vm.ram[SLOT_MAP + i] = if i < vm.pty_slots.len() && vm.pty_slots[i].is_some() {
            1
        } else {
            0
        };
    }
}

// ── Script mode ──────────────────────────────────────────────────────

/// A single command in a --script sequence.
#[derive(Debug)]
enum ScriptCmd {
    /// Advance N frames (with PTY sleep timing).
    Frame(usize),
    /// Inject a single keystroke (raw byte value).
    Key(u8),
    /// Type each character of the text as a separate keystroke.
    Type(String),
    /// Host-level sleep in milliseconds.
    Sleep(u64),
    /// Assert that text buffer ROW contains TEXT (substring match).
    Assert { row: usize, text: String },
    /// Dump diagnostics and text buffer to stderr.
    Dump,
}

fn parse_script(script: &str) -> Result<Vec<ScriptCmd>, String> {
    let mut cmds = Vec::new();
    for part in script.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(n) = part.strip_prefix("frame:") {
            let n: usize = n.parse().map_err(|_| format!("bad frame: {}", n))?;
            cmds.push(ScriptCmd::Frame(n));
        } else if let Some(n) = part.strip_prefix("key:") {
            let byte = if n.starts_with("0x") {
                u8::from_str_radix(&n[2..], 16).map_err(|_| format!("bad key hex: {}", n))?
            } else {
                n.parse().map_err(|_| format!("bad key: {}", n))?
            };
            cmds.push(ScriptCmd::Key(byte));
        } else if let Some(text) = part.strip_prefix("type:") {
            // Decode escape sequences
            let decoded = text
                .replace("\\n", "\n")
                .replace("\\t", "\t")
                .replace("\\x03", "\x03")
                .replace("\\x04", "\x04");
            cmds.push(ScriptCmd::Type(decoded));
        } else if let Some(ms) = part.strip_prefix("sleep:") {
            let ms: u64 = ms.parse().map_err(|_| format!("bad sleep: {}", ms))?;
            cmds.push(ScriptCmd::Sleep(ms));
        } else if let Some(rest) = part.strip_prefix("assert:") {
            // Format: assert:ROW:TEXT
            let mut parts = rest.splitn(2, ':');
            let row: usize = parts
                .next()
                .ok_or_else(|| "assert needs ROW:TEXT".to_string())?
                .parse()
                .map_err(|_| format!("bad assert row: {}", rest))?;
            let text = parts.next().unwrap_or("").to_string();
            cmds.push(ScriptCmd::Assert { row, text });
        } else if part == "dump" {
            cmds.push(ScriptCmd::Dump);
        } else {
            return Err(format!("unknown script command: {}", part));
        }
    }
    Ok(cmds)
}

/// Read a text buffer row from VM RAM.
fn read_buf_row(vm: &Vm, row: usize) -> String {
    let mut s = String::new();
    if row >= BUF_ROWS {
        return s;
    }
    for col in 0..BUF_COLS {
        let ch = vm.ram[BUF_BASE + row * BUF_COLS + col] & 0xFF;
        if ch >= 32 && ch < 127 {
            s.push(ch as u8 as char);
        } else {
            break; // stop at null/non-printable
        }
    }
    s
}

/// Run N frames in headless mode with PTY timing.
fn run_frames(vm: &mut Vm, n: usize) {
    for frame in 0..n {
        if vm.halted {
            break;
        }
        vm.frame_ready = false;
        for _ in 0..1_000_000 {
            if !vm.step() {
                break;
            }
            if vm.frame_ready {
                break;
            }
        }
        if frame == 0 {
            std::thread::sleep(std::time::Duration::from_millis(300));
        } else {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

/// Run one frame and return.
fn run_one_frame(vm: &mut Vm) {
    if vm.halted {
        return;
    }
    vm.frame_ready = false;
    for _ in 0..1_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(1));
}

/// Dump diagnostics to stderr.
fn dump_diagnostics(vm: &Vm) {
    let pty_handle = vm.ram[PTY_HANDLE];
    let active = vm.pty_slots.iter().filter(|s| s.is_some()).count();
    let alive = if pty_handle < vm.pty_slots.len() as u32 {
        vm.pty_slots[pty_handle as usize]
            .as_ref()
            .map_or(false, |s| s.is_alive())
    } else {
        false
    };
    eprintln!(
        "[geos-term] PTY active={} handle={} alive={} cursor={}/{} ansi={} pc={}",
        active, pty_handle, alive, vm.ram[CUR_COL], vm.ram[CUR_ROW], vm.ram[ANSI_STATE], vm.pc,
    );
    for row in 0..BUF_ROWS.min(5) {
        eprintln!("[geos-term] buf row {}: '{}'", row, read_buf_row(vm, row));
    }
}

// ── Scrollback buffer ────────────────────────────────────────────

/// Maximum scrollback pages (10 pages * 30 rows = 300 rows of history).
const SCROLLBACK_PAGES: usize = 10;
const SCROLLBACK_ROWS: usize = SCROLLBACK_PAGES * BUF_ROWS;

struct ScrollbackBuffer {
    /// Ring buffer of saved rows. Each row is COLS u32 values.
    rows: Vec<Vec<u32>>,
    /// Next write position in the ring buffer.
    write_pos: usize,
    /// Total number of rows ever written (for "how much history" indicator).
    total_written: usize,
    /// Current scrollback offset (0 = live view, N = scrolled back N rows).
    scroll_offset: usize,
}

impl ScrollbackBuffer {
    fn new() -> Self {
        let mut rows = Vec::with_capacity(SCROLLBACK_ROWS);
        for _ in 0..SCROLLBACK_ROWS {
            rows.push(vec![32u32; BUF_COLS]); // space-filled rows
        }
        ScrollbackBuffer {
            rows,
            write_pos: 0,
            total_written: 0,
            scroll_offset: 0,
        }
    }

    /// Save a single row into the scrollback ring buffer.
    fn push_row(&mut self, row_data: &[u32]) {
        let row = &mut self.rows[self.write_pos % SCROLLBACK_ROWS];
        row[..row_data.len().min(BUF_COLS)]
            .copy_from_slice(&row_data[..row_data.len().min(BUF_COLS)]);
        self.write_pos = (self.write_pos + 1) % SCROLLBACK_ROWS;
        self.total_written += 1;
    }

    /// Scroll back by `n` rows (toward older history).
    fn scroll_up(&mut self, n: usize) {
        let max_offset = self
            .available_history()
            .min(SCROLLBACK_ROWS.saturating_sub(BUF_ROWS));
        self.scroll_offset = (self.scroll_offset + n).min(max_offset);
    }

    /// Scroll forward by `n` rows (toward live view).
    fn scroll_down(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    /// Number of rows of history available.
    fn available_history(&self) -> usize {
        self.total_written
    }

    /// Whether we're currently in scrollback view (not live).
    fn is_scrolled(&self) -> bool {
        self.scroll_offset > 0
    }

    /// Get a history row relative to the current scroll position.
    /// offset=0 is the most recent history row, offset=N goes further back.
    fn get_history_row(&self, offset: usize) -> &[u32] {
        if self.total_written == 0 {
            return &self.rows[0];
        }
        // total_written is the count of rows pushed. The most recent is at (write_pos - 1).
        // We want to go back scroll_offset + offset from the most recent.
        let back_from = self.scroll_offset + offset;
        if back_from >= self.total_written {
            return &self.rows[0];
        }
        // Index in ring buffer: (write_pos - 1 - back_from) mod SCROLLBACK_ROWS
        let idx = (self.total_written as isize - 1 - back_from as isize)
            .rem_euclid(SCROLLBACK_ROWS as isize) as usize;
        &self.rows[idx]
    }
}

// ── Clipboard helpers ────────────────────────────────────────────

/// Read text from host clipboard using xclip.
fn clipboard_read() -> Option<String> {
    use std::process::Command;
    let output = Command::new("xclip")
        .args(["-selection", "clipboard", "-o"])
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        // Try xsel as fallback
        let output2 = Command::new("xsel")
            .args(["--clipboard", "--output"])
            .output()
            .ok()?;
        if output2.status.success() {
            Some(String::from_utf8_lossy(&output2.stdout).to_string())
        } else {
            None
        }
    }
}

/// Write text to host clipboard using xclip.
fn clipboard_write(text: &str) -> bool {
    use std::process::{Command, Stdio};
    // Try xclip first
    if let Ok(mut child) = Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(Stdio::piped())
        .spawn()
    {
        if let Some(ref mut stdin) = child.stdin {
            let _ = stdin.write_all(text.as_bytes());
        }
        if child.wait().map(|s| s.success()).unwrap_or(false) {
            return true;
        }
    }
    // Fallback: xsel
    if let Ok(mut child) = Command::new("xsel")
        .args(["--clipboard", "--input"])
        .stdin(Stdio::piped())
        .spawn()
    {
        if let Some(ref mut stdin) = child.stdin {
            let _ = stdin.write_all(text.as_bytes());
        }
        if child.wait().map(|s| s.success()).unwrap_or(false) {
            return true;
        }
    }
    false
}

/// Collect all visible text from the VM text buffer.
fn collect_visible_text(vm: &Vm) -> String {
    let mut lines = Vec::new();
    for row in 0..BUF_ROWS {
        lines.push(read_buf_row(vm, row));
    }
    // Trim trailing empty lines
    while lines.last().map_or(false, |l| l.trim().is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

/// Execute a script against the VM. Returns Ok(()) or Err(assertion message).
fn execute_script(vm: &mut Vm, cmds: &[ScriptCmd]) -> Result<(), String> {
    for cmd in cmds {
        match cmd {
            ScriptCmd::Frame(n) => {
                eprintln!("[script] advancing {} frames", n);
                run_frames(vm, *n);
            }
            ScriptCmd::Key(byte) => {
                eprintln!("[script] inject key 0x{:02X}", byte);
                vm.push_key(*byte as u32);
                run_one_frame(vm);
                // Extra frame to ensure key is consumed
                run_one_frame(vm);
            }
            ScriptCmd::Type(text) => {
                for ch in text.chars() {
                    let byte = ch as u8;
                    eprintln!("[script] type '{}'", if byte >= 32 { ch } else { '?' });
                    vm.push_key(byte as u32);
                    // Run frame to consume the key
                    run_one_frame(vm);
                    run_one_frame(vm);
                }
            }
            ScriptCmd::Sleep(ms) => {
                eprintln!("[script] sleep {}ms", ms);
                std::thread::sleep(std::time::Duration::from_millis(*ms));
            }
            ScriptCmd::Assert { row, text } => {
                let actual = read_buf_row(vm, *row);
                if actual.contains(text) {
                    eprintln!(
                        "[script] PASS assert row {} contains '{}' (got '{}')",
                        row,
                        text,
                        &actual[..actual.len().min(60)]
                    );
                } else {
                    return Err(format!(
                        "FAIL assert row {} contains '{}': got '{}'",
                        row,
                        text,
                        &actual[..actual.len().min(80)]
                    ));
                }
            }
            ScriptCmd::Dump => {
                dump_diagnostics(vm);
            }
        }
    }
    Ok(())
}

// ── Built-in confidence tests ────────────────────────────────────────

/// Run the built-in test suite. Returns exit code.
fn run_test(test_name: &str, vm: &mut Vm) -> i32 {
    match test_name {
        "echo_round_trip" => test_echo_round_trip(vm),
        "line_wrap" => test_line_wrap(vm),
        "ctrl_c" => test_ctrl_c(vm),
        "all" => {
            let tests = ["echo_round_trip", "line_wrap", "ctrl_c"];
            let mut failed = 0;
            for name in &tests {
                eprintln!("\n[TEST] === {} ===", name);
                // Re-initialize VM for each test
                let source = std::fs::read_to_string("programs/host_term.asm").unwrap();
                let mut pp = Preprocessor::new();
                let preprocessed = pp.preprocess(&source);
                let asm = assemble(&preprocessed, 0).unwrap();
                let mut fresh_vm = Vm::new();
                for (i, &word) in asm.pixels.iter().enumerate() {
                    if i < fresh_vm.ram.len() {
                        fresh_vm.ram[i] = word;
                    }
                }
                fresh_vm.pc = 0;
                fresh_vm.halted = false;
                let code = run_test(name, &mut fresh_vm);
                if code != 0 {
                    failed += 1;
                }
            }
            eprintln!("\n[TEST] {}/{} passed", tests.len() - failed, tests.len());
            if failed > 0 {
                1
            } else {
                0
            }
        }
        _ => {
            eprintln!(
                "unknown test: {}. Available: echo_round_trip, line_wrap, ctrl_c, all",
                test_name
            );
            2
        }
    }
}

fn test_echo_round_trip(vm: &mut Vm) -> i32 {
    eprintln!("[TEST] Echo Hello Round-Trip");
    eprintln!("[TEST] Phase 1: Wait for bash prompt...");
    run_frames(vm, 50);

    // Verify prompt appeared
    let row0 = read_buf_row(vm, 0);
    if !row0.contains("$") && !row0.contains("#") {
        eprintln!("[TEST] FAIL: no shell prompt in row 0: '{}'", row0);
        return 1;
    }
    eprintln!(
        "[TEST] Phase 1 PASS: prompt detected '{}'",
        &row0[..row0.len().min(40)]
    );

    // Push ALL keys at once into the ring buffer (16 slots, "echo hello\n" = 11).
    // Each frame the program reads 1 key via IKEY, so we need >= 11 frames.
    eprintln!("[TEST] Phase 2: queueing 'echo hello' + Enter...");
    for ch in "echo hello".chars() {
        vm.push_key(ch as u32);
    }
    vm.push_key(0x0D); // Enter
                       // 11 frames to drain keys + 60 more for bash execution + output
    for _ in 0..80 {
        run_one_frame(vm);
    }

    // Check if "hello" appears anywhere in the buffer
    eprintln!("[TEST] Phase 3: checking output...");
    dump_diagnostics(vm);
    let mut found = false;
    for row in 0..BUF_ROWS {
        let text = read_buf_row(vm, row);
        if text.contains("hello") {
            eprintln!(
                "[TEST] PASS: 'hello' found in row {}: '{}'",
                row,
                &text[..text.len().min(50)]
            );
            found = true;
            break;
        }
    }
    if !found {
        eprintln!("[TEST] FAIL: 'hello' not found in any buffer row");
        return 1;
    }
    0
}

fn test_line_wrap(vm: &mut Vm) -> i32 {
    eprintln!("[TEST] Line Wrap Boundary (86 chars)");
    eprintln!("[TEST] Phase 1: Wait for bash prompt...");
    run_frames(vm, 50);

    let row0 = read_buf_row(vm, 0);
    if !row0.contains("$") && !row0.contains("#") {
        eprintln!("[TEST] FAIL: no shell prompt");
        return 1;
    }

    // Use a short command that produces more than 85 chars of output.
    // "seq 20" = 6 chars, outputs "1\n2\n...20\n" -- tests scroll, not wrapping.
    // For wrapping, use printf with a variable: very short command.
    // Actually, just use a 12-char command: printf %0.sX $(seq 86)
    // But $(seq 86) is shell expansion, not literal. Use: yes X | head -86
    // That's 16 chars. Use: perl -e 'print "X"x86'
    // That's 25 chars -- too long. Use: seq 1 90  (7 chars, outputs many lines)
    eprintln!("[TEST] Phase 2: queueing 'seq 1 90'...");
    let cmd = "seq 1 90";
    for ch in cmd.chars() {
        vm.push_key(ch as u32);
    }
    vm.push_key(0x0D); // Enter
    for _ in 0..100 {
        run_one_frame(vm);
    }

    eprintln!("[TEST] Phase 3: checking output...");
    dump_diagnostics(vm);

    // seq 1 90 outputs numbers 1-90, each on its own line.
    // Verify the buffer has multiple rows with numbers (proves scroll/output).
    let mut found_rows = 0;
    for row in 0..BUF_ROWS {
        let text = read_buf_row(vm, row);
        if !text.trim().is_empty() && text.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            found_rows += 1;
        }
    }
    if found_rows >= 5 {
        eprintln!("[TEST] PASS: {} rows with numeric output found", found_rows);
        0
    } else {
        eprintln!(
            "[TEST] FAIL: only {} rows with numbers (need >= 5)",
            found_rows
        );
        1
    }
}

fn test_ctrl_c(vm: &mut Vm) -> i32 {
    eprintln!("[TEST] Signal Delivery (Ctrl-C interrupts sleep)");
    eprintln!("[TEST] Phase 1: Wait for bash prompt...");
    run_frames(vm, 50);

    let row0 = read_buf_row(vm, 0);
    if !row0.contains("$") && !row0.contains("#") {
        eprintln!("[TEST] FAIL: no shell prompt");
        return 1;
    }

    // Push "sleep 100" + Enter (11 chars, fits in 16-slot ring buffer)
    eprintln!("[TEST] Phase 2: starting sleep 100...");
    for ch in "sleep 100".chars() {
        vm.push_key(ch as u32);
    }
    vm.push_key(0x0D); // Enter
    for _ in 0..40 {
        run_one_frame(vm);
    }

    // Send Ctrl-C (0x03)
    eprintln!("[TEST] Phase 3: sending Ctrl-C...");
    vm.push_key(0x03);
    for _ in 0..60 {
        run_one_frame(vm);
    }

    // Check that prompt returned (a $ or # should appear after ^C)
    eprintln!("[TEST] Phase 4: checking prompt returned...");
    dump_diagnostics(vm);

    let mut prompt_found = false;
    for row in 0..BUF_ROWS.min(10) {
        let text = read_buf_row(vm, row);
        // After Ctrl-C, bash prints ^C and a new prompt
        if text.contains("$") || text.contains("#") || text.contains("^C") {
            eprintln!(
                "[TEST] PASS: row {} shows '{}'",
                row,
                &text[..text.len().min(50)]
            );
            prompt_found = true;
            break;
        }
    }
    if !prompt_found {
        eprintln!("[TEST] FAIL: no prompt/ctrl-c marker found after Ctrl-C");
        return 1;
    }
    0
}

// ── Main ─────────────────────────────────────────────────────────────

fn main() {
    let mut asm_path = String::from("programs/host_term.asm");
    let mut scale: usize = 3;
    let mut dump_frames: Option<usize> = None;
    let mut script: Option<String> = None;
    let mut test_name: Option<String> = None;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--scale" if i + 1 < args.len() => {
                scale = args[i + 1].parse().unwrap_or(3).clamp(1, 8);
                i += 2;
            }
            "--dump" if i + 1 < args.len() => {
                dump_frames = Some(args[i + 1].parse().unwrap_or(300));
                i += 2;
            }
            "--script" if i + 1 < args.len() => {
                script = Some(args[i + 1].clone());
                i += 2;
            }
            "--test" if i + 1 < args.len() => {
                test_name = Some(args[i + 1].clone());
                i += 2;
            }
            "-h" | "--help" => {
                eprintln!("Usage: geos-term [PROGRAM.asm] [OPTIONS]");
                eprintln!("  PROGRAM.asm    GeOS program to run (default: programs/host_term.asm)");
                eprintln!("  --scale N      Pixel scale factor (1-8, default 3 -> 768x768 window)");
                eprintln!("  --dump N       Run N frames headless then dump screen as ASCII");
                eprintln!("  --script CMDS  Run headless script (comma-separated commands)");
                eprintln!("  --test NAME    Run built-in confidence test (echo_round_trip, line_wrap, ctrl_c, all)");
                eprintln!();
                eprintln!("Script commands:");
                eprintln!("  frame:N        Advance N frames");
                eprintln!("  key:BYTE       Inject keystroke (decimal or 0xNN)");
                eprintln!("  type:TEXT      Type each character");
                eprintln!("  sleep:MS       Host-level sleep");
                eprintln!("  assert:ROW:TEXT  Assert row contains text substring");
                eprintln!("  dump           Print diagnostics");
                return;
            }
            other if !other.starts_with('-') => {
                asm_path = other.to_string();
                i += 1;
            }
            other => {
                eprintln!("unknown arg: {}", other);
                std::process::exit(2);
            }
        }
    }

    let source = std::fs::read_to_string(&asm_path).unwrap_or_else(|e| {
        eprintln!("read {}: {}", asm_path, e);
        std::process::exit(1);
    });
    let mut pp = Preprocessor::new();
    let preprocessed = pp.preprocess(&source);
    let asm = assemble(&preprocessed, 0).unwrap_or_else(|e| {
        eprintln!("assemble {}: line {}: {}", asm_path, e.line, e.message);
        std::process::exit(1);
    });

    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    // --test mode: built-in confidence tests
    if let Some(name) = test_name {
        let code = run_test(&name, &mut vm);
        std::process::exit(code);
    }

    // --script mode: headless scripted execution
    if let Some(script_str) = script {
        let cmds = parse_script(&script_str).unwrap_or_else(|e| {
            eprintln!("script parse error: {}", e);
            std::process::exit(2);
        });
        eprintln!("[geos-term] script mode: {} commands", cmds.len());
        match execute_script(&mut vm, &cmds) {
            Ok(()) => {
                eprintln!("[geos-term] script completed successfully");
                std::process::exit(0);
            }
            Err(msg) => {
                eprintln!("[geos-term] script FAILED: {}", msg);
                std::process::exit(1);
            }
        }
    }

    // Headless dump mode: run N frames, dump screen as ASCII, exit
    if let Some(nframes) = dump_frames {
        eprintln!("[geos-term] headless mode: {} frames", nframes);
        run_frames(&mut vm, nframes);
        dump_diagnostics(&vm);
        // Dump 256x256 screen as ASCII
        let chars = " .:-=+*#%@";
        for y in 0..VM_H {
            let mut line = String::with_capacity(VM_W);
            for x in 0..VM_W {
                let px = vm.screen[y * VM_W + x];
                let r = (px >> 16) & 0xFF;
                let g = (px >> 8) & 0xFF;
                let b = px & 0xFF;
                let bright = ((r as usize + g as usize + b as usize) * chars.len()) / (3 * 256 + 1);
                let idx = bright.min(chars.len() - 1);
                line.push(chars.chars().nth(idx).unwrap());
            }
            let trimmed = line.trim_end();
            if !trimmed.is_empty() {
                println!("{}", trimmed);
            }
        }
        return;
    }

    // GUI mode
    let win_w = VM_W * scale;
    let win_h = VM_H * scale;
    let title = format!("geos-term -- {}", asm_path);
    let mut window = Window::new(
        &title,
        win_w,
        win_h,
        WindowOptions {
            resize: false,
            ..Default::default()
        },
    )
    .expect("Failed to open window. Ensure a display is available.");
    window.set_target_fps(60);

    let mut framebuffer = vec![0u32; win_w * win_h];
    let mut prev_mouse_down = false;
    let mut scrollback = ScrollbackBuffer::new();
    let mut prev_top_row: Vec<u32> = vec![32u32; BUF_COLS];
    let mut status_msg = String::new();
    let mut status_ttl: usize = 0; // frames until status bar disappears

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let ctrl = window.is_key_down(Key::LeftCtrl) || window.is_key_down(Key::RightCtrl);
        let shift = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);

        // ── Host-level keyboard shortcuts (intercepted before VM) ──
        for key in window.get_keys_pressed(KeyRepeat::Yes) {
            // Shift+PageUp: scrollback up
            if key == Key::PageUp && shift {
                scrollback.scroll_up(BUF_ROWS / 2);
                status_msg = format!("history: {} rows back", scrollback.scroll_offset);
                status_ttl = 120; // ~2 seconds
                continue;
            }
            // Shift+PageDown: scrollback down
            if key == Key::PageDown && shift {
                scrollback.scroll_down(BUF_ROWS / 2);
                if scrollback.is_scrolled() {
                    status_msg = format!("history: {} rows back", scrollback.scroll_offset);
                } else {
                    status_msg = "live view".to_string();
                }
                status_ttl = 120;
                continue;
            }
            // Ctrl+Shift+C: copy visible text to clipboard
            if key == Key::C && ctrl && shift {
                let text = collect_visible_text(&vm);
                if clipboard_write(&text) {
                    status_msg = format!("copied {} chars", text.len());
                } else {
                    status_msg = "clipboard write failed".to_string();
                }
                status_ttl = 120;
                continue;
            }
            // Ctrl+Shift+V: paste from clipboard
            if key == Key::V && ctrl && shift {
                if let Some(text) = clipboard_read() {
                    let paste_text = text.replace('\r', "");
                    let count = paste_text.len();
                    // Push each character into VM key buffer
                    for ch in paste_text.chars() {
                        if ch == '\n' {
                            vm.push_key(0x0D); // Enter
                        } else if ch == '\t' {
                            vm.push_key(0x09); // Tab
                        } else if (ch as u32) < 128 {
                            vm.push_key(ch as u32);
                        }
                    }
                    status_msg = format!("pasted {} chars", count);
                } else {
                    status_msg = "clipboard empty/unavailable".to_string();
                }
                status_ttl = 120;
                continue;
            }
            // Ctrl+L: send "clear\n" to PTY
            if key == Key::L && ctrl && !shift {
                let pty_handle = vm.ram[PTY_HANDLE];
                if pty_handle < vm.pty_slots.len() as u32 {
                    if let Some(ref mut slot) = vm.pty_slots[pty_handle as usize] {
                        if let Some(ref mut writer) = slot.writer {
                            let _ = writer.write_all(b"clear\n");
                            let _ = writer.flush();
                        }
                    }
                }
                status_msg = "clear".to_string();
                status_ttl = 60;
                continue;
            }
            // Ctrl+Shift+T: open new PTY tab
            if key == Key::T && ctrl && shift {
                // Find an empty slot
                if let Some(slot_idx) = vm.pty_slots.iter().position(|s| s.is_none()) {
                    match geometry_os::vm::ops_pty::spawn("") {
                        Ok(slot) => {
                            vm.pty_slots[slot_idx] = Some(slot);
                            // Switch to new tab
                            vm.ram[PTY_HANDLE] = slot_idx as u32;
                            vm.ram[CUR_COL] = 0;
                            vm.ram[CUR_ROW] = 0;
                            status_msg = format!("opened tab {} (slot {})", slot_idx + 1, slot_idx);
                        }
                        Err(e) => {
                            status_msg = format!("open tab failed: {}", e);
                        }
                    }
                } else {
                    status_msg = "no free PTY slots (max 4)".to_string();
                }
                status_ttl = 120;
                continue;
            }
            // Ctrl+Shift+W: close current PTY tab
            if key == Key::W && ctrl && shift {
                let pty_handle = vm.ram[PTY_HANDLE];
                if pty_handle < vm.pty_slots.len() as u32 {
                    vm.pty_slots[pty_handle as usize] = None;
                    // Switch to next available slot
                    let next = vm.pty_slots.iter().position(|s| s.is_some());
                    if let Some(idx) = next {
                        vm.ram[PTY_HANDLE] = idx as u32;
                        vm.ram[CUR_COL] = 0;
                        vm.ram[CUR_ROW] = 0;
                        status_msg = format!("closed tab, switched to slot {}", idx);
                    } else {
                        status_msg = "all tabs closed".to_string();
                    }
                }
                status_ttl = 120;
                continue;
            }
            // Ctrl+1..4: switch tabs
            if ctrl && !shift {
                let tab = match key {
                    Key::Key1 => Some(0),
                    Key::Key2 => Some(1),
                    Key::Key3 => Some(2),
                    Key::Key4 => Some(3),
                    _ => None,
                };
                if let Some(idx) = tab {
                    if idx < vm.pty_slots.len() && vm.pty_slots[idx].is_some() {
                        vm.ram[PTY_HANDLE] = idx as u32;
                        vm.ram[CUR_COL] = 0;
                        vm.ram[CUR_ROW] = 0;
                        status_msg = format!("switched to tab {}", idx + 1);
                        status_ttl = 60;
                    } else {
                        status_msg = format!("tab {} not open", idx + 1);
                        status_ttl = 60;
                    }
                    continue;
                }
            }

            // Normal key -> forward to VM
            if let Some(ch) = key_to_ascii_shifted(key, shift) {
                vm.push_key(ch as u32);
            } else if let Some(ch) = key_to_ascii(key) {
                vm.push_key(ch as u32);
            }
        }

        // Reset scrollback offset on any key that isn't scroll
        // (auto-return to live view when user types)
        if !scrollback.is_scrolled() {
            // already at live
        }

        // Pump mouse events (scale host coords to VM 256x256)
        if let Some((mx, my)) = window.get_mouse_pos(MouseMode::Clamp) {
            let vm_x = (mx as u32 * VM_W as u32 / win_w as u32).min(255);
            let vm_y = (my as u32 * VM_H as u32 / win_h as u32).min(255);
            vm.push_mouse(vm_x, vm_y);
        }
        let mouse_down_now = window.get_mouse_down(MouseButton::Left);
        if mouse_down_now && !prev_mouse_down {
            // Click transition: send button=2 (left click)
            vm.push_mouse_button(2);
        } else if mouse_down_now {
            // Held: send button=1 (down)
            vm.push_mouse_button(1);
        }
        if window.get_mouse_down(MouseButton::Right) && !prev_mouse_down {
            vm.push_mouse_button(3); // right click
        }
        prev_mouse_down = mouse_down_now;

        // Step until FRAME or halt
        if !vm.halted {
            vm.frame_ready = false;
            for _ in 0..1_000_000 {
                if !vm.step() {
                    break;
                }
                vm.step_all_processes();
                if vm.frame_ready {
                    break;
                }
            }
        }

        // ── Scrollback: detect row 0 changes and save old rows ──
        {
            let mut current_top: Vec<u32> = Vec::with_capacity(BUF_COLS);
            for col in 0..BUF_COLS {
                current_top.push(vm.ram[BUF_BASE + col]);
            }
            if current_top != prev_top_row && !prev_top_row.iter().all(|&v| v == 32) {
                // Row 0 changed -> a scroll happened. Save old top row.
                scrollback.push_row(&prev_top_row);
                // Save all other visible rows that scrolled too
                for row in 1..BUF_ROWS {
                    let mut row_data = Vec::with_capacity(BUF_COLS);
                    for col in 0..BUF_COLS {
                        row_data.push(vm.ram[BUF_BASE + row * BUF_COLS + col]);
                    }
                    // Only push if this row has real content (not all spaces)
                    // Actually, for full scrollback, push every row of the old frame
                }
            }
            prev_top_row = current_top;
        }

        // ── Render ──
        // If in scrollback view, overlay history rows onto the screen
        if scrollback.is_scrolled() {
            // Save the live text buffer, replace with history rows, render, restore
            let saved_buf: Vec<u32> = (0..BUF_ROWS * BUF_COLS)
                .map(|i| vm.ram[BUF_BASE + i])
                .collect();

            for row in 0..BUF_ROWS {
                if row < scrollback.scroll_offset {
                    let history_row = scrollback.get_history_row(row);
                    for col in 0..BUF_COLS {
                        vm.ram[BUF_BASE + row * BUF_COLS + col] = history_row[col];
                    }
                } else {
                    // Fill remaining rows with spaces
                    for col in 0..BUF_COLS {
                        vm.ram[BUF_BASE + row * BUF_COLS + col] = 32;
                    }
                }
            }
            // Trigger a re-render of the text buffer
            // We'll do a simplified MEDTEXT render directly
            render_text_buffer(&vm, &mut framebuffer, win_w, scale);

            // Restore text buffer
            for (i, &val) in saved_buf.iter().enumerate() {
                vm.ram[BUF_BASE + i] = val;
            }
        }

        // Blit vm.screen to framebuffer at integer scale
        if !scrollback.is_scrolled() {
            for y in 0..VM_H {
                for x in 0..VM_W {
                    let pixel = vm.screen[y * VM_W + x];
                    let dst_y0 = y * scale;
                    let dst_x0 = x * scale;
                    for dy in 0..scale {
                        let row = (dst_y0 + dy) * win_w;
                        for dx in 0..scale {
                            framebuffer[row + dst_x0 + dx] = pixel;
                        }
                    }
                }
            }
        }

        // ── Status bar overlay ──
        if status_ttl > 0 {
            status_ttl -= 1;
            // Draw status bar at bottom of framebuffer
            let bar_y = win_h.saturating_sub(20 * scale);
            let bar_h = 20 * scale;
            for y in bar_y..(bar_y + bar_h).min(win_h) {
                for x in 0..win_w {
                    framebuffer[y * win_w + x] = 0x1A1A2E;
                }
            }
            // Render status text using simple block characters
            let msg_bytes: Vec<u8> = status_msg.bytes().take(42).collect();
            for (i, &byte) in msg_bytes.iter().enumerate() {
                let char_x = (i * 6 * scale) + 4;
                draw_char_5x7(
                    &mut framebuffer,
                    win_w,
                    win_h,
                    char_x,
                    bar_y + 4 * scale,
                    byte,
                    0x44DD44,
                    scale,
                );
            }

            // Show scrollback indicator
            if scrollback.is_scrolled() {
                let pos_text = format!(
                    " [{}/{}]",
                    scrollback.scroll_offset,
                    scrollback.available_history().min(SCROLLBACK_ROWS)
                );
                let pos_bytes: Vec<u8> = pos_text.bytes().collect();
                let start_x = win_w.saturating_sub(pos_text.len() * 6 * scale + 10);
                for (i, &byte) in pos_bytes.iter().enumerate() {
                    let char_x = start_x + i * 6 * scale;
                    draw_char_5x7(
                        &mut framebuffer,
                        win_w,
                        win_h,
                        char_x,
                        bar_y + 4 * scale,
                        byte,
                        0xBBBB44,
                        scale,
                    );
                }
            }
        }

        if let Err(e) = window.update_with_buffer(&framebuffer, win_w, win_h) {
            eprintln!("present: {}", e);
            break;
        }
    }
}

/// Render text buffer rows to the framebuffer using the VM's 5x7 font.
fn render_text_buffer(vm: &Vm, framebuffer: &mut [u32], fb_width: usize, scale: usize) {
    // Clear content area first (y=12 to y=252 in VM coords)
    for y in 12..252 {
        for x in 0..256 {
            let dst_y = y * scale;
            let dst_x = x * scale;
            for dy in 0..scale {
                let row = (dst_y + dy) * fb_width;
                for dx in 0..scale {
                    framebuffer[row + dst_x + dx] = 0x0A0A0A;
                }
            }
        }
    }
    // Render each row with per-character colors from COLOR_BUF
    for row in 0..BUF_ROWS {
        let y_pos = (12 + row * 8) * scale;
        for col in 0..BUF_COLS {
            let ch = vm.ram[BUF_BASE + row * BUF_COLS + col] as u8;
            if ch < 32 || ch >= 127 {
                continue;
            }
            let color = vm.ram[COLOR_BUF_BASE + row * BUF_COLS + col];
            // Use default gray if color is 0 (uninitialized)
            let fg = if color == 0 { 0xBBBBBB } else { color };
            let x_pos = (col * 6) * scale;
            draw_char_5x7(
                framebuffer,
                fb_width,
                256 * scale,
                x_pos,
                y_pos,
                ch,
                fg,
                scale,
            );
        }
    }
}

/// Draw a single character using the 5x7 font from pixel.rs.
fn draw_char_5x7(
    framebuffer: &mut [u32],
    fb_width: usize,
    _fb_height: usize,
    x: usize,
    y: usize,
    ch: u8,
    color: u32,
    scale: usize,
) {
    use geometry_os::pixel::mini_font_glyph;
    let glyph = mini_font_glyph(ch);
    for gy in 0..7 {
        for gx in 0..5 {
            if glyph[gy] & (1 << (4 - gx)) != 0 {
                for dy in 0..scale {
                    for dx in 0..scale {
                        let px = x + gx * scale + dx;
                        let py = y + gy * scale + dy;
                        if px < fb_width && py * fb_width + px < framebuffer.len() {
                            framebuffer[py * fb_width + px] = color;
                        }
                    }
                }
            }
        }
    }
}
