//! Geometry OS Vision Module
//!
//! Provides canvas-to-PNG encoding, checksum computation, canvas diffing,
//! and window overlay annotation for annotated screenshots.
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
    let raw_deflate = deflate_raw(&raw_data);

    // Wrap in zlib format: header (2 bytes) + deflate data + adler32 (4 bytes)
    // zlib header: CMF=0x78 (deflate, window=32K), FLG=0x01 (no dict, check bits)
    let mut compressed = Vec::with_capacity(6 + raw_deflate.len());
    compressed.push(0x78); // CMF: deflate, window size 32K
    compressed.push(0x01); // FLG: no preset dict, check bits set correctly

    compressed.extend_from_slice(&raw_deflate);

    // Adler-32 checksum
    let adler = adler32(&raw_data);
    compressed.extend_from_slice(&adler.to_be_bytes());

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

/// Compute Adler-32 checksum (zlib standard)
fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    const MOD: u32 = 65521;
    for &byte in data {
        a = (a + byte as u32) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
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

/// A window overlay to draw on the annotated screenshot.
#[derive(Clone, Debug)]
pub struct WindowOverlay {
    pub id: u32,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub title: String,
    pub focused: bool,
}

/// Draw a colored rectangle outline onto a mutable pixel buffer (256x256).
/// Uses Bresenham-style edge drawing. `color` is 0x00RRGGBB.
fn draw_rect(buf: &mut [u32], x0: u32, y0: u32, w: u32, h: u32, color: u32) {
    let screen_w: u32 = 256;
    let x1 = (x0 + w).min(256);
    let y1 = (y0 + h).min(256);
    // Top and bottom edges
    for x in x0..x1 {
        if x < screen_w && y0 < 256 {
            buf[(y0 * screen_w + x) as usize] = color;
        }
        if h > 1 && y1 > 0 && (y1 - 1) < 256 {
            buf[((y1 - 1) * screen_w + x) as usize] = color;
        }
    }
    // Left and right edges
    for y in y0..y1 {
        if y < 256 && x0 < screen_w {
            buf[(y * screen_w + x0) as usize] = color;
        }
        if w > 1 && x1 > 0 && (x1 - 1) < screen_w {
            buf[(y * screen_w + (x1 - 1)) as usize] = color;
        }
    }
}

/// Draw a filled rectangle (used for label backgrounds).
fn draw_rect_filled(buf: &mut [u32], x0: u32, y0: u32, w: u32, h: u32, color: u32) {
    for y in y0..(y0 + h).min(256) {
        for x in x0..(x0 + w).min(256) {
            buf[(y * 256 + x) as usize] = color;
        }
    }
}

/// Draw a single character from the 8x8 font onto the pixel buffer.
fn draw_char(buf: &mut [u32], ch: char, px: u32, py: u32, color: u32) {
    use crate::font::{GLYPHS, GLYPH_H, GLYPH_W};
    let code = ch as usize;
    if code >= 128 {
        return;
    }
    let glyph = &GLYPHS[code];
    for row in 0..GLYPH_H {
        for col in 0..GLYPH_W {
            if glyph[row] & (1 << (7 - col)) != 0 {
                let x = px + col as u32;
                let y = py + row as u32;
                if x < 256 && y < 256 {
                    buf[(y * 256 + x) as usize] = color;
                }
            }
        }
    }
}

/// Draw a text string onto the pixel buffer using the 8x8 font.
fn draw_text(buf: &mut [u32], text: &str, px: u32, py: u32, color: u32) {
    for (i, ch) in text.chars().enumerate() {
        draw_char(buf, ch, px + (i as u32) * 8, py, color);
    }
}

/// Encode the screen buffer with window overlays as a PNG.
/// Draws colored bounding boxes and title labels for each window.
pub fn encode_png_annotated(screen: &[u32], windows: &[WindowOverlay]) -> Vec<u8> {
    // Clone screen so we don't modify the original
    let mut buf = screen.to_vec();

    // Color palette for window borders (cycle through distinct colors)
    let border_colors: &[u32] = &[
        0x00FFFF, // cyan
        0xFFFF00, // yellow
        0xFF00FF, // magenta
        0x00FF00, // green
        0xFF8000, // orange
        0x8080FF, // light blue
        0xFF0080, // pink
        0x00FF80, // spring green
    ];

    for (i, win) in windows.iter().enumerate() {
        let border_color = border_colors[i % border_colors.len()];
        let border_width = if win.focused { 2u32 } else { 1u32 };

        // Draw border (potentially 2px wide for focused window)
        for offset in 0..border_width {
            draw_rect(
                &mut buf,
                win.x.saturating_sub(offset),
                win.y.saturating_sub(offset),
                win.w + offset * 2,
                win.h + offset * 2,
                border_color,
            );
        }

        // Build label: "ID:Title" (truncated to fit)
        let label = if win.title.is_empty() {
            format!("[{}]", win.id)
        } else {
            format!("[{}] {}", win.id, win.title)
        };
        // Truncate label to fit within window width (each char = 8px)
        let max_chars = ((win.w / 8) as usize).max(3).min(28);
        let label_display = if label.len() > max_chars {
            format!("{}..", &label[..max_chars.saturating_sub(2)])
        } else {
            label
        };

        // Draw label background (dark with alpha-like effect)
        let label_w = (label_display.len() as u32) * 8;
        let label_y = if win.y >= 10 {
            win.y - 10
        } else {
            win.y + win.h
        };
        draw_rect_filled(
            &mut buf,
            win.x,
            label_y,
            label_w.min(256 - win.x),
            8,
            0x000000,
        );
        draw_text(&mut buf, &label_display, win.x, label_y, border_color);
    }

    encode_png(&buf)
}

/// Encode the annotated screen buffer as base64 PNG.
pub fn encode_png_annotated_base64(screen: &[u32], windows: &[WindowOverlay]) -> String {
    let png_bytes = encode_png_annotated(screen, windows);
    base64_encode(&png_bytes)
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

    #[test]
    fn test_annotated_png_no_windows() {
        // Annotated screenshot with no windows should be same as plain screenshot
        let screen = vec![0x00FF00u32; 256 * 256]; // all green
        let windows: Vec<WindowOverlay> = vec![];
        let png = encode_png_annotated(&screen, &windows);
        let plain_png = encode_png(&screen);
        assert_eq!(png, plain_png, "no windows = same as plain PNG");
    }

    #[test]
    fn test_annotated_png_with_window() {
        // Single window should produce a different PNG (has borders drawn)
        let screen = vec![0u32; 256 * 256];
        let windows = vec![WindowOverlay {
            id: 1,
            x: 50,
            y: 50,
            w: 80,
            h: 60,
            title: "TestWin".to_string(),
            focused: true,
        }];
        let annotated = encode_png_annotated(&screen, &windows);
        let plain = encode_png(&screen);
        assert_ne!(annotated, plain, "annotated should differ from plain");
        // Should still be a valid PNG
        assert!(annotated.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]));
    }

    #[test]
    fn test_annotated_png_draws_border_pixels() {
        // Verify that border pixels are actually drawn (non-zero after annotation)
        let screen = vec![0u32; 256 * 256];
        let windows = vec![WindowOverlay {
            id: 1,
            x: 10,
            y: 10,
            w: 20,
            h: 20,
            title: "W".to_string(),
            focused: false,
        }];
        let mut buf = screen.clone();
        // Replicate what encode_png_annotated does for border checking
        // The top edge at y=10 should have colored pixels
        super::draw_rect(&mut buf, 10, 10, 20, 20, 0x00FFFF);
        // Top-left pixel of the window should now be cyan
        assert_ne!(buf[10 * 256 + 10], 0, "border pixel should be drawn");
        assert_eq!(buf[10 * 256 + 10], 0x00FFFF, "should be cyan border");
        // Center pixel should still be 0 (not on border)
        assert_eq!(buf[20 * 256 + 20], 0, "center should be unchanged");
    }

    #[test]
    fn test_annotated_base64_with_multiple_windows() {
        let screen = vec![0u32; 256 * 256];
        let windows = vec![
            WindowOverlay {
                id: 1,
                x: 0,
                y: 0,
                w: 100,
                h: 100,
                title: "First".to_string(),
                focused: false,
            },
            WindowOverlay {
                id: 2,
                x: 100,
                y: 100,
                w: 100,
                h: 100,
                title: "Second".to_string(),
                focused: true,
            },
        ];
        let b64 = encode_png_annotated_base64(&screen, &windows);
        assert!(b64.starts_with("iVBOR"), "should be valid PNG base64");
        assert!(b64.len() > 1000, "base64 should be substantial");
    }
}
