// pixel.rs -- Pixel image decode/encode for .rts.png format
//
// The .rts.png format stores binary data as RGBA pixels.
// Two layouts: Hilbert curve (small/medium) or linear (large files).
// Each pixel = 4 bytes (R=byte0, G=byte1, B=byte2, A=byte3).
// Original size, layout, and SHA256 are stored in PNG text chunks.

use png::Decoder;
use std::fs::File;
use std::io::BufReader;

/// Decoded .rts.png result with metadata.
pub struct DecodedPixels {
    pub data: Vec<u8>,
    pub source_name: String,
    pub data_size: usize,
}

/// Decode a .rts.png file back to raw bytes.
/// Returns the decoded data with metadata from the PNG text chunks.
pub fn decode_rts_png(path: &str) -> Result<DecodedPixels, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open {}: {}", path, e))?;
    let decoder = Decoder::new(BufReader::new(file));
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("PNG decode error for {}: {:?}", path, e))?;

    // Read metadata from PNG text chunks
    let info = reader.info().clone();
    let expected_size: usize = info
        .uncompressed_latin1_text
        .iter()
        .find(|c| c.keyword == "data_size")
        .and_then(|c| c.text.parse().ok())
        .unwrap_or_else(|| (info.width as usize) * (info.height as usize) * 4);

    let source_name = info
        .uncompressed_latin1_text
        .iter()
        .find(|c| c.keyword == "source" || c.keyword == "original_file")
        .map(|c| c.text.clone())
        .unwrap_or_default();

    let layout = info
        .uncompressed_latin1_text
        .iter()
        .find(|c| c.keyword == "layout")
        .map(|c| c.text.to_lowercase())
        .unwrap_or_else(|| "hilbert".to_string());

    // Read pixels
    let total_pixels = (info.width as usize) * (info.height as usize);
    let mut pixel_buf = vec![0u8; total_pixels * 4];
    reader
        .next_frame(&mut pixel_buf)
        .map_err(|e| format!("PNG read error: {:?}", e))?;

    let mut output = if layout == "linear" {
        // Linear layout: read pixels row by row, 4 bytes per pixel
        let mut out = Vec::with_capacity(expected_size);
        for chunk in pixel_buf.chunks_exact(4) {
            out.push(chunk[0]); // R
            out.push(chunk[1]); // G
            out.push(chunk[2]); // B
            out.push(chunk[3]); // A
            if out.len() >= expected_size {
                break;
            }
        }
        out
    } else {
        // Hilbert curve layout: inverse Hilbert to get linear byte order
        let grid_w = info.width;
        let grid_h = info.height;
        let grid_side = grid_w.max(grid_h);
        let grid_order = 31 - grid_side.leading_zeros();

        let mut out = Vec::with_capacity(expected_size);
        let mut linear = 0u32;

        while out.len() < expected_size && linear < total_pixels as u32 {
            let (x, y) = d2xy(grid_order, linear);
            if x < grid_h && y < grid_w {
                let pixel_offset = ((x * grid_w + y) * 4) as usize;
                if pixel_offset + 4 <= pixel_buf.len() {
                    out.push(pixel_buf[pixel_offset]); // R
                    out.push(pixel_buf[pixel_offset + 1]); // G
                    out.push(pixel_buf[pixel_offset + 2]); // B
                    out.push(pixel_buf[pixel_offset + 3]); // A
                }
            }
            linear += 1;
        }
        out
    };

    output.truncate(expected_size);

    Ok(DecodedPixels {
        data: output,
        source_name,
        data_size: expected_size,
    })
}

/// Decode a .rts.png to a temp file and return the path.
/// This is used by the QEMU bridge to pass pixel-decoded kernels.
pub fn decode_rts_to_temp(path: &str) -> Result<String, String> {
    let decoded = decode_rts_png(path)?;

    // Create temp file
    let temp_dir = std::env::temp_dir();
    let basename = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "decoded".to_string());
    let temp_name = format!("geo_qemu_{}", basename);
    let temp_path = temp_dir.join(&temp_name);

    std::fs::write(&temp_path, &decoded.data)
        .map_err(|e| format!("Failed to write temp file: {}", e))?;

    Ok(temp_path.to_string_lossy().to_string())
}

/// Check if a path looks like a .rts.png file
pub fn is_rts_png(path: &str) -> bool {
    path.to_lowercase().ends_with(".rts.png")
}

/// Hilbert curve: distance -> (x, y)
pub(crate) fn d2xy(grid_order: u32, d: u32) -> (u32, u32) {
    let mut x: u32 = 0;
    let mut y: u32 = 0;

    for s in 0..grid_order {
        let shift = 2 * s;
        let rx = (d >> shift) & 1;
        let ry = ((d >> shift) >> 1) & 1;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_d2xy_origin() {
        assert_eq!(d2xy(1, 0), (0, 0));
        assert_eq!(d2xy(2, 0), (0, 0));
        assert_eq!(d2xy(3, 0), (0, 0));
    }

    #[test]
    fn test_d2xy_order1_all_points() {
        // Order 1: 4 points, all within 2x2 grid
        for d in 0..4 {
            let (x, y) = d2xy(1, d);
            assert!(x < 2 && y < 2, "d2xy(1, {}) = ({}, {}) out of bounds", d, x, y);
        }
        // All 4 points should be unique
        let mut seen = std::collections::HashSet::new();
        for d in 0..4 {
            let (x, y) = d2xy(1, d);
            assert!(seen.insert((x, y)), "duplicate point ({}, {}) at d={}", x, y, d);
        }
        assert_eq!(seen.len(), 4);
    }

    #[test]
    fn test_d2xy_order2_continuity() {
        // Order 2: 16 points, all must be within 4x4 grid and unique
        for d in 0..16 {
            let (x, y) = d2xy(2, d);
            assert!(x < 4 && y < 4, "d2xy(2, {}) = ({}, {}) out of bounds", d, x, y);
        }
        // All points must be unique
        let mut seen = std::collections::HashSet::new();
        for d in 0..16 {
            let p = d2xy(2, d);
            assert!(seen.insert(p), "duplicate point {:?} at d={}", p, d);
        }
        assert_eq!(seen.len(), 16);
    }

    #[test]
    fn test_d2xy_order3_coverage() {
        // Order 3: 64 points, all must be within 8x8 grid
        let mut seen = std::collections::HashSet::new();
        for d in 0..64 {
            let (x, y) = d2xy(3, d);
            assert!(x < 8 && y < 8, "d2xy(3, {}) = ({}, {}) out of bounds", d, x, y);
            assert!(seen.insert((x, y)), "duplicate point ({}, {}) at d={}", x, y, d);
        }
        assert_eq!(seen.len(), 64);
    }

    #[test]
    fn test_d2xy_order4_coverage() {
        // Order 4: 256 points, all must be within 16x16 grid
        let mut seen = std::collections::HashSet::new();
        for d in 0..256 {
            let (x, y) = d2xy(4, d);
            assert!(x < 16 && y < 16, "d2xy(4, {}) = ({}, {}) out of bounds", d, x, y);
            assert!(seen.insert((x, y)), "duplicate point ({}, {}) at d={}", x, y, d);
        }
        assert_eq!(seen.len(), 256);
    }

    #[test]
    fn test_d2xy_known_values_order1() {
        // Known Hilbert curve order 1 pattern: (0,0) (1,0) (0,1) (1,1)
        assert_eq!(d2xy(1, 0), (0, 0));
        assert_eq!(d2xy(1, 1), (1, 0));
        assert_eq!(d2xy(1, 2), (0, 1));
        assert_eq!(d2xy(1, 3), (1, 1));
    }

    #[test]
    fn test_is_rts_png() {
        assert!(is_rts_png("kernel.rts.png"));
        assert!(is_rts_png("test.RTS.PNG"));
        assert!(is_rts_png("/path/to/file.rts.png"));
        assert!(!is_rts_png("kernel.png"));
        assert!(!is_rts_png("kernel.rts"));
        assert!(!is_rts_png("rts.png.txt"));
        assert!(!is_rts_png(""));
    }

    #[test]
    fn test_d2xy_large_order() {
        // Order 5: 1024 points, spot-check bounds
        let (x, y) = d2xy(5, 0);
        assert_eq!(x, 0);
        assert_eq!(y, 0);
        let (x, y) = d2xy(5, 1023);
        assert!(x < 32);
        assert!(y < 32);
    }
}
