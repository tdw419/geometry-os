// inode_fs.rs -- Geometry OS In-Memory Inode Filesystem
//
// Provides an in-memory inode-based filesystem with directory tree support.
// Inodes track file type (regular, directory, device, pipe), size, data,
// and directory children. Path resolution supports absolute and relative paths.

#![allow(dead_code)]

use std::collections::HashMap;

/// Maximum filename length
pub const MAX_NAME_LEN: usize = 64;

/// Maximum path depth (components)
pub const MAX_PATH_DEPTH: usize = 16;

/// Maximum file data size in words (u32 each)
pub const MAX_FILE_SIZE: usize = 4096;

/// Inode types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum InodeType {
    Regular = 1,
    Directory = 2,
    Device = 3,
    Pipe = 4,
}

impl InodeType {
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            1 => Some(InodeType::Regular),
            2 => Some(InodeType::Directory),
            3 => Some(InodeType::Device),
            4 => Some(InodeType::Pipe),
            _ => None,
        }
    }

    pub fn to_u32(self) -> u32 {
        self as u32
    }
}

/// An inode in the filesystem.
#[derive(Debug, Clone)]
pub struct Inode {
    /// Unique inode number
    pub ino: u32,
    /// Type of this inode
    pub itype: InodeType,
    /// File data (only for regular files and pipes)
    pub data: Vec<u32>,
    /// For directories: map of name -> child inode number
    pub children: HashMap<String, u32>,
    /// Parent inode number (0 for root)
    pub parent: u32,
    /// Size in bytes (data.len() for regular, children.len() for dirs)
    pub size: u32,
    /// Reference count (open handles)
    pub ref_count: u32,
}

impl Inode {
    fn new(ino: u32, itype: InodeType, parent: u32) -> Self {
        Inode {
            ino,
            itype,
            data: Vec::new(),
            children: HashMap::new(),
            parent,
            size: 0,
            ref_count: 0,
        }
    }
}

/// FSTAT result structure written to RAM.
/// Layout at buf_addr: [ino, itype, size, ref_count, parent_ino, num_children]
pub const FSTAT_ENTRIES: usize = 6;

/// The in-memory inode filesystem.
#[derive(Debug)]
pub struct InodeFs {
    /// All inodes indexed by inode number
    inodes: HashMap<u32, Inode>,
    /// Next available inode number
    next_ino: u32,
    /// Current working directory inode number (starts at root = 1)
    cwd: u32,
}

impl Default for InodeFs {
    fn default() -> Self {
        Self::new()
    }
}

impl InodeFs {
    /// Create a new inode filesystem with a root directory (inode 1).
    pub fn new() -> Self {
        let mut fs = InodeFs {
            inodes: HashMap::new(),
            next_ino: 2,
            cwd: 1,
        };
        // Create root directory
        let root = Inode::new(1, InodeType::Directory, 0);
        fs.inodes.insert(1, root);
        fs
    }

    /// Allocate a new inode number.
    fn alloc_ino(&mut self) -> u32 {
        let ino = self.next_ino;
        self.next_ino += 1;
        ino
    }

    /// Resolve a path to an inode number.
    /// Supports absolute paths (starting with '/') and relative paths.
    /// Returns None if any component doesn't exist.
    pub fn resolve(&self, path: &str) -> Option<u32> {
        if path.is_empty() {
            return None;
        }

        let start_ino = if path.starts_with('/') {
            1 // root
        } else {
            self.cwd
        };

        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if components.is_empty() && path.starts_with('/') {
            return Some(1); // "/" resolves to root
        }

        let mut current = start_ino;
        for component in components {
            if component.len() > MAX_NAME_LEN {
                return None;
            }
            let inode = self.inodes.get(&current)?;
            if inode.itype != InodeType::Directory {
                return None;
            }
            match inode.children.get(component) {
                Some(&child_ino) => current = child_ino,
                None => return None,
            }
        }
        Some(current)
    }

    /// Create a directory at the given path.
    /// Parent directories must exist. Returns inode number on success, 0 on error.
    pub fn mkdir(&mut self, path: &str) -> u32 {
        if path.is_empty() || path == "/" {
            return 0;
        }

        // Find parent directory and new directory name
        let (parent_path, name) = match path.rsplit_once('/') {
            Some((parent, name)) if !name.is_empty() => (parent, name),
            _ => return 0,
        };

        if name.len() > MAX_NAME_LEN || name.contains('\0') {
            return 0;
        }

        let parent_ino = match self.resolve(if parent_path.is_empty() { "/" } else { parent_path }) {
            Some(ino) => ino,
            None => return 0,
        };

        // Check parent is a directory
        let parent = match self.inodes.get(&parent_ino) {
            Some(p) if p.itype == InodeType::Directory => p,
            _ => return 0,
        };

        // Check name doesn't already exist
        if parent.children.contains_key(name) {
            return 0;
        }

        // Create new directory inode
        let new_ino = self.alloc_ino();
        let mut new_dir = Inode::new(new_ino, InodeType::Directory, parent_ino);
        // Add "." and ".." entries
        new_dir.children.insert(".".to_string(), new_ino);
        new_dir.children.insert("..".to_string(), parent_ino);
        new_dir.size = 0; // user-visible children only (not . and ..)
        self.inodes.insert(new_ino, new_dir);

        // Now insert into parent (separate borrow)
        if let Some(parent) = self.inodes.get_mut(&parent_ino) {
            parent.children.insert(name.to_string(), new_ino);
        }
        new_ino
    }

    /// Create a regular file at the given path.
    /// Returns inode number on success, 0 on error.
    pub fn create(&mut self, path: &str) -> u32 {
        self.create_file(path, InodeType::Regular)
    }

    /// Create a node at the given path with the specified type.
    /// Returns inode number on success, 0 on error.
    pub fn create_file(&mut self, path: &str, itype: InodeType) -> u32 {
        if path.is_empty() || path == "/" {
            return 0;
        }

        let (parent_path, name) = match path.rsplit_once('/') {
            Some((parent, name)) if !name.is_empty() => (parent, name),
            _ => return 0,
        };

        if name.len() > MAX_NAME_LEN || name.contains('\0') {
            return 0;
        }

        let parent_ino = match self.resolve(if parent_path.is_empty() { "/" } else { parent_path }) {
            Some(ino) => ino,
            None => return 0,
        };

        let parent = match self.inodes.get(&parent_ino) {
            Some(p) if p.itype == InodeType::Directory => p,
            _ => return 0,
        };

        if parent.children.contains_key(name) {
            return 0;
        }

        let new_ino = self.alloc_ino();
        let new_inode = Inode::new(new_ino, itype, parent_ino);
        self.inodes.insert(new_ino, new_inode);

        // Now insert into parent (separate borrow)
        if let Some(parent) = self.inodes.get_mut(&parent_ino) {
            parent.children.insert(name.to_string(), new_ino);
        }
        new_ino
    }

    /// Remove (unlink) a file or empty directory at the given path.
    /// Returns true on success. Cannot remove root or non-empty directories.
    pub fn unlink(&mut self, path: &str) -> bool {
        if path.is_empty() || path == "/" {
            return false;
        }

        let ino = match self.resolve(path) {
            Some(ino) if ino != 1 => ino, // can't remove root
            _ => return false,
        };

        let inode = match self.inodes.get(&ino) {
            Some(i) => i,
            None => return false,
        };

        // Can't remove directories with children (except . and ..)
        if inode.itype == InodeType::Directory {
            let user_children: usize = inode.children.iter()
                .filter(|(name, _)| *name != "." && *name != "..")
                .count();
            if user_children > 0 {
                return false;
            }
        }

        // Remove from parent's children
        let (parent_path, name) = match path.rsplit_once('/') {
            Some((parent, name)) => (parent, name),
            None => return false,
        };

        let parent_ino = match self.resolve(if parent_path.is_empty() { "/" } else { parent_path }) {
            Some(ino) => ino,
            None => return false,
        };

        if let Some(parent) = self.inodes.get_mut(&parent_ino) {
            parent.children.remove(name);
        }

        self.inodes.remove(&ino);

        // Update cwd if it was removed
        if self.cwd == ino {
            self.cwd = 1;
        }

        true
    }

    /// Read data from an inode into a RAM buffer.
    /// Returns number of u32 words read.
    pub fn read_inode(&self, ino: u32, offset: u32, buf: &mut [u32]) -> u32 {
        let inode = match self.inodes.get(&ino) {
            Some(i) => i,
            None => return 0,
        };

        if inode.itype != InodeType::Regular && inode.itype != InodeType::Pipe {
            return 0;
        }

        let start = offset as usize;
        let mut read = 0u32;
        for (i, slot) in buf.iter_mut().enumerate() {
            let idx = start + i;
            if idx < inode.data.len() {
                *slot = inode.data[idx];
                read += 1;
            } else {
                break;
            }
        }
        read
    }

    /// Write data from a RAM buffer into an inode.
    /// Returns number of u32 words written.
    pub fn write_inode(&mut self, ino: u32, offset: u32, buf: &[u32]) -> u32 {
        let inode = match self.inodes.get_mut(&ino) {
            Some(i) => i,
            None => return 0,
        };

        if inode.itype != InodeType::Regular && inode.itype != InodeType::Pipe {
            return 0;
        }

        let start = offset as usize;
        let mut written = 0u32;
        for (i, &val) in buf.iter().enumerate() {
            let idx = start + i;
            if idx >= MAX_FILE_SIZE {
                break;
            }
            // Extend data if needed
            while inode.data.len() <= idx {
                inode.data.push(0);
            }
            inode.data[idx] = val;
            written += 1;
        }
        inode.size = inode.data.len() as u32;
        written
    }

    /// Get file status and write to RAM buffer.
    /// Layout: [ino, itype, size, ref_count, parent_ino, num_children]
    /// Returns true on success.
    pub fn fstat(&self, ino: u32, buf: &mut [u32]) -> bool {
        let inode = match self.inodes.get(&ino) {
            Some(i) => i,
            None => return false,
        };

        let user_children: u32 = inode.children.iter()
            .filter(|(name, _)| *name != "." && *name != "..")
            .count() as u32;

        let entries = [
            inode.ino,
            inode.itype.to_u32(),
            inode.size,
            inode.ref_count,
            inode.parent,
            user_children,
        ];

        for (i, &val) in entries.iter().enumerate() {
            if i < buf.len() {
                buf[i] = val;
            }
        }
        true
    }

    /// Get the inode number for the current working directory.
    pub fn get_cwd(&self) -> u32 {
        self.cwd
    }

    /// Change the current working directory.
    /// Returns true on success.
    pub fn chdir(&mut self, path: &str) -> bool {
        match self.resolve(path) {
            Some(ino) => {
                let inode = match self.inodes.get(&ino) {
                    Some(i) => i,
                    None => return false,
                };
                if inode.itype == InodeType::Directory {
                    self.cwd = ino;
                    true
                } else {
                    false
                }
            }
            None => false,
        }
    }

    /// List directory entries into a RAM buffer.
    /// Each entry is: [name_len, char1, char2, ..., 0, name_len2, char1, ...]
    /// Skips "." and "..". Returns number of entries.
    pub fn list_dir(&self, ino: u32, buf: &mut [u32]) -> u32 {
        let inode = match self.inodes.get(&ino) {
            Some(i) if i.itype == InodeType::Directory => i,
            _ => return 0,
        };

        let mut addr = 0usize;
        let mut count = 0u32;

        let mut entries: Vec<&String> = inode.children.keys()
            .filter(|name| *name != "." && *name != "..")
            .collect();
        entries.sort();

        for name in entries {
            let name_bytes: Vec<u32> = name.bytes().map(|b| b as u32).collect();

            // Write name length
            if addr < buf.len() {
                buf[addr] = name_bytes.len() as u32;
                addr += 1;
            }

            // Write name characters + null terminator
            for &ch in &name_bytes {
                if addr < buf.len() {
                    buf[addr] = ch;
                    addr += 1;
                }
            }
            if addr < buf.len() {
                buf[addr] = 0; // null terminator
                addr += 1;
            }
            count += 1;
        }

        count
    }

    /// Truncate an inode to the given size.
    /// Returns true on success.
    pub fn truncate(&mut self, ino: u32, size: u32) -> bool {
        let inode = match self.inodes.get_mut(&ino) {
            Some(i) => i,
            None => return false,
        };

        if inode.itype != InodeType::Regular && inode.itype != InodeType::Pipe {
            return false;
        }

        inode.data.truncate(size as usize);
        inode.size = inode.data.len() as u32;
        true
    }

    /// Get total number of inodes in the filesystem.
    #[cfg(test)]
    pub fn inode_count(&self) -> usize {
        self.inodes.len()
    }

    /// Get a reference to an inode by number.
    #[cfg(test)]
    pub fn get_inode(&self, ino: u32) -> Option<&Inode> {
        self.inodes.get(&ino)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_exists() {
        let fs = InodeFs::new();
        assert_eq!(fs.resolve("/"), Some(1));
        assert_eq!(fs.inode_count(), 1);
    }

    #[test]
    fn test_create_directory() {
        let mut fs = InodeFs::new();
        let ino = fs.mkdir("/tmp");
        assert_ne!(ino, 0);
        assert_eq!(fs.resolve("/tmp"), Some(ino));
        assert_eq!(fs.inode_count(), 2);
    }

    #[test]
    fn test_nested_directories() {
        let mut fs = InodeFs::new();
        assert_ne!(fs.mkdir("/a"), 0);
        assert_ne!(fs.mkdir("/a/b"), 0);
        assert_ne!(fs.mkdir("/a/b/c"), 0);
        assert_eq!(fs.resolve("/a/b/c"), Some(4));
        assert_eq!(fs.inode_count(), 4);
    }

    #[test]
    fn test_mkdir_already_exists() {
        let mut fs = InodeFs::new();
        assert_ne!(fs.mkdir("/foo"), 0);
        assert_eq!(fs.mkdir("/foo"), 0); // already exists
    }

    #[test]
    fn test_mkdir_parent_not_exists() {
        let mut fs = InodeFs::new();
        assert_eq!(fs.mkdir("/no/such/parent"), 0);
    }

    #[test]
    fn test_mkdir_empty_path() {
        let mut fs = InodeFs::new();
        assert_eq!(fs.mkdir(""), 0);
        assert_eq!(fs.mkdir("/"), 0);
    }

    #[test]
    fn test_create_file() {
        let mut fs = InodeFs::new();
        let ino = fs.create("/hello.txt");
        assert_ne!(ino, 0);
        assert_eq!(fs.resolve("/hello.txt"), Some(ino));

        let inode = fs.get_inode(ino).unwrap();
        assert_eq!(inode.itype, InodeType::Regular);
    }

    #[test]
    fn test_write_and_read_file() {
        let mut fs = InodeFs::new();
        let ino = fs.create("/data.bin");
        assert_ne!(ino, 0);

        let write_data = vec![42u32, 100, 255, 0, 1];
        let written = fs.write_inode(ino, 0, &write_data);
        assert_eq!(written, 5);

        let mut read_buf = vec![0u32; 10];
        let read = fs.read_inode(ino, 0, &mut read_buf);
        assert_eq!(read, 5);
        assert_eq!(read_buf[..5], write_data[..]);

        // Read with offset
        let mut read_buf2 = vec![0u32; 3];
        let read2 = fs.read_inode(ino, 2, &mut read_buf2);
        assert_eq!(read2, 3);
        assert_eq!(read_buf2, vec![255, 0, 1]);
    }

    #[test]
    fn test_write_with_offset() {
        let mut fs = InodeFs::new();
        let ino = fs.create("/sparse.bin");
        assert_ne!(ino, 0);

        let write_data = vec![99u32, 88];
        let written = fs.write_inode(ino, 5, &write_data);
        assert_eq!(written, 2);

        let mut read_buf = vec![0u32; 8];
        fs.read_inode(ino, 0, &mut read_buf);
        assert_eq!(read_buf[0], 0); // gap
        assert_eq!(read_buf[1], 0);
        assert_eq!(read_buf[2], 0);
        assert_eq!(read_buf[3], 0);
        assert_eq!(read_buf[4], 0);
        assert_eq!(read_buf[5], 99);
        assert_eq!(read_buf[6], 88);
    }

    #[test]
    fn test_unlink_file() {
        let mut fs = InodeFs::new();
        let ino = fs.create("/remove_me.txt");
        assert_ne!(ino, 0);
        assert_eq!(fs.resolve("/remove_me.txt"), Some(ino));

        assert!(fs.unlink("/remove_me.txt"));
        assert_eq!(fs.resolve("/remove_me.txt"), None);
        assert_eq!(fs.inode_count(), 1); // only root left
    }

    #[test]
    fn test_unlink_non_empty_directory_fails() {
        let mut fs = InodeFs::new();
        fs.mkdir("/dir");
        fs.create("/dir/file.txt");
        assert!(!fs.unlink("/dir")); // not empty
    }

    #[test]
    fn test_unlink_empty_directory() {
        let mut fs = InodeFs::new();
        fs.mkdir("/empty_dir");
        assert!(fs.unlink("/empty_dir"));
        assert_eq!(fs.resolve("/empty_dir"), None);
    }

    #[test]
    fn test_unlink_root_fails() {
        let mut fs = InodeFs::new();
        assert!(!fs.unlink("/"));
    }

    #[test]
    fn test_unlink_nonexistent() {
        let mut fs = InodeFs::new();
        assert!(!fs.unlink("/nope"));
    }

    #[test]
    fn test_fstat() {
        let mut fs = InodeFs::new();
        let ino = fs.create("/test.txt");

        // Write some data
        fs.write_inode(ino, 0, &[1, 2, 3]);

        let mut buf = vec![0u32; 10];
        assert!(fs.fstat(ino, &mut buf));
        assert_eq!(buf[0], ino);         // ino
        assert_eq!(buf[1], 1);           // itype = Regular
        assert_eq!(buf[2], 3);           // size
        assert_eq!(buf[3], 0);           // ref_count
        assert_eq!(buf[4], 1);           // parent = root
        assert_eq!(buf[5], 0);           // num_children (not a dir)
    }

    #[test]
    fn test_fstat_directory() {
        let mut fs = InodeFs::new();
        fs.mkdir("/mydir");
        fs.create("/mydir/file1.txt");
        fs.create("/mydir/file2.txt");

        let dir_ino = fs.resolve("/mydir").unwrap();
        let mut buf = vec![0u32; 10];
        assert!(fs.fstat(dir_ino, &mut buf));
        assert_eq!(buf[1], 2);  // itype = Directory
        assert_eq!(buf[5], 2);  // num_children = 2 (file1, file2)
    }

    #[test]
    fn test_fstat_nonexistent() {
        let fs = InodeFs::new();
        let mut buf = vec![0u32; 10];
        assert!(!fs.fstat(999, &mut buf));
    }

    #[test]
    fn test_chdir_and_relative_paths() {
        let mut fs = InodeFs::new();
        fs.mkdir("/home");
        fs.mkdir("/home/user");
        fs.create("/home/user/file.txt");

        assert!(fs.chdir("/home/user"));
        assert_eq!(fs.get_cwd(), fs.resolve("/home/user").unwrap());

        // Relative path from cwd
        assert_eq!(fs.resolve("file.txt"), fs.resolve("/home/user/file.txt"));
        assert_eq!(fs.resolve(".."), fs.resolve("/home"));
        assert_eq!(fs.resolve("../.."), Some(1)); // root
    }

    #[test]
    fn test_chdir_fails_on_file() {
        let mut fs = InodeFs::new();
        fs.create("/not_a_dir.txt");
        assert!(!fs.chdir("/not_a_dir.txt"));
    }

    #[test]
    fn test_list_dir() {
        let mut fs = InodeFs::new();
        fs.mkdir("/dir");
        fs.create("/dir/alpha.txt");
        fs.create("/dir/beta.txt");
        fs.mkdir("/dir/sub");

        let dir_ino = fs.resolve("/dir").unwrap();
        let mut buf = vec![0u32; 256];
        let count = fs.list_dir(dir_ino, &mut buf);
        assert_eq!(count, 3); // alpha.txt, beta.txt, sub
    }

    #[test]
    fn test_list_dir_root() {
        let mut fs = InodeFs::new();
        fs.create("/a.txt");
        fs.create("/b.txt");

        let mut buf = vec![0u32; 256];
        let count = fs.list_dir(1, &mut buf);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_truncate() {
        let mut fs = InodeFs::new();
        let ino = fs.create("/big.txt");
        fs.write_inode(ino, 0, &[1, 2, 3, 4, 5]);
        assert!(fs.truncate(ino, 3));

        let inode = fs.get_inode(ino).unwrap();
        assert_eq!(inode.data.len(), 3);
        assert_eq!(inode.size, 3);
    }

    #[test]
    fn test_truncate_extend() {
        let mut fs = InodeFs::new();
        let ino = fs.create("/small.txt");
        fs.write_inode(ino, 0, &[1, 2]);
        assert!(fs.truncate(ino, 5));

        let inode = fs.get_inode(ino).unwrap();
        assert_eq!(inode.data.len(), 2); // truncate only shrinks, doesn't extend with zeros
    }

    #[test]
    fn test_max_file_size() {
        let mut fs = InodeFs::new();
        let ino = fs.create("/huge.txt");
        let big_data = vec![42u32; MAX_FILE_SIZE + 100];
        let written = fs.write_inode(ino, 0, &big_data);
        assert_eq!(written, MAX_FILE_SIZE as u32);
    }

    #[test]
    fn test_create_device_inode() {
        let mut fs = InodeFs::new();
        fs.mkdir("/dev");
        let ino = fs.create_file("/dev/null", InodeType::Device);
        assert_ne!(ino, 0);
        let inode = fs.get_inode(ino).unwrap();
        assert_eq!(inode.itype, InodeType::Device);
    }

    #[test]
    fn test_create_pipe_inode() {
        let mut fs = InodeFs::new();
        let ino = fs.create_file("/pipe1", InodeType::Pipe);
        assert_ne!(ino, 0);
        let inode = fs.get_inode(ino).unwrap();
        assert_eq!(inode.itype, InodeType::Pipe);
    }

    #[test]
    fn test_read_write_nonexistent_inode() {
        let fs = InodeFs::new();
        let mut buf = vec![0u32; 10];
        assert_eq!(fs.read_inode(999, 0, &mut buf), 0);
    }

    #[test]
    fn test_write_directory_fails() {
        let mut fs = InodeFs::new();
        fs.mkdir("/mydir");
        let dir_ino = fs.resolve("/mydir").unwrap();
        let data = vec![1u32, 2, 3];
        assert_eq!(fs.write_inode(dir_ino, 0, &data), 0);
    }

    #[test]
    fn test_inode_type_from_u32() {
        assert_eq!(InodeType::from_u32(1), Some(InodeType::Regular));
        assert_eq!(InodeType::from_u32(2), Some(InodeType::Directory));
        assert_eq!(InodeType::from_u32(3), Some(InodeType::Device));
        assert_eq!(InodeType::from_u32(4), Some(InodeType::Pipe));
        assert_eq!(InodeType::from_u32(99), None);
    }

    #[test]
    fn test_long_filename_rejected() {
        let mut fs = InodeFs::new();
        let long_name = "a".repeat(MAX_NAME_LEN + 1);
        assert_eq!(fs.create(&format!("/{}", long_name)), 0);
    }

    #[test]
    fn test_resolve_dot_and_dotdot() {
        let mut fs = InodeFs::new();
        fs.mkdir("/home");
        fs.mkdir("/home/user");

        // "." should resolve to the directory itself
        let user_ino = fs.resolve("/home/user").unwrap();
        assert_eq!(fs.resolve("/home/user/."), Some(user_ino));

        // ".." should resolve to parent
        assert_eq!(fs.resolve("/home/user/.."), fs.resolve("/home"));
    }
}
