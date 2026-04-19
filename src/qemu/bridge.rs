// qemu/bridge.rs -- QEMU subprocess bridge
//
// Manages a QEMU subprocess with piped stdin/stdout.
// Reads QEMU output and writes it to a canvas buffer via the ANSI handler.

use std::io::Read;
use std::io::Write;
use std::process::{Child, ChildStdin, ChildStdout};

use super::ansi::AnsiHandler;
use super::config::{arch_to_qemu, QemuConfig};
use super::cursor::Cursor;

#[allow(dead_code)]
const QEMU_READ_BUF_SIZE: usize = 4096;

/// Manages a QEMU subprocess with piped stdin/stdout.
/// Reads QEMU output and writes it to a canvas buffer via the ANSI handler.
pub struct QemuBridge {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    /// ANSI escape sequence handler.
    ansi: AnsiHandler,
    /// Whether the QEMU process is still running.
    alive: bool,
}

impl QemuBridge {
    /// Spawn a QEMU process from a config string.
    /// Config format: "arch=riscv64 kernel=linux.img ram=256M"
    pub fn spawn(config_str: &str) -> Result<QemuBridge, String> {
        let config = QemuConfig::parse(config_str)?;
        let mut cmd = config.build_command()?;

        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                let (binary, _) =
                    arch_to_qemu(&config.arch).unwrap_or(("qemu-system-unknown", None));
                format!(
                    "QEMU not found: '{}'. Install with: sudo apt install qemu-system-{}",
                    binary, config.arch
                )
            } else {
                format!("failed to spawn QEMU: {}", e)
            }
        })?;

        let stdin = child.stdin.take().ok_or("failed to open QEMU stdin")?;
        let mut stdout = child.stdout.take().ok_or("failed to open QEMU stdout")?;

        // Set stdout to non-blocking so read doesn't hang the CLI loop
        use std::os::unix::io::AsRawFd;
        let fd = stdout.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL, 0) };
        if flags >= 0 {
            unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
        }

        Ok(QemuBridge {
            child,
            stdin,
            stdout,
            ansi: AnsiHandler::new(),
            alive: true,
        })
    }

    /// Read available output from QEMU stdout.
    /// Returns the number of bytes read.
    pub fn read_output(&mut self, canvas_buffer: &mut [u32]) -> usize {
        if !self.alive {
            return 0;
        }

        let mut tmp_buf = [0u8; 1024];
        let mut total_read = 0usize;

        loop {
            match self.stdout.read(&mut tmp_buf) {
                Ok(0) => {
                    self.alive = false;
                    break;
                }
                Ok(n) => {
                    total_read += n;
                    self.ansi.process_bytes(&tmp_buf[..n], canvas_buffer);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    break;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {
                    continue;
                }
                Err(_) => {
                    self.alive = false;
                    break;
                }
            }
        }

        total_read
    }

    /// Read available output from QEMU stdout as raw text (CLI mode).
    /// Strips ANSI escape sequences and returns printable text.
    /// Returns the text read (may be empty if no data available).
    pub fn read_output_text(&mut self) -> String {
        if !self.alive {
            return String::new();
        }

        let mut tmp_buf = [0u8; 4096];
        let mut result = String::new();

        loop {
            match self.stdout.read(&mut tmp_buf) {
                Ok(0) => {
                    self.alive = false;
                    break;
                }
                Ok(n) => {
                    let bytes = &tmp_buf[..n];
                    // Strip ANSI escapes for display
                    let mut i = 0;
                    while i < bytes.len() {
                        let b = bytes[i];
                        if b == 0x1B {
                            // Skip escape sequence
                            i += 1;
                            if i < bytes.len() && bytes[i] == b'[' {
                                i += 1;
                                while i < bytes.len() {
                                    let c = bytes[i];
                                    i += 1;
                                    if c.is_ascii_alphabetic() || c == b'~' {
                                        break;
                                    }
                                }
                            } else if i < bytes.len() {
                                i += 1; // skip the char after ESC
                            }
                        } else if b >= 0x20 && b < 0x7F {
                            result.push(b as char);
                            i += 1;
                        } else if b == b'\n' {
                            result.push('\n');
                            i += 1;
                        } else if b == b'\r' {
                            i += 1;
                        } else {
                            i += 1; // skip other control chars
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    break;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {
                    continue;
                }
                Err(_) => {
                    self.alive = false;
                    break;
                }
            }
        }

        result
    }

    /// Write a byte to QEMU stdin.
    pub fn write_byte(&mut self, b: u8) -> std::io::Result<()> {
        if !self.alive {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "QEMU process is not running",
            ));
        }
        self.stdin.write_all(&[b])?;
        self.stdin.flush()?;
        Ok(())
    }

    /// Write a slice of bytes to QEMU stdin.
    pub fn write_bytes(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        if !self.alive {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "QEMU process is not running",
            ));
        }
        self.stdin.write_all(bytes)?;
        self.stdin.flush()?;
        Ok(())
    }

    /// Send a key press to QEMU stdin.
    pub fn send_key(&mut self, ascii_byte: u8) -> std::io::Result<()> {
        self.write_byte(ascii_byte)
    }

    /// Check if the QEMU process is still alive.
    pub fn is_alive(&mut self) -> bool {
        if !self.alive {
            return false;
        }
        match self.child.try_wait() {
            Ok(Some(_status)) => {
                self.alive = false;
                false
            }
            Ok(None) => true,
            Err(_) => {
                self.alive = false;
                false
            }
        }
    }

    /// Get the current cursor position.
    pub fn cursor(&self) -> Cursor {
        self.ansi.cursor()
    }

    /// Get the ANSI handler for direct access.
    pub fn ansi_handler(&self) -> &AnsiHandler {
        &self.ansi
    }

    /// Kill the QEMU process.
    pub fn kill(&mut self) -> std::io::Result<()> {
        self.child.kill()?;
        self.alive = false;
        Ok(())
    }
}

impl Drop for QemuBridge {
    fn drop(&mut self) {
        if self.alive {
            let _ = self.child.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    // QemuBridge tests require QEMU installed, so we only test config
    // and ANSI handler here (those tests live in their respective modules).
    // Integration tests for QemuBridge would go in tests/ with #[ignore].
}
