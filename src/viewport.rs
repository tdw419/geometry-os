//! Viewport coordinate transform for Phase 107: Infinite Spatial Desktop.
//!
//! Converts between world-space (infinite tile grid) and screen-space (256x256 framebuffer).
//! The camera position lives in RAM[0x7800] (cam_x) and RAM[0x7801] (cam_y) as tile coords.
//!
//! Zoom levels (same as main.rs fullscreen map zoom):
//!   0: 256px source, 2x scale  -> 32 tiles visible (8px/tile on screen)
//!   1: 256px source, 3x scale  -> 32 tiles (6.67px/tile)
//!   2: 128px source, 6x scale  -> 16 tiles (16px/tile) [default]
//!   3:  64px source, 12x scale ->  8 tiles (32px/tile)
//!   4:  32px source, 24x scale ->  4 tiles (64px/tile)

/// Tile size in VM pixels (8x8 pixels per tile, matching world_desktop.asm).
pub const TILE_SIZE: u32 = 8;

/// Framebuffer dimensions.
pub const FB_W: u32 = 256;
pub const FB_H: u32 = 256;

/// Zoom level descriptor.
#[derive(Debug, Clone, Copy)]
pub struct ZoomLevel {
    /// Source region in VM pixels (how many VM pixels of the 256x256 we show).
    #[allow(dead_code)]
    pub src_region: u32,
    /// Scale factor (host pixels per VM pixel).
    pub scale: u32,
}

impl ZoomLevel {
    pub fn from_level(level: u32) -> Self {
        match level {
            0 => ZoomLevel {
                src_region: 256,
                scale: 2,
            },
            1 => ZoomLevel {
                src_region: 256,
                scale: 3,
            },
            2 => ZoomLevel {
                src_region: 128,
                scale: 6,
            },
            3 => ZoomLevel {
                src_region: 64,
                scale: 12,
            },
            4 => ZoomLevel {
                src_region: 32,
                scale: 24,
            },
            _ => ZoomLevel {
                src_region: 128,
                scale: 6,
            },
        }
    }

    /// Pixels per tile on screen at this zoom level.
    #[allow(dead_code)]
    pub fn pixels_per_tile(&self) -> u32 {
        TILE_SIZE * self.scale
    }

    /// Number of tiles visible across the framebuffer at this zoom.
    #[allow(dead_code)]
    pub fn tiles_visible(&self) -> u32 {
        self.src_region / TILE_SIZE
    }
}

/// Viewport state: camera position + zoom.
#[derive(Debug, Clone)]
pub struct Viewport {
    /// Camera X in tile coordinates.
    pub cam_x: i32,
    /// Camera Y in tile coordinates.
    pub cam_y: i32,
    /// Zoom level (0-4).
    pub zoom: ZoomLevel,
}

impl Viewport {
    pub fn new(cam_x: i32, cam_y: i32, zoom_level: u32) -> Self {
        Viewport {
            cam_x,
            cam_y,
            zoom: ZoomLevel::from_level(zoom_level),
        }
    }

    /// Convert world tile coordinates to framebuffer pixel coordinates.
    /// Returns None if the position is outside the visible viewport.
    #[allow(dead_code)]
    pub fn world_to_screen(&self, world_x: i32, world_y: i32) -> Option<(i32, i32)> {
        let px_per_tile = self.zoom.pixels_per_tile() as i32;
        let sx = (world_x - self.cam_x) * px_per_tile;
        let sy = (world_y - self.cam_y) * px_per_tile;
        // Check if within framebuffer bounds (with some margin for partial visibility)
        if sx + px_per_tile <= 0 || sy + px_per_tile <= 0 || sx >= FB_W as i32 || sy >= FB_H as i32
        {
            None
        } else {
            Some((sx, sy))
        }
    }

    /// Convert world tile coordinates to framebuffer pixels, always returning a value.
    /// Useful for rendering where partial visibility is acceptable.
    #[allow(dead_code)]
    pub fn world_to_screen_unchecked(&self, world_x: i32, world_y: i32) -> (i32, i32) {
        let px_per_tile = self.zoom.pixels_per_tile() as i32;
        (
            (world_x - self.cam_x) * px_per_tile,
            (world_y - self.cam_y) * px_per_tile,
        )
    }

    /// Convert world tile coordinates + pixel offset within a tile to screen pixels.
    /// This handles the sub-tile positioning that world-space windows need.
    /// `world_x` and `world_y` can be multiplied by TILE_SIZE to get pixel offsets.
    pub fn world_pixels_to_screen(&self, world_pixel_x: i32, world_pixel_y: i32) -> (i32, i32) {
        let scale = self.zoom.scale as i32;
        let cam_px = self.cam_x * TILE_SIZE as i32;
        let cam_py = self.cam_y * TILE_SIZE as i32;
        (
            (world_pixel_x - cam_px) * scale,
            (world_pixel_y - cam_py) * scale,
        )
    }

    /// Check if a rectangle in world-pixel coordinates is visible on screen.
    /// Returns true if any part of the rectangle overlaps the framebuffer.
    pub fn is_rect_visible(&self, world_px: i32, world_py: i32, width: u32, height: u32) -> bool {
        let (sx, sy) = self.world_pixels_to_screen(world_px, world_py);
        let sw = (width as i32) * (self.zoom.scale as i32);
        let sh = (height as i32) * (self.zoom.scale as i32);
        sx + sw > 0 && sy + sh > 0 && sx < FB_W as i32 && sy < FB_H as i32
    }

    /// Convert screen pixel coordinates to world tile coordinates.
    #[allow(dead_code)]
    pub fn screen_to_world(&self, screen_x: i32, screen_y: i32) -> (i32, i32) {
        let px_per_tile = self.zoom.pixels_per_tile() as i32;
        (
            self.cam_x + screen_x / px_per_tile,
            self.cam_y + screen_y / px_per_tile,
        )
    }

    /// Read viewport state from VM RAM.
    /// Uses RAM[0x7800]=cam_x, RAM[0x7801]=cam_y, and the given zoom level.
    pub fn from_ram(ram: &[u32], zoom_level: u32) -> Self {
        let cam_x = ram.get(0x7800).copied().unwrap_or(0) as i32;
        let cam_y = ram.get(0x7801).copied().unwrap_or(0) as i32;
        Viewport::new(cam_x, cam_y, zoom_level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zoom_levels() {
        let z0 = ZoomLevel::from_level(0);
        assert_eq!(z0.src_region, 256);
        assert_eq!(z0.scale, 2);
        assert_eq!(z0.pixels_per_tile(), 16); // 8*2
        assert_eq!(z0.tiles_visible(), 32);

        let z2 = ZoomLevel::from_level(2);
        assert_eq!(z2.src_region, 128);
        assert_eq!(z2.scale, 6);
        assert_eq!(z2.pixels_per_tile(), 48); // 8*6
        assert_eq!(z2.tiles_visible(), 16);

        let z4 = ZoomLevel::from_level(4);
        assert_eq!(z4.pixels_per_tile(), 192); // 8*24
        assert_eq!(z4.tiles_visible(), 4);
    }

    #[test]
    fn test_world_to_screen_centered() {
        let vp = Viewport::new(10, 10, 2); // zoom 2: 48px per tile
                                           // Camera at (10,10), world tile (10,10) should be at screen (0,0)
        let (sx, sy) = vp.world_to_screen_unchecked(10, 10);
        assert_eq!(sx, 0);
        assert_eq!(sy, 0);
    }

    #[test]
    fn test_world_to_screen_offset() {
        let vp = Viewport::new(10, 10, 2); // 48px per tile
        let (sx, sy) = vp.world_to_screen_unchecked(12, 11);
        assert_eq!(sx, 96); // (12-10)*48
        assert_eq!(sy, 48); // (11-10)*48
    }

    #[test]
    fn test_world_to_screen_visible() {
        let vp = Viewport::new(10, 10, 2);
        // Tile at (10,10) -> screen (0,0), visible
        assert!(vp.world_to_screen(10, 10).is_some());
        // Tile far offscreen
        assert!(vp.world_to_screen(100, 100).is_none());
    }

    #[test]
    fn test_world_pixels_to_screen() {
        let vp = Viewport::new(10, 10, 2); // scale=6
                                           // World pixel (10*8+4, 10*8+2) = (84, 82)
        let (sx, sy) = vp.world_pixels_to_screen(84, 82);
        // cam_px = 10*8 = 80
        // (84-80)*6 = 24, (82-80)*6 = 12
        assert_eq!(sx, 24);
        assert_eq!(sy, 12);
    }

    #[test]
    fn test_is_rect_visible() {
        let vp = Viewport::new(10, 10, 2);
        // Window at world pixel (10*8, 10*8), size 64x48, should be visible
        assert!(vp.is_rect_visible(80, 80, 64, 48));
        // Window far offscreen
        assert!(!vp.is_rect_visible(800, 800, 64, 48));
    }

    #[test]
    fn test_screen_to_world() {
        let vp = Viewport::new(10, 10, 2); // 48px per tile
        let (wx, wy) = vp.screen_to_world(96, 48);
        assert_eq!(wx, 12); // 10 + 96/48
        assert_eq!(wy, 11); // 10 + 48/48
    }

    #[test]
    fn test_from_ram() {
        let mut ram = vec![0u32; 0x10000];
        ram[0x7800] = 42;
        ram[0x7801] = 17;
        let vp = Viewport::from_ram(&ram, 2);
        assert_eq!(vp.cam_x, 42);
        assert_eq!(vp.cam_y, 17);
        assert_eq!(vp.zoom.scale, 6);
    }

    #[test]
    fn test_zoom_fallback() {
        let z = ZoomLevel::from_level(99);
        assert_eq!(z.src_region, 128); // defaults to zoom 2
        assert_eq!(z.scale, 6);
    }
}
