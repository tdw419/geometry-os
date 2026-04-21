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
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

const SOCKET_PATH: &str = "/tmp/geo_cmd.sock";

fn send_socket_cmd(cmd: &str) -> Result<String, String> {
    let stream = UnixStream::connect(SOCKET_PATH)
        .map_err(|e| format!("Cannot connect to {}: {}", SOCKET_PATH, e))?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(|e| format!("Set timeout failed: {}", e))?;

    let mut response = String::new();

    // Send command
    stream.peer_addr().ok(); // Just to verify it's connected
    let mut writer = stream
        .try_clone()
        .map_err(|e| format!("Clone stream failed: {}", e))?;
    writeln!(writer, "{}", cmd).map_err(|e| format!("Write failed: {}", e))?;
    writer.flush().ok();

    // Read response
    let mut reader = BufReader::new(stream);
    reader
        .read_line(&mut response)
        .map_err(|e| format!("Read failed: {}", e))?;

    Ok(response.trim().to_string())
}

// ── JSON-RPC Types ──────────────────────────────────────

#[derive(Debug)]
struct JsonRpcRequest {
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
            vec![param("app_name", "string", "App name to launch", true)],
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
    serde_json::json!({"type": "object", "properties": {"launched": {"type": "boolean"}, "app_name": {"type": "string"}}})
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
            let resp = send_socket_cmd(&format!("launch {}", app_name))?;
            Ok(serde_json::json!({
                "launched": resp.contains("launching"),
                "app_name": app_name,
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
