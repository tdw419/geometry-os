// Siphon Demo — Watch Linux Desktop Through Geometry OS
//
// This reads a region of /dev/fb0 and displays it as pixel data.
// Use it to test the siphon before integrating with the full agent.
//
// Usage:
//   sudo ./target/release/siphon-demo
//   sudo ./target/release/siphon-demo --mouse  # Track mouse cursor

use std::time::{Duration, Instant};
use std::env;

mod siphon;
use siphon::FramebufferSiphon;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let track_mouse = args.contains(&"--mouse".to_string());
    
    println!("╔═══════════════════════════════════════════════════════╗");
    println!("║     GEOMETRY OS — Framebuffer Siphon Demo             ║");
    println!("╚═══════════════════════════════════════════════════════╝");
    println!();
    
    // Create siphon for /dev/fb0 (1920x1080 typical)
    let mut siphon = FramebufferSiphon::new("/dev/fb0", 1920, 1080);
    
    // Configure to siphon upper-left 100x100 region
    siphon.configure(
        0,      // source_x: top-left corner
        0,      // source_y: top-left corner
        100,    // source_width
        100,    // source_height
        10,     // target_x in foundry
        10,     // target_y in foundry
    );
    
    println!("Opening /dev/fb0...");
    siphon.open()?;
    println!("✓ Framebuffer mapped");
    println!();
    
    println!("Siphon configuration:");
    println!("  Source: ({}, {}) {}x{}", 
        siphon.source_x, siphon.source_y, siphon.source_width, siphon.source_height);
    println!("  Target: ({}, {}) in foundry", siphon.target_x, siphon.target_y);
    println!();
    
    if track_mouse {
        println!("Mode: Mouse tracking");
        println!("      Watching for brightest pixel in siphon region");
    } else {
        println!("Mode: Region sampling");
        println!("      Will display pixel statistics");
    }
    println!();
    println!("Press Ctrl+C to stop");
    println!("─────────────────────────────────────────────────────────");
    println!();
    
    let mut prev_samples: Vec<(u32, u32, u8, u8, u8)> = Vec::new();
    let mut frame = 0u32;
    let start = Instant::now();
    
    loop {
        let samples = siphon.sample_region();
        
        if track_mouse {
            // Track mouse by finding brightest pixel
            if let Some((mx, my)) = siphon.track_mouse(&samples) {
                let rel_x = (mx as f64 / siphon.source_width as f64 * 100.0) as u32;
                let rel_y = (my as f64 / siphon.source_height as f64 * 100.0) as u32;
                
                print!("\r Frame {}: Mouse at ({:>3}, {:>3}) relative to siphon region  ", 
                    frame, rel_x, rel_y);
            } else {
                print!("\r Frame {}: No bright pixel detected                          ", frame);
            }
        } else {
            // Show region statistics
            let pixel_count = samples.len();
            
            // Calculate average color
            let avg_r = if pixel_count > 0 {
                samples.iter().map(|(_, _, r, _, _)| *r as u32).sum::<u32>() / pixel_count as u32
            } else { 0 };
            let avg_g = if pixel_count > 0 {
                samples.iter().map(|(_, _, _, g, _)| *g as u32).sum::<u32>() / pixel_count as u32
            } else { 0 };
            let avg_b = if pixel_count > 0 {
                samples.iter().map(|(_, _, _, _, b)| *b as u32).sum::<u32>() / pixel_count as u32
            } else { 0 };
            
            // Detect motion
            let motion = siphon.detect_motion(&prev_samples, &samples, 50);
            
            print!("\r Frame {:4}: {:5} pixels, avg RGB({:3},{:3},{:3}), motion: {:3}  ",
                frame, pixel_count, avg_r, avg_g, avg_b, motion.len());
        }
        
        use std::io::Write;
        std::io::stdout().flush()?;
        
        prev_samples = samples;
        frame += 1;
        
        // Sample at ~10 Hz
        std::thread::sleep(Duration::from_millis(100));
        
        // Run for 1000 frames (~100 seconds)
        if frame >= 1000 {
            break;
        }
    }
    
    println!();
    println!();
    let elapsed = start.elapsed();
    println!("─────────────────────────────────────────────────────────");
    println!("✓ Done");
    println!("  Frames: {}", frame);
    println!("  Time: {:.2}s", elapsed.as_secs_f64());
    println!("  FPS: {:.2}", frame as f64 / elapsed.as_secs_f64());
    
    Ok(())
}
