# Geometry OS MCP Server Specification

API-first design for phases 83-90. The MCP server wraps the running Geometry OS
instance and exposes structured tool interfaces that Hermes (or any MCP client)
can call directly. Workers implement the VM-side features; this doc defines what
the MCP layer expects.

## Architecture

```
Hermes / MCP Client
       |
       | MCP protocol (JSON-RPC over stdio)
       |
  geo_mcp_server (Rust binary)
       |
       | Unix socket /tmp/geo_cmd.sock
       |
  geometry_os (running VM)
```

The MCP server is a thin translation layer. It speaks MCP to clients and raw
socket commands to the VM. As phases ship new VM features, the MCP server gains
new tools without protocol changes.

## Phase Dependency Map

```
Phase 83: Camera + Player Avatar     --> MCP tools: map_viewport, player_*
Phase 84: Buildings on Map           --> MCP tools: building_*
Phase 85: Full Desktop Integration   --> MCP tools: desktop_*
Phase 86: Hypervisor Building        --> MCP tools: hypervisor_*
Phase 88: AI Vision Bridge           --> MCP tools: vision_*
Phase 89: AI Agent Input             --> MCP tools: input_*
Phase 90: Agents on Map              --> MCP tools: agent_*
```

---

## Tools Available Now (socket-based, no new VM code needed)

These wrap existing socket commands.

### `vm_status`
Get current VM state: mode, running status, PC, cursor position.
- **Input:** none
- **Output:** `{ mode: string, running: bool, assembled: bool, pc: hex_string, cursor: [row, col] }`
- **Socket cmd:** `status`

### `vm_screenshot`
Save framebuffer as PNG.
- **Input:** `{ path?: string }` (default: "screenshot.png")
- **Output:** `{ path: string }`
- **Socket cmd:** `screenshot <path>`

### `vm_screen_dump`
Get raw framebuffer data (256x256 hex pixels).
- **Input:** none
- **Output:** `{ width: 256, height: 256, pixels: string }` (space-separated hex per row)
- **Socket cmd:** `screen`

### `vm_registers`
Read all 32 registers.
- **Input:** none
- **Output:** `{ registers: { r0: hex, r1: hex, ..., r31: hex } }`
- **Socket cmd:** `registers`

### `vm_canvas`
Read canvas text content.
- **Input:** none
- **Output:** `{ lines: [{ row: int, text: string }] }`
- **Socket cmd:** `canvas`

### `vm_type`
Type text onto canvas.
- **Input:** `{ text: string }`
- **Output:** `{ ok: true }`
- **Socket cmd:** `type <text>`

### `vm_run`
Toggle VM execution.
- **Input:** none
- **Output:** `{ running: bool }`
- **Socket cmd:** `run`

### `vm_assemble`
Assemble canvas content to bytecode.
- **Input:** none
- **Output:** `{ ok: true }`
- **Socket cmd:** `assemble`

### `vm_disasm`
Disassemble instructions around current PC.
- **Input:** none
- **Output:** `{ instructions: [{ addr: hex, text: string }] }`
- **Socket cmd:** `disasm`

### `vm_save`
Save VM state to disk.
- **Input:** none
- **Output:** `{ ok: true }`
- **Socket cmd:** `save`

---

## Tools for Phase 83: Camera + Player Avatar

The VM needs new socket commands: `player_pos`, `player_move`, `world_viewport`.

### `player_position`
Get player world coordinates.
- **Input:** none
- **Output:** `{ world_x: int, world_y: int, facing: string }`
- **Requires:** Socket cmd `player_pos` returning "world_x,world_y,facing"
- **VM needs:** Track player position in RAM, expose via socket

### `player_move`
Move player in a direction (or to absolute position).
- **Input:** `{ direction?: string, dx?: int, dy?: int }`
  - direction: "north"|"south"|"east"|"west" (one tile)
  - dx/dy: relative offset in world tiles
- **Output:** `{ world_x: int, world_y: int }`
- **Requires:** Socket cmd `player_move <dir>` or `player_move_rel <dx> <dy>`
- **VM needs:** Arrow key injection + position tracking

### `map_viewport`
Get terrain data for current viewport (the visible 32x32 tile area).
- **Input:** none
- **Output:** `{ 
    origin: { x: int, y: int },
    tiles: [[{ biome: string, elevation: int, walkable: bool }]],  // 32x32
    entities: [{ type: string, x: int, y: int, name?: string }]
  }`
- **Requires:** Socket cmd `viewport_json` returning structured terrain data
- **VM needs:** Export terrain hash results and entity positions as JSON over socket

### `map_biome_at`
Get terrain info at specific world coordinates.
- **Input:** `{ world_x: int, world_y: int }`
- **Output:** `{ biome: string, elevation: int, walkable: bool, color: hex }`
- **Requires:** Socket cmd `terrain <wx> <wy>` that runs hash and returns result
- **VM needs:** Deterministic terrain function callable from socket handler

---

## Tools for Phase 84: Buildings on Map

### `building_list`
List all buildings on the map (or within radius of player).
- **Input:** `{ radius?: int }` (default: 64 tiles, 0 = all)
- **Output:** `{ buildings: [{ 
    id: string, 
    name: string, 
    world_x: int, 
    world_y: int, 
    type: string,  // "terminal"|"editor"|"browser"|"hypervisor"|"custom"
    entered: bool 
  }] }`
- **Requires:** Socket cmd `buildings [radius]`

### `building_enter`
Walk player to building and enter it (launches the app).
- **Input:** `{ building_id: string }`
- **Output:** `{ entered: bool, app_name: string }`
- **Requires:** Socket cmd `enter_building <id>`

### `building_exit`
Exit current building, return to map.
- **Input:** none
- **Output:** `{ world_x: int, world_y: int }`
- **Requires:** Socket cmd `exit_building`

---

## Tools for Phase 85: Full Desktop Integration

### `desktop_state`
Get full desktop state: player position, visible buildings, taskbar items, open windows.
- **Input:** none
- **Output:** `{ 
    player: { x, y, facing },
    viewport: { origin: {x,y}, size: {w,h} },
    buildings: [...],
    taskbar: [{ app_name, is_open }],
    open_windows: [{ window_id, title, app_name }]
  }`
- **Requires:** Socket cmd `desktop_json`

### `desktop_launch`
Launch an app (opens window, does not require walking to building).
- **Input:** `{ app_name: string }`
- **Output:** `{ window_id: string }`
- **Requires:** Socket cmd `launch <app>`

---

## Tools for Phase 86: Hypervisor Building

### `hypervisor_boot`
Boot a guest OS inside a building/window.
- **Input:** `{ 
    config: string,  // "arch=riscv64 kernel=Image ram=256M"
    window_id?: string 
  }`
- **Output:** `{ booted: bool, window_id: string }`
- **Requires:** Socket cmd `qemu boot <config>` (already exists)

### `hypervisor_kill`
Kill guest OS.
- **Input:** none
- **Output:** `{ ok: bool }`
- **Requires:** Socket cmd `qemu kill` (already exists)

---

## Tools for Phase 88: AI Vision Bridge

### `vision_screenshot`
Take screenshot and return as base64 PNG (for vision analysis).
- **Input:** `{ region?: { x, y, w, h } }` (default: full screen)
- **Output:** `{ image_base64: string, checksum: hex, width: int, height: int }`
- **Requires:** New socket cmd `screenshot_b64 [x y w h]`
- **VM needs:** PNG encode region of framebuffer, return base64 over socket

### `vision_checksum`
Fast canvas checksum to detect changes.
- **Input:** none
- **Output:** `{ checksum: hex }`
- **Requires:** Socket cmd `canvas_checksum`
- **VM needs:** FNV-1a hash of framebuffer, exposed via socket

### `vision_diff`
Compare current screen to a reference checksum.
- **Input:** `{ reference_checksum: hex }`
- **Output:** `{ changed: bool, changed_pixels: int, change_ratio: float }`
- **Requires:** Socket cmd `canvas_diff <checksum>`

---

## Tools for Phase 89: AI Agent Input

### `input_key`
Inject a key event.
- **Input:** `{ key: string, shift?: bool, ctrl?: bool, alt?: bool }`
  - key: single char or "enter"|"escape"|"tab"|"up"|"down"|"left"|"right"
- **Output:** `{ ok: true }`
- **Requires:** Socket cmd `inject_key <key> [shift] [ctrl] [alt]`
- **VM needs:** AI_INJECT opcode (0x9A) + socket bridge

### `input_mouse`
Inject mouse event.
- **Input:** `{ action: "move"|"click", x: int, y: int, button?: int }`
- **Output:** `{ ok: true }`
- **Requires:** Socket cmd `inject_mouse <action> <x> <y> [button]`

### `input_text`
Type a string of text (injects each character sequentially).
- **Input:** `{ text: string }`
- **Output:** `{ ok: true, chars_typed: int }`
- **Requires:** Socket cmd `inject_text <text>`

---

## Tools for Phase 90: Agents on the Map

### `agent_spawn`
Spawn a new AI agent avatar on the map.
- **Input:** `{ name: string, color?: hex }` (default color: cyan 0x00FFFF)
- **Output:** `{ agent_id: string, world_x: int, world_y: int }`
- **Requires:** Socket cmd `agent_spawn <name> [color]`
- **VM needs:** Agent entity system, up to 4 concurrent agents

### `agent_move`
Move agent toward a target position.
- **Input:** `{ agent_id: string, target_x: int, target_y: int }`
- **Output:** `{ moving: bool, current: { x, y }, distance: int }`
- **Requires:** Socket cmd `agent_move <id> <tx> <ty>`
- **VM needs:** Agent pathfinding, one step per frame

### `agent_enter`
Agent enters the nearest building.
- **Input:** `{ agent_id: string, building_id?: string }`
- **Output:** `{ entered: bool, building_name: string }`
- **Requires:** Socket cmd `agent_enter <id> [building_id]`

### `agent_command`
Agent executes a command inside a building (like typing at a terminal).
- **Input:** `{ agent_id: string, command: string }`
- **Output:** `{ output: string }`
- **Requires:** Socket cmd `agent_cmd <id> <command>`

### `agent_read`
Agent reads current screen/output from inside a building.
- **Input:** `{ agent_id: string }`
- **Output:** `{ screen_text: string, screenshot_b64?: string }`
- **Requires:** Socket cmd `agent_read <id>`

### `agent_list`
List all active agents and their state.
- **Input:** none
- **Output:** `{ agents: [{ 
    id: string, name: string, 
    world_x: int, world_y: int,
    state: string,  // "idle"|"moving"|"in_building"|"working"
    current_task?: string
  }] }`
- **Requires:** Socket cmd `agent_list`

### `agent_task_assign`
Assign a task to an agent.
- **Input:** `{ agent_id: string, task: string, target_building?: string }`
- **Output:** `{ assigned: bool, task_id: string }`
- **Requires:** Socket cmd `agent_task <id> <task> [building]`
- **VM needs:** RAM-based task queue per agent

---

## Implementation Priority

### Phase 1: MCP Server Skeleton (can build now)
- Rust binary `geo_mcp_server`
- Connects to `/tmp/geo_cmd.sock`
- Implements all "Available Now" tools by wrapping existing socket commands
- Stdio-based MCP transport (works with Hermes native MCP client)
- ~300 lines of Rust

### Phase 2: Map Tools (ships with phase 83)
- Add `player_pos`, `player_move`, `viewport_json`, `terrain` socket commands to main.rs
- MCP server gains `player_position`, `player_move`, `map_viewport`, `map_biome_at`

### Phase 3: Building Tools (ships with phase 84)
- Add `buildings`, `enter_building`, `exit_building` socket commands
- MCP server gains `building_list`, `building_enter`, `building_exit`

### Phase 4: Vision + Input (ships with phases 88-89)
- Add `screenshot_b64`, `canvas_checksum`, `canvas_diff` socket commands
- Add `inject_key`, `inject_mouse`, `inject_text` socket commands
- MCP server gains vision_* and input_* tools

### Phase 5: Agent Tools (ships with phase 90)
- Add all `agent_*` socket commands
- MCP server gains full agent orchestration

---

## File Layout

```
geometry_os/
  src/
    mcp_server.rs    -- MCP server binary (separate from main VM)
    main.rs          -- existing, gains new socket commands per phase
    vm/
      agent.rs       -- new, agent entity system (phase 90)
  docs/
    MCP_SPEC.md      -- this file
  Cargo.toml         -- add [[bin]] for geo_mcp_server
```

## Testing Strategy

Each MCP tool has a corresponding test:
1. Socket command test (verify VM responds correctly)
2. MCP tool test (verify server translates correctly)
3. Integration test (Hermes calls tool end-to-end)

Tests live in `src/mcp_server.rs` with `#[test]` annotations.
The MCP server can be tested without Hermes by piping JSON-RPC directly.
