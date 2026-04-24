// riscv/vfs_surface.rs -- Pixel VFS Surface MMIO Device
//
// "Pixels move pixels" file access. Files are encoded as RGBA pixels in a
// 256x256 surface. Row 0 contains a directory index.
// Subsequent rows contain file headers and data.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

/// MMIO base address for the VFS Surface
pub const VFS_SURFACE_BASE: u64 = 0x7000_0000;
/// Total size of the VFS Surface (256x256 pixels * 4 bytes/pixel = 256KB)
pub const VFS_SURFACE_SIZE: usize = 256 * 256 * 4;
/// Control register for the VFS Surface (e.g., bit 0 = sync/flush)
pub const VFS_CONTROL_ADDR: u64 = VFS_SURFACE_BASE + VFS_SURFACE_SIZE as u64;

/// PXFS Magic number: "PXFS"
const PXFS_MAGIC: u32 = 0x50584653;

/// The VFS Pixel Surface device.
pub struct VfsSurface {
    /// 256x256 RGBA pixel buffer (256KB)
    pub pixels: Vec<u32>,
    /// Base directory on host for VFS files
    pub base_dir: PathBuf,
    /// Mapping of start_row -> filename for write-back
    pub file_map: HashMap<u16, String>,
    /// Set of rows that have been modified and need flushing
    pub dirty_rows: HashSet<u16>,
}

impl Default for VfsSurface {
    fn default() -> Self {
        Self::new()
    }
}

impl VfsSurface {
    /// Create a new empty VFS surface.
    pub fn new() -> Self {
        let mut pixels = vec![0u32; 256 * 256];
        let base_dir = PathBuf::from(".geometry_os/fs");
        let _ = fs::create_dir_all(&base_dir);

        // Initialize with empty directory index
        pixels[0] = PXFS_MAGIC;
        pixels[1] = 0; // file_count

        VfsSurface {
            pixels,
            base_dir,
            file_map: HashMap::new(),
            dirty_rows: HashSet::new(),
        }
    }

    /// Load files from the host filesystem into the pixel surface.
    pub fn load_files(&mut self) {
        // Clear surface first (except Row 0 header)
        for p in &mut self.pixels[2..] {
            *p = 0;
        }
        self.file_map.clear();
        self.dirty_rows.clear();

        let entries = match fs::read_dir(&self.base_dir) {
            Ok(rd) => rd,
            Err(_) => return,
        };

        let mut files = Vec::new();
        for e in entries.flatten() {
            if let Ok(metadata) = e.metadata() {
                if metadata.is_file() {
                    if let Some(name) = e.file_name().to_str() {
                        if !name.starts_with('.') && name.len() <= 64 {
                            if let Ok(data) = fs::read(e.path()) {
                                files.push((name.to_string(), data));
                            }
                        }
                    }
                }
            }
        }

        // Sort by name for deterministic layout
        files.sort_by(|a, b| a.0.cmp(&b.0));

        let mut current_row = 1;
        let mut file_count = 0;

        for (name, data) in files {
            if current_row >= 255 || file_count >= 254 {
                break;
            }

            let name_hash = self.fnv1a_hash(&name);
            let byte_count = data.len().min(0xFFFF); // Max 64KB per file
            let pixel_count = (byte_count + 3) / 4;
            let rows_needed = (1 + pixel_count + 255) / 256;

            if current_row + rows_needed >= 256 {
                break;
            }

            // Store mapping for write-back
            self.file_map.insert(current_row as u16, name.clone());

            // Write into directory index (Row 0)
            // Pixel 2+i: [start_row(16) | name_hash(16)]
            let index_pixel = ((current_row as u32) << 16) | (name_hash as u32 & 0xFFFF);
            self.pixels[2 + file_count] = index_pixel;

            // Write File Header Pixel: [byte_count(16) | name_hash_8(8) | flags(8)]
            // flags: bit 0 = valid
            let flags = 1u32;
            let header_pixel = ((byte_count as u32) << 16) | ((name_hash as u32 & 0xFF) << 8) | flags;
            self.pixels[current_row * 256] = header_pixel;

            // Write Data Pixels
            for i in 0..pixel_count {
                let mut pixel = 0u32;
                for j in 0..4 {
                    let byte_idx = i * 4 + j;
                    if byte_idx < byte_count {
                        pixel |= (data[byte_idx] as u32) << (j * 8);
                    }
                }
                self.pixels[current_row * 256 + 1 + i] = pixel;
            }

            current_row += rows_needed;
            file_count += 1;
        }

        // Update file count in Row 0
        self.pixels[1] = file_count as u32;
    }

    /// Read a 32-bit word from the surface (MMIO).
    pub fn read(&self, addr: u64) -> Option<u32> {
        let offset = addr.checked_sub(VFS_SURFACE_BASE)? as usize;
        if offset >= VFS_SURFACE_SIZE {
            return None;
        }
        let pixel_idx = offset / 4;
        Some(self.pixels[pixel_idx])
    }

    /// Write a 32-bit word to the surface (MMIO).
    pub fn write(&mut self, addr: u64, val: u32) {
        if addr == VFS_CONTROL_ADDR {
            if val & 1 != 0 {
                self.flush();
            }
            return;
        }

        let offset = match addr.checked_sub(VFS_SURFACE_BASE) {
            Some(o) => o as usize,
            None => return,
        };
        if offset >= VFS_SURFACE_SIZE {
            return;
        }
        let pixel_idx = offset / 4;
        let old_val = self.pixels[pixel_idx];
        if old_val == val {
            return;
        }
        self.pixels[pixel_idx] = val;

        // Mark row as dirty for write-back
        let row = (pixel_idx / 256) as u16;
        self.dirty_rows.insert(row);
    }

    /// Flush dirty rows back to the host filesystem.
    pub fn flush(&mut self) {
        if self.dirty_rows.is_empty() {
            return;
        }

        // We iterate through all mapped files and check if their rows are dirty.
        // A file starts at `start_row` and occupies `rows_needed`.
        for (&start_row, filename) in &self.file_map {
            // Read header to get current byte_count (guest might have changed it)
            let header = self.pixels[start_row as usize * 256];
            let byte_count = (header >> 16) as usize;
            let pixel_count = (byte_count + 3) / 4;
            let rows_needed = (1 + pixel_count + 255) / 256;

            let mut is_file_dirty = false;
            for r in start_row..start_row + rows_needed as u16 {
                if self.dirty_rows.contains(&r) {
                    is_file_dirty = true;
                    break;
                }
            }

            if is_file_dirty {
                // Reconstruct data
                let mut data = Vec::with_capacity(byte_count);
                for i in 0..pixel_count {
                    let pixel = self.pixels[start_row as usize * 256 + 1 + i];
                    for j in 0..4 {
                        if data.len() < byte_count {
                            data.push(((pixel >> (j * 8)) & 0xFF) as u8);
                        }
                    }
                }

                // Write back to host
                let path = self.base_dir.join(filename);
                if let Err(e) = fs::write(path, data) {
                    eprintln!("VFS Surface: Failed to flush file {}: {}", filename, e);
                }
            }
        }

        self.dirty_rows.clear();
    }

    /// FNV-1a hash (32-bit) for filename lookups.
    fn fnv1a_hash(&self, s: &str) -> u32 {
        let mut hash = 0x811c9dc5u32;
        for b in s.as_bytes() {
            hash ^= *b as u32;
            hash = hash.wrapping_mul(0x01000193);
        }
        hash
    }

    /// Check if an address is within this device's MMIO range.
    pub fn contains(addr: u64) -> bool {
        (VFS_SURFACE_BASE..VFS_SURFACE_BASE + VFS_SURFACE_SIZE as u64 + 4).contains(&addr)
    }
}

impl Drop for VfsSurface {
    fn drop(&mut self) {
        self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_vfs_surface_initialization() {
        let surface = VfsSurface::new();
        assert_eq!(surface.pixels[0], PXFS_MAGIC);
        assert_eq!(surface.pixels[1], 0);
    }

    #[test]
    fn test_vfs_surface_load_files() {
        let mut surface = VfsSurface::new();
        let test_dir = std::env::temp_dir().join("geo_vfs_surface_test");
        let _ = fs::remove_dir_all(&test_dir);
        let _ = fs::create_dir_all(&test_dir);
        surface.base_dir = test_dir.clone();

        // Create a test file
        let file_content = vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        fs::write(test_dir.join("test.bin"), &file_content).unwrap();

        surface.load_files();

        // Check file count
        assert_eq!(surface.pixels[1], 1);

        // Check directory index: Pixel 2: [start_row(16) | name_hash(16)]
        let index = surface.pixels[2];
        let start_row = index >> 16;
        assert_eq!(start_row, 1);

        // Check file header: Pixel (1,0): [byte_count(16) | name_hash_8(8) | flags(8)]
        let header = surface.pixels[256];
        assert_eq!(header >> 16, 8); // byte_count
        assert_eq!(header & 0xFF, 1); // valid flag

        // Check data pixels: Pixel (1,1) and (1,2)
        assert_eq!(surface.pixels[256 + 1], 0x44332211);
        assert_eq!(surface.pixels[256 + 2], 0x88776655);

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_vfs_surface_write_back_flush() {
        let mut surface = VfsSurface::new();
        let test_dir = std::env::temp_dir().join("geo_vfs_surface_flush_test");
        let _ = fs::remove_dir_all(&test_dir);
        let _ = fs::create_dir_all(&test_dir);
        surface.base_dir = test_dir.clone();

        // Create a test file
        let file_content = vec![0x11, 0x22, 0x33, 0x44];
        fs::write(test_dir.join("test.bin"), &file_content).unwrap();

        surface.load_files();
        assert_eq!(surface.pixels[256 + 1], 0x44332211);

        // Modify pixel in surface
        let addr = VFS_SURFACE_BASE + (256 + 1) * 4;
        surface.write(addr, 0xAABBCCDD);
        assert!(surface.dirty_rows.contains(&1));

        // Flush back to host
        surface.flush();
        assert!(surface.dirty_rows.is_empty());

        // Verify file on host has changed
        let new_content = fs::read(test_dir.join("test.bin")).unwrap();
        assert_eq!(new_content, vec![0xDD, 0xCC, 0xBB, 0xAA]);

        let _ = fs::remove_dir_all(&test_dir);
    }
}
