//! Geometry OS MCP Server
//!
//! Wraps the running Geometry OS VM via Unix socket (/tmp/geo_cmd.sock)
//! and exposes tools via Model Context Protocol (JSON-RPC over stdio).
//!
//! Usage:
//!   cargo run --bin geo_mcp_server
//!
//! The server reads JSON-RPC from stdin and writes responses to stdout.
//! Each tool call translates to one or more socket commands.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;

const SOCKET_PATH: &str = "/tmp/geo_cmd.sock";

fn send_socket_cmd(cmd: &str) -> Result<String, String> {
    let mut stream = UnixStream::connect(SOCKET_PATH)
        .map_err(|e| format!("Cannot connect to {}: {}", SOCKET_PATH, e))?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(|e| format!("Set timeout failed: {}", e))?;

    // Send command
    let mut writer = stream
        .try_clone()
        .map_err(|e| format!("Clone stream failed: {}", e))?;
    writeln!(writer, "{}", cmd).map_err(|e| format!("Write failed: {}", e))?;
    writer.flush().ok();
    drop(writer); // close write side so server sees EOF

    // Read response -- read all available data until EOF/timeout
    let mut response = String::new();
    let mut buf = [0u8; 65536];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break, // EOF
            Ok(n) => response.push_str(&String::from_utf8_lossy(&buf[..n])),
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) => return Err(format!("Read failed: {}", e)),
        }
    }

    Ok(response.trim().to_string())
}

// ── JSON-RPC Types ──────────────────────────────────────

#[derive(Debug)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    params: Option<serde_json::Value>,
}

#[derive(Debug)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug)]
struct JsonRpcError {
    code: i64,
    message: String,
}

impl JsonRpcResponse {
    fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }
    fn error(id: Option<serde_json::Value>, code: i64, msg: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: msg.into(),
            }),
        }
    }
}

// ── Tool Definitions ────────────────────────────────────

fn get_tool_list() -> Vec<serde_json::Value> {
    vec![
        // -- Available Now (wrap existing socket commands) --
        tool(
            "vm_status",
            "Get current VM state: mode, running, PC, cursor",
            vec![],
            vm_status_schema(),
        ),
        tool(
            "vm_screenshot",
            "Save framebuffer as PNG file",
            vec![param("path", "string", "Output file path", false)],
            vm_screenshot_schema(),
        ),
        tool(
            "vm_screen_dump",
            "Get raw 256x256 framebuffer hex data",
            vec![],
            vm_screen_dump_schema(),
        ),
        tool(
            "vm_registers",
            "Read all 32 registers",
            vec![],
            vm_registers_schema(),
        ),
        tool(
            "vm_canvas",
            "Read canvas text content",
            vec![],
            vm_canvas_schema(),
        ),
        tool(
            "vm_type",
            "Type text onto canvas",
            vec![param("text", "string", "Text to type", true)],
            vm_type_schema(),
        ),
        tool("vm_run", "Toggle VM execution", vec![], vm_run_schema()),
        tool(
            "vm_assemble",
            "Assemble canvas content to bytecode",
            vec![],
            vm_assemble_schema(),
        ),
        tool(
            "vm_disasm",
            "Disassemble instructions around PC",
            vec![],
            vm_disasm_schema(),
        ),
        tool("vm_save", "Save VM state to disk", vec![], vm_save_schema()),
        tool(
            "vm_save_asm",
            "Save canvas content to programs/<name>.asm on disk",
            vec![param("name", "string", "Program name (saved as programs/<name>.asm)", true)],
            vm_save_asm_schema(),
        ),
        tool(
            "vm_load_source",
            "Load multi-line ASM source into canvas (clears canvas first). Use \\n for newlines.",
            vec![param("source", "string", "Full ASM source text", true)],
            vm_load_source_schema(),
        ),
        // -- Phase 84: Building & Desktop Tools --
        tool(
            "building_list",
            "List buildings on the map, optionally filtered by radius from player",
            vec![param(
                "radius",
                "integer",
                "Max distance in tiles (0=all)",
                false,
            )],
            building_list_schema(),
        ),
        tool(
            "building_enter",
            "Walk player to building and enter it (launches the app)",
            vec![param("building_id", "string", "Building ID to enter", true)],
            building_enter_schema(),
        ),
        tool(
            "building_exit",
            "Exit current building, return to map",
            vec![],
            building_exit_schema(),
        ),
        tool(
            "desktop_state",
            "Get full desktop state: player, camera, buildings, taskbar, frame",
            vec![],
            desktop_state_schema(),
        ),
        tool(
            "desktop_launch",
            "Launch an app by name (opens window without walking to building)",
            vec![
                param("app_name", "string", "App name to launch", true),
                param("window", "boolean", "If true, launch in a WINSYS window instead of replacing map", false),
            ],
            desktop_launch_schema(),
        ),
        tool(
            "player_position",
            "Get player world coordinates and facing direction",
            vec![],
            player_position_schema(),
        ),
        // -- Phase 86: Hypervisor Tools --
        tool(
            "hypervisor_boot",
            "Boot a guest OS via hypervisor. Config string specifies arch, kernel, ram.",
            vec![
                param(
                    "config",
                    "string",
                    "Config: 'arch=riscv64 kernel=Image ram=256M'",
                    true,
                ),
                param(
                    "window_id",
                    "string",
                    "Optional WINSYS window ID to target",
                    false,
                ),
            ],
            hypervisor_boot_schema(),
        ),
        tool(
            "hypervisor_kill",
            "Kill running guest OS",
            vec![],
            hypervisor_kill_schema(),
        ),
        // -- Phase 88: AI Vision Bridge Tools --
        tool(
            "vision_screenshot",
            "Get base64-encoded PNG screenshot of the current canvas",
            vec![],
            vision_screenshot_schema(),
        ),
        tool(
            "vision_checksum",
            "Get FNV-1a checksum of the canvas (fast change detection)",
            vec![],
            vision_checksum_schema(),
        ),
        tool(
            "vision_diff",
            "Compare canvas against a previous checksum to detect changes",
            vec![param(
                "checksum",
                "string",
                "Previous FNV-1a checksum to compare against (hex)",
                true,
            )],
            vision_diff_schema(),
        ),
        // -- Phase 89: AI Input Injection Tools --
        tool(
            "input_key",
            "Inject a keyboard event into the running VM",
            vec![
                param(
                    "key",
                    "string",
                    "Key code (numeric) or single character",
                    true,
                ),
                param("shift", "integer", "Shift state (0=none, 1=shift)", false),
            ],
            input_key_schema(),
        ),
        tool(
            "input_mouse",
            "Inject a mouse event (move or click) into the running VM",
            vec![
                param("action", "string", "Action: 'move' or 'click'", true),
                param("x", "integer", "X coordinate", true),
                param("y", "integer", "Y coordinate", true),
                param(
                    "button",
                    "integer",
                    "Button for click (1=down, 2=click)",
                    false,
                ),
            ],
            input_mouse_schema(),
        ),
        tool(
            "input_text",
            "Type a text string into the VM (each character becomes a key event)",
            vec![param("text", "string", "Text to type", true)],
            input_text_schema(),
        ),
        // -- Phase 90: AI Program Control Tools --
        tool(
            "vm_screen_ascii",
            "Get the VM framebuffer as ASCII art (64x32). Readable by text-based AI agents.",
            vec![],
            vm_screen_ascii_schema(),
        ),
        tool(
            "vm_run_program",
            "One-shot: write assembly source, assemble, run, wait, return canvas + screen + status. The fastest way to execute a program and see results.",
            vec![
                param("source", "string", "Assembly source code to execute", true),
                param("frames", "integer", "Number of frames to run (default: 10000)", false),
            ],
            vm_run_program_schema(),
        ),
        // -- Phase 106: Window Management + Desktop Control Tools --
        tool(
            "window_list",
            "List all active WINSYS windows with id, title, pid, position, size, z_order",
            vec![],
            window_list_mcp_schema(),
        ),
        tool(
            "window_move",
            "Move a window to a new position",
            vec![
                param("window_id", "string", "Window ID to move", true),
                param("x", "integer", "New X position", true),
                param("y", "integer", "New Y position", true),
            ],
            window_move_mcp_schema(),
        ),
        tool(
            "window_close",
            "Close (destroy) a window",
            vec![param("window_id", "string", "Window ID to close", true)],
            window_close_mcp_schema(),
        ),
        tool(
            "window_focus",
            "Bring a window to the front (highest z-order)",
            vec![param("window_id", "string", "Window ID to focus", true)],
            window_focus_mcp_schema(),
        ),
        tool(
            "window_resize",
            "Resize a window's offscreen buffer",
            vec![
                param("window_id", "string", "Window ID to resize", true),
                param("w", "integer", "New width (1-256)", true),
                param("h", "integer", "New height (1-256)", true),
            ],
            window_resize_mcp_schema(),
        ),
        tool(
            "process_kill",
            "Kill all windows belonging to a process (PID)",
            vec![param("pid", "integer", "Process ID to kill windows for", true)],
            process_kill_mcp_schema(),
        ),
        tool(
            "desktop_key",
            "Inject a key event into the VM desktop (alias for input_key with desktop semantics)",
            vec![
                param("key", "string", "Key code (numeric) or single character", true),
                param("shift", "integer", "Shift state (0=none, 1=shift)", false),
            ],
            desktop_key_schema(),
        ),
        tool(
            "desktop_mouse_move",
            "Move the mouse cursor on the VM desktop",
            vec![
                param("x", "integer", "X coordinate", true),
                param("y", "integer", "Y coordinate", true),
            ],
            desktop_mouse_move_schema(),
        ),
        tool(
            "desktop_mouse_click",
            "Click the mouse at a position on the VM desktop",
            vec![
                param("x", "integer", "X coordinate", true),
                param("y", "integer", "Y coordinate", true),
                param("button", "integer", "Button (1=down, 2=click)", false),
            ],
            desktop_mouse_click_schema(),
        ),
        tool(
            "desktop_vision",
            "Get structured desktop state: open windows, focused window, ASCII art overlay of the desktop with window boundaries",
            vec![],
            desktop_vision_schema(),
        ),
    ]
}

fn tool(
    name: &str,
    desc: &str,
    params: Vec<serde_json::Value>,
    output: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "description": desc,
        "inputSchema": {
            "type": "object",
            "properties": params.iter().map(|p| {
                let pname = p["name"].as_str().unwrap();
                (pname.to_string(), p.clone())
            }).collect::<HashMap<_,_>>(),
            "required": params.iter()
                .filter(|p| p["required"].as_bool().unwrap_or(false))
                .map(|p| p["name"].as_str().unwrap().to_string())
                .collect::<Vec<_>>(),
        },
        "outputSchema": output,
    })
}

fn param(name: &str, ptype: &str, desc: &str, required: bool) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "type": ptype,
        "description": desc,
        "required": required,
    })
}

// ── Output Schemas ──────────────────────────────────────

fn vm_status_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "mode": {"type": "string"},
            "running": {"type": "boolean"},
            "assembled": {"type": "boolean"},
            "pc": {"type": "string"},
            "cursor": {"type": "array", "items": {"type": "integer"}},
        }
    })
}
fn vm_screenshot_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}})
}
fn vm_screen_dump_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"width": {"type": "integer"}, "height": {"type": "integer"}, "pixels": {"type": "string"}}})
}
fn vm_registers_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"registers": {"type": "object"}}})
}
fn vm_canvas_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"lines": {"type": "array"}}})
}
fn vm_type_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"ok": {"type": "boolean"}}})
}
fn vm_run_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"running": {"type": "boolean"}}})
}
fn vm_assemble_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"ok": {"type": "boolean"}}})
}
fn vm_disasm_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"instructions": {"type": "array"}}})
}
fn vm_save_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"ok": {"type": "boolean"}}})
}
fn vm_save_asm_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"ok": {"type": "boolean"}, "filename": {"type": "string"}, "lines": {"type": "integer"}}})
}
fn vm_load_source_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"ok": {"type": "boolean"}, "cursor": {"type": "string"}}})
}
fn building_list_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"buildings": {"type": "array", "items": {"type": "object", "properties": {"id": {"type": "integer"}, "world_x": {"type": "integer"}, "world_y": {"type": "integer"}, "type_color": {"type": "string"}, "name": {"type": "string"}}}}}})
}
fn building_enter_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"entered": {"type": "boolean"}, "app_name": {"type": "string"}}})
}
fn building_exit_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"world_x": {"type": "integer"}, "world_y": {"type": "integer"}}})
}
fn desktop_state_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"player": {"type": "object"}, "camera": {"type": "object"}, "frame": {"type": "integer"}, "buildings": {"type": "array"}}})
}
fn desktop_launch_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"launched": {"type": "boolean"}, "app_name": {"type": "string"}, "window_mode": {"type": "boolean"}}})
}
fn player_position_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"world_x": {"type": "integer"}, "world_y": {"type": "integer"}, "facing": {"type": "string"}}})
}
fn hypervisor_boot_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"booted": {"type": "boolean"}, "config": {"type": "string"}, "window_id": {"type": "string"}}})
}
fn hypervisor_kill_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"ok": {"type": "boolean"}}})
}

// Phase 88: AI Vision Bridge schemas
fn vision_screenshot_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "png_base64": {"type": "string", "description": "Base64-encoded PNG image of the 256x256 canvas"},
            "width": {"type": "integer"},
            "height": {"type": "integer"}
        }
    })
}

fn vision_checksum_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "checksum": {"type": "string", "description": "FNV-1a hash of the canvas (hex)"},
            "checksum_u32": {"type": "integer", "description": "FNV-1a hash as u32"}
        }
    })
}

fn vision_diff_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "changed": {"type": "boolean"},
            "checksum": {"type": "string", "description": "Current checksum (hex)"},
            "previous_checksum": {"type": "string", "description": "Previous checksum (hex)"}
        }
    })
}

// Phase 89: AI Input Injection schemas
fn input_key_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "ok": {"type": "boolean"},
            "keycode": {"type": "integer"}
        }
    })
}
fn input_mouse_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "ok": {"type": "boolean"},
            "action": {"type": "string"},
            "x": {"type": "integer"},
            "y": {"type": "integer"}
        }
    })
}
fn input_text_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "ok": {"type": "boolean"},
            "chars_injected": {"type": "integer"}
        }
    })
}
fn vm_screen_ascii_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "ascii": {"type": "string"},
            "width": {"type": "integer"},
            "height": {"type": "integer"}
        }
    })
}
fn vm_run_program_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "success": {"type": "boolean"},
            "canvas": {"type": "string"},
            "screen_ascii": {"type": "string"},
            "status": {"type": "object"},
            "registers": {"type": "object"},
            "error": {"type": "string"}
        }
    })
}

// Phase 106: Window Management + Desktop Control schemas
fn window_list_mcp_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "count": {"type": "integer"},
            "windows": {"type": "array", "items": {"type": "object", "properties": {
                "id": {"type": "integer"}, "x": {"type": "integer"}, "y": {"type": "integer"},
                "w": {"type": "integer"}, "h": {"type": "integer"},
                "z_order": {"type": "integer"}, "pid": {"type": "integer"}, "title": {"type": "string"}
            }}}
        }
    })
}
fn window_move_mcp_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {
        "ok": {"type": "boolean"}, "window_id": {"type": "string"}, "x": {"type": "integer"}, "y": {"type": "integer"}
    }})
}
fn window_close_mcp_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"ok": {"type": "boolean"}, "window_id": {"type": "string"}}})
}
fn window_focus_mcp_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {
        "ok": {"type": "boolean"}, "window_id": {"type": "string"}, "z_order": {"type": "integer"}
    }})
}
fn window_resize_mcp_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {
        "ok": {"type": "boolean"}, "window_id": {"type": "string"}, "w": {"type": "integer"}, "h": {"type": "integer"}
    }})
}
fn process_kill_mcp_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {
        "ok": {"type": "boolean"}, "pid": {"type": "integer"}, "windows_killed": {"type": "integer"}
    }})
}
fn desktop_key_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {
        "ok": {"type": "boolean"}, "keycode": {"type": "integer"}
    }})
}
fn desktop_mouse_move_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {
        "ok": {"type": "boolean"}, "x": {"type": "integer"}, "y": {"type": "integer"}
    }})
}
fn desktop_mouse_click_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {
        "ok": {"type": "boolean"}, "x": {"type": "integer"}, "y": {"type": "integer"}, "button": {"type": "integer"}
    }})
}
fn desktop_vision_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "windows": {"type": "array", "items": {"type": "object", "properties": {
                "id": {"type": "integer"}, "x": {"type": "integer"}, "y": {"type": "integer"},
                "w": {"type": "integer"}, "h": {"type": "integer"},
                "z_order": {"type": "integer"}, "pid": {"type": "integer"}, "title": {"type": "string"}
            }}},
            "focused_window": {"type": "integer"},
            "ascii_desktop": {"type": "string"}
        }
    })
}
// ── Tool Handlers ───────────────────────────────────────

fn handle_tool_call(name: &str, args: &serde_json::Value) -> Result<serde_json::Value, String> {
    match name {
        "vm_status" => {
            let resp = send_socket_cmd("status")?;
            // Parse "mode=Terminal running=false assembled=false pc=0x0000 cursor=(0,0)"
            let mut result = serde_json::Map::new();
            for part in resp.split_whitespace() {
                if let Some((k, v)) = part.split_once('=') {
                    match k {
                        "mode" => result.insert(
                            "mode".into(),
                            serde_json::Value::String(v.trim_end_matches(',').into()),
                        ),
                        "running" => {
                            result.insert("running".into(), serde_json::Value::Bool(v == "true"))
                        }
                        "assembled" => {
                            result.insert("assembled".into(), serde_json::Value::Bool(v == "true"))
                        }
                        "pc" => result.insert("pc".into(), serde_json::Value::String(v.into())),
                        _ => None,
                    };
                }
                if part.starts_with("cursor=") {
                    let inner = part.trim_start_matches("cursor=(").trim_end_matches(')');
                    let coords: Vec<&str> = inner.split(',').collect();
                    if coords.len() == 2 {
                        result.insert(
                            "cursor".into(),
                            serde_json::json!([
                                coords[0].parse::<i64>().unwrap_or(0),
                                coords[1].parse::<i64>().unwrap_or(0)
                            ]),
                        );
                    }
                }
            }
            Ok(serde_json::Value::Object(result))
        }

        "vm_screenshot" => {
            let path = args["path"].as_str().unwrap_or("screenshot.png");
            let resp = send_socket_cmd(&format!("screenshot {}", path))?;
            Ok(serde_json::json!({ "path": path, "response": resp }))
        }

        "vm_screen_dump" => {
            let resp = send_socket_cmd("screen")?;
            Ok(serde_json::json!({ "width": 256, "height": 256, "pixels": resp }))
        }

        "vm_registers" => {
            let resp = send_socket_cmd("registers")?;
            let mut regs = serde_json::Map::new();
            for line in resp.lines() {
                if let Some((name, val)) = line.split_once('=') {
                    regs.insert(name.into(), serde_json::Value::String(val.into()));
                }
            }
            Ok(serde_json::json!({ "registers": serde_json::Value::Object(regs) }))
        }

        "vm_canvas" => {
            let resp = send_socket_cmd("canvas")?;
            let lines: Vec<serde_json::Value> = resp
                .lines()
                .map(|l| {
                    if let Some((row, text)) = l.split_once('|') {
                        serde_json::json!({ "row": row.parse::<i64>().unwrap_or(0), "text": text })
                    } else {
                        serde_json::json!({ "row": 0, "text": l })
                    }
                })
                .collect();
            Ok(serde_json::json!({ "lines": lines }))
        }

        "vm_type" => {
            let text = args["text"].as_str().ok_or("Missing 'text' parameter")?;
            let resp = send_socket_cmd(&format!("type {}", text))?;
            Ok(serde_json::json!({ "ok": true, "response": resp }))
        }

        "vm_run" => {
            let resp = send_socket_cmd("run")?;
            Ok(serde_json::json!({ "response": resp }))
        }

        "vm_assemble" => {
            let resp = send_socket_cmd("assemble")?;
            Ok(serde_json::json!({ "ok": true, "response": resp }))
        }

        "vm_disasm" => {
            let resp = send_socket_cmd("disasm")?;
            let instructions: Vec<serde_json::Value> = resp
                .lines()
                .map(|l| {
                    if let Some((addr, text)) = l.split_once(':') {
                        serde_json::json!({ "addr": addr.trim(), "text": text.trim() })
                    } else {
                        serde_json::json!({ "addr": "???", "text": l })
                    }
                })
                .collect();
            Ok(serde_json::json!({ "instructions": instructions }))
        }

        "vm_save" => {
            let resp = send_socket_cmd("save")?;
            Ok(serde_json::json!({ "ok": true, "response": resp }))
        }

        "vm_save_asm" => {
            let name = args["name"].as_str().ok_or("Missing 'name' parameter")?;
            let resp = send_socket_cmd(&format!("save_asm {}", name))?;
            let ok = resp.contains("[saved:");
            Ok(serde_json::json!({ "ok": ok, "response": resp }))
        }

        "vm_load_source" => {
            let source = args["source"]
                .as_str()
                .ok_or("Missing 'source' parameter")?;
            // Convert real newlines to \n (backslash-n) for socket protocol
            let escaped = source.replace('\n', "\\n");
            let resp = send_socket_cmd(&format!("load_source {}", escaped))?;
            let ok = resp.contains("[loaded:");
            Ok(serde_json::json!({ "ok": ok, "response": resp }))
        }

        // ── Phase 84: Building & Desktop Tool Handlers ──────
        "building_list" => {
            let radius = args["radius"].as_i64().unwrap_or(256);
            let cmd = if radius > 0 {
                format!("buildings {}", radius)
            } else {
                "buildings 0".to_string()
            };
            let resp = send_socket_cmd(&cmd)?;
            let mut buildings = Vec::new();
            for line in resp.lines() {
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() >= 5 {
                    buildings.push(serde_json::json!({
                        "id": parts[0].parse::<i64>().unwrap_or(0),
                        "world_x": parts[1].parse::<i64>().unwrap_or(0),
                        "world_y": parts[2].parse::<i64>().unwrap_or(0),
                        "type_color": parts[3],
                        "name": parts[4],
                    }));
                }
            }
            Ok(serde_json::json!({ "buildings": buildings }))
        }

        "building_enter" => {
            let building_id = args["building_id"]
                .as_str()
                .ok_or("Missing 'building_id' parameter")?;
            let resp = send_socket_cmd(&format!("buildings 0"))?;
            // Find the building name from the list
            let mut app_name = String::new();
            for line in resp.lines() {
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() >= 5 && parts[0] == building_id {
                    app_name = parts[4].to_string();
                    break;
                }
            }
            if app_name.is_empty() {
                return Err(format!("Building {} not found", building_id));
            }
            let launch_resp = send_socket_cmd(&format!("launch {}", app_name))?;
            Ok(serde_json::json!({
                "entered": !launch_resp.contains("not found"),
                "app_name": app_name,
            }))
        }

        "building_exit" => {
            let resp = send_socket_cmd("player_pos")?;
            let parts: Vec<&str> = resp.split(',').collect();
            let (wx, wy) = if parts.len() >= 2 {
                (
                    parts[0].parse::<i64>().unwrap_or(0),
                    parts[1].parse::<i64>().unwrap_or(0),
                )
            } else {
                (0, 0)
            };
            Ok(serde_json::json!({ "world_x": wx, "world_y": wy }))
        }

        "desktop_state" => {
            let resp = send_socket_cmd("desktop_json")?;
            // Parse the JSON-ish response from socket
            match serde_json::from_str::<serde_json::Value>(&resp.trim()) {
                Ok(v) => Ok(v),
                Err(_) => Ok(serde_json::json!({ "raw": resp })),
            }
        }

        "desktop_launch" => {
            let app_name = args["app_name"]
                .as_str()
                .ok_or("Missing 'app_name' parameter")?;
            let window_mode = args["window"].as_bool().unwrap_or(false);
            let cmd = if window_mode {
                format!("launch --window {}", app_name)
            } else {
                format!("launch {}", app_name)
            };
            let resp = send_socket_cmd(&cmd)?;
            let launched = resp.contains("launching") || resp.contains("windowed");
            Ok(serde_json::json!({
                "launched": launched,
                "app_name": app_name,
                "window_mode": window_mode,
                "response": resp,
            }))
        }

        "player_position" => {
            let resp = send_socket_cmd("player_pos")?;
            let parts: Vec<&str> = resp.split(',').collect();
            if parts.len() >= 3 {
                Ok(serde_json::json!({
                    "world_x": parts[0].parse::<i64>().unwrap_or(0),
                    "world_y": parts[1].parse::<i64>().unwrap_or(0),
                    "facing": parts[2].trim(),
                }))
            } else {
                Ok(serde_json::json!({ "raw": resp }))
            }
        }

        // Phase 86: Hypervisor tools
        "hypervisor_boot" => {
            let config = args["config"]
                .as_str()
                .ok_or("Missing 'config' parameter")?;
            let window_id = args["window_id"].as_str().unwrap_or("0");
            let cmd = if window_id != "0" {
                format!("hypervisor_boot {} window={}", config, window_id)
            } else {
                format!("hypervisor_boot {}", config)
            };
            let resp = send_socket_cmd(&cmd)?;
            let booted = resp.contains("booted");
            Ok(serde_json::json!({
                "booted": booted,
                "config": config,
                "window_id": window_id,
                "response": resp,
            }))
        }

        "hypervisor_kill" => {
            let resp = send_socket_cmd("hypervisor_kill")?;
            Ok(serde_json::json!({
                "ok": resp.contains("killed"),
                "response": resp,
            }))
        }

        // Phase 88: AI Vision Bridge tools
        "vision_screenshot" => {
            let resp = send_socket_cmd("screenshot_annotated_b64")?;
            let wins_resp = send_socket_cmd("window_list")?;
            Ok(serde_json::json!({
                "png_base64": resp,
                "width": 256,
                "height": 256,
                "windows": wins_resp,
            }))
        }

        "vision_checksum" => {
            let resp = send_socket_cmd("canvas_checksum")?;
            Ok(serde_json::json!({
                "checksum": format!("{:08X}", resp.parse::<u64>().unwrap_or(0)),
                "checksum_u32": resp.parse::<u64>().unwrap_or(0),
            }))
        }

        "vision_diff" => {
            let prev_checksum = args["checksum"].as_str().unwrap_or("0");
            let resp = send_socket_cmd(&format!("canvas_diff {}", prev_checksum))?;
            Ok(serde_json::json!({
                "changed": !resp.contains("same"),
                "checksum": resp,
                "previous_checksum": prev_checksum,
            }))
        }

        // ── Phase 89: AI Input Injection Tool Handlers ──
        "input_key" => {
            let key_str = args["key"].as_str().unwrap_or("0");
            let cmd = format!("inject_key {}", key_str);
            let resp = send_socket_cmd(&cmd)?;
            Ok(serde_json::json!({
                "ok": resp.contains("ok"),
                "response": resp,
            }))
        }

        "input_mouse" => {
            let action = args["action"].as_str().unwrap_or("move");
            let x = args["x"].as_i64().unwrap_or(0);
            let y = args["y"].as_i64().unwrap_or(0);
            let button = args["button"].as_i64().unwrap_or(2);
            let cmd = match action {
                "click" => format!("inject_mouse click {} {} {}", x, y, button),
                _ => format!("inject_mouse move {} {}", x, y),
            };
            let resp = send_socket_cmd(&cmd)?;
            Ok(serde_json::json!({
                "ok": resp.contains("ok"),
                "action": action,
                "x": x,
                "y": y,
            }))
        }

        "input_text" => {
            let text = args["text"].as_str().unwrap_or("");
            let cmd = format!("inject_text {}", text);
            let resp = send_socket_cmd(&cmd)?;
            Ok(serde_json::json!({
                "ok": true,
                "response": resp,
            }))
        }

        // ── Phase 90: AI Program Control Handlers ──────
        "vm_screen_ascii" => {
            let resp = send_socket_cmd("vmscreen")?;
            Ok(serde_json::json!({
                "ascii": resp,
                "width": 64,
                "height": 32,
            }))
        }

        "vm_run_program" => {
            let source = args["source"]
                .as_str()
                .ok_or("Missing 'source' parameter")?;
            let _frames = args["frames"].as_i64().unwrap_or(10000) as usize;

            // Step 1: Write source to a temp file
            let tmp_path = "/tmp/geo_mcp_program.geo";
            std::fs::write(tmp_path, source)
                .map_err(|e| format!("Failed to write temp file: {}", e))?;

            // Step 2: Load source file into canvas
            let load_result = send_socket_cmd(&format!("load {}", tmp_path))?;
            if load_result.contains("error") {
                return Ok(serde_json::json!({
                    "success": false,
                    "error": load_result,
                }));
            }

            // Step 3: Assemble
            let asm_result = send_socket_cmd("assemble")?;
            if asm_result.contains("error")
                || asm_result.contains("Error")
                || asm_result.contains("ASM ERROR")
            {
                return Ok(serde_json::json!({
                    "success": false,
                    "error": asm_result,
                    "canvas": send_socket_cmd("canvas").unwrap_or_default(),
                }));
            }

            // Step 4: Run
            send_socket_cmd("run").ok();

            // Wait for execution by polling status
            let mut waited = 0;
            let poll_interval = std::time::Duration::from_millis(50);
            loop {
                std::thread::sleep(poll_interval);
                waited += 1;
                if let Ok(status) = send_socket_cmd("status") {
                    if status.contains("running=false") || status.contains("halted=true") {
                        break;
                    }
                }
                if waited * 50 > 10000 {
                    send_socket_cmd("halt").ok();
                    break;
                }
            }

            // Step 5: Collect results
            let canvas = send_socket_cmd("canvas").unwrap_or_default();
            let screen_ascii = send_socket_cmd("vmscreen").unwrap_or_default();
            let status_resp = send_socket_cmd("status").unwrap_or_default();
            let regs_resp = send_socket_cmd("registers").unwrap_or_default();

            // Parse status
            let mut status_obj = serde_json::Map::new();
            for part in status_resp.split_whitespace() {
                if let Some((k, v)) = part.split_once('=') {
                    match k {
                        "mode" => {
                            status_obj.insert("mode".into(), serde_json::Value::String(v.into()))
                        }
                        "running" => status_obj
                            .insert("running".into(), serde_json::Value::Bool(v == "true")),
                        "assembled" => status_obj
                            .insert("assembled".into(), serde_json::Value::Bool(v == "true")),
                        "pc" => status_obj.insert("pc".into(), serde_json::Value::String(v.into())),
                        _ => None,
                    };
                }
            }

            // Parse registers
            let mut regs_obj = serde_json::Map::new();
            for line in regs_resp.lines() {
                if let Some((name, val)) = line.split_once('=') {
                    regs_obj.insert(name.into(), serde_json::Value::String(val.into()));
                }
            }

            Ok(serde_json::json!({
                "success": true,
                "canvas": canvas,
                "screen_ascii": screen_ascii,
                "status": serde_json::Value::Object(status_obj),
                "registers": serde_json::Value::Object(regs_obj),
            }))
        }

        // ── Phase 106: AI Desktop Control Tool Handlers ──
        "window_list" => {
            let resp = send_socket_cmd("window_list")?;
            // Socket now returns JSON array directly: [{"id":...,"title":...}, ...]
            let resp_trimmed = resp.trim();
            let windows: Vec<serde_json::Value> = if resp_trimmed.starts_with('[') {
                serde_json::from_str(resp_trimmed).unwrap_or_default()
            } else {
                Vec::new()
            };
            Ok(serde_json::json!({
                "count": windows.len(),
                "windows": windows,
            }))
        }

        "window_move" => {
            let win_id = args["window_id"].as_str().unwrap_or("0");
            let x = args["x"].as_i64().unwrap_or(0);
            let y = args["y"].as_i64().unwrap_or(0);
            let cmd = format!("window_move {} {} {}", win_id, x, y);
            let resp = send_socket_cmd(&cmd)?;
            Ok(serde_json::json!({
                "ok": resp.contains("ok"),
                "window_id": win_id,
                "x": x,
                "y": y,
                "response": resp,
            }))
        }

        "window_close" => {
            let win_id = args["window_id"].as_str().unwrap_or("0");
            let cmd = format!("window_close {}", win_id);
            let resp = send_socket_cmd(&cmd)?;
            Ok(serde_json::json!({
                "ok": resp.contains("ok"),
                "window_id": win_id,
                "response": resp,
            }))
        }

        "window_focus" => {
            let win_id = args["window_id"].as_str().unwrap_or("0");
            let cmd = format!("window_focus {}", win_id);
            let resp = send_socket_cmd(&cmd)?;
            Ok(serde_json::json!({
                "ok": resp.contains("ok"),
                "window_id": win_id,
                "response": resp,
            }))
        }

        "window_resize" => {
            let win_id = args["window_id"].as_str().unwrap_or("0");
            let w = args["w"].as_i64().unwrap_or(64);
            let h = args["h"].as_i64().unwrap_or(48);
            let cmd = format!("window_resize {} {} {}", win_id, w, h);
            let resp = send_socket_cmd(&cmd)?;
            Ok(serde_json::json!({
                "ok": resp.contains("ok"),
                "window_id": win_id,
                "w": w,
                "h": h,
                "response": resp,
            }))
        }

        "process_kill" => {
            let pid = args["pid"].as_i64().unwrap_or(0);
            let cmd = format!("process_kill {}", pid);
            let resp = send_socket_cmd(&cmd)?;
            Ok(serde_json::json!({
                "ok": resp.contains("ok"),
                "pid": pid,
                "response": resp,
            }))
        }

        "desktop_key" => {
            let key = args["key"].as_str().unwrap_or("0");
            let shift = args["shift"].as_i64().unwrap_or(0);
            let cmd = format!("inject_key {} {}", key, shift);
            let resp = send_socket_cmd(&cmd)?;
            Ok(serde_json::json!({
                "ok": resp.contains("ok"),
                "key": key,
                "shift": shift,
                "response": resp,
            }))
        }

        "desktop_mouse_move" => {
            let x = args["x"].as_i64().unwrap_or(0);
            let y = args["y"].as_i64().unwrap_or(0);
            let cmd = format!("inject_mouse move {} {}", x, y);
            let resp = send_socket_cmd(&cmd)?;
            Ok(serde_json::json!({
                "ok": resp.contains("ok"),
                "x": x,
                "y": y,
                "response": resp,
            }))
        }

        "desktop_mouse_click" => {
            let x = args["x"].as_i64().unwrap_or(0);
            let y = args["y"].as_i64().unwrap_or(0);
            let button = args["button"].as_i64().unwrap_or(2);
            let cmd = format!("inject_mouse click {} {} {}", x, y, button);
            let resp = send_socket_cmd(&cmd)?;
            Ok(serde_json::json!({
                "ok": resp.contains("ok"),
                "x": x,
                "y": y,
                "button": button,
                "response": resp,
            }))
        }

        "desktop_vision" => {
            // Socket desktop_vision now returns JSON directly:
            // {"windows":[...], "focused_window":{...}, "ascii_desktop":"..."}
            let resp = send_socket_cmd("desktop_vision")?;
            let trimmed = resp.trim();
            let parsed: serde_json::Value = if trimmed.starts_with('{') {
                serde_json::from_str(trimmed).unwrap_or_else(|_| serde_json::json!({}))
            } else {
                serde_json::json!({})
            };
            Ok(parsed)
        }
        _ => Err(format!("Unknown tool: {}", name)),
    }
}

// ── JSON-RPC Dispatch ───────────────────────────────────

fn handle_request(request: JsonRpcRequest) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => JsonRpcResponse::success(
            request.id,
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": { "listChanged": false }
                },
                "serverInfo": {
                    "name": "geometry-os-mcp",
                    "version": "0.1.0"
                }
            }),
        ),

        "tools/list" => JsonRpcResponse::success(
            request.id,
            serde_json::json!({
                "tools": get_tool_list()
            }),
        ),

        "tools/call" => {
            let args = request.params.clone().unwrap_or(serde_json::json!({}));
            let tool_name = args["name"].as_str().unwrap_or("");
            let tool_args = args
                .get("arguments")
                .cloned()
                .unwrap_or(serde_json::json!({}));

            match handle_tool_call(tool_name, &tool_args) {
                Ok(result) => JsonRpcResponse::success(
                    request.id,
                    serde_json::json!({
                        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&result).unwrap_or_default() }]
                    }),
                ),
                Err(e) => JsonRpcResponse::error(request.id, -32000, e),
            }
        }

        "notifications/initialized" => {
            // No response needed for notifications, but we return empty to avoid hanging
            JsonRpcResponse::success(request.id, serde_json::json!({}))
        }

        _ => JsonRpcResponse::error(
            request.id,
            -32601,
            format!("Method not found: {}", request.method),
        ),
    }
}

// ── Main Loop ───────────────────────────────────────────

fn main() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    eprintln!("[geo_mcp_server] Starting, connecting to {}", SOCKET_PATH);

    // Quick connectivity check
    match UnixStream::connect(SOCKET_PATH) {
        Ok(_) => eprintln!("[geo_mcp_server] Socket OK"),
        Err(e) => eprintln!("[geo_mcp_server] WARNING: Cannot reach socket: {}", e),
    }

    let reader = BufReader::new(stdin.lock());
    for line in reader.lines() {
        match line {
            Ok(line) => {
                if line.trim().is_empty() {
                    continue;
                }

                let parsed: Result<serde_json::Value, _> = serde_json::from_str(&line);
                match parsed {
                    Ok(val) => {
                        let request = JsonRpcRequest {
                            jsonrpc: val["jsonrpc"].as_str().unwrap_or("2.0").to_string(),
                            id: val.get("id").cloned(),
                            method: val["method"].as_str().unwrap_or("").to_string(),
                            params: val.get("params").cloned(),
                        };

                        let response = handle_request(request);
                        let output = serde_json::json!({
                            "jsonrpc": response.jsonrpc,
                            "id": response.id,
                            "result": response.result,
                            "error": response.error.as_ref().map(|e| serde_json::json!({
                                "code": e.code,
                                "message": e.message,
                            })),
                        });
                        if let Ok(json_str) = serde_json::to_string(&output) {
                            let _ = writeln!(stdout, "{}", json_str);
                            let _ = stdout.flush();
                        }
                    }
                    Err(e) => {
                        eprintln!("[geo_mcp_server] Parse error: {}", e);
                    }
                }
            }
            Err(_) => break,
        }
    }

    eprintln!("[geo_mcp_server] Shutting down");
}

// ── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_list_not_empty() {
        let tools = get_tool_list();
        assert!(!tools.is_empty());
        assert!(tools
            .iter()
            .any(|t| t["name"].as_str() == Some("vm_status")));
        assert!(tools
            .iter()
            .any(|t| t["name"].as_str() == Some("vm_screenshot")));
        assert!(tools.iter().any(|t| t["name"].as_str() == Some("vm_type")));
    }

    #[test]
    fn test_status_parsing() {
        // Simulate parsing
        let resp = "mode=Terminal running=false assembled=false pc=0x0000 cursor=(5,3)";
        let mut result = serde_json::Map::new();
        for part in resp.split_whitespace() {
            if let Some((k, v)) = part.split_once('=') {
                match k {
                    "mode" => {
                        result.insert("mode".into(), serde_json::Value::String(v.into()));
                    }
                    "running" => {
                        result.insert("running".into(), serde_json::Value::Bool(v == "true"));
                    }
                    _ => {}
                }
            }
        }
        assert_eq!(result["mode"], serde_json::Value::String("Terminal".into()));
        assert_eq!(result["running"], serde_json::Value::Bool(false));
    }

    #[test]
    fn test_register_parsing() {
        let resp = "r00=00000000\nr01=00000001\nr31=FFFFFFFF";
        let mut regs = serde_json::Map::new();
        for line in resp.lines() {
            if let Some((name, val)) = line.split_once('=') {
                regs.insert(name.into(), serde_json::Value::String(val.into()));
            }
        }
        assert_eq!(regs["r00"], "00000000");
        assert_eq!(regs["r31"], "FFFFFFFF");
    }
}
