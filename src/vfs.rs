// vfs.rs -- Geometry OS Virtual Filesystem Layer
//
// Provides a simple host-backed filesystem for Geometry OS programs.
// Files live in .geometry_os/fs/ relative to the current working directory.
// Per-process file descriptor tables track open files (max 16 per process).

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
#[cfg(test)]
use std::path::Path;

/// Maximum open file descriptors per process
pub const MAX_FDS: usize = 16;

/// FOPEN modes
pub const FOPEN_READ: u32 = 0;
pub const FOPEN_WRITE: u32 = 1;
pub const FOPEN_APPEND: u32 = 2;

/// FSEEK whence values
pub const FSEEK_SET: u32 = 0;
pub const FSEEK_CUR: u32 = 1;
pub const FSEEK_END: u32 = 2;

/// Sentinel for invalid fd / error
pub const FD_ERROR: u32 = 0xFFFFFFFF;

/// VFS root directory name
const FS_DIR: &str = ".geometry_os/fs";

/// An open file handle inside the VFS.
/// Stores the host file object, the filename, and the open mode.
#[derive(Debug)]
pub struct OpenFile {
    pub file: fs::File,
    #[allow(dead_code)]
    pub name: String,
    pub mode: u32,
}

/// Per-process file descriptor table.
/// Maps fd number (0-based) to open file handle.
/// fd 0 = stdin (reserved), fd 1 = stdout (reserved), fd 2 = stderr (reserved).
#[derive(Debug)]
#[allow(dead_code)]
pub struct FdTable {
    fds: Vec<Option<OpenFile>>,
}

impl Default for FdTable {
    fn default() -> Self {
        Self::new()
    }
}

impl FdTable {
    pub fn new() -> Self {
        let mut fds = Vec::with_capacity(MAX_FDS);
        for _ in 0..MAX_FDS {
            fds.push(None);
        }
        // Reserve fd 0, 1, 2 for standard streams
        FdTable { fds }
    }

    /// Allocate the lowest available fd slot >= 3. Returns fd number or FD_ERROR.
    pub fn alloc_fd(&mut self) -> u32 {
        for (i, slot) in self.fds.iter().enumerate() {
            if i < 3 {
                continue; // skip reserved
            }
            if slot.is_none() {
                return i as u32;
            }
        }
        FD_ERROR
    }

    /// Get a mutable reference to the open file at the given fd.
    #[allow(dead_code)]
    pub fn get(&mut self, fd: u32) -> Option<&mut OpenFile> {
        let idx = fd as usize;
        if idx < self.fds.len() {
            self.fds[idx].as_mut()
        } else {
            None
        }
    }

    /// Close the file at the given fd. Returns true if file was open.
    pub fn close(&mut self, fd: u32) -> bool {
        let idx = fd as usize;
        if idx < self.fds.len() && self.fds[idx].is_some() {
            self.fds[idx] = None;
            true
        } else {
            false
        }
    }

    /// Count open file descriptors.
    #[allow(dead_code)]
    pub fn open_count(&self) -> usize {
        self.fds.iter().filter(|f| f.is_some()).count()
    }
}

/// The global VFS instance. Manages the host-backed filesystem directory
/// and the per-process fd tables.
#[derive(Debug)]
pub struct Vfs {
    /// Base directory for all Geometry OS files
    pub base_dir: PathBuf,
    /// Per-process fd tables (keyed by PID, 0 = main process)
    fd_tables: HashMap<u32, FdTable>,
}

impl Default for Vfs {
    fn default() -> Self {
        Self::new()
    }
}

impl Vfs {
    /// Create a new VFS instance, ensuring the base directory exists.
    pub fn new() -> Self {
        let base_dir = PathBuf::from(FS_DIR);
        // Create the filesystem directory if it doesn't exist
        let _ = fs::create_dir_all(&base_dir);
        Vfs {
            base_dir,
            fd_tables: HashMap::new(),
        }
    }

    /// Get or create the fd table for a given PID.
    fn fd_table(&mut self, pid: u32) -> &mut FdTable {
        self.fd_tables
            .entry(pid)
            .or_default()
    }

    /// Open a file. Returns fd number or FD_ERROR.
    ///
    /// `name_addr` reads a null-terminated filename from RAM.
    /// `mode`: 0=read, 1=write, 2=append.
    pub fn fopen(
        &mut self,
        ram: &[u32],
        name_addr: u32,
        mode: u32,
        pid: u32,
    ) -> u32 {
        let filename = match Self::read_string(ram, name_addr as usize) {
            Some(s) => s,
            None => return FD_ERROR,
        };

        // Sanitize: no path separators, no parent directory traversal
        if filename.is_empty()
            || filename.contains('/')
            || filename.contains('\\')
            || filename.contains("..")
            || filename.len() > 64
        {
            return FD_ERROR;
        }

        let filepath = self.base_dir.join(&filename);

        let result = match mode {
            FOPEN_READ => fs::File::open(&filepath),
            FOPEN_WRITE => fs::File::create(&filepath),
            FOPEN_APPEND => {
                match fs::OpenOptions::new().append(true).create(true).open(&filepath) {
                    Ok(f) => Ok(f),
                    Err(_) => fs::File::create(&filepath),
                }
            }
            _ => return FD_ERROR,
        };

        let file = match result {
            Ok(f) => f,
            Err(_) => return FD_ERROR,
        };

        let table = self.fd_table(pid);
        let fd = table.alloc_fd();
        if fd == FD_ERROR {
            return FD_ERROR;
        }

        if let Some(slot) = table.fds.get_mut(fd as usize) {
            *slot = Some(OpenFile {
                file,
                name: filename,
                mode,
            });
        }

        fd
    }

    /// Read from a file descriptor into RAM.
    /// Returns number of bytes read (each RAM cell holds one byte in low 8 bits).
    pub fn fread(
        &mut self,
        ram: &mut [u32],
        fd: u32,
        buf_addr: u32,
        len: u32,
        pid: u32,
    ) -> u32 {
        let table = self.fd_table(pid);
        let open_file = match table.fds.get_mut(fd as usize) {
            Some(Some(f)) => f,
            _ => return FD_ERROR,
        };

        if open_file.mode == FOPEN_WRITE {
            return FD_ERROR; // can't read a write-only file
        }

        let mut buf = vec![0u8; len as usize];
        match open_file.file.read(&mut buf) {
            Ok(bytes_read) => {
                // Write bytes into RAM (low 8 bits of each u32)
                for (i, &byte) in buf.iter().enumerate().take(bytes_read) {
                    let addr = (buf_addr as usize) + i;
                    if addr < ram.len() {
                        ram[addr] = byte as u32;
                    }
                }
                bytes_read as u32
            }
            Err(_) => FD_ERROR,
        }
    }

    /// Write from RAM to a file descriptor.
    /// Returns number of bytes written.
    pub fn fwrite(
        &mut self,
        ram: &[u32],
        fd: u32,
        buf_addr: u32,
        len: u32,
        pid: u32,
    ) -> u32 {
        let table = self.fd_table(pid);
        let open_file = match table.fds.get_mut(fd as usize) {
            Some(Some(f)) => f,
            _ => return FD_ERROR,
        };

        if open_file.mode == FOPEN_READ {
            return FD_ERROR; // can't write a read-only file
        }

        // Read bytes from RAM (low 8 bits of each u32)
        let buf: Vec<u8> = (0..len as usize)
            .map(|i| {
                let addr = (buf_addr as usize) + i;
                if addr < ram.len() {
                    (ram[addr] & 0xFF) as u8
                } else {
                    0
                }
            })
            .collect();

        match open_file.file.write(&buf) {
            Ok(bytes_written) => {
                let _ = open_file.file.flush();
                bytes_written as u32
            }
            Err(_) => FD_ERROR,
        }
    }

    /// Close a file descriptor. Returns 0 on success, FD_ERROR on error.
    pub fn fclose(&mut self, fd: u32, pid: u32) -> u32 {
        let table = self.fd_table(pid);
        if table.close(fd) {
            0
        } else {
            FD_ERROR
        }
    }

    /// Seek within a file descriptor.
    /// `whence`: 0=SET, 1=CUR, 2=END.
    /// Returns new position from start of file, or FD_ERROR on error.
    pub fn fseek(&mut self, fd: u32, offset: u32, whence: u32, pid: u32) -> u32 {
        let table = self.fd_table(pid);
        let open_file = match table.fds.get_mut(fd as usize) {
            Some(Some(f)) => f,
            _ => return FD_ERROR,
        };

        let seek_from = match whence {
            FSEEK_SET => SeekFrom::Start(offset as u64),
            FSEEK_CUR => SeekFrom::Current(offset as i64),
            FSEEK_END => SeekFrom::End(offset as i64),
            _ => return FD_ERROR,
        };

        match open_file.file.seek(seek_from) {
            Ok(pos) => pos as u32,
            Err(_) => FD_ERROR,
        }
    }

    /// List files in the VFS directory.
    /// Writes null-terminated filenames into RAM at `buf_addr`.
    /// Each filename is followed by a null byte. The list ends with an empty string (double null).
    /// Returns number of entries found, or FD_ERROR on error.
    pub fn fls(&mut self, ram: &mut [u32], buf_addr: u32) -> u32 {
        let entries = match fs::read_dir(&self.base_dir) {
            Ok(rd) => rd,
            Err(_) => return FD_ERROR,
        };

        let mut addr = buf_addr as usize;
        let mut count = 0u32;

        // Collect and sort entries
        let mut names: Vec<String> = Vec::new();
        for e in entries.flatten() {
            if let Some(name) = e.file_name().to_str() {
                if !name.starts_with('.') {
                    names.push(name.to_string());
                }
            }
        }
        names.sort();

        for name in &names {
            for ch in name.bytes() {
                if addr < ram.len() {
                    ram[addr] = ch as u32;
                    addr += 1;
                }
            }
            if addr < ram.len() {
                ram[addr] = 0; // null terminator
                addr += 1;
            }
            count += 1;
        }
        // End-of-list marker: extra null byte
        if addr < ram.len() {
            ram[addr] = 0;
        }

        count
    }

    /// Duplicate an fd from source_pid's table into target_pid's table at target_fd slot.
    /// This is used by EXECP to set up stdin/stdout redirection for child processes.
    /// Returns true on success.
    pub fn dup_fd(&mut self, src_fd: u32, target_fd: u32, target_pid: u32, src_pid: u32) -> bool {
        // Read the source fd from source PID's table
        let source_file = {
            let src_table = match self.fd_tables.get_mut(&src_pid) {
                Some(t) => t,
                None => return false,
            };
            match src_table.fds.get(src_fd as usize) {
                Some(Some(f)) => {
                    // Re-open the same file to get an independent handle
                    let path = f.file.try_clone();
                    match path {
                        Ok(cloned_file) => OpenFile {
                            file: cloned_file,
                            name: f.name.clone(),
                            mode: f.mode,
                        },
                        Err(_) => return false,
                    }
                }
                _ => return false,
            }
        };

        // Write into target PID's table at the specified slot
        let target_table = self.fd_table(target_pid);
        let idx = target_fd as usize;
        if idx < target_table.fds.len() {
            target_table.fds[idx] = Some(source_file);
            true
        } else {
            false
        }
    }

    /// Close all file descriptors for a given PID.
    #[allow(dead_code)]
    pub fn close_all(&mut self, pid: u32) {
        self.fd_tables.remove(&pid);
    }

    /// Get the base directory path (for testing).
    #[cfg(test)]
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Read a null-terminated string from RAM starting at `addr`.
    /// Each u32 holds one ASCII byte in the low 8 bits.
    fn read_string(ram: &[u32], addr: usize) -> Option<String> {
        if addr >= ram.len() {
            return None;
        }
        let mut chars = Vec::new();
        let mut a = addr;
        let max_len = 64; // max filename length
        while a < ram.len() && chars.len() < max_len {
            let byte = (ram[a] & 0xFF) as u8;
            if byte == 0 {
                break;
            }
            chars.push(byte as char);
            a += 1;
        }
        if chars.is_empty() {
            None
        } else {
            Some(chars.into_iter().collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fd_table_alloc_finds_free_slot() {
        let mut table = FdTable::new();
        // First free slot is 3 (0, 1, 2 reserved)
        assert_eq!(table.alloc_fd(), 3);
        // alloc_fd doesn't reserve, so calling again still returns 3
        assert_eq!(table.alloc_fd(), 3);
    }

    #[test]
    fn test_fd_table_close() {
        let mut table = FdTable::new();
        // Closing an unopened fd returns false
        assert!(!table.close(3));
    }

    #[test]
    fn test_vfs_open_read_write_cycle() {
        let dir = std::env::temp_dir().join("geo_test_vfs_rw");
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);

        let mut vfs = Vfs::new();
        vfs.base_dir = dir.clone();

        // Create a file in the VFS dir
        fs::write(dir.join("test.txt"), b"Hello").expect("filesystem operation failed");

        // Write filename into RAM
        let mut ram = vec![0u32; 256];
        let name = "test.txt";
        for (i, ch) in name.bytes().enumerate() {
            ram[100 + i] = ch as u32;
        }
        ram[100 + name.len()] = 0; // null terminator

        // Open for read (PID 0 = main process)
        let fd = vfs.fopen(&ram, 100, FOPEN_READ, 0);
        assert_ne!(fd, FD_ERROR, "fopen should succeed");
        assert_eq!(fd, 3, "first allocatable fd should be 3");

        // Read from file
        let mut buf_ram = vec![0u32; 256];
        let n = vfs.fread(&mut buf_ram, fd, 200, 5, 0);
        assert_eq!(n, 5, "should read 5 bytes");

        // Verify content
        let mut result = String::new();
        for i in 0..5 {
            result.push((buf_ram[200 + i] & 0xFF) as u8 as char);
        }
        assert_eq!(result, "Hello");

        // Close
        assert_eq!(vfs.fclose(fd, 0), 0);

        // Clean up
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_vfs_write_and_read_back() {
        let dir = std::env::temp_dir().join("geo_test_vfs_wr");
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);

        let mut vfs = Vfs::new();
        vfs.base_dir = dir.clone();

        // Write filename into RAM
        let mut ram = vec![0u32; 256];
        let name = "output.txt";
        for (i, ch) in name.bytes().enumerate() {
            ram[50 + i] = ch as u32;
        }
        ram[50 + name.len()] = 0;

        // Open for write
        let fd = vfs.fopen(&ram, 50, FOPEN_WRITE, 0);
        assert_ne!(fd, FD_ERROR);

        // Write data
        let data = b"World";
        for (i, &ch) in data.iter().enumerate() {
            ram[200 + i] = ch as u32;
        }
        let n = vfs.fwrite(&ram, fd, 200, 5, 0);
        assert_eq!(n, 5, "should write 5 bytes");

        // Close
        assert_eq!(vfs.fclose(fd, 0), 0);

        // Verify on host filesystem
        let content = fs::read(dir.join("output.txt")).expect("filesystem operation failed");
        assert_eq!(&content, b"World");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_vfs_list_directory() {
        let dir = std::env::temp_dir().join("geo_test_vfs_ls");
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);

        let mut vfs = Vfs::new();
        vfs.base_dir = dir.clone();

        // Create some files
        fs::write(dir.join("alpha.txt"), b"A").expect("filesystem operation failed");
        fs::write(dir.join("beta.txt"), b"B").expect("filesystem operation failed");

        let mut ram = vec![0u32; 256];
        let count = vfs.fls(&mut ram, 100);
        assert_eq!(count, 2, "should list 2 files");

        // Parse the entries from RAM
        let mut entries = Vec::new();
        let mut addr = 100;
        while addr < 256 {
            let ch = (ram[addr] & 0xFF) as u8;
            if ch == 0 {
                break;
            }
            let mut name = String::new();
            while addr < 256 {
                let c = (ram[addr] & 0xFF) as u8;
                if c == 0 {
                    addr += 1;
                    break;
                }
                name.push(c as char);
                addr += 1;
            }
            entries.push(name);
        }
        entries.sort();
        assert_eq!(entries, vec!["alpha.txt", "beta.txt"]);

        let _ = fs::remove_dir_all(&dir);
    }
}