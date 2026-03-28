// World Engine — Multi-Sector Biosphere
//
// Phase 11 Integration: Camera + Handoff + Layers + Ouroboros Link
// The Geometry OS unified world simulator

use image::{ImageBuffer, Rgba};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;
const SECTOR_SIZE: u32 = 64;
const GLYPH_SIZE: u32 = 3;

// ============================================================================
// GLYPH ATLAS (from Phase 9/11)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GlyphID {
    Empty,
    File,    // Blue - evolved glyph
    Folder,  // Yellow
    Exec,    // Red
    Data,    // Green - optimized glyph
    Link,    // Cyan
    Alert,   // Orange
    Check,   // Bright Green
    Cross,   // Bright Red
}

impl GlyphID {
    fn pattern(&self) -> [u8; 9] {
        match self {
            GlyphID::Empty => [0,0,0,0,0,0,0,0,0],
            GlyphID::File => [1,0,0,1,0,0,1,1,0],
            GlyphID::Folder => [1,1,1,1,0,0,0,0,0],
            GlyphID::Exec => [1,0,1,0,1,0,1,0,1],
            GlyphID::Data => [0,1,0,1,1,1,0,1,0],
            GlyphID::Link => [0,0,1,0,1,0,1,0,0],
            GlyphID::Alert => [0,1,0,0,1,0,0,1,0],
            GlyphID::Check => [0,0,1,0,1,0,1,0,0],
            GlyphID::Cross => [0,1,0,0,1,0,0,1,0],
        }
    }
    
    fn color(&self) -> Rgba<u8> {
        match self {
            GlyphID::Empty => Rgba([0, 0, 0, 0]),
            GlyphID::File => Rgba([50, 100, 255, 255]),
            GlyphID::Folder => Rgba([255, 255, 50, 255]),
            GlyphID::Exec => Rgba([255, 50, 50, 255]),
            GlyphID::Data => Rgba([50, 255, 100, 255]),
            GlyphID::Link => Rgba([50, 255, 255, 255]),
            GlyphID::Alert => Rgba([255, 150, 50, 255]),
            GlyphID::Check => Rgba([100, 255, 100, 255]),
            GlyphID::Cross => Rgba([255, 100, 100, 255]),
        }
    }
}

// ============================================================================
// WORLD STRUCTURES
// ============================================================================

#[derive(Debug, Clone)]
struct SemanticAgent {
    id: u32,
    local_x: f32,
    local_y: f32,
    sector_x: i32,
    sector_y: i32,
    vel_x: f32,
    vel_y: f32,
    cargo: GlyphID,
    origin_sector: (i32, i32),  // Where agent started
}

impl SemanticAgent {
    fn update(&mut self, dt: f32) {
        self.local_x += self.vel_x * dt;
        self.local_y += self.vel_y * dt;
        
        // Sector handoff (teleportation membrane)
        if self.local_x >= SECTOR_SIZE as f32 {
            self.local_x -= SECTOR_SIZE as f32;
            self.sector_x += 1;
        } else if self.local_x < 0.0 {
            self.local_x += SECTOR_SIZE as f32;
            self.sector_x -= 1;
        }
        
        if self.local_y >= SECTOR_SIZE as f32 {
            self.local_y -= SECTOR_SIZE as f32;
            self.sector_y += 1;
        } else if self.local_y < 0.0 {
            self.local_y += SECTOR_SIZE as f32;
            self.sector_y -= 1;
        }
    }
    
    fn global_pos(&self) -> (f32, f32) {
        let gx = self.sector_x as f32 * SECTOR_SIZE as f32 + self.local_x;
        let gy = self.sector_y as f32 * SECTOR_SIZE as f32 + self.local_y;
        (gx, gy)
    }
}

#[derive(Debug)]
struct SectorData {
    coords: (i32, i32),
    name: String,
    color: Rgba<u8>,
    semantic_grid: HashMap<(i32, i32), GlyphID>,
}

impl SectorData {
    fn new(x: i32, y: i32) -> Self {
        let (name, color) = match (x, y) {
            (0, 0) => ("Core".to_string(), Rgba([50, 200, 50, 255])),
            (1, 0) => ("Library".to_string(), Rgba([200, 200, 50, 255])),
            (2, 0) => ("Lab".to_string(), Rgba([200, 100, 50, 255])),
            (-1, 0) => ("Gateway".to_string(), Rgba([50, 200, 200, 255])),
            _ => ("Sector".to_string(), Rgba([75, 75, 100, 255])),
        };
        
        Self {
            coords: (x, y),
            name,
            color,
            semantic_grid: HashMap::new(),
        }
    }
}

struct Camera {
    offset_x: f32,
    offset_y: f32,
    zoom: f32,
    target_x: f32,
    target_y: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            zoom: 1.0,
            target_x: 0.0,
            target_y: 0.0,
        }
    }
}

impl Camera {
    fn follow(&mut self, target_x: f32, target_y: f32, lerp: f32) {
        self.target_x = target_x;
        self.target_y = target_y;
        self.offset_x += (self.target_x - self.offset_x - WIDTH as f32 / 2.0) * lerp;
        self.offset_y += (self.target_y - self.offset_y - HEIGHT as f32 / 2.0) * lerp;
    }
    
    fn screen_to_world(&self, sx: f32, sy: f32) -> (f32, f32) {
        let cx = WIDTH as f32 / 2.0;
        let cy = HEIGHT as f32 / 2.0;
        ((sx - cx) / self.zoom + self.offset_x + cx,
         (sy - cy) / self.zoom + self.offset_y + cy)
    }
}

// ============================================================================
// WORLD ENGINE
// ============================================================================

struct World {
    sectors: HashMap<(i32, i32), SectorData>,
    agents: Vec<SemanticAgent>,
    camera: Camera,
    frame: u32,
    ouroboros_link: OuroborosLink,
}

struct OuroborosLink {
    last_modified: SystemTime,
    current_iteration: u32,
    current_score: f32,
}

impl OuroborosLink {
    fn new() -> Self {
        Self {
            last_modified: SystemTime::UNIX_EPOCH,
            current_iteration: 0,
            current_score: 0.0,
        }
    }
    
    fn poll(&mut self) -> Option<(u32, f32)> {
        let checkpoint_path = "output/ouroboros_checkpoint.json";
        
        if !Path::new(checkpoint_path).exists() {
            return None;
        }
        
        if let Ok(metadata) = fs::metadata(checkpoint_path) {
            if let Ok(modified) = metadata.modified() {
                if modified > self.last_modified {
                    self.last_modified = modified;
                    
                    if let Ok(content) = fs::read_to_string(checkpoint_path) {
                        let iterations = content.matches("\"iteration\":").count() as u32;
                        
                        // Extract last score
                        let score = if let Some(last) = content.rfind("}]") {
                            let obj = &content[..last + 1];
                            if let Some(start) = obj.rfind("\"score\":") {
                                let s = &obj[start + 8..];
                                if let Some(end) = s.find(|c: char| !c.is_numeric() && c != '.' && c != '-') {
                                    s[..end].parse().unwrap_or(0.0)
                                } else { 0.0 }
                            } else { 0.0 }
                        } else { 0.0 };
                        
                        self.current_iteration = iterations;
                        self.current_score = score;
                        
                        return Some((iterations, score));
                    }
                }
            }
        }
        
        None
    }
    
    fn spawn_evolved_agents(&self, world: &mut World) {
        // Spawn agents based on Ouroboros progress
        if self.current_iteration > 0 && self.current_iteration % 10 == 0 {
            // Every 10 iterations, spawn a messenger agent
            let agent = SemanticAgent {
                id: 1000 + self.current_iteration,
                local_x: SECTOR_SIZE as f32 / 2.0,
                local_y: SECTOR_SIZE as f32 / 2.0,
                sector_x: 0,
                sector_y: 0,
                vel_x: 0.5,
                vel_y: 0.0,
                cargo: if self.current_score > 0.5 {
                    GlyphID::Check  // Good score = checkmark
                } else {
                    GlyphID::Data   // Exploring = data
                },
                origin_sector: (0, 0),
            };
            
            world.agents.push(agent);
        }
    }
}

impl World {
    fn new() -> Self {
        let mut sectors = HashMap::new();
        
        // Create sectors
        for y in -2..=2 {
            for x in -2..=2 {
                sectors.insert((x, y), SectorData::new(x, y));
            }
        }
        
        // Spawn initial agents in Core
        let mut agents = Vec::new();
        for i in 0..20 {
            agents.push(SemanticAgent {
                id: i,
                local_x: (i % 10) as f32 * 6.0,
                local_y: (i / 10) as f32 * 6.0 + 10.0,
                sector_x: 0,
                sector_y: 0,
                vel_x: 0.3 + (i as f32 % 5.0) * 0.1,
                vel_y: (i as f32 % 3.0 - 1.0) * 0.1,
                cargo: match i % 4 {
                    0 => GlyphID::File,
                    1 => GlyphID::Data,
                    2 => GlyphID::Folder,
                    _ => GlyphID::Link,
                },
                origin_sector: (0, 0),
            });
        }
        
        Self {
            sectors,
            agents,
            camera: Camera::default(),
            frame: 0,
            ouroboros_link: OuroborosLink::new(),
        }
    }
    
    fn step(&mut self, dt: f32) {
        self.frame += 1;
        
        // Poll Ouroboros and collect data
        let spawn_data = if let Some((iter, score)) = self.ouroboros_link.poll() {
            println!("[OUROBOROS] Iteration {} | Score: {:.2}", iter, score);
            Some((iter, score))
        } else {
            None
        };
        
        // Spawn evolved agents if needed
        if let Some((iter, score)) = spawn_data {
            if iter > 0 && iter % 10 == 0 {
                let agent = SemanticAgent {
                    id: 1000 + iter,
                    local_x: SECTOR_SIZE as f32 / 2.0,
                    local_y: SECTOR_SIZE as f32 / 2.0,
                    sector_x: 0,
                    sector_y: 0,
                    vel_x: 0.5,
                    vel_y: 0.0,
                    cargo: if score > 0.5 {
                        GlyphID::Check
                    } else {
                        GlyphID::Data
                    },
                    origin_sector: (0, 0),
                };
                self.agents.push(agent);
            }
        }
        
        // Update agents
        for agent in &mut self.agents {
            agent.update(dt);
            
            // Leave semantic trail
            let sector_key = (agent.sector_x, agent.sector_y);
            if let Some(sector) = self.sectors.get_mut(&sector_key) {
                let grid_x = (agent.local_x / GLYPH_SIZE as f32) as i32;
                let grid_y = (agent.local_y / GLYPH_SIZE as f32) as i32;
                sector.semantic_grid.insert((grid_x, grid_y), agent.cargo);
            }
        }
        
        // Camera follows lead agent
        if let Some(leader) = self.agents.first() {
            let (gx, gy) = leader.global_pos();
            self.camera.follow(gx, gy, 0.05);
        }
    }
    
    fn render(&self) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        let mut img = ImageBuffer::new(WIDTH, HEIGHT);
        
        // Background
        for pixel in img.pixels_mut() {
            *pixel = Rgba([5, 5, 12, 255]);
        }
        
        // Render sectors
        for y in 80..HEIGHT {
            for x in 0..WIDTH {
                let (wx, wy) = self.camera.screen_to_world(x as f32, y as f32);
                
                let sector_x = (wx / SECTOR_SIZE as f32).floor() as i32;
                let sector_y = (wy / SECTOR_SIZE as f32).floor() as i32;
                
                let local_x = wx % SECTOR_SIZE as f32;
                let local_y = wy % SECTOR_SIZE as f32;
                
                // Grid lines
                if local_x < 1.0 || local_x > SECTOR_SIZE as f32 - 1.0 ||
                   local_y < 1.0 || local_y > SECTOR_SIZE as f32 - 1.0 {
                    img.put_pixel(x, y, Rgba([25, 25, 38, 255]));
                }
                
                // Sector markers
                let dist = ((local_x - SECTOR_SIZE as f32/2.0).powi(2) +
                           (local_y - SECTOR_SIZE as f32/2.0).powi(2)).sqrt();
                
                if dist < 4.0 {
                    if let Some(sector) = self.sectors.get(&(sector_x, sector_y)) {
                        img.put_pixel(x, y, sector.color);
                    }
                }
            }
        }
        
        // Render semantic trails (expanded glyphs)
        for (_, sector) in &self.sectors {
            for ((gx, gy), glyph) in &sector.semantic_grid {
                let pattern = glyph.pattern();
                let color = glyph.color();
                
                let base_x = sector.coords.0 as f32 * SECTOR_SIZE as f32 +
                            *gx as f32 * GLYPH_SIZE as f32;
                let base_y = sector.coords.1 as f32 * SECTOR_SIZE as f32 +
                            *gy as f32 * GLYPH_SIZE as f32;
                
                // Draw 3x3 pattern
                for ly in 0..3 {
                    for lx in 0..3 {
                        if pattern[(ly * 3 + lx) as usize] == 1 {
                            let px = base_x + lx as f32;
                            let py = base_y + ly as f32;
                            
                            // Transform to screen
                            let screen_x = ((px - self.camera.offset_x - WIDTH as f32/2.0) *
                                          self.camera.zoom + WIDTH as f32/2.0) as i32;
                            let screen_y = ((py - self.camera.offset_y - HEIGHT as f32/2.0) *
                                          self.camera.zoom + HEIGHT as f32/2.0) as i32;
                            
                            if screen_x >= 0 && screen_x < WIDTH as i32 &&
                               screen_y >= 80 && screen_y < HEIGHT as i32 {
                                img.put_pixel(screen_x as u32, screen_y as u32, color);
                            }
                        }
                    }
                }
            }
        }
        
        // Render agents
        for agent in &self.agents {
            let (gx, gy) = agent.global_pos();
            
            let screen_x = ((gx - self.camera.offset_x - WIDTH as f32/2.0) *
                          self.camera.zoom + WIDTH as f32/2.0) as i32;
            let screen_y = ((gy - self.camera.offset_y - HEIGHT as f32/2.0) *
                          self.camera.zoom + HEIGHT as f32/2.0) as i32;
            
            // Draw agent (5x5 dot)
            for dy in -2..=2 {
                for dx in -2..=2 {
                    let px = screen_x + dx;
                    let py = screen_y + dy;
                    
                    if px >= 0 && px < WIDTH as i32 && py >= 80 && py < HEIGHT as i32 {
                        let color = agent.cargo.color();
                        img.put_pixel(px as u32, py as u32, color);
                    }
                }
            }
        }
        
        // HUD
        self.render_hud(&mut img);
        
        img
    }
    
    fn render_hud(&self, img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>) {
        // HUD background
        for y in 0..80u32 {
            for x in 0..WIDTH {
                img.put_pixel(x, y, Rgba([10, 10, 20, 255]));
            }
        }
        
        // Ouroboros status bar
        let progress = (self.ouroboros_link.current_iteration as f32 / 1000.0).min(1.0);
        let bar_width = (progress * 300.0) as u32;
        
        for y in 20..30u32 {
            for x in 10..(10 + bar_width) {
                let score_color = if self.ouroboros_link.current_score > 0.5 {
                    Rgba([50, 200, 50, 255])
                } else if self.ouroboros_link.current_score > 0.3 {
                    Rgba([200, 200, 50, 255])
                } else {
                    Rgba([200, 50, 50, 255])
                };
                img.put_pixel(x, y, score_color);
            }
        }
        
        // Agent count
        let agent_bar = (self.agents.len() as f32 / 100.0 * 50.0) as u32;
        for y in 40..50u32 {
            for x in 10..(10 + agent_bar) {
                img.put_pixel(x, y, Rgba([100, 150, 255, 255]));
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
    println!("║            WORLD ENGINE — MULTI-SECTION BIOSPHERE        ║");
    println!("║     Phase 11 Integration: Ouroboros → Infinite Map       ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    fs::create_dir_all("output").ok();
    
    let mut world = World::new();
    
    println!("[INIT] World initialized with {} sectors", world.sectors.len());
    println!("[INIT] {} agents spawned in Core", world.agents.len());
    println!("[INIT] Ouroboros link active");
    println!();
    
    // Run simulation
    let frames = 200;
    
    for _ in 0..frames {
        world.step(1.0);
        
        if world.frame % 20 == 0 {
            let img = world.render();
            let path = format!("output/world_engine_frame_{:04}.png", world.frame);
            img.save(&path).expect("Failed to save");
            
            // Count agents by sector
            let mut sector_counts: HashMap<(i32, i32), u32> = HashMap::new();
            for agent in &world.agents {
                *sector_counts.entry((agent.sector_x, agent.sector_y)).or_insert(0) += 1;
            }
            
            println!("[FRAME {}] Agents: {} | Ouroboros: iter {} | Sectors: {:?}",
                world.frame,
                world.agents.len(),
                world.ouroboros_link.current_iteration,
                sector_counts
            );
        }
    }
    
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║         WORLD ENGINE — INTEGRATION COMPLETE              ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  ✅ Multi-sector world simulation                        ║");
    println!("║  ✅ Camera follows agents across sectors                 ║");
    println!("║  ✅ Semantic trails rendered with glyph expansion        ║");
    println!("║  ✅ Ouroboros link spawns evolved agents                 ║");
    println!("║  ✅ Teleportation membrane active                        ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    println!("Output: output/world_engine_frame_*.png");
    println!("Watch the biosphere evolve! 🌈");
}
