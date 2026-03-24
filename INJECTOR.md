# Signal Injector - CLI Tool for GPU Pixel Agents

Inject signals into the running pixel agent grid to control logic circuits in real-time.

## Quick Start

```bash
# 1. Start the pixel universe (in one terminal)
cd ~/zion/projects/ascii_world/gpu
./start-universe.sh

# 2. Inject signals (in another terminal)
./target/release/injector interactive
```

## Commands

### Inject Signal
Inject a single agent at specific coordinates:

```bash
# Inject a REPLICATE agent (red) at (100, 100)
./target/release/injector inject -x 100 -y 100 -o REPLICATE -r 255 -g 0 -b 0

# Inject a MOVE_RIGHT agent (green)
./target/release/injector inject -x 50 -y 50 -o RIGHT -r 0 -g 255 -b 0

# Inject using hex opcode
./target/release/injector inject -x 200 -y 100 -o 0x06 -r 255 -g 100 -b 50
```

### Draw Wire
Draw a line of agents connecting two points:

```bash
# Draw green wire from (50,50) to (200,50)
./target/release/injector wire --x1 50 --y1 50 --x2 200 --y2 50 --color 00FF00

# Draw red wire (diagonal)
./target/release/injector wire --x1 0 --y1 0 --x2 100 --y2 100 --color FF0000
```

### Logic Gates
Place AND/XOR gates that sense their neighbors:

```bash
# Place AND gate at (150, 100)
./target/release/injector and-gate -x 150 -y 100

# Place XOR gate at (200, 100)
./target/release/injector xor-gate -x 200 -y 100
```

### Interactive Mode
REPL for real-time control:

```bash
./target/release/injector interactive

pixel> inject 100 100 REPLICATE 255 0 0
✓ Injected REPLICATE at (100, 100)

pixel> wire 0 0 100 0 00FF00
Drawing wire from (0, 0) to (100, 0)
✓ Drew 101 wire pixels

pixel> gate 150 100
✓ AND gate at (150, 100)

pixel> quit
Goodbye!
```

## Opcodes

| Name | Hex | Description |
|------|-----|-------------|
| `IDLE` | 0x01 | Stay in place, render color |
| `RIGHT` / `MOVE_RIGHT` | 0x02 | Move right 1 pixel/frame |
| `LEFT` / `MOVE_LEFT` | 0x03 | Move left 1 pixel/frame |
| `UP` / `MOVE_UP` | 0x04 | Move up 1 pixel/frame |
| `DOWN` / `MOVE_DOWN` | 0x05 | Move down 1 pixel/frame |
| `REPLICATE` / `COPY` | 0x06 | Copy to all 8 neighbors |
| `INFECT` | 0x07 | Convert neighbors to self |
| `EMIT` / `SIGNAL` | 0x20 | Wake up dormant neighbors |
| `AND` | 0x30 | AND gate (replicate if N AND W are agents) |
| `XOR` | 0x31 | XOR gate (replicate if N XOR W are agents) |

## Architecture

```
┌─────────────────────────────────────────┐
│         Signal Injector (CLI)           │
│   Reads/writes /tmp/pixel-universe.mem  │
└────────────┬────────────────────────────┘
             │
             ▼
┌─────────────────────────────────────────┐
│       Shared Memory (1.76 MB)           │
│   480×240×16 bytes (pixel grid)         │
└────────────┬────────────────────────────┘
             │
             ▼
┌─────────────────────────────────────────┐
│      GPU Agent (Rust + wgpu)            │
│   - Reads from shared memory            │
│   - Executes WGSL compute shader        │
│   - Writes to /dev/fb0 or PNG           │
└─────────────────────────────────────────┘
```

## Building Logic Circuits

### Example: Half Adder

```bash
# Input A (wire)
./target/release/injector wire --x1 50 --y1 100 --x2 150 --y2 100

# Input B (wire)
./target/release/injector wire --x1 100 --y1 50 --x2 100 --y2 150

# AND gate (carry out)
./target/release/injector and-gate -x 150 -y 100

# XOR gate (sum)
./target/release/injector xor-gate -x 200 -y 100
```

### Example: Signal Propagation

```bash
# Start a signal emitter
./target/release/injector inject -x 50 -y 120 -o EMIT

# The signal will wake up neighbors, creating a wave
```

## Performance

- **Resolution:** 480×240 (115,200 pixels)
- **Frame rate:** 30 FPS
- **Memory:** 1.76 MB shared memory
- **Latency:** <1ms for signal injection

## Troubleshooting

**"Agent not running" error:**
```bash
# Start the agent first
./start-universe.sh
```

**Permission denied on /dev/fb0:**
```bash
# Run with sudo
sudo ./target/release/agent
```

**No visual output:**
```bash
# Check if frames are being saved
ls output/frame_*.png

# Or run on actual framebuffer (TTY only)
sudo ./target/release/agent
```

## Future: pxOS Integration

The shared memory bridge enables:

1. **Dashboard cells** → Inject agents as visualizations
2. **Agent state** → Read back to JavaScript for UI updates
3. **Real-time control** → Interactive dashboards controlling GPU computation

```javascript
// Future: pxOS dashboard integration
const pixelUniverse = new PixelUniverse('/tmp/pixel-universe.mem');

// Inject signal
pixelUniverse.inject(100, 100, 'REPLICATE', {r: 255, g: 0, b: 0});

// Read state
const pixel = pixelUniverse.read(150, 100);
if (pixel.a === TYPE_AGENT) {
  dashboard.update('agent-count', pixelUniverse.countAgents());
}
```

## Files

```
gpu/
├── src/injector.rs          — CLI tool (this file)
├── src/agent_main.rs        — GPU agent runner
├── pixel-agent-shader.wgsl  — WGSL compute shader
├── start-universe.sh        — Start script
└── /tmp/pixel-universe.mem  — Shared memory (runtime)
```

## See Also

- `pixel-agent-shader.wgsl` — Opcode implementations
- `agent_main.rs` — Shared memory setup
- `HEARTBEAT.md` — Project status
