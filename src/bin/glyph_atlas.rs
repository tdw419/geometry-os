// Glyph Atlas — 3x3 Visual Patterns
//
// Phase 9 Alpha: Symbolic Communication
// Agents drop structured patterns instead of single pixels

/// 3x3 glyph patterns (9 bits, row-major order)
/// Each bit represents a pixel: 1 = filled, 0 = empty
#[derive(Debug, Clone, Copy)]
pub struct GlyphAtlas;

impl GlyphAtlas {
    // Glyph IDs and their bit patterns
    pub const EMPTY: u32 = 0b000_000_000;    // ...
    pub const FILE: u32 = 0b000_110_110;     // .■■
    pub const FOLDER: u32 = 0b111_100_111;   // ▣▣▣
    pub const EXEC: u32 = 0b010_111_010;     // ◈◈◈
    pub const DATA: u32 = 0b000_010_000;     // ●●●
    pub const LINK: u32 = 0b001_001_110;     // →→→
    pub const ALERT: u32 = 0b010_010_000;    // !!!
    pub const CHECK: u32 = 0b000_100_110;    // ✓✓✓
    pub const CROSS: u32 = 0b101_010_101;    // ✗✗✗

    /// All glyphs in an array for easy indexing
    pub const GLYPHS: [u32; 9] = [
        Self::EMPTY,
        Self::FILE,
        Self::FOLDER,
        Self::EXEC,
        Self::DATA,
        Self::LINK,
        Self::ALERT,
        Self::CHECK,
        Self::CROSS,
    ];

    /// Get the bit value at position (x, y) within a glyph
    /// x, y ∈ {0, 1, 2}
    pub fn get_pixel(glyph_id: u32, x: usize, y: usize) -> bool {
        let idx = y * 3 + x;
        let glyph = Self::GLYPHS[glyph_id as usize % 9];
        (glyph >> idx) & 1 == 1
    }

    /// Get all pixel positions for a glyph (relative to center)
    /// Returns vec of (dx, dy) offsets where pixels should be drawn
    pub fn get_pixels(glyph_id: u32) -> Vec<(i32, i32)> {
        let mut pixels = Vec::new();
        for y in 0..3 {
            for x in 0..3 {
                if Self::get_pixel(glyph_id, x, y) {
                    // Convert to offset from center (-1, 0, 1)
                    let dx = x as i32 - 1;
                    let dy = y as i32 - 1;
                    pixels.push((dx, dy));
                }
            }
        }
        pixels
    }

    /// Render glyph as ASCII art (for debugging)
    pub fn to_ascii(glyph_id: u32) -> String {
        let mut s = String::new();
        for y in 0..3 {
            for x in 0..3 {
                if Self::get_pixel(glyph_id, x, y) {
                    s.push('■');
                } else {
                    s.push('·');
                }
            }
            s.push('\n');
        }
        s
    }

    /// Get glyph name
    pub fn name(glyph_id: u32) -> &'static str {
        match glyph_id % 9 {
            0 => "Empty",
            1 => "File",
            2 => "Folder",
            3 => "Exec",
            4 => "Data",
            5 => "Link",
            6 => "Alert",
            7 => "Check",
            8 => "Cross",
            _ => "Unknown",
        }
    }
}

fn main() {
    // Demo: print all glyphs
    for i in 0..9 {
        println!("Glyph {}: {}", GlyphAtlas::name(i as u32));
        print!("{}", GlyphAtlas::to_ascii(i as u32));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_glyph() {
        let pixels = GlyphAtlas::get_pixels(GlyphAtlas::FILE);
        assert!(pixels.contains(&(0, 0)));  // Center
        assert!(pixels.contains(&(1, 0)));  // Right
        assert!(pixels.contains(&(0, 1)));  // Bottom
        assert!(pixels.contains(&(1, 1)));  // Bottom-right
        assert_eq!(pixels.len(), 4);
    }

    #[test]
    fn test_exec_glyph() {
        let pixels = GlyphAtlas::get_pixels(GlyphAtlas::EXEC);
        // Cross pattern
        assert!(pixels.contains(&(0, -1))); // Top
        assert!(pixels.contains(&(-1, 0))); // Left
        assert!(pixels.contains(&(0, 0)));  // Center
        assert!(pixels.contains(&(1, 0)));  // Right
        assert!(pixels.contains(&(0, 1)));  // Bottom
        assert_eq!(pixels.len(), 5);
    }

    #[test]
    fn test_ascii_output() {
        let ascii = GlyphAtlas::to_ascii(GlyphAtlas::FILE);
        assert!(ascii.contains('■'));
        assert!(ascii.contains('·'));
    }
}

fn main() {
    // Demo: Print all glyphs
    println!("=== GLYPH ATLAS ===\n");
    for i in 0..9 {
        println!("{} (ID {}):", GlyphAtlas::name(i), i);
        print!("{}", GlyphAtlas::to_ascii(i));
        println!("Pixels: {:?}", GlyphAtlas::get_pixels(i));
        println!();
    }
}
