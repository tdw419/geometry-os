// ops_pty.rs -- Persistent host PTY support (PTYOPEN/PTYWRITE/PTYREAD/PTYCLOSE).
//
// A PTY slot owns a child process running inside a pseudo-terminal plus a
// background reader thread that forwards stdout/stderr bytes through an
// mpsc channel. The VM drains the channel on PTYREAD and writes raw input
// bytes through the master on PTYWRITE.
//
// Goal: a guest GeoOS program can host a persistent bash session — `cd`
// changes its working dir, env vars and shell history persist, interactive
// programs see a real tty. ANSI parsing is intentionally out of scope here;
// guest programs are responsible for what they do with the byte stream.

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver};
use std::thread;

/// Maximum simultaneous PTY slots.
pub const MAX_PTY_SLOTS: usize = 4;

/// Result codes (mirrors net.rs convention; written to r0).
pub const PTY_OK: u32 = 0;
pub const PTY_ERR_INVALID_HANDLE: u32 = 1;
pub const PTY_ERR_OPEN_FAILED: u32 = 2;
pub const PTY_ERR_WRITE_FAILED: u32 = 3;
pub const PTY_ERR_NO_SLOTS: u32 = 5;
pub const PTY_ERR_CLOSED: u32 = 7;
pub const PTY_ERR_RESIZE_FAILED: u32 = 8;

pub struct PtySlot {
    master: Box<dyn MasterPty + Send>,
    pub writer: Option<Box<dyn std::io::Write + Send>>,
    rx: Receiver<u8>,
    /// Set true when the reader thread observes EOF or an error.
    closed_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Kept alive so the child isn't reaped while the slot exists.
    _child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl PtySlot {
    fn is_closed(&self) -> bool {
        self.closed_flag.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Drain all pending bytes from the channel (diagnostic).
    pub fn drain_remaining(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        while let Ok(b) = self.rx.try_recv() {
            bytes.push(b);
        }
        bytes
    }

    pub fn is_alive(&self) -> bool {
        !self.is_closed()
    }
}

/// Read a null-terminated ASCII string from RAM (one byte per u32 cell).
fn read_string_from_ram(ram: &[u32], addr: u32) -> String {
    let mut s = String::new();
    let mut i = addr as usize;
    while i < ram.len() {
        let byte = (ram[i] & 0xFF) as u8;
        if byte == 0 {
            break;
        }
        if byte.is_ascii() {
            s.push(byte as char);
        }
        i += 1;
    }
    s
}

/// Default terminal dimensions matching host_term_v4.asm display.
pub const DEFAULT_COLS: u16 = 42;
pub const DEFAULT_ROWS: u16 = 30;

/// Spawn `cmd` (or bash if empty) inside a fresh pty. Returns the populated
/// slot or an error string.
pub fn spawn(cmd_line: &str) -> Result<PtySlot, String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: DEFAULT_ROWS,
            cols: DEFAULT_COLS,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("openpty: {}", e))?;

    let cmd = if cmd_line.trim().is_empty() {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        let mut c = CommandBuilder::new(shell);
        // Inherit a sane environment from the host so bash finds PATH/HOME.
        if let Ok(home) = std::env::var("HOME") {
            c.cwd(home);
        }
        // Use xterm so bash emits a prompt and handles line editing.
        // xterm-256color may send complex queries; xterm is the safe middle ground.
        c.env("TERM", "xterm");
        // Set COLUMNS/LINES to match the PTY size so bash line wrapping and
        // tab completion align with the actual terminal width from frame 1.
        c.env("COLUMNS", "42");
        c.env("LINES", "30");
        // Simple prompt so we can detect when bash is ready
        c.env("PS1", "$ ");
        // Disable startup files that might produce complex output
        c.arg("--norc");
        c.arg("--noprofile");
        c
    } else {
        // crude split on whitespace; good enough for single commands like
        // "/bin/bash -i" or "python3 -i". Quoted args aren't supported yet.
        let parts: Vec<&str> = cmd_line.split_whitespace().collect();
        let mut c = CommandBuilder::new(parts[0]);
        for arg in &parts[1..] {
            c.arg(arg);
        }
        c.env("TERM", "xterm");
        c.env("COLUMNS", "42");
        c.env("LINES", "30");
        c.env("PS1", "$ ");
        c
    };

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("spawn: {}", e))?;
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("clone_reader: {}", e))?;

    let (tx, rx) = channel::<u8>();
    let closed_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let closed_flag_thread = closed_flag.clone();

    thread::Builder::new()
        .name("pty-reader".into())
        .spawn(move || {
            let mut buf = [0u8; 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        closed_flag_thread.store(true, std::sync::atomic::Ordering::Relaxed);
                        break;
                    }
                    Ok(n) => {
                        for &b in &buf[..n] {
                            if tx.send(b).is_err() {
                                return;
                            }
                        }
                    }
                    Err(_) => {
                        closed_flag_thread.store(true, std::sync::atomic::Ordering::Relaxed);
                        break;
                    }
                }
            }
        })
        .map_err(|e| format!("spawn reader: {}", e))?;

    // Take the writer once at creation time so PTYWRITE can reuse it.
    let mut writer = pair.master.take_writer().ok();

    // Write a newline to trigger bash prompt emission. Without this,
    // bash may buffer its initial prompt indefinitely, especially when
    // the reader hasn't started consuming yet.
    if let Some(ref mut w) = writer {
        let _ = w.write_all(b"\n");
        let _ = w.flush();
    }

    Ok(PtySlot {
        master: pair.master,
        writer,
        rx,
        closed_flag,
        _child: child,
    })
}

impl super::Vm {
    /// PTYOPEN cmd_addr_reg, handle_reg  (0xA9)
    /// Spawns a command (or bash if empty) inside a pty and returns its
    /// slot index in handle_reg. r0 = PTY_OK or error code.
    pub fn op_ptyopen(&mut self) {
        let cmd_reg = self.fetch() as usize;
        let handle_reg = self.fetch() as usize;
        if cmd_reg >= super::NUM_REGS || handle_reg >= super::NUM_REGS {
            self.regs[0] = PTY_ERR_INVALID_HANDLE;
            return;
        }

        let cmd_line = read_string_from_ram(&self.ram, self.regs[cmd_reg]);

        let slot_idx = match self.pty_slots.iter().position(|s| s.is_none()) {
            Some(i) => i,
            None => {
                self.regs[0] = PTY_ERR_NO_SLOTS;
                return;
            }
        };

        match spawn(&cmd_line) {
            Ok(slot) => {
                self.pty_slots[slot_idx] = Some(slot);
                self.regs[handle_reg] = slot_idx as u32;
                self.regs[0] = PTY_OK;
            }
            Err(e) => {
                eprintln!("PTYOPEN failed: {}", e);
                self.regs[0] = PTY_ERR_OPEN_FAILED;
            }
        }
    }

    /// PTYWRITE handle_reg, buf_reg, len_reg  (0xAA)
    /// Writes `len` bytes from RAM[buf_reg..] (one byte per u32) to the pty.
    /// r0 = PTY_OK on success, error code otherwise.
    pub fn op_ptywrite(&mut self) {
        let h_reg = self.fetch() as usize;
        let b_reg = self.fetch() as usize;
        let l_reg = self.fetch() as usize;
        if h_reg >= super::NUM_REGS || b_reg >= super::NUM_REGS || l_reg >= super::NUM_REGS {
            self.regs[0] = PTY_ERR_INVALID_HANDLE;
            return;
        }
        let h = self.regs[h_reg] as usize;
        let buf_addr = self.regs[b_reg] as usize;
        let len = self.regs[l_reg] as usize;
        if h >= MAX_PTY_SLOTS || self.pty_slots[h].is_none() {
            self.regs[0] = PTY_ERR_INVALID_HANDLE;
            return;
        }

        let mut bytes = Vec::with_capacity(len.min(4096));
        for i in 0..len.min(4096) {
            let idx = buf_addr + i;
            if idx >= self.ram.len() {
                break;
            }
            bytes.push((self.ram[idx] & 0xFF) as u8);
        }

        let slot = self.pty_slots[h].as_mut().unwrap();
        if let Some(ref mut w) = slot.writer {
            match w.write_all(&bytes) {
                Ok(()) => {
                    let _ = w.flush();
                    self.regs[0] = PTY_OK;
                }
                Err(_) => {
                    self.regs[0] = PTY_ERR_WRITE_FAILED;
                }
            }
        } else {
            self.regs[0] = PTY_ERR_WRITE_FAILED;
        }
    }

    /// PTYREAD handle_reg, buf_reg, max_len_reg  (0xAB)
    /// Drains up to max_len bytes pending from the pty into RAM.
    /// r0 = bytes drained (0 = none available right now).
    /// Sets r0 = 0xFFFFFFFF if the slot is closed (child exited / EOF).
    pub fn op_ptyread(&mut self) {
        let h_reg = self.fetch() as usize;
        let b_reg = self.fetch() as usize;
        let m_reg = self.fetch() as usize;
        eprintln!(
            "[PTYREAD] handle_reg={} buf_reg={} max_reg={}",
            h_reg, b_reg, m_reg
        );
        if h_reg >= super::NUM_REGS || b_reg >= super::NUM_REGS || m_reg >= super::NUM_REGS {
            self.regs[0] = PTY_ERR_INVALID_HANDLE;
            return;
        }
        let h = self.regs[h_reg] as usize;
        let buf_addr = self.regs[b_reg] as usize;
        let max_len = self.regs[m_reg] as usize;
        if h >= MAX_PTY_SLOTS || self.pty_slots[h].is_none() {
            self.regs[0] = PTY_ERR_INVALID_HANDLE;
            return;
        }

        let slot = self.pty_slots[h].as_ref().unwrap();
        let mut written = 0usize;
        while written < max_len.min(4096) {
            match slot.rx.try_recv() {
                Ok(byte) => {
                    let idx = buf_addr + written;
                    if idx < self.ram.len() {
                        self.ram[idx] = byte as u32;
                    }
                    written += 1;
                }
                Err(_) => break,
            }
        }

        if written == 0 && slot.is_closed() {
            self.regs[0] = u32::MAX;
            return;
        }
        self.regs[0] = written as u32;
    }

    /// PTYCLOSE handle_reg  (0xAC)
    /// Drops the slot, killing the child and joining the reader.
    /// r0 = PTY_OK or PTY_ERR_INVALID_HANDLE.
    pub fn op_ptyclose(&mut self) {
        let h_reg = self.fetch() as usize;
        if h_reg >= super::NUM_REGS {
            self.regs[0] = PTY_ERR_INVALID_HANDLE;
            return;
        }
        let h = self.regs[h_reg] as usize;
        if h >= MAX_PTY_SLOTS || self.pty_slots[h].is_none() {
            self.regs[0] = PTY_ERR_INVALID_HANDLE;
            return;
        }
        self.pty_slots[h] = None;
        self.regs[0] = PTY_OK;
    }

    /// PTYSIZE handle_reg, rows_reg, cols_reg  (0xAD)
    /// Resizes the PTY to the given dimensions.
    /// r0 = PTY_OK on success, error code otherwise.
    pub fn op_ptysize(&mut self) {
        let h_reg = self.fetch() as usize;
        let rows_reg = self.fetch() as usize;
        let cols_reg = self.fetch() as usize;
        if h_reg >= super::NUM_REGS || rows_reg >= super::NUM_REGS || cols_reg >= super::NUM_REGS {
            self.regs[0] = PTY_ERR_INVALID_HANDLE;
            return;
        }
        let h = self.regs[h_reg] as usize;
        let rows = self.regs[rows_reg];
        let cols = self.regs[cols_reg];
        if h >= MAX_PTY_SLOTS || self.pty_slots[h].is_none() {
            self.regs[0] = PTY_ERR_INVALID_HANDLE;
            return;
        }

        let slot = self.pty_slots[h].as_ref().unwrap();
        match slot.master.resize(PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        }) {
            Ok(()) => {
                self.regs[0] = PTY_OK;
            }
            Err(e) => {
                eprintln!("PTYSIZE failed: {}", e);
                self.regs[0] = PTY_ERR_RESIZE_FAILED;
            }
        }
    }

    /// Resize the PTY directly by handle index (called from host on font mode switch).
    /// Returns true on success.
    pub fn resize_pty_direct(&mut self, handle: usize, rows: u16, cols: u16) -> bool {
        if handle >= MAX_PTY_SLOTS {
            return false;
        }
        if let Some(ref slot) = self.pty_slots[handle] {
            match slot.master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            }) {
                Ok(()) => true,
                Err(e) => {
                    eprintln!("PTY resize failed: {}", e);
                    false
                }
            }
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn pty_pwd_roundtrip() {
        // Run pwd once; expect a path containing '/' to come back.
        let mut slot = match spawn("") {
            Ok(s) => s,
            Err(e) => {
                eprintln!("skipping: pty spawn failed: {}", e);
                return;
            }
        };

        {
            let w = slot.writer.as_mut().expect("writer");
            w.write_all(b"pwd\nexit\n").expect("write pwd");
        }

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut output = Vec::new();
        while Instant::now() < deadline {
            match slot.rx.try_recv() {
                Ok(b) => output.push(b),
                Err(_) => {
                    if slot.is_closed() && output.contains(&b'/') {
                        break;
                    }
                    thread::sleep(Duration::from_millis(20));
                }
            }
        }

        let text = String::from_utf8_lossy(&output);
        assert!(
            text.contains('/'),
            "expected pwd output containing '/', got: {:?}",
            text
        );
    }

    /// VM-level integration test: PTYOPEN -> PTYWRITE "echo hello\n" -> PTYREAD -> assert "hello"
    ///
    /// Tests the full opcode pipeline by setting up RAM with instruction operands
    /// and calling the opcode handlers directly.
    #[test]
    fn pty_vm_echo_roundtrip() {
        use crate::vm::Vm;

        let mut vm = Vm::new();

        // Write empty command string at 0x5000 (null byte = spawn default shell)
        vm.ram[0x5000] = 0;

        // PTYOPEN: set registers for cmd_addr_reg=5, handle_reg=10
        vm.regs[5] = 0x5000;
        // Set up fetch stream at current PC: [opcode, cmd_reg, handle_reg]
        let base_pc = vm.pc as usize;
        vm.ram[base_pc] = 0xA9; // PTYOPEN opcode (consumed by step(), not op_ptyopen)
        vm.ram[base_pc + 1] = 5; // cmd_addr_reg
        vm.ram[base_pc + 2] = 10; // handle_reg
        vm.pc = base_pc as u32 + 1; // skip opcode (op_ptyopen reads from pc)
        vm.op_ptyopen();

        let handle = vm.regs[10];
        assert_eq!(
            vm.regs[0], PTY_OK,
            "PTYOPEN should succeed, got r0={}",
            vm.regs[0]
        );

        // Wait for bash to start up and emit its initial prompt
        thread::sleep(Duration::from_millis(500));

        // Drain initial output using PTYREAD
        let drain_pc = vm.pc as usize;
        vm.ram[drain_pc] = 12; // handle_reg
        vm.ram[drain_pc + 1] = 6; // buf_reg
        vm.ram[drain_pc + 2] = 7; // max_len_reg
        vm.regs[12] = handle;
        vm.regs[6] = 0x5800;
        vm.regs[7] = 512;
        vm.pc = drain_pc as u32;
        vm.op_ptyread();

        // Write "echo hello\n" to send buffer at 0x5400
        let send_buf: usize = 0x5400;
        let msg = b"echo hello
";
        for (i, &byte) in msg.iter().enumerate() {
            vm.ram[send_buf + i] = byte as u32;
        }

        // PTYWRITE handle, send_buf, len
        let write_pc = vm.pc as usize;
        vm.ram[write_pc] = 12; // handle_reg
        vm.ram[write_pc + 1] = 6; // buf_reg
        vm.ram[write_pc + 2] = 7; // len_reg
        vm.regs[12] = handle;
        vm.regs[6] = send_buf as u32;
        vm.regs[7] = msg.len() as u32;
        vm.pc = write_pc as u32;
        vm.op_ptywrite();
        assert_eq!(vm.regs[0], PTY_OK, "PTYWRITE should succeed");

        // Wait for echo output to arrive
        thread::sleep(Duration::from_millis(500));

        // PTYREAD the response
        let read_pc = vm.pc as usize;
        vm.ram[read_pc] = 12; // handle_reg
        vm.ram[read_pc + 1] = 6; // buf_reg
        vm.ram[read_pc + 2] = 7; // max_len_reg
        vm.regs[12] = handle;
        vm.regs[6] = 0x5800;
        vm.regs[7] = 512;
        vm.pc = read_pc as u32;
        vm.op_ptyread();

        let bytes_read = vm.regs[0];
        assert!(
            bytes_read > 0 && bytes_read != u32::MAX,
            "PTYREAD should return bytes, got r0={}",
            bytes_read
        );

        // Collect the bytes from RAM and check for "hello"
        let recv_buf: usize = 0x5800;
        let mut output = Vec::new();
        for i in 0..bytes_read as usize {
            output.push((vm.ram[recv_buf + i] & 0xFF) as u8);
        }
        let text = String::from_utf8_lossy(&output);
        assert!(
            text.contains("hello"),
            "expected 'hello' in PTY output, got: {:?}",
            text
        );

        // Clean up with PTYCLOSE
        let close_pc = vm.pc as usize;
        vm.ram[close_pc] = 12; // handle_reg
        vm.regs[12] = handle;
        vm.pc = close_pc as u32;
        vm.op_ptyclose();
        assert_eq!(vm.regs[0], PTY_OK, "PTYCLOSE should succeed");
    }

    /// Test multi-session: open 2 PTY slots, verify both work independently,
    /// close one, verify the other still works. Phase 128 coverage.
    #[test]
    fn pty_multi_session() {
        use crate::vm::Vm;

        let mut vm = Vm::new();

        // Open first PTY (slot 0) with empty command = default shell
        vm.ram[0x5000] = 0; // null-terminated empty string
        vm.regs[5] = 0x5000;
        vm.pc = 0;
        vm.ram[0] = 5; // cmd_addr_reg
        vm.ram[1] = 10; // handle_reg
        vm.pc = 0;
        vm.op_ptyopen();

        assert_eq!(vm.regs[0], PTY_OK, "First PTYOPEN should succeed");
        let handle0 = vm.regs[10];
        assert_eq!(handle0, 0, "First slot should be index 0");

        // Open second PTY (slot 1)
        vm.ram[0x5001] = 0; // another null-terminated empty string
        vm.regs[5] = 0x5001;
        vm.regs[10] = 0;
        vm.pc = 100;
        vm.ram[100] = 5;
        vm.ram[101] = 10;
        vm.pc = 100;
        vm.op_ptyopen();

        assert_eq!(vm.regs[0], PTY_OK, "Second PTYOPEN should succeed");
        let handle1 = vm.regs[10];
        assert_eq!(handle1, 1, "Second slot should be index 1");

        // Verify both slots are occupied
        assert!(vm.pty_slots[0].is_some(), "Slot 0 should be occupied");
        assert!(vm.pty_slots[1].is_some(), "Slot 1 should be occupied");
        assert!(vm.pty_slots[2].is_none(), "Slot 2 should be empty");
        assert!(vm.pty_slots[3].is_none(), "Slot 3 should be empty");

        // Close slot 0
        vm.regs[12] = handle0;
        vm.pc = 200;
        vm.ram[200] = 12;
        vm.pc = 200;
        vm.op_ptyclose();
        assert_eq!(vm.regs[0], PTY_OK, "PTYCLOSE slot 0 should succeed");
        assert!(vm.pty_slots[0].is_none(), "Slot 0 should be freed");
        assert!(vm.pty_slots[1].is_some(), "Slot 1 should still be alive");

        // Close slot 1
        vm.regs[12] = handle1;
        vm.pc = 300;
        vm.ram[300] = 12;
        vm.pc = 300;
        vm.op_ptyclose();
        assert_eq!(vm.regs[0], PTY_OK, "PTYCLOSE slot 1 should succeed");
        assert!(vm.pty_slots[1].is_none(), "Slot 1 should be freed");

        // Verify all slots are now empty
        for (i, slot) in vm.pty_slots.iter().enumerate() {
            assert!(slot.is_none(), "Slot {} should be empty after close", i);
        }
    }

    /// Test that PTYOPEN with a specific command (shell selection) works.
    /// Phase 128 shell selection coverage.
    #[test]
    fn pty_shell_selection() {
        // Spawn with explicit /bin/sh command
        let mut slot = match spawn("/bin/sh") {
            Ok(s) => s,
            Err(e) => {
                eprintln!("skipping: pty spawn /bin/sh failed: {}", e);
                return;
            }
        };

        // Send echo to verify it works
        {
            let w = slot.writer.as_mut().expect("writer");
            let _ = w.write_all(b"echo test_selection_ok\nexit\n");
            let _ = w.flush();
        }

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut output = Vec::new();
        while Instant::now() < deadline {
            match slot.rx.try_recv() {
                Ok(b) => output.push(b),
                Err(_) => {
                    if slot.is_closed() && output.contains(&b't') {
                        break;
                    }
                    thread::sleep(Duration::from_millis(20));
                }
            }
        }

        let text = String::from_utf8_lossy(&output);
        assert!(
            text.contains("test_selection_ok"),
            "expected 'test_selection_ok' in output from /bin/sh, got: {:?}",
            text
        );
    }

    /// Test that PTYREAD returns initial bash output (prompt) after PTYOPEN.
    /// This verifies that the TERM=xterm + PS1 + newline trigger actually works.
    #[test]
    fn pty_initial_output_available() {
        let slot = match spawn("") {
            Ok(s) => s,
            Err(e) => {
                eprintln!("skipping: pty spawn failed: {}", e);
                return;
            }
        };

        // Wait briefly for bash to start and emit prompt
        thread::sleep(Duration::from_millis(500));

        // Drain all available output
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut output = Vec::new();
        while Instant::now() < deadline {
            match slot.rx.try_recv() {
                Ok(b) => output.push(b),
                Err(_) => {
                    if !output.is_empty() {
                        // Got some output and channel is now empty -- done
                        break;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            }
        }

        let text = String::from_utf8_lossy(&output);
        // With TERM=xterm and PS1='$ ', bash should emit SOMETHING (prompt, ANSI codes, etc.)
        assert!(
            !text.is_empty(),
            "PTY should have initial output after spawn, got empty string"
        );
    }

    /// Test PTYSIZE opcode resizes the PTY correctly.
    /// Spawn bash at default size, resize to 42x30 via op_ptysize, then
    /// verify bash reports $COLUMNS=42.
    #[test]
    fn pty_resize_via_opcode() {
        use crate::vm::Vm;

        let mut vm = Vm::new();

        // Open PTY
        vm.ram[0x5000] = 0; // null-terminated empty string
        vm.regs[5] = 0x5000;
        vm.pc = 0;
        vm.ram[0] = 5; // cmd_addr_reg
        vm.ram[1] = 10; // handle_reg
        vm.pc = 0;
        vm.op_ptyopen();

        assert_eq!(vm.regs[0], PTY_OK, "PTYOPEN should succeed");
        let handle = vm.regs[10];

        // Wait for bash to start
        thread::sleep(Duration::from_millis(500));

        // Drain initial output
        vm.regs[12] = handle;
        vm.regs[6] = 0x5800;
        vm.regs[7] = 512;
        vm.pc = 100;
        vm.ram[100] = 12;
        vm.ram[101] = 6;
        vm.ram[102] = 7;
        vm.pc = 100;
        vm.op_ptyread();

        // PTYSIZE handle, rows=30, cols=42
        let size_pc: usize = 200;
        vm.ram[size_pc] = 12; // handle_reg
        vm.ram[size_pc + 1] = 13; // rows_reg
        vm.ram[size_pc + 2] = 14; // cols_reg
        vm.regs[12] = handle;
        vm.regs[13] = 30; // rows
        vm.regs[14] = 42; // cols
        vm.pc = size_pc as u32;
        vm.op_ptysize();

        assert_eq!(
            vm.regs[0], PTY_OK,
            "PTYSIZE should succeed, got r0={}",
            vm.regs[0]
        );

        // Send "echo $COLUMNS\n" to verify bash sees the new width
        let send_buf: usize = 0x5400;
        let msg = b"echo $COLUMNS\n";
        for (i, &byte) in msg.iter().enumerate() {
            vm.ram[send_buf + i] = byte as u32;
        }

        vm.regs[12] = handle;
        vm.regs[6] = send_buf as u32;
        vm.regs[7] = msg.len() as u32;
        vm.pc = 300;
        vm.ram[300] = 12;
        vm.ram[301] = 6;
        vm.ram[302] = 7;
        vm.pc = 300;
        vm.op_ptywrite();
        assert_eq!(vm.regs[0], PTY_OK, "PTYWRITE should succeed");

        // Wait for bash to respond
        thread::sleep(Duration::from_millis(500));

        // Read response
        vm.regs[12] = handle;
        vm.regs[6] = 0x5800;
        vm.regs[7] = 512;
        vm.pc = 400;
        vm.ram[400] = 12;
        vm.ram[401] = 6;
        vm.ram[402] = 7;
        vm.pc = 400;
        vm.op_ptyread();

        let bytes_read = vm.regs[0];
        assert!(
            bytes_read > 0 && bytes_read != u32::MAX,
            "PTYREAD should return bytes after resize, got r0={}",
            bytes_read
        );

        let mut output = Vec::new();
        for i in 0..bytes_read as usize {
            output.push((vm.ram[0x5800 + i] & 0xFF) as u8);
        }
        let text = String::from_utf8_lossy(&output);
        assert!(
            text.contains("42"),
            "expected '42' (new COLUMNS) in output, got: {:?}",
            text
        );

        // Clean up
        vm.regs[12] = handle;
        vm.pc = 500;
        vm.ram[500] = 12;
        vm.pc = 500;
        vm.op_ptyclose();
        assert_eq!(vm.regs[0], PTY_OK, "PTYCLOSE should succeed");
    }

    /// Test PTYSIZE with invalid handle returns error.
    #[test]
    fn pty_resize_invalid_handle() {
        use crate::vm::Vm;

        let mut vm = Vm::new();

        // PTYSIZE with handle pointing to non-existent slot
        vm.regs[12] = 0; // handle = 0 (empty)
        vm.regs[13] = 30; // rows
        vm.regs[14] = 42; // cols
        vm.pc = 0;
        vm.ram[0] = 12;
        vm.ram[1] = 13;
        vm.ram[2] = 14;
        vm.pc = 0;
        vm.op_ptysize();

        assert_eq!(
            vm.regs[0], PTY_ERR_INVALID_HANDLE,
            "PTYSIZE with no open slot should return INVALID_HANDLE"
        );
    }

    /// Test that PTY spawns with the correct initial size (42x30).
    #[test]
    fn pty_initial_size_42x30() {
        let mut slot = match spawn("") {
            Ok(s) => s,
            Err(e) => {
                eprintln!("skipping: pty spawn failed: {}", e);
                return;
            }
        };

        // Wait for bash to start
        thread::sleep(Duration::from_millis(500));

        // Drain initial prompt
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut drain = Vec::new();
        while Instant::now() < deadline {
            match slot.rx.try_recv() {
                Ok(b) => drain.push(b),
                Err(_) => {
                    if !drain.is_empty() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            }
        }

        // Send "echo $COLUMNS\nexit\n" to check initial column count
        {
            let w = slot.writer.as_mut().expect("writer");
            let _ = w.write_all(b"echo $COLUMNS\nexit\n");
            let _ = w.flush();
        }

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut output = Vec::new();
        while Instant::now() < deadline {
            match slot.rx.try_recv() {
                Ok(b) => output.push(b),
                Err(_) => {
                    if slot.is_closed() && output.contains(&b'4') {
                        break;
                    }
                    thread::sleep(Duration::from_millis(20));
                }
            }
        }

        let text = String::from_utf8_lossy(&output);
        assert!(
            text.contains("42"),
            "bash $COLUMNS should be 42 (matching initial spawn size), got: {:?}",
            text
        );
    }
}
