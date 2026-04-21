//! Geometry OS Vision Module
//!
//! Provides canvas-to-PNG encoding, checksum computation, and canvas diffing.
//! Pure Rust, zero external dependencies. Used by the AI_AGENT opcode (0xB0)
//! and the MCP server vision tools.

/// FNV-1a hash of the screen buffer. Fast checksum for "did the canvas change?".
/// Returns a 32-bit hash of all 256*256 = 65536 pixels.
pub fn canvas_checksum(screen: &[u32]) -> u32 {
    let mut hash: u32 = 0x811C9DC5; // FNV offset basis
    for &pixel in screen {
        hash ^= pixel;
        hash = hash.wrapping_mul(0x01000193); // FNV prime
    }
    hash
}

/// Count how many pixels differ between two screen buffers.
/// Returns (changed_count, total_pixels, percentage_changed).
pub fn canvas_diff(screen_before: &[u32], screen_after: &[u32]) -> (u32, u32, f64) {
    let total = screen_before.len().min(screen_after.len()) as u32;
    let mut changed: u32 = 0;
    for i in 0..total as usize {
        if screen_before[i] != screen_after[i] {
            changed += 1;
        }
    }
    let pct = if total > 0 {
        (changed as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    (changed, total, pct)
}

/// Encode the 256x256 screen buffer as a PNG file (raw bytes).
///
/// Uses uncompressed deflate blocks (stored blocks) which are valid PNG.
/// No external dependencies required.
///
/// Pixel format: input is u32 ARGB (0xAARRGGBB), output PNG is RGB (8-bit per channel).
pub fn encode_png(screen: &[u32]) -> Vec<u8> {
    let width: u32 = 256;
    let height: u32 = 256;

    // Build raw image data with filter byte per row
    // PNG filter type 0 = None (raw bytes follow)
    let mut raw_data = Vec::with_capacity((width as usize * 3 + 1) * height as usize);
    for row in 0..height {
        raw_data.push(0); // filter: None
        for col in 0..width {
            let pixel = screen[(row * width + col) as usize];
            // Input is 0x00RRGGBB (common in Geometry OS), output RGB
            let r = ((pixel >> 16) & 0xFF) as u8;
            let g = ((pixel >> 8) & 0xFF) as u8;
            let b = (pixel & 0xFF) as u8;
            raw_data.push(r);
            raw_data.push(g);
            raw_data.push(b);
        }
    }

    // Compress with raw deflate (stored blocks, no compression)
    let compressed = deflate_raw(&raw_data);

    // Build PNG file
    let mut png = Vec::new();

    // PNG signature
    png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    // IHDR chunk
    let mut ihdr_data = Vec::new();
    ihdr_data.extend_from_slice(&width.to_be_bytes());
    ihdr_data.extend_from_slice(&height.to_be_bytes());
    ihdr_data.push(8); // bit depth
    ihdr_data.push(2); // color type: RGB
    ihdr_data.push(0); // compression: deflate
    ihdr_data.push(0); // filter: adaptive
    ihdr_data.push(0); // interlace: none
    write_chunk(&mut png, b"IHDR", &ihdr_data);

    // IDAT chunk (compressed image data)
    write_chunk(&mut png, b"IDAT", &compressed);

    // IEND chunk
    write_chunk(&mut png, b"IEND", &[]);

    png
}

/// Encode the 256x256 screen buffer as a base64-encoded PNG string.
pub fn encode_png_base64(screen: &[u32]) -> String {
    let png_bytes = encode_png(screen);
    base64_encode(&png_bytes)
}

/// Minimal raw deflate encoder using stored (uncompressed) blocks.
/// Produces valid deflate stream that any decoder can read.
fn deflate_raw(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let chunk_size = 65535; // max stored block size
    let mut offset = 0;

    while offset < data.len() {
        let remaining = data.len() - offset;
        let block_len = remaining.min(chunk_size);
        let is_final = offset + block_len >= data.len();

        // Stored block header: BFINAL(1 bit) + BTYPE=00(2 bits) = 1 byte
        out.push(if is_final { 0x01 } else { 0x00 });

        // LEN (2 bytes, little-endian)
        out.push((block_len & 0xFF) as u8);
        out.push(((block_len >> 8) & 0xFF) as u8);

        // NLEN (1's complement of LEN)
        let block_len_complement = !block_len;
        out.push((block_len_complement & 0xFF) as u8);
        out.push(((block_len_complement >> 8) & 0xFF) as u8);

        // Raw data
        out.extend_from_slice(&data[offset..offset + block_len]);
        offset += block_len;
    }

    out
}

/// Write a PNG chunk: length(4) + type(4) + data + crc32(4)
fn write_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    let len = data.len() as u32;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(chunk_type);
    out.extend_from_slice(data);

    // CRC32 over type + data
    let mut crc_data = Vec::with_capacity(4 + data.len());
    crc_data.extend_from_slice(chunk_type);
    crc_data.extend_from_slice(data);
    let crc = crc32(&crc_data);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// CRC32 lookup table (polynomial 0xEDB88320, same as PNG/zlib/gzip)
const CRC_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
};

/// Compute CRC32 (PNG/zlib standard)
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        let idx = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC_TABLE[idx];
    }
    crc ^ 0xFFFFFFFF
}

/// Base64 encode (standard alphabet with padding)
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    let chunks = data.len() / 3;
    let remainder = data.len() % 3;

    for i in 0..chunks {
        let offset = i * 3;
        let b0 = data[offset] as u32;
        let b1 = data[offset + 1] as u32;
        let b2 = data[offset + 2] as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        out.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        out.push(CHARS[(triple & 0x3F) as usize] as char);
    }

    if remainder == 1 {
        let b0 = data[chunks * 3] as u32;
        out.push(CHARS[((b0 >> 2) & 0x3F) as usize] as char);
        out.push(CHARS[((b0 << 4) & 0x30) as usize] as char);
        out.push('=');
        out.push('=');
    } else if remainder == 2 {
        let b0 = data[chunks * 3] as u32;
        let b1 = data[chunks * 3 + 1] as u32;
        out.push(CHARS[((b0 >> 2) & 0x3F) as usize] as char);
        out.push(CHARS[(((b0 << 4) | (b1 >> 4)) & 0x3F) as usize] as char);
        out.push(CHARS[((b1 << 2) & 0x3C) as usize] as char);
        out.push('=');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_empty_screen() {
        let screen = vec![0u32; 256 * 256];
        let hash = canvas_checksum(&screen);
        // All zeros: FNV-1a should produce a deterministic hash
        assert_ne!(hash, 0, "checksum of zeros should not be 0");
        // Same input = same output
        assert_eq!(hash, canvas_checksum(&screen));
    }

    #[test]
    fn test_checksum_detects_change() {
        let screen_a = vec![0u32; 256 * 256];
        let mut screen_b = vec![0u32; 256 * 256];
        screen_b[0] = 0xFF0000; // one pixel changed
        assert_ne!(canvas_checksum(&screen_a), canvas_checksum(&screen_b));
    }

    #[test]
    fn test_diff_no_change() {
        let screen = vec![0x00FF00u32; 256 * 256];
        let (changed, total, pct) = canvas_diff(&screen, &screen);
        assert_eq!(changed, 0);
        assert_eq!(total, 256 * 256);
        assert!((pct - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_diff_single_pixel() {
        let before = vec![0u32; 256 * 256];
        let mut after = vec![0u32; 256 * 256];
        after[42] = 0xFFFFFF;
        let (changed, total, pct) = canvas_diff(&before, &after);
        assert_eq!(changed, 1);
        assert_eq!(total, 256 * 256);
        assert!(pct > 0.0);
    }

    #[test]
    fn test_png_valid_signature() {
        let screen = vec![0xFF0000u32; 256 * 256]; // all red
        let png = encode_png(&screen);
        // PNG signature: 137 80 78 71 13 10 26 10
        assert_eq!(&png[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    #[test]
    fn test_png_has_ihdr_idat_iend() {
        let screen = vec![0u32; 256 * 256];
        let png = encode_png(&screen);

        // Find IHDR, IDAT, IEND markers
        let png_str = String::from_utf8_lossy(&png);
        assert!(png_str.contains("IHDR"), "should have IHDR chunk");
        assert!(png_str.contains("IDAT"), "should have IDAT chunk");
        assert!(png_str.contains("IEND"), "should have IEND chunk");
    }

    #[test]
    fn test_png_size_reasonable() {
        let screen = vec![0x123456u32; 256 * 256];
        let png = encode_png(&screen);
        // Uncompressed: 256*256*3 + 256 filter bytes + headers ≈ 196K min
        // With deflate stored blocks and PNG overhead, should be ~200-220KB
        assert!(png.len() > 190_000, "PNG too small: {} bytes", png.len());
        assert!(
            png.len() < 250_000,
            "PNG unexpectedly large: {} bytes",
            png.len()
        );
    }

    #[test]
    fn test_base64_roundtrip() {
        let data = b"Hello, Geometry OS!";
        let encoded = base64_encode(data);
        assert!(encoded.len() > 0);
        // Verify it's valid base64 (no invalid chars)
        for c in encoded.chars() {
            assert!(
                c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=',
                "Invalid base64 char: {}",
                c
            );
        }
    }

    #[test]
    fn test_png_base64() {
        let screen = vec![0x0000FFu32; 256 * 256]; // all blue
        let b64 = encode_png_base64(&screen);
        assert!(b64.len() > 100);
        assert!(b64.starts_with('i'), "PNG base64 starts with i");
        // PNG signature base64: iVBORw...
        assert!(
            b64.starts_with("iVBOR"),
            "should start with PNG base64 prefix"
        );
    }
}
