// Circuit Heat-Map - Terminal visualization of live GPU signals
// Shows signal flow overlaid on ASCII circuit designs

use clap::Parser;
use std::io::{self, Write};
use std::time::Duration;

#[derive(Parser)]
#[command(name = "circuit-heatmap")]
#[command(about = "Terminal heat-map of GPU signal flow", long_about = None)]
struct Cli {
    /// ASCII circuit file to overlay
    #[arg(short, long)]
    file: String,
    
    /// X offset in GPU memory
    #[arg(long, default_value = "100")]
    offset_x: u32,
    
    /// Y offset in GPU memory
    #[arg(long, default_value = "100")]
    offset_y: u32,
    
    /// Update interval in milliseconds
    #[arg(short, long, default_value = "100")]
    interval: u64,
    
    /// Color mode: full, mono, or ascii
    #[arg(long, default_value = "full")]
    color: String,
}

// ANSI color codes
const RESET: &str = "\x1B[0m";
const DIM: &str = "\x1B[2m";
const BRIGHT: &str = "\x1B[1m";

// Signal intensity colors (0-9)
fn signal_color(signal: u32, mode: &str) -> String {
    match mode {
        "mono" => {
            if signal > 200 {
                format!("{}█{}", BRIGHT, RESET)
            } else if signal > 100 {
                "▓".to_string()
            } else if signal > 50 {
                "░".to_string()
            } else {
                "·".to_string()
            }
        }
        "ascii" => {
            if signal > 200 {
                "#".to_string()
            } else if signal > 150 {
                "+".to_string()
            } else if signal > 100 {
                "-".to_string()
            } else if signal > 50 {
                ".".to_string()
            } else {
                " ".to_string()
            }
        }
        _ => { // "full" color mode
            let color_code = match signal {
                200..=255 => 196, // Bright red
                150..=199 => 208, // Orange
                100..=149 => 226, // Yellow
                50..=99 => 46,   // Cyan
                1..=49 => 28,    // Dim green
                _ => 236,        // Dark gray
            };
            format!("\x1B[38;5;{}m", color_code)
        }
    }
}

fn read_gpu_region(x: u32, y: u32, width: u32, height: u32) -> Vec<Vec<(u32, u32)>> {
    let mem_path = "/tmp/pixel-universe.mem";
    
    let data = match std::fs::read(mem_path) {
        Ok(d) => d,
        Err(_) => {
            // Return empty grid if GPU not running
            return vec![vec![(0, 0); width as usize]; height as usize];
        }
    };
    
    let grid_width = 480u32;
    let mut result = Vec::new();
    
    for row in 0..height {
        let mut line = Vec::new();
        
        for col in 0..width {
            let px = x + col;
            let py = y + row;
            let offset = ((py * grid_width + px) * 16) as usize;
            
            if offset + 16 <= data.len() {
                let opcode = u32::from_le_bytes([
                    data[offset], data[offset+1], data[offset+2], data[offset+3]
                ]);
                let g = u32::from_le_bytes([
                    data[offset+8], data[offset+9], data[offset+10], data[offset+11]
                ]);
                let a = u32::from_le_bytes([
                    data[offset+12], data[offset+13], data[offset+14], data[offset+15]
                ]);
                
                let signal = if a >= 254 { g } else { 0 };
                line.push((opcode, signal));
            } else {
                line.push((0, 0));
            }
        }
        
        result.push(line);
    }
    
    result
}

fn render_heatmap(
    ascii_lines: &[String],
    gpu_data: &[Vec<(u32, u32)>],
    color_mode: &str,
) -> Vec<String> {
    let mut result = Vec::new();
    
    for (y, ascii_line) in ascii_lines.iter().enumerate() {
        let mut output_line = String::new();
        
        for (x, ch) in ascii_line.chars().enumerate() {
            if ch == ' ' {
                output_line.push(' ');
            } else {
                // Get signal from GPU data
                let signal = if y < gpu_data.len() && x < gpu_data[y].len() {
                    gpu_data[y][x].1
                } else {
                    0
                };
                
                if color_mode == "full" {
                    // Full color mode: colored character
                    let color = signal_color(signal, color_mode);
                    output_line.push_str(&format!("{}{}{}", color, ch, RESET));
                } else {
                    // Mono/ASCII mode: replace character with intensity
                    let intensity_char = signal_color(signal, color_mode);
                    output_line.push_str(&intensity_char);
                }
            }
        }
        
        result.push(output_line);
    }
    
    result
}

fn render_legend(color_mode: &str) -> String {
    let mut legend = String::new();
    
    legend.push_str("\n┌─────────────────────────────────┐\n");
    legend.push_str("│ SIGNAL INTENSITY                │\n");
    legend.push_str("├─────────────────────────────────┤\n");
    
    if color_mode == "full" {
        legend.push_str(&format!("│ {}████{} High (200-255)           │\n", 
            signal_color(255, color_mode), RESET));
        legend.push_str(&format!("│ {}████{} Medium (150-199)         │\n", 
            signal_color(175, color_mode), RESET));
        legend.push_str(&format!("│ {}████{} Low (100-149)            │\n", 
            signal_color(125, color_mode), RESET));
        legend.push_str(&format!("│ {}████{} Minimal (50-99)          │\n", 
            signal_color(75, color_mode), RESET));
        legend.push_str(&format!("│ {}····{} Idle (0-49)              │\n", 
            signal_color(25, color_mode), RESET));
    } else if color_mode == "mono" {
        legend.push_str("│ █ High (200+)                   │\n");
        legend.push_str("│ ▓ Medium (100-199)              │\n");
        legend.push_str("│ ░ Low (50-99)                   │\n");
        legend.push_str("│ · Idle (0-49)                   │\n");
    } else {
        legend.push_str("│ # High (200+)                   │\n");
        legend.push_str("│ + Medium (100-199)              │\n");
        legend.push_str("│ - Low (50-99)                   │\n");
        legend.push_str("│ . Idle (0-49)                   │\n");
    }
    
    legend.push_str("└─────────────────────────────────┘\n");
    
    legend
}

fn render_stats(gpu_data: &[Vec<(u32, u32)>]) -> String {
    let mut stats = String::new();
    
    let total_pixels = gpu_data.iter().map(|row| row.len()).sum::<usize>();
    let active_pixels = gpu_data.iter()
        .flat_map(|row| row.iter())
        .filter(|(_, signal)| *signal > 0)
        .count();
    
    let high_signal = gpu_data.iter()
        .flat_map(|row| row.iter())
        .filter(|(_, signal)| *signal > 200)
        .count();
    
    let avg_signal: u32 = if active_pixels > 0 {
        gpu_data.iter()
            .flat_map(|row| row.iter())
            .map(|(_, s)| *s)
            .filter(|s| *s > 0)
            .sum::<u32>() / active_pixels as u32
    } else {
        0
    };
    
    stats.push_str("\n┌─────────────────────────────────┐\n");
    stats.push_str("│ LIVE STATISTICS                 │\n");
    stats.push_str("├─────────────────────────────────┤\n");
    stats.push_str(&format!("│ Total pixels:    {:>14} │\n", total_pixels));
    stats.push_str(&format!("│ Active pixels:   {:>14} │\n", active_pixels));
    stats.push_str(&format!("│ High signal:     {:>14} │\n", high_signal));
    stats.push_str(&format!("│ Avg signal:      {:>14} │\n", avg_signal));
    stats.push_str("└─────────────────────────────────┘\n");
    
    stats
}

fn main() {
    let cli = Cli::parse();
    
    // Load ASCII circuit
    let ascii_content = match std::fs::read_to_string(&cli.file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            return;
        }
    };
    
    let ascii_lines: Vec<String> = ascii_content.lines().map(|s| s.to_string()).collect();
    
    if ascii_lines.is_empty() {
        eprintln!("Error: Empty circuit file");
        return;
    }
    
    let width = ascii_lines.iter().map(|s| s.len()).max().unwrap_or(0) as u32;
    let height = ascii_lines.len() as u32;
    
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║           CIRCUIT HEAT-MAP - Live Signal Flow             ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!();
    println!("Circuit: {}", cli.file);
    println!("Region: ({}, {}) {}x{}", cli.offset_x, cli.offset_y, width, height);
    println!("Color mode: {}", cli.color);
    println!("Update interval: {}ms", cli.interval);
    println!();
    println!("Press Ctrl+C to stop");
    println!();
    
    let color_mode = cli.color.as_str();
    
    loop {
        // Read GPU state
        let gpu_data = read_gpu_region(cli.offset_x, cli.offset_y, width, height);
        
        // Render heatmap
        let heatmap = render_heatmap(&ascii_lines, &gpu_data, color_mode);
        
        // Clear screen and move cursor to top
        print!("\x1B[2J\x1B[1;1H");
        
        println!("┌{}┐", "─".repeat(width as usize));
        for line in &heatmap {
            println!("│{}│", line);
        }
        println!("└{}┘", "─".repeat(width as usize));
        
        // Show legend and stats
        println!("{}", render_legend(color_mode));
        println!("{}", render_stats(&gpu_data));
        
        println!("\nLast update: {}", chrono::Local::now().format("%H:%M:%S%.3f"));
        
        io::stdout().flush().ok();
        
        std::thread::sleep(Duration::from_millis(cli.interval));
    }
}
