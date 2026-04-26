/// Phase 137: Host Filesystem Bridge
/// FSOPEN, FSCLOSE, FSREAD, FSWRITE, FSLS opcodes
/// Allows Geometry OS programs to read/write files on the host Linux filesystem.
/// Security: restricted to the user's home directory by default.
use super::types::*;
use super::Vm;
use std::io::{Read, Write};

/// Maximum path length for host filesystem operations
const MAX_PATH_LEN: usize = 512;
/// Maximum directory listing size (bytes)
const MAX_DIR_BUF: usize = 4096;
/// Maximum bytes per FSREAD/FSWRITE call
const MAX_IO_CHUNK: usize = 4096;

impl Vm {
    /// Check if a path is within the user's home directory (security sandbox).
    /// Returns the canonicalized path if safe, or None if the path escapes the sandbox.
    fn sandbox_check(path: &str) -> Option<std::path::PathBuf> {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let home_dir = std::path::PathBuf::from(&home);

        // Canonicalize the home dir (resolve symlinks)
        let home_canon = match home_dir.canonicalize() {
            Ok(h) => h,
            Err(_) => return None,
        };

        // Handle paths starting with ~
        let expanded = if path.starts_with("~/") {
            format!("{}/{}", home, &path[2..])
        } else if path == "~" {
            home.clone()
        } else if path.starts_with('/') {
            path.to_string()
        } else {
            // Relative paths are relative to home
            format!("{}/{}", home, path)
        };

        let target = std::path::PathBuf::from(&expanded);

        // Canonicalize if it exists, otherwise check the parent + join
        let canon = if target.exists() {
            match target.canonicalize() {
                Ok(c) => c,
                Err(_) => return None,
            }
        } else {
            // For new files, canonicalize parent and join the filename
            let parent = target.parent()?;
            let canon_parent = match parent.canonicalize() {
                Ok(c) => c,
                Err(_) => return None,
            };
            let filename = target.file_name()?;
            canon_parent.join(filename)
        };

        // Check if canonicalized path starts with home directory
        if canon.starts_with(&home_canon) {
            Some(canon)
        } else {
            None
        }
    }

    /// FSOPEN path_reg, mode_reg  (0xB9) -- Open a host file.
    /// path_reg: register containing RAM address of null-terminated path string.
    /// mode_reg: 0=read, 1=write(create), 2=append, 3=read+write
    /// Returns: handle index in r0 (0..MAX_HOST_FILES-1), or 0xFFFFFFFF on error.
    /// Encoding: 3 words [0xB9, path_reg, mode_reg]
    pub(super) fn op_fsopen(&mut self) {
        let path_reg = self.fetch() as usize;
        let mode_reg = self.fetch() as usize;

        if path_reg >= NUM_REGS || mode_reg >= NUM_REGS {
            self.regs[0] = 0xFFFFFFFF;
            return;
        }

        let path_addr = self.regs[path_reg] as usize;
        let mode = self.regs[mode_reg];

        // Read null-terminated path from RAM
        let path_str = match Self::read_string_static(&self.ram, path_addr) {
            Some(s) => s,
            None => {
                self.regs[0] = 0xFFFFFFFF;
                return;
            }
        };

        if path_str.is_empty() || path_str.len() > MAX_PATH_LEN {
            self.regs[0] = 0xFFFFFFFF;
            return;
        }

        // Security: sandbox to home directory
        let safe_path = match Self::sandbox_check(&path_str) {
            Some(p) => p,
            None => {
                self.regs[0] = 0xFFFFFFFE; // EACCES
                return;
            }
        };

        // Find a free slot
        let slot = self.host_file_handles.iter().position(|h| h.is_none());
        let slot = match slot {
            Some(s) => s,
            None => {
                self.regs[0] = 0xFFFFFFFD; // EMFILE (too many open files)
                return;
            }
        };

        let file = match mode {
            0 => std::fs::File::open(&safe_path),   // read
            1 => std::fs::File::create(&safe_path), // write (create/truncate)
            2 => std::fs::OpenOptions::new() // append
                .write(true)
                .append(true)
                .open(&safe_path),
            3 => std::fs::OpenOptions::new() // read+write
                .read(true)
                .write(true)
                .create(true)
                .open(&safe_path),
            _ => {
                self.regs[0] = 0xFFFFFFFF; // EINVAL
                return;
            }
        };

        match file {
            Ok(f) => {
                self.host_file_handles[slot] = Some((f, mode as u8));
                self.regs[0] = slot as u32;
            }
            Err(_) => {
                self.regs[0] = 0xFFFFFFFF;
            }
        }
    }

    /// FSCLOSE handle_reg  (0xBA) -- Close a host file handle.
    /// handle_reg: register containing the handle index from FSOPEN.
    /// Returns: 0 in r0 on success, 0xFFFFFFFF on error.
    /// Encoding: 2 words [0xBA, handle_reg]
    pub(super) fn op_fsclose(&mut self) {
        let handle_reg = self.fetch() as usize;
        if handle_reg >= NUM_REGS {
            self.regs[0] = 0xFFFFFFFF;
            return;
        }

        let handle = self.regs[handle_reg] as usize;
        if handle >= self.host_file_handles.len() {
            self.regs[0] = 0xFFFFFFFF;
            return;
        }

        if self.host_file_handles[handle].take().is_some() {
            self.regs[0] = 0; // success
        } else {
            self.regs[0] = 0xFFFFFFFF; // EBADF
        }
    }

    /// FSREAD handle_reg, buf_reg, len_reg  (0xBB) -- Read from host file.
    /// Reads up to len bytes into RAM buffer starting at buf_reg address.
    /// Each byte goes into one u32 RAM cell.
    /// Returns: bytes read in r0, 0xFFFFFFFF on error.
    /// Encoding: 4 words [0xBB, handle_reg, buf_reg, len_reg]
    pub(super) fn op_fsread(&mut self) {
        let handle_reg = self.fetch() as usize;
        let buf_reg = self.fetch() as usize;
        let len_reg = self.fetch() as usize;

        if handle_reg >= NUM_REGS || buf_reg >= NUM_REGS || len_reg >= NUM_REGS {
            self.regs[0] = 0xFFFFFFFF;
            return;
        }

        let handle = self.regs[handle_reg] as usize;
        let buf_addr = self.regs[buf_reg] as usize;
        let max_len = self.regs[len_reg] as usize;

        if handle >= self.host_file_handles.len() || max_len > MAX_IO_CHUNK {
            self.regs[0] = 0xFFFFFFFF;
            return;
        }

        let file_ref = match self.host_file_handles.get_mut(handle) {
            Some(Some((f, _mode))) => f,
            _ => {
                self.regs[0] = 0xFFFFFFFF;
                return;
            }
        };

        let mut tmp_buf = vec![0u8; max_len];
        match file_ref.read(&mut tmp_buf) {
            Ok(n) => {
                for i in 0..n {
                    let addr = buf_addr + i;
                    if addr < self.ram.len() {
                        self.ram[addr] = tmp_buf[i] as u32;
                    }
                }
                self.regs[0] = n as u32;
            }
            Err(_) => {
                self.regs[0] = 0xFFFFFFFF;
            }
        }
    }

    /// FSWRITE handle_reg, buf_reg, len_reg  (0xBC) -- Write to host file.
    /// Writes len bytes from RAM buffer starting at buf_reg address.
    /// Each RAM cell's low byte is written.
    /// Returns: bytes written in r0, 0xFFFFFFFF on error.
    /// Encoding: 4 words [0xBC, handle_reg, buf_reg, len_reg]
    pub(super) fn op_fswrite(&mut self) {
        let handle_reg = self.fetch() as usize;
        let buf_reg = self.fetch() as usize;
        let len_reg = self.fetch() as usize;

        if handle_reg >= NUM_REGS || buf_reg >= NUM_REGS || len_reg >= NUM_REGS {
            self.regs[0] = 0xFFFFFFFF;
            return;
        }

        let handle = self.regs[handle_reg] as usize;
        let buf_addr = self.regs[buf_reg] as usize;
        let len = self.regs[len_reg] as usize;

        if handle >= self.host_file_handles.len() || len > MAX_IO_CHUNK {
            self.regs[0] = 0xFFFFFFFF;
            return;
        }

        let file_ref = match self.host_file_handles.get_mut(handle) {
            Some(Some((f, _mode))) => f,
            _ => {
                self.regs[0] = 0xFFFFFFFF;
                return;
            }
        };

        // Collect bytes from RAM
        let mut tmp_buf = Vec::with_capacity(len);
        for i in 0..len {
            let addr = buf_addr + i;
            if addr < self.ram.len() {
                tmp_buf.push((self.ram[addr] & 0xFF) as u8);
            } else {
                break;
            }
        }

        match file_ref.write_all(&tmp_buf) {
            Ok(()) => {
                if let Err(_) = file_ref.flush() {
                    self.regs[0] = 0xFFFFFFFF;
                    return;
                }
                self.regs[0] = tmp_buf.len() as u32;
            }
            Err(_) => {
                self.regs[0] = 0xFFFFFFFF;
            }
        }
    }

    /// FSLS path_reg, buf_reg, max_len_reg  (0xBD) -- List directory contents.
    /// path_reg: register containing RAM address of null-terminated directory path.
    /// Writes null-separated filenames to RAM buffer at buf_reg.
    /// Returns: total bytes written in r0 (including null separators), 0xFFFFFFFF on error.
    /// Encoding: 4 words [0xBD, path_reg, buf_reg, max_len_reg]
    pub(super) fn op_fsls(&mut self) {
        let path_reg = self.fetch() as usize;
        let buf_reg = self.fetch() as usize;
        let max_len_reg = self.fetch() as usize;

        if path_reg >= NUM_REGS || buf_reg >= NUM_REGS || max_len_reg >= NUM_REGS {
            self.regs[0] = 0xFFFFFFFF;
            return;
        }

        let path_addr = self.regs[path_reg] as usize;
        let buf_addr = self.regs[buf_reg] as usize;
        let max_len = self.regs[max_len_reg] as usize;

        if max_len > MAX_DIR_BUF {
            self.regs[0] = 0xFFFFFFFF;
            return;
        }

        // Read path from RAM
        let path_str = match Self::read_string_static(&self.ram, path_addr) {
            Some(s) => s,
            None => {
                self.regs[0] = 0xFFFFFFFF;
                return;
            }
        };

        // Security: sandbox check
        let safe_path = match Self::sandbox_check(&path_str) {
            Some(p) => p,
            None => {
                self.regs[0] = 0xFFFFFFFE; // EACCES
                return;
            }
        };

        // Read directory
        let entries = match std::fs::read_dir(&safe_path) {
            Ok(rd) => rd,
            Err(_) => {
                self.regs[0] = 0xFFFFFFFF;
                return;
            }
        };

        let mut offset = 0;
        for entry in entries {
            if let Ok(e) = entry {
                if let Some(name) = e.file_name().to_str() {
                    for ch in name.chars() {
                        if offset < max_len && buf_addr + offset < self.ram.len() {
                            self.ram[buf_addr + offset] = ch as u32;
                            offset += 1;
                        }
                    }
                    // Null separator between names
                    if offset < max_len && buf_addr + offset < self.ram.len() {
                        self.ram[buf_addr + offset] = 0;
                        offset += 1;
                    }
                }
            }
            if offset >= max_len {
                break;
            }
        }

        self.regs[0] = offset as u32;
    }
}
