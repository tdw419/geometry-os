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
/// Maximum number of files in the directory index
const VFS_MAX_FILES: usize = 254;
/// CANV marker: "CANV" in ASCII — set by geos_save_canvas() at pixel (0, 255)
const CANV_MARKER: u32 = 0x43414E56;
/// Column in row 0 where the CANV marker lives
const CANV_MARKER_COL: usize = 255;
/// Number of canvas data rows (rows 1-255 of the VFS surface)
const CANV_DATA_ROWS: usize = 255;
/// Raw canvas file name for cross-session persistence
const CANVAS_RAW_FILE: &str = "canvas.raw";

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
    /// Create a new VFS surface. Does NOT restore canvas — call restore_canvas()
    /// after load_files() to recover saved canvas from a previous session.
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

    /// Restore canvas data from canvas.raw if it exists.
    /// Call this AFTER load_files() to avoid the file loading clearing the canvas.
    pub fn restore_canvas(&mut self) {
        let canvas_path = self.base_dir.join(CANVAS_RAW_FILE);
        if !canvas_path.exists() {
            return;
        }
        let data = match fs::read(&canvas_path) {
            Ok(d) => d,
            Err(_) => return,
        };
        let expected_size = CANV_DATA_ROWS * 256 * 4; // 261,120 bytes
        if data.len() != expected_size {
            return;
        }
        // Decode u32 pixels from little-endian bytes and write to rows 1-255
        for i in 0..CANV_DATA_ROWS * 256 {
            let off = i * 4;
            let px = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
            // File row 0 maps to VFS row 1
            self.pixels[256 + i] = px;
        }
        // Set CANV marker so geos_load_canvas() can find it
        self.pixels[CANV_MARKER_COL] = CANV_MARKER;
    }

    /// Create a new VFS surface with a custom base directory.
    /// Useful for testing with isolated directories.
    /// Restores canvas from the custom base_dir.
    pub fn new_with_base(base_dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&base_dir);
        let mut surface = Self::new();
        surface.base_dir = base_dir;
        surface.restore_canvas();
        surface
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
            let header_pixel =
                ((byte_count as u32) << 16) | ((name_hash as u32 & 0xFF) << 8) | flags;
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
    ///
    /// Handles both host-loaded and guest-created files.
    pub fn flush(&mut self) {
        if self.dirty_rows.is_empty() {
            return;
        }
        let fc = self.pixels[1] as usize;
        for fi in 0..fc.min(VFS_MAX_FILES) {
            let idx = self.pixels[2 + fi];
            let sr = (idx >> 16) as u16;
            if sr == 0 || sr >= 256 {
                continue;
            }
            let h = self.pixels[sr as usize * 256];
            let flags = h & 0xFF;
            if flags & 0x01 == 0 {
                continue;
            }
            let bc = (h >> 16) as usize;
            let guest = (flags & 0x02) != 0;
            let dc: usize = if guest { 64 } else { 1 };
            let fname = if guest {
                let mut nb = Vec::new();
                for c in 1..64 {
                    let ch = (self.pixels[sr as usize * 256 + c] & 0xFF) as u8;
                    if ch == 0 {
                        break;
                    }
                    nb.push(ch);
                }
                String::from_utf8_lossy(&nb).to_string()
            } else if let Some(nm) = self.file_map.get(&sr) {
                nm.clone()
            } else {
                format!("unknown_{:04x}.bin", idx & 0xFFFF)
            };
            if fname.is_empty() {
                continue;
            }
            let pc = (bc + 3) / 4;
            let rn = ((dc + pc + 255) / 256) as u16;
            let mut dirty = false;
            for r in sr..sr + rn {
                if self.dirty_rows.contains(&r) {
                    dirty = true;
                    break;
                }
            }
            if !dirty {
                continue;
            }
            let mut data = Vec::with_capacity(bc);
            for pi in 0..pc {
                let col = dc + pi;
                let row = sr as usize + col / 256;
                let cr = col % 256;
                if row < 256 {
                    let px = self.pixels[row * 256 + cr];
                    for j in 0..4 {
                        if data.len() < bc {
                            data.push(((px >> (j * 8)) & 0xFF) as u8);
                        }
                    }
                }
            }
            let path = self.base_dir.join(&fname);
            if fs::write(&path, &data).is_ok() {
                self.file_map.insert(sr, fname);
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
    /// Create a new file entry in the pixel surface.
    pub fn create_file_entry(&mut self, name: &str) -> Option<u16> {
        let count = self.pixels[1] as usize;
        if count >= VFS_MAX_FILES || name.len() > 63 || name.is_empty() {
            return None;
        }
        let mut free_row = 1u16;
        for fi in 0..count {
            let idx = self.pixels[2 + fi];
            let sr = (idx >> 16) as u16;
            if sr == 0 {
                continue;
            }
            let h = self.pixels[sr as usize * 256];
            if h & 0xFF & 0x01 == 0 {
                continue;
            }
            let bc = (h >> 16) as usize;
            let pc = (bc + 3) / 4;
            let dc: usize = if (h & 0xFF & 0x02) != 0 { 64 } else { 1 };
            let end = sr + ((dc + pc + 255) / 256) as u16;
            if end > free_row {
                free_row = end;
            }
        }
        if free_row == 0 || free_row >= 255 {
            return None;
        }
        let nh = self.fnv1a_hash(name);
        let sr = free_row;
        self.pixels[2 + count] = ((sr as u32) << 16) | (nh & 0xFFFF);
        self.pixels[sr as usize * 256] = ((0u32) << 16) | ((nh & 0xFF) << 8) | 0x01;
        self.pixels[1] = (count + 1) as u32;
        self.file_map.insert(sr, name.to_string());
        Some(sr)
    }

    /// Update a file header with new byte_count and flags.
    pub fn update_file_header(&mut self, sr: u16, bc: u32, flags: u32) {
        if sr == 0 || sr >= 256 {
            return;
        }
        let nh = (self.pixels[sr as usize * 256] >> 8) & 0xFF;
        self.pixels[sr as usize * 256] = ((bc & 0xFFFF) << 16) | (nh << 8) | (flags & 0xFF);
        self.dirty_rows.insert(sr);
    }

    /// Delete a file entry by filename.
    pub fn delete_file_entry(&mut self, name: &str) -> bool {
        let count = self.pixels[1] as usize;
        let nh = self.fnv1a_hash(name);
        for i in 0..count {
            let idx = self.pixels[2 + i];
            let sr = (idx >> 16) as u16;
            if sr == 0 {
                continue;
            }
            if (idx & 0xFFFF) != (nh & 0xFFFF) {
                continue;
            }
            match self.file_map.get(&sr) {
                Some(n) if n == name => {}
                _ => continue,
            }
            let h = self.pixels[sr as usize * 256];
            self.pixels[sr as usize * 256] = h & !0x01u32;
            self.file_map.remove(&sr);
            for j in i..count.saturating_sub(1) {
                self.pixels[2 + j] = self.pixels[2 + j + 1];
            }
            self.pixels[2 + count - 1] = 0;
            self.pixels[1] = (count - 1) as u32;
            let _ = fs::remove_file(self.base_dir.join(name));
            return true;
        }
        false
    }
}

impl Drop for VfsSurface {
    fn drop(&mut self) {
        // First, flush any VFS file entries
        self.flush();

        // Then, check for CANV marker and persist raw canvas data to disk
        // This enables cross-session persistence: save today, load tomorrow
        if self.pixels[CANV_MARKER_COL] == CANV_MARKER {
            let canvas_path = self.base_dir.join(CANVAS_RAW_FILE);
            let expected_size = CANV_DATA_ROWS * 256 * 4; // 261,120 bytes
            let mut buf = Vec::with_capacity(expected_size);
            // Encode rows 1-255 (CANV_DATA_ROWS rows × 256 cols) as little-endian u32
            for row in 1..=CANV_DATA_ROWS {
                for col in 0..256 {
                    let px = self.pixels[row * 256 + col];
                    buf.extend_from_slice(&px.to_le_bytes());
                }
            }
            let _ = fs::write(&canvas_path, &buf);
        }
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
        let addr = VFS_SURFACE_BASE + ((1u64 * 256 + 1) * 4);
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

    #[test]
    fn test_vfs_surface_create_file_entry() {
        let mut s = VfsSurface::new();
        let td = std::env::temp_dir().join("geo_vfs_crt");
        let _ = fs::remove_dir_all(&td);
        let _ = fs::create_dir_all(&td);
        s.base_dir = td.clone();
        assert_eq!(s.pixels[1], 0);
        let row = s.create_file_entry("guest_log.txt").unwrap();
        assert_eq!(row, 1);
        assert_eq!(s.pixels[1], 1);
        assert_eq!(s.pixels[2] >> 16, 1);
        assert_eq!(s.file_map[&1], "guest_log.txt");
        assert!(s.create_file_entry("data.bin").is_some());
        assert_eq!(s.pixels[1], 2);
        let _ = fs::remove_dir_all(&td);
    }

    #[test]
    fn test_vfs_surface_delete_file_entry() {
        let mut s = VfsSurface::new();
        let td = std::env::temp_dir().join("geo_vfs_del");
        let _ = fs::remove_dir_all(&td);
        let _ = fs::create_dir_all(&td);
        s.base_dir = td.clone();
        fs::write(td.join("to_del.txt"), b"bye").unwrap();
        s.load_files();
        assert_eq!(s.pixels[1], 1);
        assert!(s.delete_file_entry("to_del.txt"));
        assert_eq!(s.pixels[1], 0);
        assert!(!td.join("to_del.txt").exists());
        let _ = fs::remove_dir_all(&td);
    }

    #[test]
    fn test_vfs_surface_guest_create_and_flush() {
        let mut s = VfsSurface::new();
        let td = std::env::temp_dir().join("geo_vfs_gcf");
        let _ = fs::remove_dir_all(&td);
        let _ = fs::create_dir_all(&td);
        s.base_dir = td.clone();
        let name = "guest_out.txt";
        for (i, &b) in name.as_bytes().iter().enumerate() {
            s.pixels[256 + 1 + i] = b as u32;
        }
        s.pixels[256 + 1 + name.len()] = 0;
        let nh = s.fnv1a_hash(name);
        s.pixels[256] = (12u32 << 16) | ((nh & 0xFF) << 8) | 0x03;
        let content = b"Hello Guest!";
        for i in 0..3 {
            let mut px = 0u32;
            for j in 0..4 {
                let idx = i * 4 + j;
                if idx < content.len() {
                    px |= (content[idx] as u32) << (j * 8);
                }
            }
            s.pixels[256 + 64 + i] = px;
        }
        s.pixels[2] = (1u32 << 16) | (nh & 0xFFFF);
        s.pixels[1] = 1;
        s.dirty_rows.insert(1);
        s.flush();
        let data = fs::read(td.join("guest_out.txt")).unwrap();
        assert_eq!(data, b"Hello Guest!");
        assert_eq!(s.file_map[&1], "guest_out.txt");
        let _ = fs::remove_dir_all(&td);
    }

    #[test]
    fn test_vfs_surface_create_write_flush() {
        let mut s = VfsSurface::new();
        let td = std::env::temp_dir().join("geo_vfs_cwf");
        let _ = fs::remove_dir_all(&td);
        let _ = fs::create_dir_all(&td);
        s.base_dir = td.clone();
        let row = s.create_file_entry("hello.txt").unwrap();
        s.update_file_header(row, 6, 1);
        let ba = VFS_SURFACE_BASE + ((row as u64 * 256 + 1) * 4);
        s.write(ba, 0x6C6C6548);
        s.write(ba + 4, 0x0000216F);
        s.flush();
        let data = fs::read(td.join("hello.txt")).unwrap();
        assert_eq!(&data[..6], b"Hello!");
        let _ = fs::remove_dir_all(&td);
    }

    #[test]
    fn test_canvas_persist_on_drop() {
        let td = std::env::temp_dir().join("geo_vfs_canv_drop");
        let _ = fs::remove_dir_all(&td);
        let _ = fs::create_dir_all(&td);

        // Create a surface with known pixel data and CANV marker
        let mut s = VfsSurface::new();
        s.base_dir = td.clone();

        // Write known test pattern to VFS rows 1-3 (canvas data)
        s.pixels[256] = 0xFF0000; // row 1 col 0 = red
        s.pixels[257] = 0x00FF00; // row 1 col 1 = green
        s.pixels[512] = 0x0000FF; // row 2 col 0 = blue
        s.pixels[768] = 0xFFFFFF; // row 3 col 0 = white

        // Set CANV marker
        s.pixels[CANV_MARKER_COL] = CANV_MARKER;

        // Drop the surface — should persist canvas.raw
        drop(s);

        // Verify canvas.raw exists and has correct size
        let raw_path = td.join(CANVAS_RAW_FILE);
        assert!(raw_path.exists(), "canvas.raw should be created on drop");
        let data = fs::read(&raw_path).unwrap();
        let expected_size = CANV_DATA_ROWS * 256 * 4; // 261,120 bytes
        assert_eq!(
            data.len(),
            expected_size,
            "canvas.raw should be 261,120 bytes"
        );

        // Verify first few pixels
        assert_eq!(
            u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            0xFF0000,
            "first pixel should be red"
        );
        assert_eq!(
            u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
            0x00FF00,
            "second pixel should be green"
        );
        assert_eq!(
            u32::from_le_bytes([data[1024], data[1025], data[1026], data[1027]]),
            0x0000FF,
            "row 2 col 0 should be blue"
        );

        let _ = fs::remove_dir_all(&td);
    }

    #[test]
    fn test_canvas_restore_on_new() {
        let td = std::env::temp_dir().join("geo_vfs_canv_restore");
        let _ = fs::remove_dir_all(&td);
        let _ = fs::create_dir_all(&td);

        // Write a canvas.raw file manually (simulating previous session's drop)
        let mut raw_data = vec![0u8; CANV_DATA_ROWS * 256 * 4];
        // Put known pixels at the start
        let red_bytes = 0xFF0000u32.to_le_bytes();
        raw_data[0..4].copy_from_slice(&red_bytes);
        let green_bytes = 0x00FF00u32.to_le_bytes();
        raw_data[4..8].copy_from_slice(&green_bytes);
        let blue_bytes = 0x0000FFu32.to_le_bytes();
        raw_data[1024..1028].copy_from_slice(&blue_bytes); // row 2 col 0
        fs::write(td.join(CANVAS_RAW_FILE), &raw_data).unwrap();

        // Create new surface — should restore from canvas.raw
        let mut s = VfsSurface::new_with_base(td.clone());

        // CANV marker should be set
        assert_eq!(
            s.pixels[CANV_MARKER_COL], CANV_MARKER,
            "CANV marker should be restored"
        );

        // Canvas data should be restored to rows 1-255
        assert_eq!(s.pixels[256], 0xFF0000, "row 1 col 0 = red");
        assert_eq!(s.pixels[257], 0x00FF00, "row 1 col 1 = green");
        assert_eq!(s.pixels[512], 0x0000FF, "row 2 col 0 = blue");

        let _ = fs::remove_dir_all(&td);
    }

    #[test]
    fn test_canvas_round_trip_persistence() {
        let td = std::env::temp_dir().join("geo_vfs_canv_rt");
        let _ = fs::remove_dir_all(&td);
        let _ = fs::create_dir_all(&td);

        // Session 1: Create surface, write canvas data, set CANV marker, drop
        {
            let mut s1 = VfsSurface::new();
            s1.base_dir = td.clone();

            // Write a recognizable pattern across multiple rows
            for row in 1..=5u32 {
                for col in 0..10u32 {
                    s1.pixels[(row * 256 + col) as usize] = (row << 16) | (col << 8) | (row * col);
                }
            }
            s1.pixels[CANV_MARKER_COL] = CANV_MARKER;
            drop(s1); // persists canvas.raw
        }

        // Session 2: Create new surface, verify canvas data restored
        {
            let s2 = VfsSurface::new_with_base(td.clone());

            // CANV marker present
            assert_eq!(s2.pixels[CANV_MARKER_COL], CANV_MARKER);

            // All pattern pixels match
            for row in 1..=5u32 {
                for col in 0..10u32 {
                    let expected = (row << 16) | (col << 8) | (row * col);
                    let actual = s2.pixels[(row * 256 + col) as usize];
                    assert_eq!(
                        actual, expected,
                        "pixel at row {} col {} should match after round-trip",
                        row, col
                    );
                }
            }

            // Zero pixels outside the pattern should still be zero
            assert_eq!(s2.pixels[256 + 10], 0, "unset pixels should be 0");
        }

        let _ = fs::remove_dir_all(&td);
    }

    #[test]
    fn test_no_canvas_raw_without_marker() {
        let td = std::env::temp_dir().join("geo_vfs_canv_no_marker");
        let _ = fs::remove_dir_all(&td);
        let _ = fs::create_dir_all(&td);

        // Create surface with data but NO CANV marker — should NOT persist
        {
            let mut s = VfsSurface::new();
            s.base_dir = td.clone();
            s.pixels[256] = 0x12345678; // some data
                                        // No CANV marker set
            drop(s);
        }

        // canvas.raw should NOT exist
        assert!(
            !td.join(CANVAS_RAW_FILE).exists(),
            "canvas.raw should NOT be created without CANV marker"
        );

        let _ = fs::remove_dir_all(&td);
    }
}
