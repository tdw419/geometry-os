// hermes_bridge -- Bridge between Hermes chat and Geometry OS terminal
//
// Listens on a TCP port (default 9123). For each "SEND <text>" from a client,
// runs `hermes chat -Q -q "<text>" --resume <session>` and streams the
// response back line by line.
//
// Protocol:
//   Client sends:  "SEND <text>\n"          -- send a message to hermes
//   Server sends:  "LINE <text>\n"          -- one line of hermes output
//   Server sends:  "DONE\n"                 -- hermes finished responding
//   Server sends:  "READY\n"                -- initial handshake
//   Server sends:  "ERR <msg>\n"            -- error condition
//
// Uses --resume to maintain conversation context across sends.
//
// Usage:
//   hermes_bridge                    # listen on 0.0.0.0:9123
//   hermes_bridge 9124               # custom port
//   hermes_bridge 9124 --model xai/grok-4  # pass extra flags to hermes

use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port: u16 = args
        .get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(9123);

    // Collect extra args to pass to hermes chat (e.g. --model)
    let hermes_extra: Vec<String> = args[2..].to_vec();

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
        .unwrap_or_else(|e| {
            eprintln!("Failed to bind port {}: {}", port, e);
            std::process::exit(1);
        });

    eprintln!("hermes_bridge listening on port {}", port);
    eprintln!("Waiting for Geometry OS connection...");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let addr = stream.peer_addr().map(|a| a.to_string()).unwrap_or_else(|_| "unknown".into());
                eprintln!("Client connected from {}", addr);
                if let Err(e) = handle_client(stream, &hermes_extra) {
                    eprintln!("Client error: {}", e);
                }
                eprintln!("Client disconnected, waiting for next...");
            }
            Err(e) => eprintln!("Accept error: {}", e),
        }
    }
}

fn handle_client(mut stream: TcpStream, hermes_extra: &[String]) -> Result<(), String> {
    stream.set_nonblocking(false).map_err(|e| e.to_string())?;

    // Shared session ID for --resume
    let session_id: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    // Send initial READY
    send_line(&stream, "READY")?;

    stream.set_nonblocking(true).map_err(|e| e.to_string())?;
    let mut input_buf = vec![0u8; 4096];

    loop {
        match stream.read(&mut input_buf) {
            Ok(0) => break, // client disconnected
            Ok(n) => {
                let text = String::from_utf8_lossy(&input_buf[..n]);
                for line in text.lines() {
                    if let Some(msg) = line.strip_prefix("SEND ") {
                        eprintln!(">> {}", msg);
                        match run_hermes_query(msg, hermes_extra, &session_id) {
                            Ok(response) => {
                                // Send response lines
                                for resp_line in response.lines() {
                                    let trimmed = resp_line.trim();
                                    if trimmed.is_empty() {
                                        continue;
                                    }
                                    // Skip session_id line
                                    if trimmed.starts_with("session_id:") {
                                        // Extract session ID for --resume
                                        let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
                                        if parts.len() == 2 {
                                            let sid = parts[1].trim().to_string();
                                            let mut sid_lock = session_id.lock().unwrap();
                                            *sid_lock = Some(sid);
                                        }
                                        continue;
                                    }
                                    send_line(&stream, &format!("LINE {}", trimmed))?;
                                }
                                send_line(&stream, "DONE")?;
                            }
                            Err(e) => {
                                send_line(&stream, &format!("ERR {}", e))?;
                                send_line(&stream, "DONE")?;
                            }
                        }
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                eprintln!("Read error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

/// Run hermes chat -Q -q "<query>" with optional --resume and extra flags.
/// Returns the stdout output.
fn run_hermes_query(
    query: &str,
    extra_flags: &[String],
    session_id: &Arc<Mutex<Option<String>>>,
) -> Result<String, String> {
    let mut cmd = std::process::Command::new("hermes");
    cmd.args(["chat", "-Q", "-q", query]);

    // Resume session if we have one
    {
        let sid = session_id.lock().unwrap();
        if let Some(ref id) = *sid {
            cmd.arg("--resume");
            cmd.arg(id);
        }
    }

    // Add extra flags
    for flag in extra_flags {
        cmd.arg(flag);
    }

    cmd.env("TERM", "dumb");

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run hermes: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("hermes exited: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(stdout)
}

fn send_line(stream: &TcpStream, msg: &str) -> Result<(), String> {
    let mut s = stream.try_clone().map_err(|e| e.to_string())?;
    use std::io::Write;
    writeln!(s, "{}", msg).map_err(|e| e.to_string())
}
