# AI Demo Tour — Guided Walkthrough

This document is a step-by-step guided tour for AI agents to demonstrate the full MCP bridge capabilities of Geometry OS. Each step exercises specific MCP tools and verifies the expected behavior.

## Overview

The tour covers:
1. **Desktop Navigation** — Walk the map, observe buildings
2. **Building Entry** — Enter buildings, launch applications
3. **Window Management** — Create, move, resize, focus, close windows
4. **Application Demos** — Shell, calculator, paint, tetris
5. **RISC-V Hypervisor** — Boot a guest OS
6. **Cleanup** — Close everything, return to map

## Prerequisites

- Geometry OS running with the GUI window open
- MCP server connected (the agent uses `mcp_geo_*` tools)
- Desktop map loaded (`programs/world_desktop.asm`)

## Step 1: Check Desktop State

Verify the desktop is active and get current state:

```
mcp_geo_desktop_state -> { player, camera, buildings, frame }
```

**Expected:** Player position at valid coordinates, multiple buildings listed with names and types.

## Step 2: Observe the Map

Get a visual snapshot of what's on screen:

```
mcp_geo_vision_screenshot -> { png_base64 }
```

**Expected:** 256x256 PNG showing the desktop map with buildings, terrain, and player.

## Step 3: Walk to a Building

Use input injection to walk the player to a building:

```
mcp_geo_input_key key=87     # 'W' = walk up
mcp_geo_input_key key=68     # 'D' = walk right
```

Or use arrow keys:

```
mcp_geo_input_key key=Up     # ArrowUp
mcp_geo_input_key key=Right  # ArrowRight
```

**Tip:** Check `player_position` between moves to track progress.

## Step 4: List Nearby Buildings

```
mcp_geo_building_list radius=50 -> { buildings: [...] }
```

**Expected:** Buildings within 50 tiles, with IDs, names, and positions.

## Step 5: Enter a Building (Launch an App)

```
mcp_geo_building_enter building_id=1 -> { entered: true, app_name: "shell" }
```

**Expected:** Building entry animation, app launches in a window.

## Step 6: Check Windows

After entering a building, verify the window was created:

```
mcp_geo_desktop_vision -> { window_count, windows, focused_window_id, screen_ascii }
```

**Expected:** At least 1 window listed with the app's PID, position, and size.

## Step 7: Window Management

### Move a window:
```
window_move window_id=1 x=50 y=50 -> { ok: true }
```

### Resize a window:
```
window_resize window_id=1 w=128 h=96 -> { ok: true }
```

### Bring to front:
```
window_focus window_id=1 -> { ok: true }
```

### List all windows:
```
window_list -> { count: N, windows: [...] }
```

## Step 8: Type in the Application

If the launched app is a shell or notepad:

```
desktop_key key=72     # 'H'
desktop_key key=69     # 'E'
desktop_key key=76     # 'L'
desktop_key key=76     # 'L'
desktop_key key=79     # 'O'
desktop_key key=13     # Enter
```

Or use text injection:
```
input_text text=hello
```

## Step 9: Mouse Interaction

For paint or tetris:

```
desktop_mouse_move x=128 y=128 -> { ok: true }
desktop_mouse_click x=128 y=128 button=2 -> { ok: true }
```

## Step 10: Launch Multiple Apps

Enter different buildings to launch multiple apps:

```
building_enter building_id=2 -> { entered: true, app_name: "calculator" }
building_enter building_id=3 -> { entered: true, app_name: "tetris" }
```

Then check windows:
```
desktop_vision -> { window_count: 3, windows: [...], focused_window_id: N }
```

## Step 11: Boot RISC-V Guest (if hypervisor available)

```
hypervisor_boot config="arch=riscv64 kernel=Image ram=256M" -> { booted: true }
```

Wait for boot, then observe:
```
vision_screenshot -> { png_base64 }
```

When done:
```
hypervisor_kill -> { ok: true }
```

## Step 12: Cleanup

Close all windows:
```
window_list -> { windows: [...] }
window_close window_id=1
window_close window_id=2
...
```

Or kill all windows for a process:
```
process_kill pid=1
process_kill pid=2
```

## Verification Checklist

After completing the tour, verify:

- [ ] `desktop_state` returns valid player position
- [ ] `building_list` shows expected buildings
- [ ] `building_enter` launches apps successfully
- [ ] `window_list` shows correct window count after app launches
- [ ] `window_move` repositions windows correctly
- [ ] `window_resize` changes window dimensions
- [ ] `window_focus` brings window to front
- [ ] `window_close` removes windows
- [ ] `process_kill` removes all windows for a PID
- [ ] `desktop_key` injects key events
- [ ] `desktop_mouse_move` moves cursor
- [ ] `desktop_mouse_click` clicks at position
- [ ] `desktop_vision` returns structured window data with ASCII screen
- [ ] `hypervisor_boot`/`hypervisor_kill` manage guest OS

## MCP Tool Reference

| Tool | Purpose |
|------|---------|
| `desktop_state` | Full desktop state (player, camera, buildings, frame) |
| `building_list` | List buildings with optional radius filter |
| `building_enter` | Walk to building and launch its app |
| `window_list` | List all active windows with metadata |
| `window_move` | Move window to new position |
| `window_close` | Destroy a window |
| `window_focus` | Bring window to front (highest z-order) |
| `window_resize` | Resize window's offscreen buffer |
| `process_kill` | Kill all windows belonging to a PID |
| `desktop_key` | Inject key event (character or keycode) |
| `desktop_mouse_move` | Move mouse cursor |
| `desktop_mouse_click` | Click mouse at position |
| `desktop_vision` | Structured desktop overview: windows + ASCII screen |
| `vision_screenshot` | PNG screenshot of VM screen |
| `input_text` | Type a string of characters |
| `hypervisor_boot` | Boot a guest OS via hypervisor |
| `hypervisor_kill` | Kill running guest OS |
