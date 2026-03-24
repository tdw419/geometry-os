// Pixel Network Bridge - Link GPU instances over the network
// Allows signals to travel between Geometry OS instances

use clap::{Parser, Subcommand};
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write, BufReader, BufWriter};
use std::thread;

#[derive(Parser)]
#[command(name = "pixel-bridge")]
#[command(about = "Network bridge for distributed pixel computation", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a bridge server (receive signals)
    Server {
        /// Port to listen on
        #[arg(short, long, default_value = "7890")]
        port: u16,
        
        /// X offset in local GPU memory
        #[arg(long, default_value = "400")]
        offset_x: u32,
        
        /// Y offset in local GPU memory
        #[arg(long, default_value = "0")]
        offset_y: u32,
    },
    
    /// Connect to a bridge server (send signals)
    Connect {
        /// Server address (host:port)
        #[arg(short, long)]
        server: String,
        
        /// Local X offset to watch
        #[arg(long, default_value = "0")]
        local_x: u32,
        
        /// Local Y offset to watch
        #[arg(long, default_value = "0")]
        local_y: u32,
        
        /// Width of region to watch
        #[arg(long, default_value = "80")]
        width: u32,
        
        /// Height of region to watch
        #[arg(long, default_value = "60")]
        height: u32,
    },
    
    /// Bidirectional bridge (both send and receive)
    Bridge {
        /// Port to listen on
        #[arg(long, default_value = "7890")]
        port: u16,
        
        /// Remote server to connect to
        #[arg(long)]
        remote: Option<String>,
        
        /// Local region X offset
        #[arg(long, default_value = "0")]
        local_x: u32,
        
        /// Local region Y offset
        #[arg(long, default_value = "0")]
        local_y: u32,
        
        /// Remote region X offset
        #[arg(long, default_value = "400")]
        remote_x: u32,
        
        /// Remote region Y offset
        #[arg(long, default_value = "0")]
        remote_y: u32,
        
        /// Region width
        #[arg(long, default_value = "80")]
        width: u32,
        
        /// Region height
        #[arg(long, default_value = "60")]
        height: u32,
    },
}

const GPU_WIDTH: u32 = 480;
const SHARED_MEM: &str = "/tmp/pixel-universe.mem";

/// Read pixel from GPU shared memory
fn read_pixel(x: u32, y: u32) -> Option<[u8; 16]> {
    let data = std::fs::read(SHARED_MEM).ok()?;
    let offset = ((y * GPU_WIDTH + x) * 16) as usize;
    
    if offset + 16 > data.len() {
        return None;
    }
    
    let mut buf = [0u8; 16];
    buf.copy_from_slice(&data[offset..offset+16]);
    Some(buf)
}

/// Write pixel to GPU shared memory
fn write_pixel(x: u32, y: u32, pixel: &[u8; 16]) -> std::io::Result<()> {
    use std::fs::OpenOptions;
    use std::io::{Seek, SeekFrom, Write};
    
    let mut file = OpenOptions::new().write(true).open(SHARED_MEM)?;
    let offset = ((y * GPU_WIDTH + x) * 16) as u64;
    file.seek(SeekFrom::Start(offset))?;
    file.write_all(pixel)?;
    
    Ok(())
}

/// Handle incoming client connection
fn handle_client(mut stream: TcpStream, offset_x: u32, offset_y: u32) {
    println!("Client connected from {}", stream.peer_addr().unwrap_or_else(|_| "unknown".parse().unwrap()));
    
    let mut buf = [0u8; 8];  // x, y as u32
    
    loop {
        match stream.read_exact(&mut buf) {
            Ok(_) => {
                let x = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
                let y = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
                
                // Read 16 bytes of pixel data
                let mut pixel_buf = [0u8; 16];
                if stream.read_exact(&mut pixel_buf).is_ok() {
                    // Write to local GPU at offset
                    let local_x = x + offset_x;
                    let local_y = y + offset_y;
                    
                    if write_pixel(local_x, local_y, &pixel_buf).is_ok() {
                        println!("Received pixel at ({}, {})", local_x, local_y);
                    }
                }
            }
            Err(_) => {
                println!("Client disconnected");
                break;
            }
        }
    }
}

/// Run bridge server
fn run_server(port: u16, offset_x: u32, offset_y: u32) {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
        .expect("Failed to bind port");
    
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║           PIXEL BRIDGE SERVER                            ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!();
    println!("Listening on port {}", port);
    println!("Incoming signals → offset ({}, {})", offset_x, offset_y);
    println!();
    println!("Waiting for connections...");
    println!();
    
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let ox = offset_x;
                let oy = offset_y;
                thread::spawn(move || {
                    handle_client(stream, ox, oy);
                });
            }
            Err(e) => eprintln!("Connection failed: {}", e),
        }
    }
}

/// Connect to remote bridge and send local pixels
fn run_connect(server: String, local_x: u32, local_y: u32, width: u32, height: u32) {
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║           PIXEL BRIDGE CLIENT                            ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!();
    println!("Connecting to {}...", server);
    
    let mut stream = TcpStream::connect(&server)
        .expect("Failed to connect");
    
    println!("Connected!");
    println!("Watching region ({}, {}) {}x{}", local_x, local_y, width, height);
    println!();
    
    let mut last_count = 0;
    
    loop {
        thread::sleep(std::time::Duration::from_millis(33));  // ~30 FPS
        
        let mut active_pixels = Vec::new();
        
        // Scan local region for active pixels
        for y in local_y..local_y+height {
            for x in local_x..local_x+width {
                if let Some(pixel) = read_pixel(x, y) {
                    // Check if pixel is active (a > 0)
                    let a = u32::from_le_bytes([pixel[12], pixel[13], pixel[14], pixel[15]]);
                    if a >= 254 {
                        active_pixels.push((x, y, pixel));
                    }
                }
            }
        }
        
        // Send new/changed pixels
        if active_pixels.len() != last_count {
            for (x, y, pixel) in &active_pixels {
                // Send coordinates (relative)
                let rel_x = x - local_x;
                let rel_y = y - local_y;
                
                let mut buf = Vec::new();
                buf.extend_from_slice(&rel_x.to_le_bytes());
                buf.extend_from_slice(&rel_y.to_le_bytes());
                buf.extend_from_slice(pixel);
                
                if stream.write_all(&buf).is_ok() {
                    // Success
                } else {
                    eprintln!("Connection lost");
                    return;
                }
            }
            
            println!("Sent {} pixels", active_pixels.len());
            last_count = active_pixels.len();
        }
    }
}

/// Run bidirectional bridge
fn run_bridge(port: u16, remote: Option<String>, local_x: u32, local_y: u32,
              remote_x: u32, remote_y: u32, width: u32, height: u32) {
    // Start server in background
    let server_port = port;
    let rx = remote_x;
    let ry = remote_y;
    
    thread::spawn(move || {
        run_server(server_port, rx, ry);
    });
    
    // Connect to remote if specified
    if let Some(remote_addr) = remote {
        thread::sleep(std::time::Duration::from_secs(1));  // Wait for server startup
        
        println!("╔═══════════════════════════════════════════════════════════╗");
        println!("║           PIXEL BRIDGE - BIDIRECTIONAL                   ║");
        println!("╚═══════════════════════════════════════════════════════════╝");
        println!();
        println!("Local region ({}, {}) → Remote offset ({}, {})", local_x, local_y, remote_x, remote_y);
        println!("Listening on port {} | Connected to {}", port, remote_addr);
        println!();
        
        run_connect(remote_addr, local_x, local_y, width, height);
    } else {
        println!("╔═══════════════════════════════════════════════════════════╗");
        println!("║           PIXEL BRIDGE - SERVER MODE                     ║");
        println!("╚═══════════════════════════════════════════════════════════╝");
        println!();
        println!("Listening on port {}", port);
        println!("Incoming signals → offset ({}, {})", remote_x, remote_y);
        println!();
        println!("(No remote specified - server only mode)");
        println!("Use --remote host:port to enable bidirectional bridge");
        println!();
        
        loop {
            thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}

fn main() {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Server { port, offset_x, offset_y } => {
            run_server(port, offset_x, offset_y);
        }
        Commands::Connect { server, local_x, local_y, width, height } => {
            run_connect(server, local_x, local_y, width, height);
        }
        Commands::Bridge { port, remote, local_x, local_y, remote_x, remote_y, width, height } => {
            run_bridge(port, remote, local_x, local_y, remote_x, remote_y, width, height);
        }
    }
}
