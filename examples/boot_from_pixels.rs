/// Boot Linux from pixel images.
///
/// Reads kernel.rts.png + initramfs.rts.png, decodes the Hilbert curve
/// layout back to raw bytes, then boots via the Geometry OS RISC-V VM.
///
/// Usage:
///   cargo run --example boot_from_pixels -- /path/to/kernel.rts.png [/path/to/initramfs.rts.png]
///
/// The .rts.png format stores binary data as RGBA pixels in Hilbert curve order.
/// Each pixel = 4 bytes (R=byte0, G=byte1, B=byte2, A=byte3).
/// Original size and SHA256 are stored in PNG text chunks.
use geometry_os::riscv::RiscvVm;
use png::Decoder;
use std::env;
use std::fs::File;
use std::io::BufReader;

/// Decode a .rts.png file back to raw bytes.
fn decode_rts_png(path: &str) -> Vec<u8> {
    let file = File::open(path).unwrap_or_else(|e| {
        eprintln!("Cannot open {}: {}", path, e);
        std::process::exit(1);
    });
    let decoder = Decoder::new(BufReader::new(file));
    let mut reader = decoder.read_info().unwrap_or_else(|e| {
        eprintln!("PNG decode error for {}: {:?}", path, e);
        std::process::exit(1);
    });

    // Read metadata
    let info = reader.info().clone();
    let expected_size: usize = info
        .uncompressed_latin1_text
        .iter()
        .find(|c| c.keyword == "data_size")
        .and_then(|c| c.text.parse().ok())
        .unwrap_or_else(|| {
            // Fallback: width * height * 4
            (info.width as usize) * (info.height as usize) * 4
        });

    let source_name = info
        .uncompressed_latin1_text
        .iter()
        .find(|c| c.keyword == "source")
        .map(|c| c.text.clone())
        .unwrap_or_default();

    let expected_sha = info
        .uncompressed_latin1_text
        .iter()
        .find(|c| c.keyword == "sha256")
        .map(|c| c.text.clone());

    // Allocate output buffer
    let total_pixels = (info.width as usize) * (info.height as usize);
    let mut pixel_buf = vec![0u8; total_pixels * 4];
    reader.next_frame(&mut pixel_buf).expect("read PNG frame");

    // Inverse Hilbert: pixel (x,y) -> linear index
    let grid_w = info.width as u32;
    let grid_h = info.height as u32;
    let grid_side = grid_w.max(grid_h);
    let grid_order = 31 - grid_side.leading_zeros(); // log2

    // Decode Hilbert curve: for each linear index, compute (x,y),
    // then read the pixel at that position.
    let mut output = Vec::with_capacity(expected_size);
    let mut linear = 0u32;

    while output.len() < expected_size && linear < total_pixels as u32 {
        let (x, y) = d2xy(grid_order, linear);
        // Python encoder uses arr[hx, hy] which in numpy means row=hx, col=hy.
        // Our (x,y) from Hilbert maps to hx=x, hy=y in Python's convention.
        // So pixel position is row=x, col=y, i.e. offset = (x * grid_w + y) * 4.
        if x < grid_h && y < grid_w {
            let pixel_offset = ((x * grid_w + y) * 4) as usize;
            if pixel_offset + 4 <= pixel_buf.len() {
                output.push(pixel_buf[pixel_offset]); // R
                output.push(pixel_buf[pixel_offset + 1]); // G
                output.push(pixel_buf[pixel_offset + 2]); // B
                output.push(pixel_buf[pixel_offset + 3]); // A
            }
        }
        linear += 1;
    }

    output.truncate(expected_size);

    // Verify SHA256
    if let Some(sha) = expected_sha {
        let mut hasher = openssl_sha256();
        hasher.update(&output);
        let hash = hasher.finalize();
        let hash_hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
        // Stored hash may be truncated; compare the prefix
        if hash_hex.starts_with(&sha) {
            eprintln!("[pixel-boot] SHA256 verified: {}...", &hash_hex[..16]);
        } else {
            eprintln!(
                "[pixel-boot] SHA256 MISMATCH! expected {} got {}",
                sha,
                &hash_hex[..sha.len()]
            );
        }
    }

    eprintln!(
        "[pixel-boot] Decoded {}: {} -> {} bytes",
        path,
        source_name,
        output.len()
    );
    output
}

/// Minimal SHA256 without external deps - just use std approach
fn openssl_sha256() -> Sha256State {
    Sha256State {
        state: [
            0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
            0x5be0cd19,
        ],
        count: 0,
        buffer: [0u8; 64],
        buf_len: 0,
    }
}

struct Sha256State {
    state: [u32; 8],
    count: u64,
    buffer: [u8; 64],
    buf_len: usize,
}

impl Sha256State {
    fn update(&mut self, data: &[u8]) {
        for &byte in data {
            self.buffer[self.buf_len] = byte;
            self.buf_len += 1;
            if self.buf_len == 64 {
                self.transform();
                self.count += 512;
                self.buf_len = 0;
            }
        }
    }

    fn finalize(&mut self) -> [u8; 32] {
        self.count += (self.buf_len * 8) as u64;
        let bit_len = self.count.to_be_bytes();

        self.buffer[self.buf_len] = 0x80;
        self.buf_len += 1;

        if self.buf_len > 56 {
            self.buffer[self.buf_len..].fill(0);
            self.transform();
            self.buffer[..56].fill(0);
        } else {
            self.buffer[self.buf_len..56].fill(0);
        }
        self.buffer[56..64].copy_from_slice(&bit_len);
        self.transform();

        let mut out = [0u8; 32];
        for (i, &s) in self.state.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&s.to_be_bytes());
        }
        out
    }

    fn transform(&mut self) {
        let k: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];

        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                self.buffer[i * 4],
                self.buffer[i * 4 + 1],
                self.buffer[i * 4 + 2],
                self.buffer[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(k[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

/// Convert Hilbert 1D index d to 2D coordinates (x, y).
/// Uses LSB-first bit processing to match the Python encoder.
fn d2xy(grid_order: u32, d: u32) -> (u32, u32) {
    let mut x: u32 = 0;
    let mut y: u32 = 0;

    for s in 0..grid_order {
        let shift = 2 * s;
        let rx = (d >> shift) & 1;
        let ry = ((d >> shift) >> 1) & 1;

        // If ry == 0, swap x and y. If additionally rx == 1, flip.
        if ry == 0 {
            if rx == 1 {
                let s_val = 1u32 << s;
                x = s_val - 1 - x;
                y = s_val - 1 - y;
            }
            std::mem::swap(&mut x, &mut y);
        }

        let s_val = 1u32 << s;
        x += rx * s_val;
        y += ry * s_val;
    }

    (x, y)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: boot_from_pixels <kernel.rts.png> [initramfs.rts.png]");
        eprintln!("\nBoots Linux from pixelpack .rts.png pixel images.");
        std::process::exit(1);
    }

    let kernel_path = &args[1];
    let initramfs_path = args.get(2).map(|s| s.as_str());

    eprintln!("[pixel-boot] Decoding kernel from pixels...");
    let kernel_image = decode_rts_png(kernel_path);

    let initramfs = if let Some(path) = initramfs_path {
        eprintln!("[pixel-boot] Decoding initramfs from pixels...");
        Some(decode_rts_png(path))
    } else {
        eprintln!("[pixel-boot] No initramfs specified, booting kernel only");
        None
    };

    eprintln!("[pixel-boot] Kernel: {} bytes", kernel_image.len());
    if let Some(ref initrd) = initramfs {
        eprintln!("[pixel-boot] Initramfs: {} bytes", initrd.len());
    }

    eprintln!("[pixel-boot] Starting Linux boot...");
    let (mut vm, info) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        50_000_000,
        "console=ttyS0 loglevel=8",
    )
    .unwrap();

    eprintln!(
        "[pixel-boot] Boot completed: {} instructions, PC=0x{:08X}",
        info.instructions, vm.cpu.pc
    );

    // Check for UART output
    let uart_output = vm.bus.uart.drain_tx();
    if !uart_output.is_empty() {
        let s: String = uart_output.iter().map(|&b| b as char).collect();
        eprintln!("[pixel-boot] UART output ({} bytes):", s.len());
        for line in s.lines() {
            eprintln!("  {}", line);
        }
    } else {
        eprintln!("[pixel-boot] No UART output");
    }
}
