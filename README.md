# Geometry OS

**A pixel-native operating system running on GPU compute shaders.**

*The screen is the hard drive. Code is geometry. Memory is space.*

---

## What is Geometry OS?

Geometry OS is a computational universe where:
- **Pixels are agents** — Each pixel is an autonomous computational unit
- **Code is spatial** — Programs are geometric arrangements of pixels
- **Distance = Time** — Physical wire length determines clock speed
- **The GPU is the CPU** — RTX 5090 executes 115,200 parallel agents at 30 FPS

```
┌─────────────────────────────────────────────────────────────┐
│  ARCHITECTURE STACK                                         │
├─────────────────────────────────────────────────────────────┤
│  ZONES: foundry (1x) | typist (2x) | architect (4x) | screen│
│  MACROS: 6 modules (clock, adders, pc, alu)                │
│  MIRRORS: Architect → Foundry (4x expansion)               │
│  PORTALS: Cross-zone signal teleport (1-cycle latency)     │
│  SIPHON: Linux desktop → foundry (parasitic mode)          │
└─────────────────────────────────────────────────────────────┘
```

---

## Quick Start

### Prerequisites
- Rust 1.70+
- NVIDIA GPU with Vulkan support
- Linux (for /dev/fb0 access)

### Build

```bash
git clone https://github.com/yourname/geometry-os.git
cd geometry-os
./build.sh
```

### Run

```bash
# Simulation mode (PNG output)
./target/release/agent

# Live framebuffer (requires sudo)
sudo ./target/release/agent

# Desktop siphon (see your desktop as pixel data)
sudo ./target/release/siphon-demo
sudo ./target/release/siphon-demo --mouse  # Track mouse cursor
```

---

## The Abstraction Stack

### Zones

The framebuffer is divided into functional zones:

| Zone | Coordinates | Scale | Purpose |
|------|-------------|-------|---------|
| Foundry | 0-239 | 1x | Raw pixel logic |
| Typist | 240-359 | 2x | Glyph-based logic |
| Architect | 360-479 | 4x | Macro blocks |
| Screen | 0-479, y=200+ | 1x | UI output |

### Macros

Pre-built circuit modules:

| Module | Size | Description |
|--------|------|-------------|
| `clock_5px` | 12×4 | Ring oscillator (3 Hz) |
| `half_adder` | 8×6 | 1-bit addition |
| `full_adder` | 16×12 | 1-bit with carry |
| `adder_4bit` | 80×16 | 4-bit ripple-carry |
| `2bit_pc` | 24×18 | Auto-incrementing counter |
| `alu_2bit` | 20×12 | ADD/AND mode select |

### Mirrors

High-level blocks auto-expand to raw pixels:
- Architect (1 pixel) → Foundry (16 pixels at 4x scale)
- Typist (1 pixel) → Foundry (4 pixels at 2x scale)

### Portals

Signals teleport between zones in 1 GPU cycle:
- `OP_PORTAL_IN` — Sink signal, teleport to target
- `OP_PORTAL_OUT` — Receive teleported signal
- `OP_PORTAL_BIDIR` — Bidirectional portal

### Siphon

Reads Linux desktop pixels and injects as agent data:
- Motion detection
- Mouse tracking
- Terminal text sensing

---

## Example Circuits

### 2-Bit Program Counter

```bash
cd ~/.openclaw/workspace/macro-manager
python3 -c "
from macro import MacroManager
from mirror_sync import MirrorSync

m = MacroManager()
s = MirrorSync(m)

m.place_module('clock_5px', 2, 2, 'clk', zone='architect')
m.place_module('2bit_pc', 2, 8, 'pc', zone='architect')
m.place_module('alu_2bit', 2, 30, 'alu', zone='architect')

for p in m.placed:
    s.mirror_to_foundry(p)

print(s.export_injector())
" > cpu_circuit.txt
```

### Portal Test

Clock signal teleports from foundry to architect:

```
Clock (10,10) → Wire → Portal IN (238,10) → Portal OUT (362,10) → Receiver
```

---

## Opcodes

### Movement
- `OP_MOVE_RIGHT` (0x02) — Move agent to right neighbor
- `OP_MOVE_LEFT` (0x03) — Move agent to left neighbor
- `OP_MOVE_UP` (0x04) — Move agent up
- `OP_MOVE_DOWN` (0x05) — Move agent down

### Replication
- `OP_REPLICATE` (0x06) — Copy self to all empty neighbors
- `OP_INFECT` (0x07) — Convert neighbor to self

### Logic
- `OP_AND` (0x30) — Only replicate if N AND W are agents
- `OP_XOR` (0x31) — Only replicate if N XOR W are agents

### Signals
- `OP_EMIT_SIGNAL` (0x20) — Wake up all neighbors
- `OP_SLEEP` (0x21) — Become dormant

### Portals
- `OP_PORTAL_IN` (0x50) — Teleport signal to (g, b) coordinates
- `OP_PORTAL_OUT` (0x51) — Receive teleported signal
- `OP_PORTAL_BIDIR` (0x52) — Bidirectional portal

---

## Performance

On RTX 5090:
- **Resolution:** 480×240 (115,200 pixels)
- **Frame rate:** 3.5 FPS (100 frames in 28s)
- **Active agents:** 1,440+ (CPU circuit)
- **Portal latency:** 1 GPU cycle

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     COMPUTATIONAL UNIVERSE                   │
│                      (480×240 pixels)                        │
├─────────────────────────────────────────────────────────────┤
│  [CLOCK] ─────► [PC] ─────► [ALU] ─────► [OUTPUT]           │
│     │              │             │                           │
│     │         2-bit counter  ADD/AND                        │
│     │              │             │                           │
│     └──────────────┴─────────────┴──► Heatmap visualization │
└─────────────────────────────────────────────────────────────┘
```

---

## The Vision

**"The Game to Help Our World"**

Geometry OS is designed for collective computation:
- Multiple users run the stack
- Their siphons network together
- Mouse movements portal signals across machines
- The world's screen-time becomes a distributed computer

---

## Files

```
geometry-os/
├── src/
│   ├── agent_main.rs       — Main GPU agent runner
│   ├── bin/
│   │   ├── siphon.rs       — Framebuffer siphon module
│   │   └── siphon-demo.rs  — Siphon demo binary
│   └── pixel-agent-shader.wgsl — GPU compute shader
├── circuits/
│   └── ascii/              — ASCII circuit definitions
├── output/                 — PNG frames
├── build.sh                — Build script
└── README.md               — This file
```

---

## Documentation

- `ALU.md` — ALU design and operation
- `BUS.md` — Universal bus protocol
- `CLOCK.md` — Clock circuits
- `QUICKSTART.md` — 30-second start guide

---

## License

MIT

---

## Credits

Built with:
- Rust + wgpu
- NVIDIA RTX 5090
- The vision of spatial computing

---

*"The screen is the hard drive. Pixels are the programs."*
