// Glyph Test — Visual verification of glyph atlas
//
// Renders all 9 glyphs to verify legibility

use image::{ImageBuffer, Rgba};

mod glyph_atlas;
use glyph_atlas::GlyphAtlas;

const CELL_SIZE: u32 = 20;  // Pixels per glyph cell
const GRID_SIZE: u32 = 3;   // 3x3 grid
const PADDING: u32 = 10;

fn main() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║              PHASE 9 ALPHA — GLYPH TEST                  ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    // Calculate image size
    let cols = 3u32;  // 3 glyphs per row
    let rows = 3u32;  // 3 rows
    let width = cols * CELL_SIZE + (cols + 1) * PADDING;
    let height = rows * CELL_SIZE + (rows + 1) * PADDING;

    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);

    // Fill background
    for pixel in img.pixels_mut() {
        *pixel = Rgba([20, 25, 35, 255]);  // Dark blue-gray
    }

    // Render each glyph
    for glyph_id in 0..9 {
        let col = glyph_id % 3;
        let row = glyph_id / 3;

        let cell_x = PADDING + col * (CELL_SIZE + PADDING);
        let cell_y = PADDING + row * (CELL_SIZE + PADDING);

        // Choose color based on glyph type
        let color = match glyph_id {
            0 => Rgba([50, 50, 50, 255]),      // Empty (dark)
            1 => Rgba([100, 255, 100, 255]),   // File (green)
            2 => Rgba([255, 200, 100, 255]),   // Folder (yellow-orange)
            3 => Rgba([255, 100, 100, 255]),   // Exec (red)
            4 => Rgba([100, 100, 255, 255]),   // Data (blue)
            5 => Rgba([100, 255, 255, 255]),   // Link (cyan)
            6 => Rgba([255, 100, 255, 255]),   // Alert (magenta)
            7 => Rgba([100, 255, 100, 255]),   // Check (green)
            8 => Rgba([255, 80, 80, 255]),     // Cross (red)
            _ => Rgba([128, 128, 128, 255]),
        };

        // Get glyph pixels
        let pixels = GlyphAtlas::get_pixels(glyph_id);

        // Calculate scale (how many pixels per glyph pixel)
        let scale = CELL_SIZE / GRID_SIZE;

        // Render glyph
        for (dx, dy) in pixels {
            let px = cell_x + ((dx + 1) as u32 * scale);
            let py = cell_y + ((dy + 1) as u32 * scale);

            // Draw a small rectangle for each glyph pixel
            for sy in 0..scale {
                for sx in 0..scale {
                    let x = px + sx;
                    let y = py + sy;
                    if x < width && y < height {
                        img.put_pixel(x, y, color);
                    }
                }
            }
        }

        // Print glyph info
        println!("[{}] {}:", glyph_id, GlyphAtlas::name(glyph_id));
        print!("{}", GlyphAtlas::to_ascii(glyph_id));
    }

    // Save image
    std::fs::create_dir_all("output").ok();
    img.save("output/glyph_test.png").expect("Failed to save");

    println!();
    println!("✓ Rendered 9 glyphs to output/glyph_test.png");
    println!("  Size: {}x{} pixels", width, height);
    println!("  Cell size: {}x{} pixels", CELL_SIZE, CELL_SIZE);
}
