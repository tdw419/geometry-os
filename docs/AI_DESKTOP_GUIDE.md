# AI Agent Desktop Guide

This document explains how AI agents interact with the Geometry OS desktop.
Read this if you are an Oracle worker, an autonomous builder, or any agent that
needs to navigate, verify, or develop through the running desktop.

---

## The Big Picture

Geometry OS has an infinite procedural map with buildings (apps) placed at fixed
world coordinates. The desktop runs as a native binary with a Unix socket at
`/tmp/geo_cmd.sock`. You send text commands, you get text back. No screenshots
needed -- the map renders as color-aware ASCII.

```
+------------------+     socket      +------------------+
|  Geometry OS     | <-------------> |  AI Agent        |
|  (native binary) |   text commands |  (you)           |
|  minifb window   |   text replies  |                  |
+------------------+                 +------------------+
```

---

## Socket Connection

```python
import socket

def geo_cmd(cmd, timeout=5):
    """Send a command to the Geometry OS desktop. Returns response text."""
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(timeout)
    s.connect('/tmp/geo_cmd.sock')
    s.sendall((cmd + '\n').encode())
    s.shutdown(socket.SHUT_WR)
    data = b''
    while True:
        try:
            chunk = s.recv(65536)
            if not chunk:
                break
            data += chunk
        except socket.timeout:
            break
    s.close()
    return data.decode().strip()
```

Every command opens a new connection. There is no session state between commands
-- the VM state persists, but the socket does not.

---

## Navigation Commands

### `menu` -- See What Exists

Always start here. Returns a numbered list of all apps on the map.

```
=== Geometry OS Desktop Menu (10 apps) ===
Player: (32,32) | Commands: goto <N>, launch <name>
  [0] snake (at 52,48, dist 36)
  [1] ball (at 78,85, dist 99)
  [2] plasma (at 110,55, dist 101)
  ...
=== End Menu ===
```

The numbers in `[N]` are building IDs you can use with `goto`.

### `goto <name_or_id>` -- Teleport to a Building

Moves the player and camera to a building instantly.

```
geo_cmd('goto snake')
# [teleported to snake (52,50), camera updated]

geo_cmd('goto 0')
# [teleported to snake (52,50), camera updated]
```

Works with building name (exact match) or numeric ID from `menu`.

Player is placed 2 tiles below the building (in front of the door).
Camera centers on the player.

### `nearby` -- What's Closest

Returns all buildings sorted by Manhattan distance from current position.

```
geo_cmd('nearby')
# player=(70,132), 10 buildings:
#   [8] linux (70,130) dist=2
#   [6] init (25,140) dist=53
#   [1] ball (78,85) dist=55
```

Use this to find what's nearby after teleporting.

### `player_pos` -- Current Location

```
geo_cmd('player_pos')
# 70,132,down
```

Returns `x,y,facing` where facing is down/up/left/right.

---

## Vision: Reading the Screen

### `vmscreen` -- Color-Aware ASCII Map

The most important command for visual verification. Returns a 64x32 character
grid where different colors map to different characters:

```
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~:+~+:+:+
++++++++++++++++++++++++++++++++++++++++++++++++++++++++:.:#~+++
::::::::::::::::::----=::~+++++::::::::+:+@+@++++=----=++:+:::+++
:::::::+++%:.-@@++++++++++++++++++:++:+++++++%+++::::::+++.:@"+++
:::::::++++M::+@+++++++++++++++++++++++++%+++++++::::::+++::++@+
```

**Character map:**

```
Char  Meaning            Color signature
----  -----------------  --------------------------
  ~   Water (bright)     Blue pixels, luminance > 120
  =   Water (medium)     Blue pixels, luminance 50-120
  -   Water (deep)       Blue pixels, luminance < 50
  #   Bright green       Green pixels, luminance > 140
  +   Green land         Green pixels, luminance 80-140
  :   Dark terrain       Dark or mixed, low luminance
  %   Desert / sand      Brown-yellow pixels
  M   Mountains          Purple pixels
  ^   Buildings          Red pixels (building markers)
  "   White text          Near-white pixels
  *   Yellow highlights   Yellow pixels (taskbar, UI)
  @   Mixed bright       High luminance, no dominant hue
  .   Very dark / night   Near-black pixels
```

**How to read the map:**

- Water clusters = `~~~`, `===`, `---` (bright to deep)
- Land masses = `+`, `#`, `:` (bright to dark green)
- Building markers = `^` (red dots on the map)
- Mountain ranges = `M` clusters
- Desert patches = `%` clusters
- The player is a small shape at the center of the viewport

### `desktop_json` -- Structured State

Returns JSON with player position, camera, and all buildings:

```json
{"player":{"x":70,"y":132},"camera":{"x":38,"y":100},"frame":1407,
 "nearby_building":8,"buildings":[
   {"id":0,"x":52,"y":48,"color":"ff4444","name":"snake"},
   ...
]}
```

### `status` -- VM Mode

```
geo_cmd('status')
# mode=Terminal running=false assembled=true pc=0x2002 cursor=(4,5)
```

Tells you if a program is currently running.

---

## App Interaction

### `launch <name>` -- Enter a Building

Loads and runs the program associated with a building. The map is replaced
by the app's output on screen.

```
geo_cmd('launch snake')
# [launching: snake from building 0 (755 words)]
```

After launching, `vmscreen` shows the app's visual output instead of the map.

### Reloading the Map

After launching an app, the map desktop is no longer running. The binary
needs to reload `programs/world_desktop.asm`. Currently this requires
restarting the binary or loading the map program manually:

```python
geo_cmd('loadasm programs/world_desktop.asm')
```

---

## Program Development

### `canvas` -- Read the Editor

Shows the text content of the canvas (code editor):

```
geo_cmd('canvas')
# 0|Geometry OS v1.0.0
# 1|40 opcodes | 32 regs | 256x256
# 4|geo>
```

### `type <text>` -- Write to Canvas

Types text at the current cursor position. Use `\n` (literal backslash-n) for
newlines:

```python
geo_cmd('type LDI r1, 42\\nPSETI 100, 100, r1\\nHALT')
```

### `clear` -- Clear Canvas

```python
geo_cmd('clear')
```

### `assemble` -- Compile Canvas to Bytecode

```python
result = geo_cmd('assemble')
# [assembled: 3 words at 0x2000]
```

### `run` -- Toggle Execution

```python
geo_cmd('run')  # starts execution
# ... wait a moment ...
geo_cmd('run')  # pauses execution
```

---

## Typical Workflows

### Workflow 1: Verify a New Program

```python
geo_cmd('clear')
geo_cmd('type LDI r1, 0xFF0000\\nPSETI 128, 128, r1\\nHALT')
geo_cmd('assemble')
geo_cmd('run')

import time; time.sleep(0.5)

screen = geo_cmd('vmscreen')
# Look for the red pixel (^ or @ near center of screen)
```

### Workflow 2: Navigate to a Building and Launch

```python
# Step 1: See what's available
print(geo_cmd('menu'))

# Step 2: Go to the snake building
print(geo_cmd('goto snake'))

# Step 3: Verify you can see the building on the map
screen = geo_cmd('vmscreen')
# Look for '^' (red building marker) near center

# Step 4: Enter the building
print(geo_cmd('launch snake'))

# Step 5: Verify the app is running
time.sleep(1)
screen = geo_cmd('vmscreen')
# Snake body shows as green '+', food as red '^'
```

### Workflow 3: Verify a New Building Was Added

```python
# After adding a building to the map program and reloading:
print(geo_cmd('buildings'))
# Check your building appears in the list

print(geo_cmd('menu'))
# Check it appears with correct name and position

print(geo_cmd('goto my_new_app'))
# Verify teleport works

screen = geo_cmd('vmscreen')
# Verify the building marker '^' is visible
```

### Workflow 4: Full Build-Test-Verify Cycle

```python
# 1. Edit source files (opcodes, assembler, programs) on disk
# 2. Build
#    terminal: cd ~/zion/projects/geometry_os/geometry_os && cargo build --release
# 3. Restart binary with new build
#    pkill -f geometry_os; ./target/release/geometry_os &
# 4. Run tests
#    terminal: cargo test --lib
# 5. Verify through desktop
screen = geo_cmd('vmscreen')
menu = geo_cmd('menu')
print(screen, menu)
```

---

## Memory Map Reference

The infinite map desktop stores state in RAM. Key addresses:

```
RAM[0x7500-0x757F]  Building table (up to 32 buildings)
                      Each entry: 4 words (world_x, world_y, color, name_addr)
RAM[0x7580]          Building count
RAM[0x7588]          Nearby building ID (proximity detection)
RAM[0x7600-0x76FF]   Building name strings (null-terminated)
RAM[0x7800]          Camera X (world tile coords)
RAM[0x7801]          Camera Y
RAM[0x7802]          Frame counter
RAM[0x7806]          Current tile biome type
RAM[0x7808]          Player X (world tile coords, initially 32)
RAM[0x7809]          Player Y
RAM[0x780A]          Player facing (0=down, 1=up, 2=left, 3=right)
RAM[0x780B]          Walk frame (0 or 1, toggles for animation)
RAM[0x7812]          Zoom level (0=1px, 1=2px, 2=4px default)
RAM[0x7814]          Detail level (0=minimal, 1=medium, 2=full)
RAM[0x7830]          Command mode (0=move, 1=type)
RAM[0x7831]          Command buffer length
RAM[0x7832-0x7871]   Command buffer (64 chars)
RAM[0x7872]          Oracle response ready flag
RAM[0x7873-0x7A72]   Oracle response buffer (895 chars)
RAM[0x7100-0x74FF]   Minimap pixel cache (32x32)
```

---

## Tile System

The map uses a standard tile grid:

- **Default tile size:** 4x4 pixels (zoom level 2)
- **Viewport:** 64x64 tiles = 256x256 pixels (full screen)
- **Zoom level 0:** 1px tiles, 256x256 tiles visible (zoomed out)
- **Zoom level 1:** 2px tiles, 128x128 tiles visible
- **Zoom level 2:** 4px tiles, 64x64 tiles visible (default)

World coordinates are in tile units, not pixels. Player at (52, 48) means
tile column 52, tile row 48.

---

## Building System

Buildings are placed at deterministic world coordinates in the map program
(`programs/world_desktop.asm`). Each building has:

- **world_x, world_y:** Position in tile coordinates
- **type_color:** RGB color (used for the building marker)
  - Red (0xFF4444): Games
  - Green (0x44FF44): Utilities
  - Blue (0x4444FF): Creative
  - Yellow (0xFFFF44): System
  - Gold (0xFFD700): Special
  - Magenta (0xFF00FF): New additions
- **name:** Null-terminated string, used for `launch` and `goto`
- **program:** A matching `programs/<name>.asm` file that `launch` loads

Adding a new building requires:
1. Write the program in `programs/<name>.asm`
2. Add a building entry in `programs/world_desktop.asm` at RAM[0x7500+]
3. Add the name string at RAM[0x7600+]
4. Increment the building count at RAM[0x7580]
5. Register the app in `src/main.rs` in the `desktop_apps` array

---

## Health Check and Auto-Restart

The binary is kept alive by `~/.hermes/scripts/geo_desktop_alive.py`. The
Oracle preflight calls this before each worker cycle. It:

1. Checks if `/tmp/geo_cmd.sock` exists and responds
2. If not, kills any stale process and starts a fresh one
3. Waits up to 15 seconds for the socket to become responsive
4. Has a 60-second cooldown between restart attempts

If the socket doesn't respond, the preflight outputs `DESKTOP_ALIVE: false`
and workers skip desktop verification for that cycle.

---

## Quick Reference Card

```
COMMAND          RETURNS                  USE FOR
─────────────────────────────────────────────────────────────
menu             numbered app list        "what can I do?"
goto <name|id>   teleport + camera        "go to building X"
nearby           distance-sorted list     "what's closest?"
player_pos       x,y,facing               "where am I?"
vmscreen         64x32 ASCII art          "what do I see?"
status           mode,running,pc          "is the VM alive?"
buildings        id,x,y,color,name        "verify building table"
desktop_json     full state as JSON       "dump everything"
launch <name>    loads+runs program       "enter this building"
canvas           text editor content      "read the code"
type <text>      writes to canvas         "write code"
clear            clears canvas            "start fresh"
assemble         compiles canvas          "build the code"
run              toggles execution        "run the code"
screenshot       saves PNG to disk        "save visual proof"
```

---

## Character-to-Terrain Quick Reference

For reading `vmscreen` output at a glance:

```
Water:   ~  =  -        (bright to deep)
Land:    #  +  :        (bright to dark green)
Desert:  %               (brown/sand)
Mountain: M              (purple/gray)
Building: ^              (red marker)
Text:    "               (white/bright)
UI:      *               (yellow taskbar)
Dark:    .               (near-black)
```
