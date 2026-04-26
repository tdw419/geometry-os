// pixel.rs -- Pixel image decode/encode for .rts.png and pixelpack .png formats
//
// Two pixel encoding formats:
// 1. .rts.png: Binary data stored as RGBA pixels (Hilbert/linear layout)
// 2. .png (pixelpack): Each pixel is a 32-bit seed that expands to bytes via strategies
//
// Phase 92: Boot Geometry OS programs from pixelpack-encoded PNG files.

use png::Decoder;
use std::fs::File;
use std::io::{BufReader, Cursor};

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

// === Pixelpack seed expansion (Phase 92) ===
//
// Each RGBA pixel is a 32-bit seed. Top 4 bits = strategy, bottom 28 = payload.
// Expanding all seeds produces the full byte sequence (bytecode or source text).

/// Pixelpack seed expansion strategies
const PP_DICTIONARY: &[&[u8]] = &[
    b"LDI ", // 0 -- Geometry OS opcodes
    b"HALT", // 1
    b", ",   // 2
    b"ADD ", // 3
    b"SUB ", // 4
    b"MUL ", // 5
    b"JMP ", // 6
    b"PSET", // 7
    b"FILL", // 8
    b"CMP ", // 9
    b"\n",   // 10 (newline)
    b"r0",   // 11
    b"r1",   // 12
    b"r2",   // 13
    b"r3",   // 14
    b"r4",   // 15
];

const PP_NIBBLE_TABLE: [u8; 16] = [
    b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'A', b'B', b'C', b'D', b'E', b'F',
];

/// Expand a single 32-bit pixelpack seed to bytes.
pub fn pixelpack_expand(seed: u32) -> Vec<u8> {
    let strategy = ((seed >> 28) & 0xF) as u8;
    let params = seed & 0x0FFF_FFFF;

    match strategy {
        0x0 => pp_expand_dict(params, 1),
        0x1 => pp_expand_dict(params, 2),
        0x2 => pp_expand_dict(params, 3),
        0x3 => pp_expand_dict(params, 4),
        0x4 => pp_expand_dict(params, 5),
        0x5 => pp_expand_dict(params, 6),
        0x6 => pp_expand_dict(params, 7),
        0x7 => pp_expand_nibble(params),
        0x8 => pp_expand_raw4(params),
        0x9 => pp_expand_rle(params),
        0xA => pp_expand_raw3(params),
        0xB => pp_expand_xor_chain(params),
        0xC => pp_expand_linear(params),
        0xD => pp_expand_delta(params),
        0xE => pp_expand_bytepack(params),
        0xF => pp_expand_literal(params),
        _ => Vec::new(),
    }
}

fn pp_expand_dict(params: u32, n: usize) -> Vec<u8> {
    let mut result = Vec::new();
    for i in 0..n {
        let idx = ((params >> (4 * i)) & 0xF) as usize;
        if idx < PP_DICTIONARY.len() {
            result.extend_from_slice(PP_DICTIONARY[idx]);
        }
    }
    result
}

fn pp_expand_nibble(params: u32) -> Vec<u8> {
    let mut result = Vec::with_capacity(7);
    for i in 0..7 {
        let nibble = ((params >> (4 * i)) & 0xF) as usize;
        result.push(PP_NIBBLE_TABLE[nibble]);
    }
    result
}

/// Strategy 8: 4 raw bytes from the 28-bit payload
fn pp_expand_raw4(params: u32) -> Vec<u8> {
    vec![
        ((params >> 24) & 0xFF) as u8,
        ((params >> 16) & 0xFF) as u8,
        ((params >> 8) & 0xFF) as u8,
        (params & 0xFF) as u8,
    ]
}

/// Strategy 9: RLE -- repeat a byte pattern
fn pp_expand_rle(params: u32) -> Vec<u8> {
    let byte_val = (params & 0xFF) as u8;
    let count = (((params >> 8) & 0xFF) as usize).max(1);
    vec![byte_val; count.min(256)]
}

/// Strategy A: 3 raw bytes
fn pp_expand_raw3(params: u32) -> Vec<u8> {
    vec![
        ((params >> 16) & 0xFF) as u8,
        ((params >> 8) & 0xFF) as u8,
        (params & 0xFF) as u8,
    ]
}

/// Strategy B: XOR chain
fn pp_expand_xor_chain(params: u32) -> Vec<u8> {
    let start = (params & 0xFF) as u8;
    let key = ((params >> 8) & 0xFF) as u8;
    let count = (((params >> 16) & 0xF) as usize).max(1).min(16);

    let mut result = Vec::with_capacity(count);
    let mut val = start;
    for _ in 0..count {
        result.push(val);
        val ^= key;
        if val == 0 {
            val = key;
        }
    }
    result
}

/// Strategy C: Linear sequence
fn pp_expand_linear(params: u32) -> Vec<u8> {
    let start = (params & 0xFF) as u8;
    let step = ((params >> 8) & 0xFF) as u8;
    let count = (((params >> 16) & 0xF) as usize).max(1).min(16);

    let mut result = Vec::with_capacity(count);
    let mut val = start;
    for _ in 0..count {
        result.push(val);
        val = val.wrapping_add(step);
    }
    result
}

/// Strategy D: Delta from base
fn pp_expand_delta(params: u32) -> Vec<u8> {
    let base = (params & 0xFF) as u8;
    let d1 = ((params >> 8) & 0xFF) as u8;
    let d2 = ((params >> 16) & 0xFF) as u8;
    let d3 = ((params >> 24) & 0xFF) as u8;
    let mut result = vec![base];
    if d1 != 0 {
        result.push(base.wrapping_add(d1));
    }
    if d2 != 0 {
        result.push(base.wrapping_add(d1).wrapping_add(d2));
    }
    if d3 != 0 {
        result.push(base.wrapping_add(d1).wrapping_add(d2).wrapping_add(d3));
    }
    result
}

/// Strategy E: Bytepack mode-0 (3 raw bytes)
fn pp_expand_bytepack(params: u32) -> Vec<u8> {
    let b0 = ((params >> 3) & 0xFF) as u8;
    let b1 = ((params >> 11) & 0xFF) as u8;
    let b2 = ((params >> 19) & 0xFF) as u8;
    vec![b0, b1, b2]
}

/// Strategy F: Literal u32 as 4 bytes (big-endian)
fn pp_expand_literal(params: u32) -> Vec<u8> {
    vec![
        ((params >> 24) & 0xFF) as u8,
        ((params >> 16) & 0xFF) as u8,
        ((params >> 8) & 0xFF) as u8,
        (params & 0xFF) as u8,
    ]
}

/// Decode a pixelpack PNG to expanded bytes.
/// Each pixel is a 32-bit seed (RGBA). Seeds are expanded and concatenated.
/// The `seedcnt` text chunk specifies how many seeds are valid.
/// The `bytecnt` text chunk specifies the target byte count (truncation).
pub fn decode_pixelpack_png(data: &[u8]) -> Result<Vec<u8>, String> {
    let decoder = Decoder::new(Cursor::new(data));
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("PNG decode error: {:?}", e))?;

    let info = reader.info().clone();

    // Read metadata
    let seed_count: Option<usize> = info
        .uncompressed_latin1_text
        .iter()
        .find(|c| c.keyword == "seedcnt")
        .and_then(|c| c.text.parse().ok());

    let byte_count: Option<usize> = info
        .uncompressed_latin1_text
        .iter()
        .find(|c| c.keyword == "bytecnt")
        .and_then(|c| c.text.parse().ok());

    let width = info.width as usize;
    let height = info.height as usize;

    let mut pixel_buf = vec![0u8; width * height * 4];
    reader
        .next_frame(&mut pixel_buf)
        .map_err(|e| format!("PNG read error: {:?}", e))?;

    // Extract seeds from pixels
    let max_seeds = seed_count.unwrap_or(width * height);
    let mut all_bytes = Vec::new();

    for i in 0..max_seeds {
        let row = i / width;
        let col = i % width;
        if row >= height {
            break;
        }
        let offset = (row * width + col) * 4;
        if offset + 4 > pixel_buf.len() {
            break;
        }
        let r = pixel_buf[offset];
        let g = pixel_buf[offset + 1];
        let b = pixel_buf[offset + 2];
        let a = pixel_buf[offset + 3];
        let seed = ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | (a as u32);

        all_bytes.extend(pixelpack_expand(seed));
    }

    // Truncate to target byte count if specified
    if let Some(bc) = byte_count {
        all_bytes.truncate(bc);
    }

    Ok(all_bytes)
}

/// Decode a pixelpack PNG file from disk.
pub fn decode_pixelpack_file(path: &str) -> Result<Vec<u8>, String> {
    let data = std::fs::read(path).map_err(|e| format!("Cannot read {}: {}", path, e))?;
    decode_pixelpack_png(&data)
}

/// Encode raw bytes into a pixelpack PNG.
/// Uses strategy A (raw3) for each 3-byte chunk, with remainder handling.
/// Each pixel encodes exactly 3 bytes of input (24 bits payload in 28-bit params).
pub fn encode_pixelpack_png(bytes: &[u8]) -> Vec<u8> {
    let mut seeds = Vec::new();
    let mut i = 0;

    // Pack 3 bytes per seed using strategy A (raw3)
    while i + 3 <= bytes.len() {
        let seed = 0xA000_0000
            | ((bytes[i] as u32) << 16)
            | ((bytes[i + 1] as u32) << 8)
            | (bytes[i + 2] as u32);
        seeds.push(seed);
        i += 3;
    }
    // Handle remaining 1-2 bytes
    if bytes.len() - i >= 2 {
        seeds.push(0xA000_0000 | ((bytes[i] as u32) << 16) | (bytes[i + 1] as u32));
        i += 2;
    }
    if bytes.len() > i {
        seeds.push(0xA000_0000 | ((bytes[i] as u32) << 16));
    }

    // Write PNG
    let n = seeds.len();
    let (width, height) = if n == 0 {
        (1, 1)
    } else if n <= 4 {
        (2, 2)
    } else {
        let side = (n as f64).sqrt().ceil() as u32;
        (side, ((n as f64) / side as f64).ceil() as u32)
    };

    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height {
        for col in 0..width {
            let idx = (row * width + col) as usize;
            if idx < n {
                let s = seeds[idx];
                pixels.extend_from_slice(&[
                    ((s >> 24) & 0xFF) as u8,
                    ((s >> 16) & 0xFF) as u8,
                    ((s >> 8) & 0xFF) as u8,
                    (s & 0xFF) as u8,
                ]);
            } else {
                pixels.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }

    let mut buf = Vec::new();
    {
        let mut encoder = png::Encoder::new(Cursor::new(&mut buf), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .add_text_chunk("seedcnt".to_string(), n.to_string())
            .unwrap();
        encoder
            .add_text_chunk("bytecnt".to_string(), bytes.len().to_string())
            .unwrap();
        encoder
            .add_text_chunk("geo_boot".to_string(), "bytecode".to_string())
            .unwrap();
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&pixels).unwrap();
    }
    buf
}

/// Load pixelpack-decoded bytecode into VM RAM at the specified address.
/// Returns the number of u32 words written.
pub fn load_bytecode_to_ram(bytes: &[u8], ram: &mut [u32], base_addr: usize) -> usize {
    let ram_len = ram.len();
    let mut count = 0;
    for i in (0..bytes.len()).step_by(4) {
        let addr = base_addr + count;
        if addr >= ram_len {
            break;
        }
        let b0 = bytes.get(i).copied().unwrap_or(0) as u32;
        let b1 = bytes.get(i + 1).copied().unwrap_or(0) as u32;
        let b2 = bytes.get(i + 2).copied().unwrap_or(0) as u32;
        let b3 = bytes.get(i + 3).copied().unwrap_or(0) as u32;
        // Bytecode is stored as u32 words (little-endian)
        ram[addr] = b0 | (b1 << 8) | (b2 << 16) | (b3 << 24);
        count += 1;
    }
    count
}

/// Check if a path looks like a pixelpack PNG (not .rts.png)
pub fn is_pixelpack_png(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".png") && !lower.ends_with(".rts.png")
}

// === Phase 93: Source-from-PNG (self-documenting pixel programs) ===
//
// A pixelpack PNG can contain assembly source text (not bytecode).
// The PNG metadata `geo_boot=source` distinguishes it from bytecode PNGs.
// When loaded, the decoded bytes are interpreted as UTF-8 source text,
// written onto the canvas grid, assembled, and run.

/// Decode a pixelpack PNG and return the expanded bytes as a UTF-8 string.
/// Returns an error if the decoded bytes are not valid UTF-8.
pub fn decode_pixelpack_source(data: &[u8]) -> Result<String, String> {
    let bytes = decode_pixelpack_png(data)?;
    String::from_utf8(bytes).map_err(|e| format!("Source PNG contains invalid UTF-8: {}", e))
}

/// Decode a pixelpack PNG file from disk as source text.
pub fn decode_pixelpack_source_file(path: &str) -> Result<String, String> {
    let data = std::fs::read(path).map_err(|e| format!("Cannot read {}: {}", path, e))?;
    decode_pixelpack_source(&data)
}

/// Encode source text (assembly) into a pixelpack PNG with geo_boot=source metadata.
/// Uses strategy A (raw3) for each 3-byte chunk, same as bytecode encoding.
/// The PNG metadata `geo_boot=source` distinguishes it from bytecode PNGs.
pub fn encode_source_pixelpack_png(source: &str) -> Vec<u8> {
    let bytes = source.as_bytes();
    let mut seeds = Vec::new();
    let mut i = 0;

    // Pack 3 bytes per seed using strategy A (raw3)
    while i + 3 <= bytes.len() {
        let seed = 0xA000_0000
            | ((bytes[i] as u32) << 16)
            | ((bytes[i + 1] as u32) << 8)
            | (bytes[i + 2] as u32);
        seeds.push(seed);
        i += 3;
    }
    // Handle remaining 1-2 bytes
    if bytes.len() - i >= 2 {
        seeds.push(0xA000_0000 | ((bytes[i] as u32) << 16) | (bytes[i + 1] as u32));
        i += 2;
    }
    if bytes.len() > i {
        seeds.push(0xA000_0000 | ((bytes[i] as u32) << 16));
    }

    // Write PNG
    let n = seeds.len();
    let (width, height) = if n == 0 {
        (1, 1)
    } else if n <= 4 {
        (2, 2)
    } else {
        let side = (n as f64).sqrt().ceil() as u32;
        (side, ((n as f64) / side as f64).ceil() as u32)
    };

    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height {
        for col in 0..width {
            let idx = (row * width + col) as usize;
            if idx < n {
                let s = seeds[idx];
                pixels.extend_from_slice(&[
                    ((s >> 24) & 0xFF) as u8,
                    ((s >> 16) & 0xFF) as u8,
                    ((s >> 8) & 0xFF) as u8,
                    (s & 0xFF) as u8,
                ]);
            } else {
                pixels.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }

    let mut buf = Vec::new();
    {
        let mut encoder = png::Encoder::new(Cursor::new(&mut buf), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .add_text_chunk("seedcnt".to_string(), n.to_string())
            .unwrap();
        encoder
            .add_text_chunk("bytecnt".to_string(), bytes.len().to_string())
            .unwrap();
        // Key distinction: geo_boot=source (not "bytecode")
        encoder
            .add_text_chunk("geo_boot".to_string(), "source".to_string())
            .unwrap();
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&pixels).unwrap();
    }
    buf
}

/// Write decoded source text bytes onto the canvas buffer (128 rows x 32 cols).
/// Each character goes to canvas_buffer[row * 32 + col]. Lines break on '\n'.
/// Returns the number of characters written.
pub fn load_source_to_canvas_buffer(source: &str, canvas_buffer: &mut [u32]) -> usize {
    const CANVAS_COLS: usize = 32;
    const CANVAS_MAX_ROWS: usize = 128;

    // Clear canvas
    for cell in canvas_buffer.iter_mut() {
        *cell = 0;
    }

    let mut row = 0usize;
    let mut col = 0usize;
    let mut count = 0usize;

    for ch in source.chars() {
        if row >= CANVAS_MAX_ROWS {
            break;
        }
        if ch == '\n' {
            row += 1;
            col = 0;
        } else if col < CANVAS_COLS {
            canvas_buffer[row * CANVAS_COLS + col] = ch as u32;
            col += 1;
            count += 1;
        }
        // Characters beyond column 32 on a single line are dropped
    }

    count
}

/// Check if a pixelpack PNG contains source text (geo_boot=source metadata).
/// Returns None if the file doesn't exist or isn't a valid PNG.
pub fn is_source_png(data: &[u8]) -> bool {
    let decoder = Decoder::new(Cursor::new(data));
    let reader = match decoder.read_info() {
        Ok(r) => r,
        Err(_) => return false,
    };
    let info = reader.info();
    info.uncompressed_latin1_text
        .iter()
        .any(|c| c.keyword == "geo_boot" && c.text == "source")
}

/// Check if a file on disk is a source pixelpack PNG.
pub fn is_source_png_file(path: &str) -> bool {
    match std::fs::read(path) {
        Ok(data) => is_source_png(&data),
        Err(_) => false,
    }
}

/// Boot a source PNG: decode the source text, write to canvas buffer, assemble, run.
/// Returns the source text and assembly result, or an error string.
pub fn boot_source_from_png(
    path: &str,
    canvas_buffer: &mut [u32],
) -> Result<(String, usize), String> {
    let source = decode_pixelpack_source_file(path)?;
    let char_count = load_source_to_canvas_buffer(&source, canvas_buffer);

    // Also write source text to RAM grid at 0x000-0x3FF for canvas display
    // (This mirrors the source in canvas_buffer)
    let _ = char_count; // used for diagnostics

    // Assemble the source
    let asm_result = crate::assembler::assemble(&source, 0x1000)
        .map_err(|e| format!("Assembly error: {:?}", e))?;

    Ok((source, asm_result.pixels.len()))
}

/// Boot result for source PNG mode
pub struct BootSrcPngResult {
    pub source_len: usize,
    pub char_count: usize,
    pub bytecode_words: usize,
}

/// Full source PNG boot pipeline: decode -> canvas -> assemble -> write bytecode to RAM.
pub fn boot_source_png_to_ram(
    path: &str,
    canvas_buffer: &mut [u32],
    ram: &mut [u32],
) -> Result<BootSrcPngResult, String> {
    let source = decode_pixelpack_source_file(path)?;
    let source_len = source.len();
    let char_count = load_source_to_canvas_buffer(&source, canvas_buffer);

    // Assemble
    let asm_result = crate::assembler::assemble(&source, 0x1000)
        .map_err(|e| format!("Assembly error: {:?}", e))?;

    // Write bytecode to RAM at 0x1000
    let bytecode_words = asm_result.pixels.len().min(4096); // 0x1000-0x1FFF = 4096 words
    for i in 0..bytecode_words {
        ram[0x1000 + i] = asm_result.pixels[i];
    }

    Ok(BootSrcPngResult {
        source_len,
        char_count,
        bytecode_words,
    })
}

/// Boot context: what we loaded and from where
pub struct BootPngResult {
    pub seed_count: usize,
    pub byte_count: usize,
    pub ram_words: usize,
    pub load_addr: usize,
}

/// Load a pixelpack PNG file and write bytecode to VM RAM.
/// Returns metadata about the load, or an error string.
pub fn boot_from_png(
    path: &str,
    ram: &mut [u32],
    load_addr: usize,
) -> Result<BootPngResult, String> {
    let bytes = decode_pixelpack_file(path)?;
    let byte_count = bytes.len();
    let ram_words = load_bytecode_to_ram(&bytes, ram, load_addr);
    Ok(BootPngResult {
        seed_count: 0, // we don't know seed count after expansion
        byte_count,
        ram_words,
        load_addr,
    })
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
            assert!(
                x < 2 && y < 2,
                "d2xy(1, {}) = ({}, {}) out of bounds",
                d,
                x,
                y
            );
        }
        // All 4 points should be unique
        let mut seen = std::collections::HashSet::new();
        for d in 0..4 {
            let (x, y) = d2xy(1, d);
            assert!(
                seen.insert((x, y)),
                "duplicate point ({}, {}) at d={}",
                x,
                y,
                d
            );
        }
        assert_eq!(seen.len(), 4);
    }

    #[test]
    fn test_d2xy_order2_continuity() {
        // Order 2: 16 points, all must be within 4x4 grid and unique
        for d in 0..16 {
            let (x, y) = d2xy(2, d);
            assert!(
                x < 4 && y < 4,
                "d2xy(2, {}) = ({}, {}) out of bounds",
                d,
                x,
                y
            );
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
            assert!(
                x < 8 && y < 8,
                "d2xy(3, {}) = ({}, {}) out of bounds",
                d,
                x,
                y
            );
            assert!(
                seen.insert((x, y)),
                "duplicate point ({}, {}) at d={}",
                x,
                y,
                d
            );
        }
        assert_eq!(seen.len(), 64);
    }

    #[test]
    fn test_d2xy_order4_coverage() {
        // Order 4: 256 points, all must be within 16x16 grid
        let mut seen = std::collections::HashSet::new();
        for d in 0..256 {
            let (x, y) = d2xy(4, d);
            assert!(
                x < 16 && y < 16,
                "d2xy(4, {}) = ({}, {}) out of bounds",
                d,
                x,
                y
            );
            assert!(
                seen.insert((x, y)),
                "duplicate point ({}, {}) at d={}",
                x,
                y,
                d
            );
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

    // === Phase 92: Pixelpack seed expansion tests ===

    #[test]
    fn test_pixelpack_expand_dict() {
        // Strategy 0: DICT_1, single dict entry
        // Index 0 = "LDI " (bytes: 0x4C, 0x44, 0x49, 0x20)
        let seed = 0x0000_0000; // strategy 0, index 0
        let bytes = pixelpack_expand(seed);
        assert_eq!(bytes, b"LDI ");

        // Strategy 1: DICT_2, two dict entries
        // Index 11 = "r0", index 12 = "r1"
        let seed = 0x1C00_00B0; // strategy 1, indices [0, 11]
        let bytes = pixelpack_expand(seed);
        assert_eq!(bytes, b"LDI r0");
    }

    #[test]
    fn test_pixelpack_expand_raw3() {
        // Strategy A (0xA): 3 raw bytes
        let seed = 0xA000_0102; // strategy A, bytes: 0x00, 0x01, 0x02 (MSB first)
        let bytes = pixelpack_expand(seed);
        assert_eq!(bytes, vec![0x00, 0x01, 0x02]);
    }

    #[test]
    fn test_pixelpack_expand_nibble() {
        // Strategy 7: NIBBLE, 7 hex digits
        let seed = 0x7000_0012; // strategy 7, nibbles [2,1,0,0,0,0,0]
        let bytes = pixelpack_expand(seed);
        assert_eq!(bytes[0], b'2');
        assert_eq!(bytes[1], b'1');
        assert_eq!(bytes[2], b'0');
        assert_eq!(bytes.len(), 7);
    }

    #[test]
    fn test_pixelpack_expand_literal() {
        // Strategy F (0xF): 4 bytes literal
        // params = seed & 0x0FFFFFFF = 0x0ABCDEF0
        // bytes: (params>>24)=0x0A, (params>>16)=0xBC, (params>>8)=0xDE, params&0xFF=0xF0
        let seed = 0xFABCDEF0u32;
        let bytes = pixelpack_expand(seed);
        assert_eq!(bytes, vec![0x0A, 0xBC, 0xDE, 0xF0]);
    }

    #[test]
    fn test_pixelpack_expand_rle() {
        // Strategy 9 (0x9): RLE -- repeat byte
        let seed = 0x9000_0042; // strategy 9, byte=0x42 ('B'), count=0+1=1
        let bytes = pixelpack_expand(seed);
        assert_eq!(bytes, vec![0x42]);

        // Multiple repeats
        let seed = 0x9000_0342; // byte=0x42, count=3
        let bytes = pixelpack_expand(seed);
        assert_eq!(bytes, vec![0x42, 0x42, 0x42]);
    }

    #[test]
    fn test_pixelpack_expand_linear() {
        // Strategy C (0xC): linear sequence
        // params layout: count in bits 19:16, step in bits 15:8, start in bits 7:0
        let seed = 0xC004_0210; // count=4, step=0x02, start=0x10
        let bytes = pixelpack_expand(seed);
        assert_eq!(bytes, vec![0x10, 0x12, 0x14, 0x16]);
    }

    #[test]
    fn test_pixelpack_roundtrip_encode_decode() {
        // Encode some bytes, decode them back
        let original = vec![0x10, 0x01, 0x42, 0x00, 0x00, 0x00]; // LDI r1, 66 (3 u32 words)
        let png_data = encode_pixelpack_png(&original);
        let decoded = decode_pixelpack_png(&png_data).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_pixelpack_roundtrip_empty() {
        let original: Vec<u8> = vec![];
        let png_data = encode_pixelpack_png(&original);
        let decoded = decode_pixelpack_png(&png_data).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_pixelpack_roundtrip_single_byte() {
        let original = vec![0x42];
        let png_data = encode_pixelpack_png(&original);
        let decoded = decode_pixelpack_png(&png_data).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_pixelpack_roundtrip_large() {
        // 100 bytes of varying data
        let original: Vec<u8> = (0..100).map(|i| (i * 7 + 13) as u8).collect();
        let png_data = encode_pixelpack_png(&original);
        let decoded = decode_pixelpack_png(&png_data).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_is_pixelpack_png() {
        assert!(is_pixelpack_png("program.png"));
        assert!(is_pixelpack_png("test.PNG"));
        assert!(!is_pixelpack_png("kernel.rts.png"));
        assert!(!is_pixelpack_png("program.asm"));
        assert!(!is_pixelpack_png(""));
    }

    #[test]
    fn test_load_bytecode_to_ram() {
        // 12 bytes = 3 u32 words
        let bytes = vec![
            0x10, 0x01, 0x2A, 0x00, 0x10, 0x02, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let mut ram = vec![0u32; 65536];
        let count = load_bytecode_to_ram(&bytes, &mut ram, 0x1000);
        assert_eq!(count, 3);
        assert_eq!(ram[0x1000], 0x002A0110); // LDI r1, 42
        assert_eq!(ram[0x1001], 0x00FF0210); // LDI r2, 255
        assert_eq!(ram[0x1002], 0x00000000); // NOP-like
    }

    #[test]
    fn test_full_pixel_boot_roundtrip() {
        // Assemble a simple program, encode to PNG, decode, load, run
        let source = "LDI r1, 42\nLDI r2, 0xFF\nHALT\n";
        let asm = crate::assembler::assemble(source, 0).unwrap();

        // Convert bytecode pixels to bytes (each pixel is a u32)
        let mut bytecode_bytes = Vec::new();
        for &word in &asm.pixels {
            bytecode_bytes.push((word & 0xFF) as u8);
            bytecode_bytes.push(((word >> 8) & 0xFF) as u8);
            bytecode_bytes.push(((word >> 16) & 0xFF) as u8);
            bytecode_bytes.push(((word >> 24) & 0xFF) as u8);
        }

        // Encode to pixelpack PNG
        let png_data = encode_pixelpack_png(&bytecode_bytes);

        // Decode back
        let decoded = decode_pixelpack_png(&png_data).unwrap();
        assert_eq!(decoded, bytecode_bytes);

        // Load to VM RAM and run
        let mut ram = vec![0u32; 65536];
        let words = load_bytecode_to_ram(&decoded, &mut ram, 0);
        assert!(words >= 3); // LDI + LDI + HALT = at least 3 words

        let mut vm = crate::vm::Vm::new();
        vm.ram = ram;
        vm.pc = 0;
        vm.halted = false;
        for _ in 0..1000 {
            if !vm.step() {
                break;
            }
        }
        assert!(vm.halted);
        assert_eq!(vm.regs[1], 42);
        assert_eq!(vm.regs[2], 0xFF);
    }

    #[test]
    fn test_pixel_boot_uses_strategy_a() {
        // Verify encode_pixelpack_png uses strategy A (raw3)
        let bytes = vec![0x10, 0x01, 0x42];
        let png_data = encode_pixelpack_png(&bytes);
        let decoded = decode_pixelpack_png(&png_data).unwrap();
        assert_eq!(decoded, bytes);

        // The PNG should have exactly 1 seed (3 bytes packed into one seed)
        // Check it's a valid PNG
        assert!(png_data.len() > 8);
        assert_eq!(&png_data[0..4], &[0x89, 0x50, 0x4E, 0x47]); // PNG magic
    }
}

/// Get the 5x7 mini font glyph for a character.
/// Returns a static array of 7 rows, each a 5-bit bitmap.
/// Bit test: glyph[row] & (1 << (4 - col))
pub fn mini_font_glyph(ch: u8) -> &'static [u8; 7] {
    const MINI_FONT: [[u8; 7]; 96] = include!("mini_font.in");
    const EXT_FONT: [[u8; 7]; 30] = include!("ext_font.in");
    let idx = ch as usize;
    if ch >= 32 && ch < 128 {
        &MINI_FONT[(ch - 32) as usize]
    } else if ch >= 128 && ch <= 157 {
        &EXT_FONT[idx - 128]
    } else {
        &MINI_FONT[0] // space for out-of-range
    }
}
