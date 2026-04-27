// riscv/framebuf.rs -- MMIO Framebuffer Device
//
// 256x256 RGBA pixel framebuffer mapped at 0x6000_0000.
// Guest reads/writes pixels directly via load/store -- zero ecall overhead.
// This is the "pixel-native" bridge: RISC-V programs draw to the canonical screen.
//
// Memory layout:
//   0x6000_0000 .. 0x603F_FFFF : pixel data (256 * 256 * 4 = 262,144 bytes)
//     Each pixel is 32-bit: bits [31:24]=R, [23:16]=G, [15:8]=B, [7:0]=A
//     Pixel at (x, y) is at offset (y * 256 + x) * 4
//   0x6040_0000 : control register
//     Write 1 to flush/signal present

use std::cell::RefCell;
use std::rc::Rc;

/// MMIO base address for the framebuffer.
pub const FB_BASE: u64 = 0x6000_0000;
/// Size of the pixel buffer in bytes (256 * 256 * 4).
pub const FB_PIXEL_SIZE: usize = 256 * 256 * 4;
/// Control register address (immediately after pixel buffer).
pub const FB_CONTROL_ADDR: u64 = FB_BASE + FB_PIXEL_SIZE as u64;
/// Total MMIO range size (pixel buffer + control register).
pub const FB_TOTAL_SIZE: u64 = FB_PIXEL_SIZE as u64 + 4;

/// Convert a framebuffer pixel from SBI format (0xRRGGBBAA) to minifb format (0x00RRGGBB).
///
/// The SBI contract exposes pixels as 0xRRGGBBAA (alpha in low byte).
/// minifb's `update_with_buffer` expects 0x00RRGGBB. Every consumer that
/// blits framebuffer pixels to a minifb buffer should call this instead of
/// shifting manually -- so the next render site doesn't independently forget.
#[inline]
pub fn pixel_to_minifb(rgba: u32) -> u32 {
    rgba >> 8
}

/// Framebuffer width in pixels.
pub const FB_WIDTH: usize = 256;
/// Framebuffer height in pixels.
pub const FB_HEIGHT: usize = 256;

/// Callback type fired when guest writes to the control register (fb_present).
/// Receives a reference to the pixel buffer for display sync.
pub type PresentCallback = Rc<RefCell<dyn FnMut(&[u32])>>;

/// MMIO Framebuffer device.
pub struct Framebuffer {
    /// 256x256 RGBA pixel buffer (256KB).
    pub pixels: Vec<u32>,
    /// Set when guest writes 1 to the control register.
    pub present_flag: bool,
    /// Optional callback fired on fb_present for live display bridge.
    /// When the guest writes 1 to the control register, this callback
    /// is invoked with the current pixel buffer so the host can sync
    /// to its display surface (Geometry OS screen, PNG dump, etc).
    pub on_present: Option<PresentCallback>,
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Framebuffer {
    pub fn new() -> Self {
        Self {
            pixels: vec![0u32; FB_WIDTH * FB_HEIGHT],
            present_flag: false,
            on_present: None,
        }
    }

    /// Create a framebuffer with a live present callback.
    pub fn with_callback(cb: PresentCallback) -> Self {
        Self {
            pixels: vec![0u32; FB_WIDTH * FB_HEIGHT],
            present_flag: false,
            on_present: Some(cb),
        }
    }

    /// Check if a physical address falls within the framebuffer MMIO range.
    pub fn contains(addr: u64) -> bool {
        addr >= FB_BASE && addr < FB_BASE + FB_TOTAL_SIZE
    }

    /// Read a 32-bit word from the framebuffer.
    pub fn read(&self, addr: u64) -> Option<u32> {
        if addr >= FB_CONTROL_ADDR {
            // Control register: return present flag
            return Some(if self.present_flag { 1 } else { 0 });
        }
        let offset = addr.checked_sub(FB_BASE)? as usize;
        if offset >= FB_PIXEL_SIZE {
            return None;
        }
        let pixel_idx = offset / 4;
        if pixel_idx < self.pixels.len() {
            Some(self.pixels[pixel_idx])
        } else {
            None
        }
    }

    /// Write a 32-bit word to the framebuffer.
    pub fn write(&mut self, addr: u64, val: u32) {
        if addr >= FB_CONTROL_ADDR {
            // Control register: bit 0 = present/flush
            if val & 1 != 0 {
                self.present_flag = true;
                // Fire the live display callback if registered
                if let Some(ref cb) = self.on_present {
                    cb.borrow_mut()(&self.pixels);
                }
            }
            return;
        }
        let offset = match addr.checked_sub(FB_BASE) {
            Some(o) => o as usize,
            None => return,
        };
        if offset >= FB_PIXEL_SIZE {
            return;
        }
        let pixel_idx = offset / 4;
        if pixel_idx < self.pixels.len() {
            self.pixels[pixel_idx] = val;
        }
    }
}
