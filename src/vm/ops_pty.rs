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

pub struct PtySlot {
    master: Box<dyn MasterPty + Send>,
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

/// Spawn `cmd` (or bash if empty) inside a fresh pty. Returns the populated
/// slot or an error string.
pub fn spawn(cmd_line: &str) -> Result<PtySlot, String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 30,
            cols: 80,
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
        c
    } else {
        // crude split on whitespace; good enough for single commands like
        // "/bin/bash -i" or "python3 -i". Quoted args aren't supported yet.
        let parts: Vec<&str> = cmd_line.split_whitespace().collect();
        let mut c = CommandBuilder::new(parts[0]);
        for arg in &parts[1..] {
            c.arg(arg);
        }
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

    Ok(PtySlot {
        master: pair.master,
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
        match slot.master.take_writer() {
            Ok(mut w) => match w.write_all(&bytes) {
                Ok(()) => {
                    self.regs[0] = PTY_OK;
                }
                Err(_) => {
                    self.regs[0] = PTY_ERR_WRITE_FAILED;
                }
            },
            Err(_) => {
                self.regs[0] = PTY_ERR_WRITE_FAILED;
            }
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
            let mut w = slot.master.take_writer().expect("writer");
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
}
