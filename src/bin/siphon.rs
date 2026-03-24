// Framebuffer Siphon — Linux Desktop → Geometry OS Bridge
// 
// Reads pixels from /dev/fb0 (Linux framebuffer) and injects them
// into the Geometry OS foundry as sensor data for pixel-agents.
//
// This creates a "parasitic" relationship where pixel-agents can
// "see" and react to the actual Linux desktop.

use std::fs::{OpenOptions, File};
use memmap2::MmapMut;

pub struct FramebufferSiphon {
    fb_path: String,
    width: u32,
    height: u32,
    bytes_per_pixel: u32,
    mmap: Option<MmapMut>,
    
    // Siphon configuration
    pub source_x: u32,      // X offset in Linux framebuffer
    pub source_y: u32,      // Y offset in Linux framebuffer
    pub source_width: u32,  // Width of region to siphon
    pub source_height: u32, // Height of region to siphon
    
    pub target_x: u32,      // X offset in Geometry OS foundry
    pub target_y: u32,      // Y offset in Geometry OS foundry
    sample_rate: u32,   // How often to sample (in frames)
}

impl FramebufferSiphon {
    pub fn new(fb_path: &str, width: u32, height: u32) -> Self {
        Self {
            fb_path: fb_path.to_string(),
            width,
            height,
            bytes_per_pixel: 4, // ARGB
            mmap: None,
            source_x: 0,
            source_y: 0,
            source_width: 100,
            source_height: 100,
            target_x: 0,
            target_y: 0,
            sample_rate: 1,
        }
    }
    
    pub fn open(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.fb_path)?;
        
        self.mmap = Some(unsafe { MmapMut::map_mut(&file)? });
        Ok(())
    }
    
    /// Configure siphon region
    pub fn configure(
        &mut self,
        source_x: u32,
        source_y: u32,
        source_width: u32,
        source_height: u32,
        target_x: u32,
        target_y: u32,
    ) {
        self.source_x = source_x;
        self.source_y = source_y;
        self.source_width = source_width;
        self.source_height = source_height;
        self.target_x = target_x;
        self.target_y = target_y;
    }
    
    /// Read a pixel from the Linux framebuffer
    pub fn read_pixel(&self, x: u32, y: u32) -> (u8, u8, u8) {
        if let Some(ref mmap) = self.mmap {
            let offset = ((y * self.width + x) * self.bytes_per_pixel) as usize;
            
            if offset + 3 < mmap.len() {
                // Assume ARGB or RGBA format
                let b = mmap[offset];
                let g = mmap[offset + 1];
                let r = mmap[offset + 2];
                return (r, g, b);
            }
        }
        (0, 0, 0)
    }
    
    /// Sample a region and convert to sensor data
    /// Returns a vector of (x, y, r, g, b) tuples
    pub fn sample_region(&self) -> Vec<(u32, u32, u8, u8, u8)> {
        let mut pixels = Vec::new();
        
        for dy in 0..self.source_height {
            for dx in 0..self.source_width {
                let src_x = self.source_x + dx;
                let src_y = self.source_y + dy;
                
                let (r, g, b) = self.read_pixel(src_x, src_y);
                
                // Only include non-black pixels (save bandwidth)
                if r > 10 || g > 10 || b > 10 {
                    pixels.push((dx, dy, r, g, b));
                }
            }
        }
        
        pixels
    }
    
    /// Convert sampled pixels to Geometry OS agent format
    /// Returns pixel data ready for injection into foundry
    pub fn to_agent_pixels(&self, samples: &[(u32, u32, u8, u8, u8)]) -> Vec<AgentPixel> {
        samples
            .iter()
            .map(|(dx, dy, r, g, b)| {
                AgentPixel {
                    x: self.target_x + dx,
                    y: self.target_y + dy,
                    opcode: 0x20, // OP_EMIT_SIGNAL
                    r: *r,
                    g: *g,
                    b: *b,
                }
            })
            .collect()
    }
    
    /// Detect motion by comparing two samples
    pub fn detect_motion(
        &self,
        prev: &[(u32, u32, u8, u8, u8)],
        curr: &[(u32, u32, u8, u8, u8)],
        threshold: u8,
    ) -> Vec<(u32, u32)> {
        let mut motion = Vec::new();
        
        // Create hash maps for fast lookup
        let prev_map: std::collections::HashMap<(u32, u32), (u8, u8, u8)> = prev
            .iter()
            .map(|(x, y, r, g, b)| ((*x, *y), (*r, *g, *b)))
            .collect();
        
        let curr_map: std::collections::HashMap<(u32, u32), (u8, u8, u8)> = curr
            .iter()
            .map(|(x, y, r, g, b)| ((*x, *y), (*r, *g, *b)))
            .collect();
        
        // Check for new pixels (appearance)
        for ((x, y), (r, g, b)) in &curr_map {
            if let Some((pr, pg, pb)) = prev_map.get(&(*x, *y)) {
                // Check for significant change
                let dr = (*r as i16 - *pr as i16).abs() as u8;
                let dg = (*g as i16 - *pg as i16).abs() as u8;
                let db = (*b as i16 - *pb as i16).abs() as u8;
                
                if dr > threshold || dg > threshold || db > threshold {
                    motion.push((*x, *y));
                }
            } else {
                // New pixel appeared
                motion.push((*x, *y));
            }
        }
        
        // Check for disappeared pixels
        for ((x, y), _) in &prev_map {
            if !curr_map.contains_key(&(*x, *y)) {
                motion.push((*x, *y));
            }
        }
        
        motion
    }
    
    /// Track mouse cursor by detecting motion in upper-left corner
    pub fn track_mouse(&self, samples: &[(u32, u32, u8, u8, u8)]) -> Option<(u32, u32)> {
        // Find the brightest cluster in the sampled region
        let mut max_brightness = 0u32;
        let mut brightest_pos: Option<(u32, u32)> = None;
        
        for (x, y, r, g, b) in samples {
            let brightness = (*r as u32) + (*g as u32) + (*b as u32);
            if brightness > max_brightness {
                max_brightness = brightness;
                brightest_pos = Some((*x, *y));
            }
        }
        
        brightest_pos
    }
}

#[derive(Clone, Copy)]
pub struct AgentPixel {
    pub x: u32,
    pub y: u32,
    pub opcode: u32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

// Integration with Geometry OS agent
impl FramebufferSiphon {
    /// Inject siphoned pixels into the agent's pixel buffer
    pub fn inject_into_buffer(&self, buffer: &mut [u32], buffer_width: u32) {
        let samples = self.sample_region();
        let agents = self.to_agent_pixels(&samples);
        
        for agent in agents {
            let idx = (agent.y * buffer_width + agent.x) as usize;
            if idx < buffer.len() {
                // Encode as RGBA where R=opcode, GBA=color
                buffer[idx] = (agent.opcode << 24) 
                    | ((agent.r as u32) << 16)
                    | ((agent.g as u32) << 8)
                    | (agent.b as u32);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_siphon_creation() {
        let siphon = FramebufferSiphon::new("/dev/fb0", 1920, 1080);
        assert_eq!(siphon.width, 1920);
        assert_eq!(siphon.height, 1080);
    }
    
    #[test]
    fn test_configure() {
        let mut siphon = FramebufferSiphon::new("/dev/fb0", 1920, 1080);
        siphon.configure(100, 100, 50, 50, 10, 10);
        
        assert_eq!(siphon.source_x, 100);
        assert_eq!(siphon.target_x, 10);
    }
}
