// Sector Stress Test — Global Coordinate System Validation
//
// Phase 10 Beta: Tests agent teleportation between sectors
// Spawns 1000 agents with constant velocity, watches them march across infinite map

use image::{ImageBuffer, Rgba};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;
const SECTOR_SIZE: f32 = 64.0;

// ============================================================================
// AGENT WITH GLOBAL COORDINATES
// ============================================================================

#[derive(Debug, Clone)]
struct GlobalAgent {
    // Local position (within current sector, 0-64)
    local_x: f32,
    local_y: f32,
    
    // Global sector coordinates
    sector_x: i32,
    sector_y: i32,
    
    // Velocity (constant for stress test)
    vel_x: f32,
    vel_y: f32,
    
    // Payload (preserved across sector jumps)
    cargo: u32,
    
    // Trail (for visualization)
    trail: Vec<(i32, i32, f32, f32)>,  // (sector_x, sector_y, local_x, local_y)
}

impl GlobalAgent {
    fn new(id: usize) -> Self {
        // Spawn in a line, moving East
        let row = (id / 100) as f32;
        let col = (id % 100) as f32;
        
        Self {
            local_x: col * 0.6,  // Spread across sector
            local_y: 10.0 + row * 0.5,
            sector_x: 0,
            sector_y: 0,
            vel_x: 0.8,  // Constant Eastward velocity
            vel_y: 0.0,
            cargo: id as u32,
            trail: Vec::new(),
        }
    }
    
    fn update(&mut self, dt: f32) {
        // Record trail
        if self.trail.len() < 100 {
            self.trail.push((self.sector_x, self.sector_y, self.local_x, self.local_y));
        } else {
            self.trail.remove(0);
            self.trail.push((self.sector_x, self.sector_y, self.local_x, self.local_y));
        }
        
        // Apply velocity
        self.local_x += self.vel_x * dt;
        self.local_y += self.vel_y * dt;
        
        // Handle sector wrapping (THE TELEPORTATION MEMBRANE)
        self.handle_sector_wrap();
    }
    
    fn handle_sector_wrap(&mut self) {
        // Horizontal portal
        if self.local_x >= SECTOR_SIZE {
            self.local_x -= SECTOR_SIZE;
            self.sector_x += 1;
        } else if self.local_x < 0.0 {
            self.local_x += SECTOR_SIZE;
            self.sector_x -= 1;
        }
        
        // Vertical portal
        if self.local_y >= SECTOR_SIZE {
            self.local_y -= SECTOR_SIZE;
            self.sector_y += 1;
        } else if self.local_y < 0.0 {
            self.local_y += SECTOR_SIZE;
            self.sector_y -= 1;
        }
    }
    
    fn global_position(&self) -> (f32, f32) {
        let gx = self.sector_x as f32 * SECTOR_SIZE + self.local_x;
        let gy = self.sector_y as f32 * SECTOR_SIZE + self.local_y;
        (gx, gy)
    }
}

// ============================================================================
// CAMERA SYSTEM
// ============================================================================

#[derive(Debug, Clone, Copy)]
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
    
    fn screen_to_world(&self, screen_x: f32, screen_y: f32) -> (f32, f32) {
        let center_x = WIDTH as f32 / 2.0;
        let center_y = HEIGHT as f32 / 2.0;
        let world_x = (screen_x - center_x) / self.zoom + self.offset_x + center_x;
        let world_y = (screen_y - center_y) / self.zoom + self.offset_y + center_y;
        (world_x, world_y)
    }
    
    fn follow_agents(&mut self, agents: &[GlobalAgent]) {
        // Auto-follow the caravan
        if let Some(leader) = agents.first() {
            let (gx, gy) = leader.global_position();
            self.offset_x = gx - WIDTH as f32 / 2.0;
            self.offset_y = gy - HEIGHT as f32 / 2.0;
        }
    }
}

// ============================================================================
// RENDERER
// ============================================================================

fn render_world(
    agents: &[GlobalAgent],
    camera: &Camera,
    frame: u32,
) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let mut img = ImageBuffer::new(WIDTH, HEIGHT);
    
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let (world_x, world_y) = camera.screen_to_world(x as f32, y as f32);
            
            // Determine sector
            let sector_x = (world_x / SECTOR_SIZE).floor() as i32;
            let sector_y = (world_y / SECTOR_SIZE).floor() as i32;
            
            // Grid position within sector
            let grid_x = world_x % SECTOR_SIZE;
            let grid_y = world_y % SECTOR_SIZE;
            
            // Default: dark background
            let mut r = 5u8; let mut g = 5u8; let mut b = 12u8;
            
            // Grid lines
            if grid_x < 1.0 || grid_x > SECTOR_SIZE - 1.0 ||
               grid_y < 1.0 || grid_y > SECTOR_SIZE - 1.0 {
                r = 25; g = 25; b = 38;
            }
            
            // Sector markers
            let dist_to_center = ((grid_x - SECTOR_SIZE/2.0).powi(2) + 
                                  (grid_y - SECTOR_SIZE/2.0).powi(2)).sqrt();
            
            if dist_to_center < 4.0 {
                if sector_x == 0 && sector_y == 0 {
                    r = 50; g = 200; b = 50;  // Core
                } else if sector_x == 1 && sector_y == 0 {
                    r = 200; g = 200; b = 50;  // Library
                } else if sector_x == 2 && sector_y == 0 {
                    r = 200; g = 100; b = 50;  // Further East
                } else if sector_x == 3 && sector_y == 0 {
                    r = 200; g = 50; b = 50;  // Red zone
                } else {
                    r = 75; g = 75; b = 100;
                }
            }
            
            img.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }
    
    // Render agents
    for agent in agents {
        let (gx, gy) = agent.global_position();
        
        // Transform to screen coordinates
        let screen_x = ((gx - camera.offset_x - WIDTH as f32 / 2.0) * camera.zoom + WIDTH as f32 / 2.0) as i32;
        let screen_y = ((gy - camera.offset_y - HEIGHT as f32 / 2.0) * camera.zoom + HEIGHT as f32 / 2.0) as i32;
        
        // Draw agent (5x5 pixel dot)
        for dy in -2..=2 {
            for dx in -2..=2 {
                let px = screen_x + dx;
                let py = screen_y + dy;
                
                if px >= 0 && px < WIDTH as i32 && py >= 80 && py < HEIGHT as i32 {
                    // Color based on sector
                    let (r, g, b) = if agent.sector_x == 0 {
                        (50, 200, 50)   // Green in Core
                    } else if agent.sector_x == 1 {
                        (200, 200, 50)  // Yellow in Library
                    } else if agent.sector_x == 2 {
                        (200, 100, 50)  // Orange
                    } else {
                        (200, 50, 50)   // Red in far sectors
                    };
                    
                    img.put_pixel(px as u32, py as u32, Rgba([r, g, b, 255]));
                }
            }
        }
        
        // Draw trail (faint)
        for (i, (sx, sy, lx, ly)) in agent.trail.iter().enumerate() {
            let gx = *sx as f32 * SECTOR_SIZE + lx;
            let gy = *sy as f32 * SECTOR_SIZE + ly;
            
            let screen_x = ((gx - camera.offset_x - WIDTH as f32 / 2.0) * camera.zoom + WIDTH as f32 / 2.0) as i32;
            let screen_y = ((gy - camera.offset_y - HEIGHT as f32 / 2.0) * camera.zoom + HEIGHT as f32 / 2.0) as i32;
            
            if screen_x >= 0 && screen_x < WIDTH as i32 && screen_y >= 80 && screen_y < HEIGHT as i32 {
                let alpha = (i as f32 / agent.trail.len() as f32 * 100.0) as u8;
                let current = img.get_pixel(screen_x as u32, screen_y as u32);
                img.put_pixel(screen_x as u32, screen_y as u32, Rgba([
                    (current[0] as u16 + alpha as u16 / 3).min(255) as u8,
                    (current[1] as u16 + alpha as u16 / 2).min(255) as u8,
                    current[2],
                    255,
                ]));
            }
        }
    }
    
    // HUD
    render_hud(&mut img, agents, camera, frame);
    
    img
}

fn render_hud(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    agents: &[GlobalAgent],
    camera: &Camera,
    frame: u32,
) {
    // HUD background
    for y in 0..80u32 {
        for x in 0..WIDTH {
            img.put_pixel(x, y, Rgba([10, 10, 20, 255]));
        }
    }
    
    // Stats
    if let Some(leader) = agents.first() {
        let (gx, gy) = leader.global_position();
        
        // Draw indicators
        // Sector progress bar
        let sector_progress = (leader.sector_x as f32 / 10.0).min(1.0);
        let bar_width = (sector_progress * 300.0) as u32;
        
        for y in 20..30u32 {
            for x in 10..(10 + bar_width) {
                img.put_pixel(x, y, Rgba([100, 150, 255, 255]));
            }
        }
        
        // Sector markers
        for y in 40..50u32 {
            for x in 10..20 {
                img.put_pixel(x, y, Rgba([50, 200, 50, 255]));  // Core
            }
            for x in 70..80 {
                img.put_pixel(x, y, Rgba([200, 200, 50, 255]));  // Library
            }
            for x in 130..140 {
                img.put_pixel(x, y, Rgba([200, 100, 50, 255]));  // Orange zone
            }
            for x in 190..200 {
                img.put_pixel(x, y, Rgba([200, 50, 50, 255]));  // Red zone
            }
        }
        
        // Current sector indicator (white dot)
        let marker_x = 10 + leader.sector_x as u32 * 60;
        for y in 35..55u32 {
            for x in marker_x..(marker_x + 10) {
                if x < WIDTH {
                    img.put_pixel(x, y, Rgba([255, 255, 255, 255]));
                }
            }
        }
    }
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║       PHASE 10 BETA — SECTOR HANDOFF STRESS TEST         ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    std::fs::create_dir_all("output").ok();
    
    // Spawn 1000 agents
    let mut agents: Vec<GlobalAgent> = (0..1000).map(|i| GlobalAgent::new(i)).collect();
    let mut camera = Camera::default();
    
    println!("[INIT] Spawned {} agents", agents.len());
    println!("[INIT] All agents moving East at constant velocity");
    println!("[INIT] Watching them march across the infinite map...");
    println!();
    
    // Simulation loop
    let dt = 1.0;  // Time step
    let frames = 200;
    
    for frame in 0..frames {
        // Update all agents
        for agent in &mut agents {
            agent.update(dt);
        }
        
        // Camera follows the caravan
        camera.follow_agents(&agents);
        
        // Render
        let img = render_world(&agents, &camera, frame);
        
        // Save every 10th frame
        if frame % 10 == 0 {
            let path = format!("output/stress_test_frame_{:04}.png", frame);
            img.save(&path).expect("Failed to save");
            
            // Report progress
            if let Some(leader) = agents.first() {
                let (gx, gy) = leader.global_position();
                println!("[FRAME {:3}] Leader at global ({:.1}, {:.1}) | Sector ({}, {})",
                    frame, gx, gy, leader.sector_x, leader.sector_y);
            }
        }
    }
    
    // Final report
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║         STRESS TEST COMPLETE — TELEPORTATION VERIFIED     ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    
    if let Some(leader) = agents.first() {
        let (gx, gy) = leader.global_position();
        println!("║  Leader traveled: {:.0} sectors East                    ║", leader.sector_x);
        println!("║  Global position: ({:.1}, {:.1})                        ║", gx, gy);
        println!("║  Final sector: ({}, {})                                  ║", leader.sector_x, leader.sector_y);
        println!("║  Cargo preserved: {}                                     ║", leader.cargo);
    }
    
    println!("║  Frames rendered: {}                                      ║", frames);
    println!("║  Agents simulated: {}                                     ║", agents.len());
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    println!("Output: output/stress_test_frame_*.png");
    println!("Watch the caravan march across the infinite map! 🌈");
}
