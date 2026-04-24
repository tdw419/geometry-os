# Pixel VFS Surface -- "Pixels Move Pixels" File Access

## The Problem

The current RISC-V VFS access path is:

```
Guest: ecall(GEO_VFS_READ, "test.txt", buf, 512)
  -> SBI trap -> host reads ext4 via std::fs::read()
  -> byte-by-byte copy into guest RAM
  -> return bytes_read
```

This is `read(2)` with a paint job. The data never touches a pixel. The
guest gets opaque bytes in an opaque buffer. It's invisible computation.

## The Principle

"Pixels move pixels" means: the data IS the pixels. To read a file, you
sample pixels from a known screen region. To write a file, you paint pixels.
The screen shows your data. There is no hidden substrate.

## Design: VFS Pixel Surface

### Memory Map Addition

```
0x8000_0000 - 0x80FF_FFFF   Guest RAM (existing, up to 16MB)
0x7000_0000 - 0x700F_FFFF   VFS Pixel Surface (new MMIO device)
0x1000_0000 - 0x1000_0007   UART (existing)
0x0200_0000 - 0x0200_FFFF   CLINT (existing)
```

The VFS Pixel Surface is a 256x256 RGBA framebuffer at MMIO 0x7000_0000.
Total: 256x256x4 = 262,144 bytes (256KB).

### Encoding

Each pixel is a 32-bit RGBA word. In the VFS surface, each pixel holds
4 bytes of file data:

```
Pixel (x, y) at address 0x7000_0000 + (y * 256 + x) * 4:
  R = byte[0], G = byte[1], B = byte[2], A = byte[3]
```

The surface layout:

```
Row 0:    [Header for file 0]
Row 1-N:  [File 0 data pixels]
Row N+1:  [Header for file 1]
Row N+2-M:[File 1 data pixels]
...
Row 255:  [unused = all zeros]
```

### Header Pixel Format

Each file's header is ONE pixel (4 bytes = 32 bits):

```
Bits 31-16: byte_count (up to 65535 bytes per file = ~64KB)
Bits 15-8:  name_hash (FNV-1a of filename, for lookup)
Bits 7-0:   flags (bit 0 = valid, bit 1 = dirty/write-pending)
```

For filenames longer than a hash match can resolve, we add an EXTENDED
HEADER (row 0 of the surface is a directory index):

```
Row 0:  Directory index
          Col 0:   magic = 0x50584653 ("PXFS")
          Col 1:   file_count
          Col 2-N: one pixel per file: [start_row(16) | name_hash(16)]
Row 1+: File data regions (header + data per file)
```

### Guest Access (Read)

No ecall. No trap. No hypercall. Just a normal load:

```asm
# Read file at row 5 into registers
li t0, 0x70000000          # VFS surface base
li t1, 5                    # row 5
slli t1, t1, 10             # t1 = row * 256 * 4 = row << 10
add t0, t0, t1              # t0 = header pixel address
lw t2, 0(t0)                # t2 = header: byte_count | name_hash | flags

# Now read data pixels starting at row 6
addi t0, t0, 0x400          # next row = +256 pixels = +0x400 bytes
lw t3, 0(t0)                # first 4 bytes of file data
```

This is a single load instruction. The guest reads pixels and gets bytes.
The data is visible on screen if the surface is rendered.

### Guest Access (Write)

Paint pixels, set the dirty flag:

```asm
# Write "Hi!" to a file region starting at row 10
li t0, 0x70000000
li t1, 10
slli t1, t1, 10
add t0, t0, t1

# Set header: 3 bytes, valid + dirty
li t2, (3 << 16) | 0x03    # byte_count=3, flags=valid|dirty
sw t2, 0(t0)

# Paint data: H=0x48, i=0x69, !=0x21, pad=0x00
li t3, 0x48692100           # RGBA = "Hi!\0"
sw t3, 0x400(t0)            # row 11
```

The host polls the dirty bits and writes back to the real filesystem.
Or the host renders the surface to screen -- the file literally appears
as colored pixels.

### Host Side: Bus Integration

Add a new MMIO device `VfsSurface` to the bus, similar to UART:

```rust
// In bus.rs
pub struct Bus {
    pub mem: GuestMemory,
    pub clint: Clint,
    pub uart: Uart,
    pub plic: Plic,
    pub virtio_blk: VirtioBlk,
    pub sbi: Sbi,
    pub vfs_surface: VfsSurface,  // NEW
    // ...
}
```

The device:

```rust
// vfs_surface.rs

const VFS_SURFACE_BASE: u64 = 0x7000_0000;
const VFS_SURFACE_SIZE: usize = 256 * 256 * 4; // 256KB
const VFS_SURFACE_ROWS: usize = 256;
const VFS_SURFACE_COLS: usize = 256;

pub struct VfsSurface {
    /// Raw pixel buffer: RGBA, 256x256
    pixels: Vec<u32>,
    /// Track which rows have dirty headers (guest wrote to them)
    dirty_rows: Vec<bool>,
}

impl VfsSurface {
    pub fn new() -> Self {
        let mut surface = Self {
            pixels: vec![0u32; VFS_SURFACE_ROWS * VFS_SURFACE_COLS],
            dirty_rows: vec![false; VFS_SURFACE_ROWS],
        };
        surface.load_from_host_fs();
        surface
    }

    /// On init: encode all files from .geometry_os/fs/ into pixel rows
    fn load_from_host_fs(&mut self) {
        let fs_dir = std::path::Path::new(".geometry_os/fs");
        if !fs_dir.exists() { return; }

        let mut row = 1; // row 0 = directory index
        let mut file_count = 0u32;

        // Write magic
        self.pixels[0] = 0x50584653; // "PXFS"

        for entry in std::fs::read_dir(fs_dir).unwrap_or_else(|_| panic!("no fs dir")) {
            let entry = match entry { Ok(e) => e, Err(_) => continue };
            let name = match entry.file_name().into_string() {
                Ok(n) => n, Err(_) => continue
            };
            if name.starts_with('.') { continue; }

            let data = match std::fs::read(entry.path()) {
                Ok(d) => d, Err(_) => continue
            };
            let byte_count = data.len().min(65535) as u16;
            let name_hash = fnv1a_16(&name);

            // Directory index entry in row 0
            let idx_col = 2 + file_count as usize;
            if idx_col < VFS_SURFACE_COLS && row < VFS_SURFACE_ROWS {
                self.pixels[idx_col] =
                    ((row as u32) << 16) | (name_hash as u32);
                file_count += 1;

                // File header at start row
                self.pixels[row * VFS_SURFACE_COLS] =
                    ((byte_count as u32) << 16) |
                    ((name_hash as u32) << 8) |
                    0x01; // valid flag

                // Encode file data as pixels starting at next row
                let mut pixel_idx = (row + 1) * VFS_SURFACE_COLS;
                for chunk in data.chunks(4) {
                    if pixel_idx >= self.pixels.len() { break; }
                    let mut px = 0u32;
                    for (i, &b) in chunk.iter().enumerate() {
                        px |= (b as u32) << (i * 8);
                    }
                    self.pixels[pixel_idx] = px;
                    pixel_idx += 1;
                }

                // Advance row past header + data pixels
                let data_rows = (byte_count as usize + 3) / 4 / VFS_SURFACE_COLS + 1;
                row += 1 + data_rows;
            }

            if file_count >= 254 || row >= VFS_SURFACE_ROWS - 2 { break; }
        }

        // Update file count in directory index
        self.pixels[1] = file_count;
    }

    /// Flush dirty rows back to host filesystem
    pub fn flush_dirty(&mut self) {
        for (row, dirty) in self.dirty_rows.iter_mut().enumerate() {
            if !*dirty { continue; }
            *dirty = false;
            self.flush_row(row);
        }
    }

    fn flush_row(&mut self, header_row: usize) {
        // Decode header, extract bytes from data pixels, write to file
        // ... (inverse of load_from_host_fs encoding)
    }

    pub fn contains(addr: u64) -> bool {
        addr >= VFS_SURFACE_BASE &&
        addr < VFS_SURFACE_BASE + VFS_SURFACE_SIZE as u64
    }

    pub fn read_word(&self, addr: u64) -> Option<u32> {
        let offset = (addr - VFS_SURFACE_BASE) as usize;
        let idx = offset / 4;
        self.pixels.get(idx).copied()
    }

    pub fn write_word(&mut self, addr: u64, val: u32) {
        let offset = (addr - VFS_SURFACE_BASE) as usize;
        let idx = offset / 4;
        if idx < self.pixels.len() {
            self.pixels[idx] = val;
            let row = idx / VFS_SURFACE_COLS;
            self.dirty_rows[row] = true;
        }
    }
}
```

### Guest C API

```c
// vfs_pixel.h -- no ecall, no syscall, just memory reads

#define VFS_SURFACE_BASE  0x70000000
#define VFS_COLS          256

// Read the directory index from row 0
// Returns number of files, fills entries with (start_row, name_hash) pairs
static inline int vfs_list_files(struct vfs_entry *entries, int max) {
    volatile uint32_t *surface = (volatile uint32_t *)VFS_SURFACE_BASE;
    uint32_t magic = surface[0];
    if (magic != 0x50584653) return -1;  // "PXFS"
    uint32_t count = surface[1];
    if (count > max) count = max;
    for (int i = 0; i < count; i++) {
        uint32_t entry = surface[2 + i];
        entries[i].start_row = entry >> 16;
        entries[i].name_hash = entry & 0xFFFF;
    }
    return count;
}

// Read file data from pixel rows into buffer
// Returns bytes read
static inline int vfs_read_file(int start_row, void *buf, int maxlen) {
    volatile uint32_t *surface = (volatile uint32_t *)VFS_SURFACE_BASE;
    uint32_t header = surface[start_row * VFS_COLS];
    int byte_count = header >> 16;
    if (byte_count > maxlen) byte_count = maxlen;

    volatile uint32_t *data = &surface[(start_row + 1) * VFS_COLS];
    uint8_t *out = (uint8_t *)buf;
    for (int i = 0; i < byte_count; i++) {
        int pixel = i / 4;
        int byte_in_pixel = i % 4;
        out[i] = (data[pixel] >> (byte_in_pixel * 8)) & 0xFF;
    }
    return byte_count;
}

// Write file data by painting pixels
static inline void vfs_write_file(int start_row, const void *buf, int len) {
    volatile uint32_t *surface = (volatile uint32_t *)VFS_SURFACE_BASE;
    uint32_t header = surface[start_row * VFS_COLS];
    uint32_t name_hash = (header >> 8) & 0xFF;
    // Set header: byte_count | name_hash | valid | dirty
    surface[start_row * VFS_COLS] =
        ((uint32_t)len << 16) | (name_hash << 8) | 0x03;

    const uint8_t *src = (const uint8_t *)buf;
    volatile uint32_t *data = &surface[(start_row + 1) * VFS_COLS];
    int full_pixels = len / 4;
    for (int i = 0; i < full_pixels; i++) {
        data[i] = src[i*4] | (src[i*4+1] << 8) |
                  (src[i*4+2] << 16) | (src[i*4+3] << 24);
    }
    // Handle remaining bytes
    int rem = len % 4;
    if (rem > 0) {
        uint32_t px = 0;
        for (int j = 0; j < rem; j++) {
            px |= (src[full_pixels*4 + j] as u32) << (j * 8);
        }
        data[full_pixels] = px;
    }
}
```

### Why This Is Different

| Aspect | GEO_VFS_READ (current) | Pixel VFS Surface |
|--------|----------------------|-------------------|
| Access | ecall trap + hypercall | Normal lw/sw (MMIO) |
| Visibility | Data hidden in RAM buffer | Data IS pixels on screen |
| Alignment | POSIX read(2) with paint job | pixels move pixels |
| Latency | Trap + host fs read + copy | Direct memory read |
| Write path | Not implemented (read-only) | Paint pixels, set dirty bit |
| Mental model | "Call the OS to get bytes" | "Read the screen, it has your data" |

### Migration Path

1. ~~Implement VfsSurface as a Bus device (new file: src/riscv/vfs_surface.rs)~~ DONE
2. ~~Wire it into Bus::read_word/write_word routing~~ DONE
3. ~~Write guest-side C header (vfs_pixel.h) in examples/riscv-hello/~~ DONE
4. ~~Port cat.c from ecall-based to pixel-based reads~~ DONE (vfs_pixel_cat.c)
5. ~~Render the VFS surface on the Geometry OS desktop as a visible window~~ DONE (WINSYS op=8)
6. ~~Deprecate GEO_VFS_READ ecall~~ DONE (returns NOT_SUPPORTED, removed from probe)
7. ~~Implement write-back (display -> storage)~~ DONE (flush, Drop, VFS_CONTROL_ADDR, WINSYS op=9)

All migration steps complete. GEO_VFS_READ is hard-deprecated.

### The Deeper Point

"Pixels move pixels" isn't just about making file access faster. It's about
making computation legible. When a program reads a file, you can SEE the file
on screen as colored pixels. When it writes, you can watch the pixels change.
The storage and the display are the same thing.

This is the difference between a computer that hides its work and a computer
that shows its work. The pixel surface makes every data transfer a visible
event. There are no invisible buffers. No hidden copies. What you see is what
the machine sees.
