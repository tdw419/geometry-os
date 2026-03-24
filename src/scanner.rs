// Circuit Scanner - Read pixel state from GPU and save as ASCII
// Part of the Geometry OS "Spatial Keyboard" system

use clap::{Parser, Subcommand};
use std::fs;

#[derive(Parser)]
#[command(name = "circuit-scanner")]
#[command(about = "Scan GPU pixel grid and save/load circuits as ASCII", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan a region and save as ASCII art
    Scan {
        /// X start coordinate
        #[arg(short, long)]
        x: u32,
        
        /// Y start coordinate
        #[arg(short, long)]
        y: u32,
        
        /// Width to scan
        #[arg(short, long, default_value = "40")]
        width: u32,
        
        /// Height to scan
        #[arg(short, long, default_value = "20")]
        height: u32,
        
        /// Output file
        #[arg(short, long)]
        output: Option<String>,
    },
    
    /// Load ASCII circuit and inject into GPU
    Load {
        /// ASCII circuit file
        #[arg(short, long)]
        file: String,
        
        /// X offset for placement
        #[arg(short, long, default_value = "0")]
        x: u32,
        
        /// Y offset for placement
        #[arg(short, long, default_value = "0")]
        y: u32,
    },
    
    /// Watch a region and display live state
    Watch {
        /// X start coordinate
        #[arg(short, long)]
        x: u32,
        
        /// Y start coordinate
        #[arg(short, long)]
        y: u32,
        
        /// Width to watch
        #[arg(short, long, default_value = "40")]
        width: u32,
        
        /// Height to watch
        #[arg(short, long, default_value = "20")]
        height: u32,
    },
}

// Opcode mappings (must match injector)
const OP_IDLE: u32 = 0x01;
const OP_MOVE_RIGHT: u32 = 0x02;
const OP_MOVE_LEFT: u32 = 0x03;
const OP_MOVE_UP: u32 = 0x04;
const OP_MOVE_DOWN: u32 = 0x05;
const OP_REPLICATE: u32 = 0x06;
const OP_INFECT: u32 = 0x07;
const OP_AND: u32 = 0x30;
const OP_XOR: u32 = 0x31;
const TYPE_AGENT: u32 = 254;

// Glyph mappings
const GLYPH_WIRE_H: char = '-';      // MOVE_RIGHT/LEFT
const GLYPH_WIRE_V: char = '|';      // MOVE_UP/DOWN
const GLYPH_AND: char = '&';         // AND gate
const GLYPH_XOR: char = 'X';         // XOR gate
const GLYPH_REPLICATE: char = '*';   // Replicator
const GLYPH_INFECT: char = '@';      // Infect
const GLYPH_SIGNAL: char = '+';      // High signal
const GLYPH_EMPTY: char = ' ';       // Empty
const GLYPH_UNKNOWN: char = '?';     // Unknown opcode

fn opcode_to_glyph(opcode: u32, signal: u32) -> char {
    if signal > 200 {
        return GLYPH_SIGNAL;  // Active signal
    }
    
    match opcode {
        OP_MOVE_RIGHT | OP_MOVE_LEFT => GLYPH_WIRE_H,
        OP_MOVE_UP | OP_MOVE_DOWN => GLYPH_WIRE_V,
        OP_AND => GLYPH_AND,
        OP_XOR => GLYPH_XOR,
        OP_REPLICATE => GLYPH_REPLICATE,
        OP_INFECT => GLYPH_INFECT,
        OP_IDLE => '·',  // Idle agent (dim)
        0 => GLYPH_EMPTY,
        _ => GLYPH_UNKNOWN,
    }
}

fn glyph_to_opcode(glyph: char) -> Option<(u32, u32, u32)> {
    match glyph {
        '-' => Some((OP_MOVE_RIGHT, 0, 255)),
        '|' => Some((OP_MOVE_DOWN, 0, 255)),
        '&' => Some((OP_AND, 255, 255)),
        'X' => Some((OP_XOR, 255, 0)),
        '*' => Some((OP_REPLICATE, 255, 50)),
        '@' => Some((OP_INFECT, 50, 50)),
        '+' => Some((OP_MOVE_RIGHT, 255, 255)),  // Signal wire
        '·' => Some((OP_IDLE, 100, 100)),
        ' ' => None,
        _ => None,
    }
}

fn scan_region(x: u32, y: u32, width: u32, height: u32) -> Vec<String> {
    let mem_path = "/tmp/pixel-universe.mem";
    
    // Try to read from shared memory
    let data = match fs::read(mem_path) {
        Ok(d) => d,
        Err(_) => {
            eprintln!("Warning: Cannot read {} - agent not running?", mem_path);
            eprintln!("Returning empty grid.");
            return vec![" ".repeat(width as usize); height as usize];
        }
    };
    
    let grid_width = 480u32;
    let mut result = Vec::new();
    
    for row in 0..height {
        let mut line = String::new();
        
        for col in 0..width {
            let px = x + col;
            let py = y + row;
            let offset = ((py * grid_width + px) * 16) as usize;
            
            if offset + 16 <= data.len() {
                let opcode = u32::from_le_bytes([
                    data[offset], data[offset+1], data[offset+2], data[offset+3]
                ]);
                let _r = u32::from_le_bytes([
                    data[offset+4], data[offset+5], data[offset+6], data[offset+7]
                ]);
                let g = u32::from_le_bytes([
                    data[offset+8], data[offset+9], data[offset+10], data[offset+11]
                ]);
                let a = u32::from_le_bytes([
                    data[offset+12], data[offset+13], data[offset+14], data[offset+15]
                ]);
                
                // Use g as signal strength, a as type
                let signal = if a >= TYPE_AGENT { g } else { 0 };
                let glyph = opcode_to_glyph(opcode, signal);
                line.push(glyph);
            } else {
                line.push(GLYPH_EMPTY);
            }
        }
        
        result.push(line);
    }
    
    result
}

fn load_ascii_circuit(file: &str, offset_x: u32, offset_y: u32) {
    println!("Loading ASCII circuit: {}", file);
    
    let content = match fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            return;
        }
    };
    
    let mem_path = "/tmp/pixel-universe.mem";
    let mut file_handle = match fs::OpenOptions::new()
        .write(true)
        .open(mem_path)
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error opening {}: {}", mem_path, e);
            return;
        }
    };
    
    use std::io::{Seek, Write};
    
    let grid_width = 480u32;
    let mut pixel_count = 0;
    
    for (row, line) in content.lines().enumerate() {
        for (col, glyph) in line.chars().enumerate() {
            if let Some((opcode, r, b)) = glyph_to_opcode(glyph) {
                let px = offset_x + col as u32;
                let py = offset_y + row as u32;
                let offset = ((py * grid_width + px) * 16) as u64;
                
                // Write pixel to shared memory
                let pixel_data = [
                    &opcode.to_le_bytes()[..],
                    &r.to_le_bytes()[..],
                    &b.to_le_bytes()[..],
                    &TYPE_AGENT.to_le_bytes()[..],
                ].concat();
                
                file_handle.seek(std::io::SeekFrom::Start(offset)).ok();
                file_handle.write_all(&pixel_data).ok();
                
                pixel_count += 1;
            }
        }
    }
    
    println!("✓ Loaded {} pixels from ASCII", pixel_count);
}

fn watch_region(x: u32, y: u32, width: u32, height: u32) {
    use std::time::Duration;
    
    println!("Watching region ({}, {}) {}x{}", x, y, width, height);
    println!("Press Ctrl+C to stop\n");
    
    let mut last_grid = Vec::new();
    
    loop {
        let grid = scan_region(x, y, width, height);
        
        // Only redraw if changed
        if grid != last_grid {
            // Clear screen
            print!("\x1B[2J\x1B[1;1H");
            
            println!("┌{}┐", "─".repeat(width as usize));
            for line in &grid {
                println!("│{}│", line);
            }
            println!("└{}┘", "─".repeat(width as usize));
            
            // Count active pixels
            let active = grid.iter()
                .flat_map(|s| s.chars())
                .filter(|&c| c != ' ')
                .count();
            
            println!("\nActive pixels: {}", active);
            
            last_grid = grid;
        }
        
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn main() {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Scan { x, y, width, height, output } => {
            let grid = scan_region(x, y, width, height);
            
            let output_text = grid.join("\n");
            
            if let Some(file) = output {
                fs::write(&file, &output_text).ok();
                println!("✓ Saved circuit to {}", file);
            } else {
                println!("Scanned region ({}, {}) {}x{}:", x, y, width, height);
                println!("┌{}┐", "─".repeat(width as usize));
                for line in &grid {
                    println!("│{}│", line);
                }
                println!("└{}┘", "─".repeat(width as usize));
            }
            
            // Stats
            let active = grid.iter()
                .flat_map(|s| s.chars())
                .filter(|&c| c != ' ')
                .count();
            
            println!("\nStatistics:");
            println!("  Active pixels: {}", active);
            println!("  Empty pixels: {}", (width * height) as usize - active);
            
            // Glyph counts
            let mut glyph_counts = std::collections::HashMap::new();
            for line in &grid {
                for c in line.chars() {
                    if c != ' ' {
                        *glyph_counts.entry(c).or_insert(0) += 1;
                    }
                }
            }
            
            if !glyph_counts.is_empty() {
                println!("\nGlyph breakdown:");
                for (glyph, count) in glyph_counts.iter() {
                    let name = match *glyph {
                        '-' => "wire_h",
                        '|' => "wire_v",
                        '&' => "AND",
                        'X' => "XOR",
                        '*' => "replicate",
                        '@' => "infect",
                        '+' => "signal",
                        _ => "unknown",
                    };
                    println!("  {} ({}): {}", glyph, name, count);
                }
            }
        }
        Commands::Load { file, x, y } => {
            load_ascii_circuit(&file, x, y);
        }
        Commands::Watch { x, y, width, height } => {
            watch_region(x, y, width, height);
        }
    }
}
