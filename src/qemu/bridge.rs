// qemu/bridge.rs -- QEMU subprocess bridge
//
// Manages a QEMU subprocess with piped stdin/stdout.
// Reads QEMU output and writes it to a canvas buffer via the ANSI handler.

use std::io::Read;
use std::io::Write;
use std::process::{Child, ChildStdin, ChildStdout};

use super::ansi::{AnsiHandler, Cursor};
use super::config::{arch_to_qemu, QemuConfig};

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
                let (binary, _) = arch_to_qemu(&config.arch)
                    .unwrap_or(("qemu-system-unknown", None));
                format!(
                    "QEMU not found: '{}'. Install with: sudo apt install qemu-system-{}",
                    binary, config.arch
                )
            } else {
                format!("failed to spawn QEMU: {}", e)
            }
        })?;

        let stdin = child.stdin.take().ok_or("failed to open QEMU stdin")?;
        let stdout = child.stdout.take().ok_or("failed to open QEMU stdout")?;

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
