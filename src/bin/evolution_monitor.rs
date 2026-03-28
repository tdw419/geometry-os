// Evolution Monitor — Mission Control for Ouroboros
//
// Phase 10 Parallel: Read-only observer for the evolution loop
// Watches output files, renders with camera, saves snapshots

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};
use image::{ImageBuffer, Rgba};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;

// ============================================================================
// MONITOR STATE
// ============================================================================

#[derive(Debug)]
struct MonitorState {
    last_modified: SystemTime,
    current_iteration: u32,
    last_score: f32,
    snapshot_count: u32,
    is_running: bool,
}

impl MonitorState {
    fn new() -> Self {
        Self {
            last_modified: SystemTime::UNIX_EPOCH,
            current_iteration: 0,
            last_score: 0.0,
            snapshot_count: 0,
            is_running: false,
        }
    }
    
    fn check_for_evolution(&mut self) -> bool {
        let checkpoint_path = "output/ouroboros_checkpoint.json";
        
        if !Path::new(checkpoint_path).exists() {
            return false;
        }
        
        if let Ok(metadata) = fs::metadata(checkpoint_path) {
            if let Ok(modified) = metadata.modified() {
                if modified > self.last_modified {
                    self.last_modified = modified;
                    return true;
                }
            }
        }
        
        false
    }
    
    fn load_latest_state(&mut self) -> Option<(u32, f32)> {
        let checkpoint_path = "output/ouroboros_checkpoint.json";
        
        if !Path::new(checkpoint_path).exists() {
            return None;
        }
        
        if let Ok(content) = fs::read_to_string(checkpoint_path) {
            // Parse JSON to get iteration count and last score
            // Simple parsing - look for array length and last element
            let iterations = content.matches("\"iteration\":").count() as u32;
            
            // Try to extract last score
            let last_score = if let Some(last_bracket) = content.rfind("}]") {
                let last_obj = &content[..last_bracket + 1];
                if let Some(score_start) = last_obj.rfind("\"score\":") {
                    let score_str = &last_obj[score_start + 8..];
                    if let Some(score_end) = score_str.find(|c: char| !c.is_numeric() && c != '.' && c != '-') {
                        score_str[..score_end].parse::<f32>().unwrap_or(0.0)
                    } else {
                        0.0
                    }
                } else {
                    0.0
                }
            } else {
                0.0
            };
            
            self.current_iteration = iterations;
            self.last_score = last_score;
            
            return Some((iterations, last_score));
        }
        
        None
    }
    
    fn take_snapshot(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        fs::create_dir_all("snapshots")?;
        
        let snapshot_name = format!(
            "snapshots/evolution_iter{}_score{:.2}_{}.json",
            self.current_iteration,
            self.last_score,
            timestamp
        );
        
        fs::copy("output/ouroboros_checkpoint.json", &snapshot_name)?;
        
        // Also copy the latest frame if it exists
        let frame_source = "output/courier_swarm.png";
        if Path::new(frame_source).exists() {
            let frame_dest = format!(
                "snapshots/frame_iter{}_{}.png",
                self.current_iteration,
                timestamp
            );
            fs::copy(frame_source, frame_dest)?;
        }
        
        self.snapshot_count += 1;
        
        Ok(snapshot_name)
    }
}

// ============================================================================
// VISUALIZATION
// ============================================================================

struct Camera {
    offset_x: f32,
    offset_y: f32,
    zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self { offset_x: 0.0, offset_y: 0.0, zoom: 1.0 }
    }
}

impl Camera {
    fn pan(&mut self, dx: f32, dy: f32) {
        self.offset_x += dx / self.zoom;
        self.offset_y += dy / self.zoom;
    }
    
    fn zoom(&mut self, factor: f32) {
        self.zoom = (self.zoom * factor).clamp(0.1, 10.0);
    }
    
    fn reset(&mut self) {
        *self = Self::default();
    }
}

fn render_telemetry_hud(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    iteration: u32,
    score: f32,
    snapshot_count: u32,
    is_running: bool,
) {
    // HUD background (top 80 pixels)
    for y in 0..80u32 {
        for x in 0..WIDTH {
            img.put_pixel(x, y, Rgba([10, 10, 20, 255]));
        }
    }
    
    // Draw simple text indicators using colored rectangles
    // (Full text rendering requires font support)
    
    // Status indicator
    let status_color = if is_running {
        Rgba([50, 200, 50, 255])  // Green = running
    } else {
        Rgba([200, 50, 50, 255])  // Red = stopped
    };
    
    for y in 10..20u32 {
        for x in 10..20 {
            img.put_pixel(x, y, status_color);
        }
    }
    
    // Iteration bar (visual progress)
    let progress = (iteration as f32 / 1000.0).min(1.0);
    let bar_width = (progress * 200.0) as u32;
    
    for y in 30..40u32 {
        for x in 10..(10 + bar_width) {
            img.put_pixel(x, y, Rgba([100, 150, 255, 255]));
        }
    }
    
    // Score indicator (color-coded)
    let score_color = if score > 0.7 {
        Rgba([50, 200, 50, 255])  // Green = good
    } else if score > 0.4 {
        Rgba([200, 200, 50, 255])  // Yellow = medium
    } else {
        Rgba([200, 50, 50, 255])  // Red = exploring
    };
    
    for y in 50..60u32 {
        for x in 10..20 {
            img.put_pixel(x, y, score_color);
        }
    }
    
    // Snapshot count indicator
    for y in 60..70u32 {
        for x in 10..(10 + snapshot_count * 5) {
            img.put_pixel(x, y, Rgba([200, 100, 200, 255]));
        }
    }
}

fn render_grid(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, camera: &Camera) {
    let sector_size = 64.0;
    
    for y in 80..HEIGHT {
        for x in 0..WIDTH {
            // Apply camera transform
            let center_x = WIDTH as f32 / 2.0;
            let center_y = HEIGHT as f32 / 2.0;
            let world_x = (x as f32 - center_x) / camera.zoom + camera.offset_x + center_x;
            let world_y = (y as f32 - center_y) / camera.zoom + camera.offset_y + center_y;
            
            // Grid
            let grid_x = world_x % sector_size;
            let grid_y = world_y % sector_size;
            
            let sector_x = (world_x / sector_size).floor() as i32;
            let sector_y = (world_y / sector_size).floor() as i32;
            
            // Background
            let mut r = 5u8; let mut g = 5u8; let mut b = 12u8;
            
            // Grid lines
            if grid_x < 1.0 || grid_x > sector_size - 1.0 ||
               grid_y < 1.0 || grid_y > sector_size - 1.0 {
                r = 25; g = 25; b = 38;
            }
            
            // Sector markers
            let dist = ((grid_x - sector_size/2.0).powi(2) + 
                       (grid_y - sector_size/2.0).powi(2)).sqrt();
            
            if dist < 4.0 {
                if sector_x == 0 && sector_y == 0 {
                    r = 50; g = 200; b = 50;  // Core
                } else if sector_x == 1 && sector_y == 0 {
                    r = 200; g = 200; b = 50;  // Library
                } else if sector_x == 0 && sector_y == 1 {
                    r = 200; g = 50; b = 200;  // Lab
                } else if sector_x == -1 && sector_y == 0 {
                    r = 50; g = 200; b = 200;  // Gateway
                } else {
                    r = 75; g = 75; b = 100;
                }
            }
            
            img.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║       EVOLUTION MONITOR — MISSION CONTROL                ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  [S] Save snapshot    [R] Reset camera    [Q] Quit       ║");
    println!("║  [WASD] Pan           [Mouse] Zoom                       ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    fs::create_dir_all("output").ok();
    fs::create_dir_all("snapshots").ok();
    
    let mut monitor = MonitorState::new();
    let mut camera = Camera::default();
    
    println!("[MONITOR] Watching output/ouroboros_checkpoint.json");
    println!("[MONITOR] Press Ctrl+C to stop");
    println!();
    
    // Initial load
    if let Some((iter, score)) = monitor.load_latest_state() {
        println!("[INIT] Iteration: {}, Score: {:.2}", iter, score);
        monitor.is_running = true;
    } else {
        println!("[INIT] No checkpoint found - waiting for Ouroboros to start...");
    }
    
    // Main monitoring loop
    let mut frame_count = 0u32;
    
    loop {
        frame_count += 1;
        
        // Check for evolution updates
        if monitor.check_for_evolution() {
            if let Some((iter, score)) = monitor.load_latest_state() {
                println!("[UPDATE] Iteration: {}, Score: {:.2}", iter, score);
            }
        }
        
        // Render current view
        let mut img = ImageBuffer::new(WIDTH, HEIGHT);
        render_grid(&mut img, &camera);
        render_telemetry_hud(
            &mut img,
            monitor.current_iteration,
            monitor.last_score,
            monitor.snapshot_count,
            monitor.is_running,
        );
        
        // Save frame
        let frame_path = format!("output/monitor_frame_{:04}.png", frame_count % 100);
        if let Err(e) = img.save(&frame_path) {
            eprintln!("[ERROR] Failed to save frame: {}", e);
        }
        
        // Every 10 frames, print status
        if frame_count % 10 == 0 {
            print!("\r[FRAME {}] Iter: {} | Score: {:.2} | Snapshots: {}  ",
                frame_count,
                monitor.current_iteration,
                monitor.last_score,
                monitor.snapshot_count
            );
            use std::io::Write;
            std::io::stdout().flush().ok();
        }
        
        // Simulate camera movement for demo
        if frame_count % 30 == 0 {
            camera.pan(5.0, 0.0);
        }
        if frame_count % 60 == 0 {
            camera.pan(-5.0, 0.0);
        }
        
        // Auto-snapshot every 50 iterations
        if monitor.current_iteration > 0 && monitor.current_iteration % 50 == 0 {
            if monitor.snapshot_count < monitor.current_iteration / 50 {
                match monitor.take_snapshot() {
                    Ok(path) => println!("\n[SNAPSHOT] Saved: {}", path),
                    Err(e) => eprintln!("\n[ERROR] Snapshot failed: {}", e),
                }
            }
        }
        
        // Sleep to avoid CPU burn
        std::thread::sleep(Duration::from_millis(100));
        
        // Stop after demo frames
        if frame_count >= 100 {
            break;
        }
    }
    
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║          EVOLUTION MONITOR — SESSION COMPLETE            ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Iterations observed: {:5}                             ║", monitor.current_iteration);
    println!("║  Final score: {:5.2}                                   ║", monitor.last_score);
    println!("║  Snapshots saved: {:5}                                 ║", monitor.snapshot_count);
    println!("╚══════════════════════════════════════════════════════════╝");
}
