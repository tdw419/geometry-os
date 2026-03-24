# Circuit Heat-Map - Live Signal Flow Visualization

Real-time terminal visualization of GPU signal flow overlaid on ASCII circuits.

## What It Does

- **Visualizes signal flow** — See active signals in your circuits
- **Color-coded intensity** — High/medium/low signals shown in different colors
- **Live updates** — 10 FPS refresh (configurable)
- **Three color modes** — Full color, monochrome, or ASCII-only

## Quick Start

```bash
# Terminal 1: Start GPU agent
cd ~/zion/projects/ascii_world/gpu
cargo run --release --bin agent

# Terminal 2: Load a circuit
./target/release/scanner load -f circuits/ascii/half-adder.txt -x 100 -y 100

# Terminal 3: Watch the heat-map
./target/release/heatmap -f circuits/ascii/half-adder.txt --offset-x 100 --offset-y 100
```

## Usage

```bash
./target/release/heatmap [OPTIONS] --file <FILE>

Options:
  -f, --file <FILE>          ASCII circuit file to overlay
      --offset-x <OFFSET_X>  X offset in GPU memory [default: 100]
      --offset-y <OFFSET_Y>  Y offset in GPU memory [default: 100]
  -i, --interval <INTERVAL>  Update interval in milliseconds [default: 100]
      --color <COLOR>        Color mode: full, mono, or ascii [default: full]
  -h, --help                 Print help
```

## Color Modes

### Full Color (default)

256-color terminal with signal intensity mapped to colors:

```
┌─────────────────────────────────┐
│ SIGNAL INTENSITY                │
├─────────────────────────────────┤
│ █████ High (200-255)            │  <- Bright red
│ █████ Medium (150-199)          │  <- Orange
│ █████ Low (100-149)             │  <- Yellow
│ █████ Minimal (50-99)           │  <- Cyan
│ ······ Idle (0-49)              │  <- Dim green
└─────────────────────────────────┘
```

### Monochrome

ASCII art intensity levels:

```
█ High (200+)
▓ Medium (100-199)
░ Low (50-99)
· Idle (0-49)
```

### ASCII Only

For terminals without color support:

```
# High (200+)
+ Medium (100-199)
- Low (50-99)
. Idle (0-49)
```

## Example Output

```
╔═══════════════════════════════════════════════════════════╗
║           CIRCUIT HEAT-MAP - Live Signal Flow             ║
╚═══════════════════════════════════════════════════════════╝

Circuit: circuits/ascii/half-adder.txt
Region: (100, 100) 20x15
Color mode: full
Update interval: 100ms

Press Ctrl+C to stop

┌────────────────────┐
│--------------------│
│      ┃  ┃          │  <- Wires with flowing signals
│      █━━━█         │  <- Active AND gate (high signal)
│      ┃  ┃          │
│      ┣━━━┫         │  <- XOR gate (medium signal)
│      ┃  ┃          │
│+++++++++++++++++++ │  <- High signal wires
└────────────────────┘

┌─────────────────────────────────┐
│ SIGNAL INTENSITY                │
├─────────────────────────────────┤
│ █████ High (200-255)            │
│ █████ Medium (150-199)          │
│ █████ Low (100-149)             │
│ █████ Minimal (50-99)           │
│ ······ Idle (0-49)              │
└─────────────────────────────────┘

┌─────────────────────────────────┐
│ LIVE STATISTICS                 │
├─────────────────────────────────┤
│ Total pixels:                14 │
│ Active pixels:               12 │
│ High signal:                  8 │
│ Avg signal:                  180 │
└─────────────────────────────────┘

Last update: 15:45:23.456
```

## Workflow

### Debug Circuit Behavior

```bash
# 1. Load circuit
./target/release/scanner load -f circuits/ascii/replicator-field.txt -x 200 -y 100

# 2. Inject a signal
./target/release/injector inject -x 200 -y 100 -o REPLICATE -r 255 -g 255 -b 0

# 3. Watch it spread
./target/release/heatmap -f circuits/ascii/replicator-field.txt --offset-x 200 --offset-y 100

# You'll see the signal propagate as the replicators activate!
```

### Monitor Logic Gates

```bash
# 1. Load AND gate
echo "--&--" > test-and.txt
./target/release/scanner load -f test-and.txt -x 100 -y 100

# 2. Watch the gate
./target/release/heatmap -f test-and.txt --offset-x 100 --offset-y 100

# 3. Inject signal from left
./target/release/injector inject -x 100 -y 100 -o MOVE_RIGHT -r 0 -g 255 -b 0

# 4. Inject signal from top
./target/release/injector inject -x 104 -y 98 -o MOVE_DOWN -r 0 -g 255 -b 0

# 5. Watch the AND gate light up when both inputs arrive!
```

### Multi-Circuit Dashboard

```bash
# Terminal 1: Watch circuit 1
./target/release/heatmap -f circuits/ascii/half-adder.txt --offset-x 100 --offset-y 100 --interval 100

# Terminal 2: Watch circuit 2
./target/release/heatmap -f circuits/ascii/replicator-field.txt --offset-x 200 --offset-y 100 --interval 100

# Terminal 3: Control panel
./target/release/injector interactive
```

## Integration with Other Tools

### With Circuit Watcher

```bash
# Terminal 1: Auto-reload on file changes
./circuit-watcher.js circuits/ascii 100 100

# Terminal 2: Live heat-map
./target/release/heatmap -f circuits/ascii/my-circuit.txt --offset-x 100 --offset-y 100

# Terminal 3: Edit circuit
vim circuits/ascii/my-circuit.txt

# Save and see changes instantly in both terminals!
```

### With Scanner

```bash
# Compare snapshots
# 1. Scan circuit state
./target/release/scanner scan -x 100 -y 100 --width 20 --height 15 -o before.txt

# 2. Watch live changes
./target/release/heatmap -f before.txt --offset-x 100 --offset-y 100

# 3. Inject signal and watch propagation
./target/release/injector inject -x 100 -y 100 -o REPLICATE -r 255 -g 255 -b 0
```

## Performance

- **Update rate:** 10 FPS default (100ms interval)
- **Latency:** <1ms to read from shared memory
- **CPU usage:** <1% (minimal overhead)
- **Terminal:** Works in any terminal with ANSI support

## Troubleshooting

**"GPU agent not running" warning:**
```bash
# Start the agent
cargo run --release --bin agent
```

**No colors showing:**
```bash
# Try monochrome mode
./target/release/heatmap -f circuit.txt --offset-x 100 --offset-y 100 --color mono

# Or ASCII-only mode
./target/release/heatmap -f circuit.txt --offset-x 100 --offset-y 100 --color ascii
```

**Circuit not visible:**
1. Verify offset matches load position
2. Check circuit file exists
3. Ensure GPU agent is running

**Signal not flowing:**
1. Inject a signal: `injector inject -x 100 -y 100 -o REPLICATE -r 255 -g 255 -b 0`
2. Verify circuit has active components (not just empty space)
3. Check if opcodes are correct (scan the region first)

## Advanced Usage

### Custom Update Rate

```bash
# 60 FPS (16ms interval)
./target/release/heatmap -f circuit.txt --offset-x 100 --offset-y 100 --interval 16

# 1 FPS (1000ms interval)
./target/release/heatmap -f circuit.txt --offset-x 100 --offset-y 100 --interval 1000
```

### Multiple Regions

```bash
# Terminal 1: Top-left region
./target/release/heatmap -f region1.txt --offset-x 0 --offset-y 0 --interval 100

# Terminal 2: Center region
./target/release/heatmap -f region2.txt --offset-x 240 --offset-y 120 --interval 100

# Terminal 3: Bottom-right region
./target/release/heatmap -f region3.txt --offset-x 360 --offset-y 180 --interval 100
```

### Record to File

```bash
# Capture heat-map state every second
while true; do
    ./target/release/heatmap -f circuit.txt --offset-x 100 --offset-y 100 --interval 1000 > "heatmap-$(date +%s).txt"
    sleep 1
done
```

## Visual Design

The heat-map uses **signal intensity** from GPU memory to color each character:

1. **Read GPU state** — Each pixel's green channel (signal strength)
2. **Map to color** — Signal 0-255 → Color gradient
3. **Overlay on ASCII** — Replace circuit characters with colored versions
4. **Display stats** — Show active pixel count, average signal, etc.

## Future Enhancements

- **Signal trails** — Show movement history (fading trails)
- **Direction arrows** — Indicate signal flow direction
- **Wave visualization** — Animated waves for propagating signals
- **3D view** — Height-mapped intensity (higher signal = taller)
- **Audio feedback** — Beep on signal changes

## Files

```
gpu/
├── src/heatmap.rs           — Heat-map tool (this file)
├── demo-heatmap.sh          — Demo script
├── HEATMAP.md               — This documentation
└── circuits/ascii/          — Circuit files to visualize
```

## See Also

- `WATCHER.md` — Hot-reload circuits on file change
- `SCANNER.md` — Scan GPU state to ASCII
- `INJECTOR.md` — Inject signals manually
- `CIRCUITS.md` — Circuit templates and examples
