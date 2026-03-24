// Signal Injector - CLI tool to inject signals into pixel agent grid
// Usage: signal-injector [command] [args]

use clap::{Parser, Subcommand};
use std::io::{self, Write};

#[derive(Parser)]
#[command(name = "signal-injector")]
#[command(about = "Inject signals into GPU pixel agent grid", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Inject a signal at specific coordinates
    Inject {
        /// X coordinate
        #[arg(short, long)]
        x: u32,
        
        /// Y coordinate
        #[arg(short, long)]
        y: u32,
        
        /// Opcode (0x00-0xFF or name)
        #[arg(short, long)]
        opcode: String,
        
        /// Red value (0-255)
        #[arg(short, long, default_value = "255")]
        red: u32,
        
        /// Green value (0-255)
        #[arg(short, long, default_value = "0")]
        green: u32,
        
        /// Blue value (0-255)
        #[arg(short, long, default_value = "0")]
        blue: u32,
    },
    
    /// Load a circuit template from JSON
    Load {
        /// Circuit template file
        #[arg(short, long)]
        file: Option<String>,
        
        /// X offset for placement
        #[arg(short, long, default_value = "0")]
        x: u32,
        
        /// Y offset for placement
        #[arg(short, long, default_value = "0")]
        y: u32,
        
        /// List available circuits
        #[arg(short, long)]
        list: bool,
    },
    
    /// Read pixel state at coordinates
    Read {
        /// X coordinate
        #[arg(short, long)]
        x: u32,
        
        /// Y coordinate
        #[arg(short, long)]
        y: u32,
    },
    
    /// Draw a wire (line of MOVE_RIGHT agents)
    Wire {
        /// Start X
        #[arg(long)]
        x1: u32,
        
        /// Start Y
        #[arg(long)]
        y1: u32,
        
        /// End X
        #[arg(long)]
        x2: u32,
        
        /// End Y
        #[arg(long)]
        y2: u32,
        
        /// Wire color (hex)
        #[arg(long, default_value = "00FF00")]
        color: String,
    },
    
    /// Place an AND gate
    AndGate {
        /// X coordinate
        #[arg(short, long)]
        x: u32,
        
        /// Y coordinate
        #[arg(short, long)]
        y: u32,
    },
    
    /// Place an XOR gate
    XorGate {
        /// X coordinate
        #[arg(short, long)]
        x: u32,
        
        /// Y coordinate
        #[arg(short, long)]
        y: u32,
    },
    
    /// Clear the grid
    Clear,
    
    /// Interactive mode (REPL)
    Interactive,
    
    /// Show grid statistics
    Stats,
}

// Opcodes
const OP_NOP: u32 = 0x00;
const OP_IDLE: u32 = 0x01;
const OP_MOVE_RIGHT: u32 = 0x02;
const OP_MOVE_LEFT: u32 = 0x03;
const OP_MOVE_UP: u32 = 0x04;
const OP_MOVE_DOWN: u32 = 0x05;
const OP_REPLICATE: u32 = 0x06;
const OP_INFECT: u32 = 0x07;
const OP_AND: u32 = 0x30;
const OP_XOR: u32 = 0x31;
const OP_EMIT_SIGNAL: u32 = 0x20;

const TYPE_AGENT: u32 = 254;

fn parse_opcode(s: &str) -> Result<u32, String> {
    let upper = s.to_uppercase();
    match upper.as_str() {
        "NOP" | "IDLE" => Ok(OP_IDLE),
        "RIGHT" | "MOVE_RIGHT" => Ok(OP_MOVE_RIGHT),
        "LEFT" | "MOVE_LEFT" => Ok(OP_MOVE_LEFT),
        "UP" | "MOVE_UP" => Ok(OP_MOVE_UP),
        "DOWN" | "MOVE_DOWN" => Ok(OP_MOVE_DOWN),
        "REPLICATE" | "COPY" => Ok(OP_REPLICATE),
        "INFECT" => Ok(OP_INFECT),
        "AND" => Ok(OP_AND),
        "XOR" => Ok(OP_XOR),
        "EMIT" | "SIGNAL" => Ok(OP_EMIT_SIGNAL),
        _ => {
            // Try parsing as hex
            if s.starts_with("0x") || s.starts_with("0X") {
                u32::from_str_radix(&s[2..], 16)
                    .map_err(|_| format!("Invalid hex opcode: {}", s))
            } else {
                s.parse::<u32>()
                    .map_err(|_| format!("Unknown opcode: {}", s))
            }
        }
    }
}

fn parse_color(hex: &str) -> Result<(u32, u32, u32), String> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Err("Color must be 6 hex digits (RRGGBB)".to_string());
    }
    
    let r = u32::from_str_radix(&hex[0..2], 16).map_err(|_| "Invalid red")?;
    let g = u32::from_str_radix(&hex[2..4], 16).map_err(|_| "Invalid green")?;
    let b = u32::from_str_radix(&hex[4..6], 16).map_err(|_| "Invalid blue")?;
    
    Ok((r, g, b))
}

fn create_pixel(opcode: u32, r: u32, g: u32, b: u32) -> [u8; 16] {
    let mut buf = [0u8; 16];
    buf[0..4].copy_from_slice(&opcode.to_le_bytes());
    buf[4..8].copy_from_slice(&r.to_le_bytes());
    buf[8..12].copy_from_slice(&g.to_le_bytes());
    buf[12..16].copy_from_slice(&TYPE_AGENT.to_le_bytes());
    buf
}

fn inject_at(x: u32, y: u32, opcode: u32, r: u32, g: u32, b: u32) {
    let width = 480u32;
    let offset = (y * width + x) * 16;
    
    // Write to shared memory file (if agent is running)
    let mem_path = "/tmp/pixel-universe.mem";
    
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .write(true)
        .open(mem_path)
    {
        use std::io::Seek;
        let pixel = create_pixel(opcode, r, g, b);
        file.seek(std::io::SeekFrom::Start(offset as u64)).ok();
        file.write_all(&pixel).ok();
        println!("✓ Injected {} at ({}, {})", opcode_name(opcode), x, y);
    } else {
        println!("⚠ Agent not running (no memory file at {})", mem_path);
        println!("  Start agent first: cargo run --release --bin agent");
        println!("  Or use --direct to write directly to grid state file");
    }
}

fn opcode_name(opcode: u32) -> &'static str {
    match opcode {
        OP_NOP => "NOP",
        OP_IDLE => "IDLE",
        OP_MOVE_RIGHT => "MOVE_RIGHT",
        OP_MOVE_LEFT => "MOVE_LEFT",
        OP_MOVE_UP => "MOVE_UP",
        OP_MOVE_DOWN => "MOVE_DOWN",
        OP_REPLICATE => "REPLICATE",
        OP_INFECT => "INFECT",
        OP_AND => "AND_GATE",
        OP_XOR => "XOR_GATE",
        OP_EMIT_SIGNAL => "EMIT_SIGNAL",
        _ => "UNKNOWN",
    }
}

fn draw_wire(x1: u32, y1: u32, x2: u32, y2: u32, color: &str) {
    let (r, g, b) = parse_color(color).unwrap_or((0, 255, 0));
    
    println!("Drawing wire from ({}, {}) to ({}, {})", x1, y1, x2, y2);
    
    // Bresenham's line algorithm
    let dx = (x2 as i32 - x1 as i32).abs();
    let dy = (y2 as i32 - y1 as i32).abs();
    let sx = if x1 < x2 { 1 } else { -1 };
    let sy = if y1 < y2 { 1 } else { -1 };
    let mut err = dx - dy;
    let mut x = x1 as i32;
    let mut y = y1 as i32;
    
    let mut count = 0;
    
    loop {
        // Determine direction based on line slope
        let opcode = if dx > dy {
            if sx > 0 { OP_MOVE_RIGHT } else { OP_MOVE_LEFT }
        } else {
            if sy > 0 { OP_MOVE_DOWN } else { OP_MOVE_UP }
        };
        
        inject_at(x as u32, y as u32, opcode, r, g, b);
        count += 1;
        
        if x == x2 as i32 && y == y2 as i32 {
            break;
        }
        
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
    
    println!("✓ Drew {} wire pixels", count);
}

fn interactive_mode() {
    println!("Signal Injector - Interactive Mode");
    println!("Commands: inject <x> <y> <opcode> [r] [g] [b], wire, gate, clear, quit");
    println!();
    
    loop {
        print!("pixel> ");
        io::stdout().flush().ok();
        
        let mut input = String::new();
        io::stdin().read_line(&mut input).ok();
        let input = input.trim();
        
        if input.is_empty() {
            continue;
        }
        
        let parts: Vec<&str> = input.split_whitespace().collect();
        
        match parts[0] {
            "quit" | "exit" | "q" => {
                println!("Goodbye!");
                break;
            }
            "inject" | "i" => {
                if parts.len() >= 4 {
                    if let (Ok(x), Ok(y), Ok(opcode)) = (
                        parts[1].parse::<u32>(),
                        parts[2].parse::<u32>(),
                        parse_opcode(parts[3]),
                    ) {
                        let r = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(255);
                        let g = parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
                        let b = parts.get(6).and_then(|s| s.parse().ok()).unwrap_or(0);
                        inject_at(x, y, opcode, r, g, b);
                    } else {
                        println!("Usage: inject <x> <y> <opcode> [r] [g] [b]");
                    }
                } else {
                    println!("Usage: inject <x> <y> <opcode> [r] [g] [b]");
                }
            }
            "wire" => {
                if parts.len() >= 5 {
                    if let (Ok(x1), Ok(y1), Ok(x2), Ok(y2)) = (
                        parts[1].parse::<u32>(),
                        parts[2].parse::<u32>(),
                        parts[3].parse::<u32>(),
                        parts[4].parse::<u32>(),
                    ) {
                        let color = parts.get(5).unwrap_or(&"00FF00");
                        draw_wire(x1, y1, x2, y2, color);
                    }
                } else {
                    println!("Usage: wire <x1> <y1> <x2> <y2> [color]");
                }
            }
            "gate" | "and" => {
                if parts.len() >= 3 {
                    if let (Ok(x), Ok(y)) = (parts[1].parse::<u32>(), parts[2].parse::<u32>()) {
                        inject_at(x, y, OP_AND, 255, 255, 255);
                        println!("✓ AND gate at ({}, {})", x, y);
                    }
                }
            }
            "xor" => {
                if parts.len() >= 3 {
                    if let (Ok(x), Ok(y)) = (parts[1].parse::<u32>(), parts[2].parse::<u32>()) {
                        inject_at(x, y, OP_XOR, 255, 0, 255);
                        println!("✓ XOR gate at ({}, {})", x, y);
                    }
                }
            }
            "clear" => {
                println!("Clearing grid...");
                // Would need to communicate with running agent
            }
            "help" | "?" => {
                println!("Commands:");
                println!("  inject <x> <y> <opcode> [r] [g] [b] - Inject signal");
                println!("  wire <x1> <y1> <x2> <y2> [color]  - Draw wire");
                println!("  gate <x> <y>                     - Place AND gate");
                println!("  xor <x> <y>                      - Place XOR gate");
                println!("  clear                            - Clear grid");
                println!("  quit                             - Exit");
            }
            _ => {
                println!("Unknown command: {}. Type 'help' for commands.", parts[0]);
            }
        }
    }
}

fn main() {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Inject { x, y, opcode, red, green, blue } => {
            match parse_opcode(&opcode) {
                Ok(op) => inject_at(x, y, op, red, green, blue),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        Commands::Read { x, y } => {
            println!("Reading pixel at ({}, {})...", x, y);
            // Would need to read from shared memory
            println!("  (Feature coming soon - requires running agent)");
        }
        Commands::Wire { x1, y1, x2, y2, color } => {
            draw_wire(x1, y1, x2, y2, &color);
        }
        Commands::AndGate { x, y } => {
            inject_at(x, y, OP_AND, 255, 255, 255);
            println!("✓ AND gate at ({}, {})", x, y);
        }
        Commands::XorGate { x, y } => {
            inject_at(x, y, OP_XOR, 255, 0, 255);
            println!("✓ XOR gate at ({}, {})", x, y);
        }
        Commands::Clear => {
            println!("Clearing grid...");
            println!("  (Feature coming soon - requires running agent)");
        }
        Commands::Interactive => {
            interactive_mode();
        }
        Commands::Stats => {
            println!("Grid Statistics:");
            println!("  Resolution: 480x240 (115,200 pixels)");
            println!("  Memory: /tmp/pixel-universe.mem");
            println!("  (Detailed stats require running agent)");
        }
        Commands::Load { file, x, y, list } => {
            if list {
                println!("Available circuits:");
                println!("  circuits/half-adder.json     — Half adder (A+B)");
                println!("  circuits/sr-flipflop.json    — SR flip-flop (1-bit memory)");
                println!("  circuits/clock-oscillator.json — Clock signal generator");
                println!();
                println!("Usage: load -f circuits/half-adder.json -x 100 -y 50");
            } else if let Some(f) = file {
                load_circuit(&f, x, y);
            } else {
                eprintln!("Error: --file required when not using --list");
            }
        }
    }
}

fn load_circuit(file: &str, offset_x: u32, offset_y: u32) {
    use std::fs;
    
    println!("Loading circuit: {}", file);
    
    let content = match fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            return;
        }
    };
    
    let circuit: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error parsing JSON: {}", e);
            return;
        }
    };
    
    let name = circuit["name"].as_str().unwrap_or("Unknown");
    let description = circuit["description"].as_str().unwrap_or("");
    
    println!("  Name: {}", name);
    println!("  Description: {}", description);
    println!("  Offset: ({}, {})", offset_x, offset_y);
    println!();
    
    // Load pixels
    if let Some(pixels) = circuit["pixels"].as_array() {
        let mut count = 0;
        
        for pixel in pixels {
            let px = pixel["x"].as_u64().unwrap_or(0) as u32 + offset_x;
            let py = pixel["y"].as_u64().unwrap_or(0) as u32 + offset_y;
            let opcode_str = pixel["opcode"].as_str().unwrap_or("IDLE");
            let r = pixel["r"].as_u64().unwrap_or(255) as u32;
            let g = pixel["g"].as_u64().unwrap_or(0) as u32;
            let b = pixel["b"].as_u64().unwrap_or(0) as u32;
            
            match parse_opcode(opcode_str) {
                Ok(opcode) => {
                    inject_at(px, py, opcode, r, g, b);
                    count += 1;
                }
                Err(e) => {
                    eprintln!("  Warning: {}", e);
                }
            }
        }
        
        println!("✓ Loaded {} pixels", count);
    }
    
    // Show inputs
    if let Some(inputs) = circuit["inputs"].as_array() {
        if !inputs.is_empty() {
            println!();
            println!("Inputs:");
            for input in inputs {
                let name = input["name"].as_str().unwrap_or("?");
                let ix = input["x"].as_u64().unwrap_or(0) as u32 + offset_x;
                let iy = input["y"].as_u64().unwrap_or(0) as u32 + offset_y;
                let desc = input["description"].as_str().unwrap_or("");
                println!("  {} at ({}, {}) — {}", name, ix, iy, desc);
            }
        }
    }
    
    // Show outputs
    if let Some(outputs) = circuit["outputs"].as_array() {
        if !outputs.is_empty() {
            println!();
            println!("Outputs:");
            for output in outputs {
                let name = output["name"].as_str().unwrap_or("?");
                let ox = output["x"].as_u64().unwrap_or(0) as u32 + offset_x;
                let oy = output["y"].as_u64().unwrap_or(0) as u32 + offset_y;
                let desc = output["description"].as_str().unwrap_or("");
                println!("  {} at ({}, {}) — {}", name, ox, oy, desc);
            }
        }
    }
}
