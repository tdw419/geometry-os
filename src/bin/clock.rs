// Clock — The Heartbeat of Geometry OS
//
// Phase 12 Beta: Ring Oscillator for system-wide synchronization
// Creates a unified TICK/TOCK that all components subscribe to

use image::{ImageBuffer, Rgba};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;

// ============================================================================
// CLOCK STATE
// ============================================================================

#[derive(Debug, Clone)]
struct Clock {
    tick: u64,
    state: bool,
    frequency_ms: u64,
    subscribers: Vec<String>,
}

impl Clock {
    fn new(frequency_ms: u64) -> Self {
        Self {
            tick: 0,
            state: false,
            frequency_ms,
            subscribers: vec![
                "camera_test".to_string(),
                "evolution_monitor".to_string(),
                "world_engine".to_string(),
                "logic_gate_test".to_string(),
            ],
        }
    }
    
    fn pulse(&mut self) -> bool {
        self.tick += 1;
        self.state = !self.state;  // Oscillate
        self.state
    }
    
    fn broadcast(&self) -> ClockSignal {
        ClockSignal {
            tick: self.tick,
            state: self.state,
            phase: if self.state { "TICK" } else { "TOCK" },
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ClockSignal {
    tick: u64,
    state: bool,
    phase: &'static str,
}

// ============================================================================
// RING OSCILLATOR (Visual)
// ============================================================================

struct RingOscillator {
    nodes: Vec<bool>,  // 5 NOT gates in a ring
    active_node: usize,
}

impl RingOscillator {
    fn new() -> Self {
        Self {
            nodes: vec![true, false, true, false, true],  // Odd number = oscillation
            active_node: 0,
        }
    }
    
    fn step(&mut self) {
        // Propagate signal through ring
        let next = (self.active_node + 1) % self.nodes.len();
        
        // NOT gate: invert the signal
        self.nodes[next] = !self.nodes[self.active_node];
        self.active_node = next;
    }
    
    fn render(&self, img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, center_x: u32, center_y: u32) {
        let radius = 100.0f32;
        let node_count = self.nodes.len();
        
        // Draw ring
        for (i, &active) in self.nodes.iter().enumerate() {
            let angle = (i as f32 / node_count as f32) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
            let x = center_x as f32 + radius * angle.cos();
            let y = center_y as f32 + radius * angle.sin();
            
            // Draw node
            let color = if i == self.active_node {
                Rgba([255, 255, 100, 255])  // Yellow = active
            } else if active {
                Rgba([100, 255, 100, 255])  // Green = HIGH
            } else {
                Rgba([50, 50, 50, 255])     // Gray = LOW
            };
            
            // Draw 10x10 pixel node
            for dy in -5..=5 {
                for dx in -5..=5 {
                    let px = x as i32 + dx;
                    let py = y as i32 + dy;
                    if px >= 0 && px < WIDTH as i32 && py >= 80 && py < HEIGHT as i32 {
                        let dist = ((dx * dx + dy * dy) as f32).sqrt();
                        if dist <= 5.0 {
                            img.put_pixel(px as u32, py as u32, color);
                        }
                    }
                }
            }
        }
        
        // Draw connections
        for i in 0..node_count {
            let next_i = (i + 1) % node_count;
            
            let angle1 = (i as f32 / node_count as f32) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
            let angle2 = (next_i as f32 / node_count as f32) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
            
            let x1 = center_x as f32 + radius * angle1.cos();
            let y1 = center_y as f32 + radius * angle1.sin();
            let x2 = center_x as f32 + radius * angle2.cos();
            let y2 = center_y as f32 + radius * angle2.sin();
            
            // Draw line (simple)
            let steps = 20;
            for s in 0..=steps {
                let t = s as f32 / steps as f32;
                let px = x1 + (x2 - x1) * t;
                let py = y1 + (y2 - y1) * t;
                
                let px_i = px as i32;
                let py_i = py as i32;
                
                if px_i >= 0 && px_i < WIDTH as i32 && py_i >= 80 && py_i < HEIGHT as i32 {
                    img.put_pixel(px_i as u32, py_i as u32, Rgba([30, 30, 40, 255]));
                }
            }
        }
    }
}

// ============================================================================
// SYSTEM BUS (Simplified)
// ============================================================================

struct SystemBus {
    clock_signal: ClockSignal,
    data_lane: [u8; 8],
    teleport_trigger: bool,
}

impl SystemBus {
    fn new() -> Self {
        Self {
            clock_signal: ClockSignal {
                tick: 0,
                state: false,
                phase: "TOCK",
            },
            data_lane: [0; 8],
            teleport_trigger: false,
        }
    }
    
    fn sync(&mut self, signal: ClockSignal) {
        self.clock_signal = signal;
        
        // On TICK, process logic
        if self.clock_signal.state {
            self.process_logic_flow();
        }
    }
    
    fn process_logic_flow(&mut self) {
        // Example: Check if Carry bit is set
        if self.data_lane[7] == 1 {
            self.teleport_trigger = true;
        }
    }
    
    fn render_status(&self, img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>) {
        // Draw bus status bar
        let bar_y = HEIGHT - 40;
        
        for x in 0..WIDTH {
            for y in bar_y..HEIGHT {
                img.put_pixel(x, y, Rgba([10, 10, 20, 255]));
            }
        }
        
        // Draw data lane indicators
        for (i, &bit) in self.data_lane.iter().enumerate() {
            let x = 10 + i as u32 * 40;
            let color = if bit == 1 {
                Rgba([100, 255, 100, 255])
            } else {
                Rgba([50, 50, 50, 255])
            };
            
            for dy in 0..20u32 {
                for dx in 0..30u32 {
                    if x + dx < WIDTH {
                        img.put_pixel(x + dx, bar_y + 5 + dy, color);
                    }
                }
            }
        }
        
        // Draw teleport trigger
        if self.teleport_trigger {
            for x in (WIDTH - 100)..WIDTH {
                for y in (bar_y + 5)..(bar_y + 25) {
                    img.put_pixel(x, y, Rgba([255, 100, 100, 255]));
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
    println!("║       PHASE 12 BETA — CLOCK / HEARTBEAT                  ║");
    println!("║       Ring Oscillator for System Synchronization         ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    std::fs::create_dir_all("output").ok();
    
    let mut clock = Clock::new(100);  // 100ms frequency
    let mut oscillator = RingOscillator::new();
    let mut bus = SystemBus::new();
    
    println!("[CLOCK] Initializing ring oscillator with 5 NOT gates");
    println!("[CLOCK] Frequency: 100ms per tick");
    println!("[CLOCK] Subscribers: {:?}", clock.subscribers);
    println!();
    
    // Run for 50 frames
    for frame in 0..50 {
        // Pulse the clock
        let signal = clock.broadcast();
        clock.pulse();
        
        // Step the oscillator
        oscillator.step();
        
        // Sync the bus
        bus.sync(signal);
        
        // Render frame
        let mut img = ImageBuffer::new(WIDTH, HEIGHT);
        
        // Background
        for pixel in img.pixels_mut() {
            *pixel = Rgba([5, 5, 12, 255]);
        }
        
        // HUD
        for y in 0..80u32 {
            for x in 0..WIDTH {
                img.put_pixel(x, y, Rgba([10, 10, 20, 255]));
            }
        }
        
        // Draw clock info
        let phase_color = if signal.state {
            Rgba([100, 255, 100, 255])
        } else {
            Rgba([50, 50, 50, 255])
        };
        
        for y in 30..50u32 {
            for x in 10..200 {
                img.put_pixel(x, y, phase_color);
            }
        }
        
        // Draw oscillator
        oscillator.render(&mut img, WIDTH / 2, HEIGHT / 2);
        
        // Draw bus status
        bus.render_status(&mut img);
        
        // Save frame
        if frame % 2 == 0 {
            let path = format!("output/clock_frame_{:03}.png", frame);
            img.save(&path).expect("Failed to save");
        }
        
        // Log every 10 frames
        if frame % 10 == 0 {
            println!("[FRAME {:3}] {} | Tick: {} | Active Node: {}",
                frame,
                signal.phase,
                signal.tick,
                oscillator.active_node
            );
        }
    }
    
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║         CLOCK — HEARTBEAT ESTABLISHED                    ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  ✅ Ring oscillator (5 NOT gates)                        ║");
    println!("║  ✅ TICK/TOCK broadcast system                           ║");
    println!("║  ✅ System bus synchronization                           ║");
    println!("║  ✅ 25 frames rendered                                   ║");
    println!("║                                                            ║");
    println!("║  THE GEOMETRY OS HAS A HEARTBEAT                          ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    println!("Output: output/clock_frame_*.png");
    println!("Watch the pulse propagate! 💓");
}
