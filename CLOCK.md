# CLOCK.md - 5-Pixel Clock System

**The heartbeat of Geometry OS — a self-oscillating circuit that pulses continuously.**

## What It Does

A clock circuit generates regular pulses that synchronize computation across the system.

- **Output:** Regular TICK signal
- **Frequency:** ~20 frames per cycle (0.67 seconds at 30 FPS)
- **Power:** Self-sustaining (no external input needed)

## Architecture

```
    *───│  ← Emitter (OP_REPLICATE)
    │   │
    │   │  ← Delay line (wire)
    └───▼
        │
        └──► TICK (to ALU clock input)
```

### Signal Flow

1. **Emitter (*)** sends signal right
2. **Wire (─)** carries signal down
3. **Corner (└)** redirects signal left
4. **Loop** returns to emitter
5. **Cycle repeats** → continuous pulses

## How It Works

### Ring Oscillator Principle

The clock is a **ring oscillator** — the simplest form of digital clock:

```
Signal → Gate → Gate → Gate → back to start
```

In our case:
```
Signal → Wire → Wire → Wire → back to emitter
```

### Frequency Determination

The clock frequency is determined by **wire length**:

| Wire Length | Cycle Time | Frequency |
|-------------|------------|-----------|
| 5 pixels    | ~10 frames | 3 Hz      |
| 10 pixels   | ~20 frames | 1.5 Hz    |
| 20 pixels   | ~40 frames | 0.75 Hz   |
| 40 pixels   | ~80 frames | 0.375 Hz  |

**Spatial frequency control:** Change the geometry, change the speed!

## Connecting to ALU

```
CLOCK.OUTPUT ──────► ALU.CLK
```

Every clock pulse triggers one ALU computation cycle:

```
Frame 0:  Clock emits pulse
Frame 5:  Pulse reaches ALU
Frame 10: ALU computes (ADD or AND)
Frame 15: Result appears at output
Frame 20: Next clock pulse
```

## Clock Circuits

| Circuit | Purpose | Location |
|---------|---------|----------|
| `clock-5pixel.txt` | Minimal 5-pixel oscillator | (50, 10) |
| `clock-simple.txt` | Simple feedback loop | (50, 20) |
| `clock-ring.txt` | 3-stage ring oscillator | (200, 20) |

## Demo

```bash
cd ~/zion/projects/ascii_world/gpu
./demo-clock.sh

# Shows:
# • Clock architecture
# • Signal flow
# • Frequency control
# • ALU connection
```

## Advanced: Clock Divider

To create slower clocks from the main clock:

```
CLOCK (1 Hz) ──► DIV2 ──► 0.5 Hz
                 │
                 └──► DIV4 ──► 0.25 Hz
```

A clock divider counts pulses:
- After 2 pulses, emit 1 pulse (÷2)
- After 4 pulses, emit 1 pulse (÷4)
- After 8 pulses, emit 1 pulse (÷8)

## Advanced: Gated Clock

To control when the ALU runs:

```
CLOCK ──┐
        ├─► AND ──► GATED CLOCK
ENABLE ─┘
```

- **ENABLE=1:** Clock passes through
- **ENABLE=0:** Clock blocked

This allows conditional execution based on external signals.

## Why This Matters

### Real CPU Clocks

Modern CPUs use similar ring oscillators:
- Phase-Locked Loops (PLLs) multiply frequency
- Clock distribution networks deliver to all components
- Frequency scaling adjusts power/performance

### Geometry OS Clocks

Our clocks are **spatial**:
- Wire length = propagation delay
- Parallel clocks = multiple frequencies
- No external oscillator needed

### Self-Hosting

The clock is the first step toward **self-hosting computation**:
1. Clock pulses
2. ALU computes
3. Memory stores
4. CPU sequences
5. Program runs

All powered by spatial physics!

## Performance

| Metric | Value |
|--------|-------|
| Area | 5 pixels (0.004% of grid) |
| Power | GPU parallel execution |
| Jitter | ±1 frame (33ms) |
| Stability | Runs forever |

## Testing

```bash
# 1. Load clock
./target/release/scanner load -f circuits/ascii/clock-5pixel.txt -x 50 -y 10

# 2. Watch it pulse
./target/release/heatmap -f circuits/ascii/clock-5pixel.txt --offset-x 50 --offset-y 10

# 3. Connect to ALU
./target/release/injector inject -x 55 -y 14 -o MOVE_DOWN -r 255 -g 255 -b 0
```

## Files

```
circuits/ascii/
├── clock-5pixel.txt   — 5-pixel minimal clock
├── clock-simple.txt   — Simple feedback loop
├── clock-ring.txt     — 3-stage ring oscillator
└── positions.json     — Updated with clock positions

demo-clock.sh          — Interactive demo
CLOCK.md               — This documentation
```

## See Also

- `ALU.md` — Arithmetic Logic Unit
- `README.md` — Complete system guide
- `demo-alu.sh` — ALU with clock integration

---

*The clock is the heartbeat. Without it, there is no rhythm. With it, there is computation.* 💓
