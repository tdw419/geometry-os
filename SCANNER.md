# Circuit Scanner - ASCII ↔ GPU Bridge

Bidirectional bridge between ASCII art and GPU pixel circuits.

## What It Does

- **Scan** GPU pixel state → ASCII art
- **Load** ASCII art → GPU pixel circuits
- **Watch** Live region monitoring with auto-refresh

## Quick Start

```bash
# Scan a region (reads from running GPU agent)
./target/release/scanner scan -x 100 -y 50 --width 40 --height 20

# Load an ASCII circuit
./target/release/scanner load -f circuits/ascii/replicator-field.txt -x 200 -y 100

# Watch a region in real-time
./target/release/scanner watch -x 100 -y 50 --width 40 --height 20
```

## Commands

### scan - Read GPU → ASCII

Scans a region of the pixel grid and converts to ASCII art.

```bash
# Scan 40x20 region starting at (100, 50)
./target/release/scanner scan -x 100 -y 50 --width 40 --height 20

# Save to file
./target/release/scanner scan -x 0 -y 0 --width 80 --height 30 -o my-circuit.txt
```

**Output:**
```
┌────────────────────────────┐
│------&--X--                │
│      |  |                  │
│      +--+                  │
└────────────────────────────┘

Statistics:
  Active pixels: 14
  Empty pixels: 1186

Glyph breakdown:
  - (wire_h): 6
  & (AND): 1
  X (XOR): 1
  | (wire_v): 2
  + (signal): 4
```

### load - ASCII → GPU

Loads ASCII art and injects it as functional circuits.

```bash
# Load at origin
./target/release/scanner load -f circuits/ascii/half-adder.txt

# Load at specific position
./target/release/scanner load -f my-circuit.txt -x 100 -y 50
```

### watch - Live Monitoring

Watches a region and displays it with auto-refresh.

```bash
./target/release/scanner watch -x 100 -y 50 --width 40 --height 20
```

Displays live grid:
```
┌────────────────────────────┐
│------&--X--                │
│      |  |                  │
│      +--+                  │
└────────────────────────────┘

Active pixels: 14
```

## ASCII Glyph Reference

| Glyph | Meaning | Opcode | Color |
|-------|---------|--------|-------|
| `-` | Horizontal wire | MOVE_RIGHT | Green |
| `|` | Vertical wire | MOVE_DOWN | Green |
| `&` | AND gate | AND | Cyan |
| `X` | XOR gate | XOR | Magenta |
| `*` | Replicator | REPLICATE | Yellow |
| `@` | Infect | INFECT | Red |
| `+` | Signal wire (active) | MOVE_RIGHT + signal | White |
| `·` | Idle agent | IDLE | Dim |
| ` ` | Empty | (none) | Black |
| `?` | Unknown opcode | (varies) | Gray |

## Creating ASCII Circuits

### Example: Half-Adder

```
--&--  (AND gate for carry)
   |
   |
--X--  (XOR gate for sum)
```

Save as `half-adder.txt`, then:
```bash
./target/release/scanner load -f half-adder.txt -x 50 -y 50
```

### Example: Signal Splitter

```
  |
  |
  *
 / | \
```

The `*` replicates to all 8 neighbors, splitting the signal.

### Example: Memory Cell

```
  S ---&-- Q
       |
  R ---+
       |
       &-- Q'
```

SR flip-flop using AND gates.

## Workflow

### Design → Test → Save

```bash
# 1. Design your circuit in a text editor
vim my-circuit.txt

# 2. Load it into the GPU
./target/release/scanner load -f my-circuit.txt -x 100 -y 100

# 3. Watch it run
./target/release/scanner watch -x 100 -y 100 --width 20 --height 15

# 4. Scan it back and save
./target/release/scanner scan -x 100 -y 100 --width 20 --height 15 -o backup.txt
```

### Copy Existing Circuits

```bash
# Scan a working circuit
./target/release/scanner scan -x 50 -y 50 --width 30 --height 20 -o working-circuit.txt

# Load it elsewhere
./target/release/scanner load -f working-circuit.txt -x 300 -y 200
```

## Integration with Other Tools

### With Signal Injector

```bash
# Load a circuit
./target/release/scanner load -f circuits/ascii/half-adder.txt -x 100 -y 100

# Inject a signal to test it
./target/release/injector inject -x 100 -y 100 -o MOVE_RIGHT -r 255 -g 255 -b 0

# Watch the signal propagate
./target/release/scanner watch -x 100 -y 100 --width 30 --height 20
```

### With Circuit Templates

```bash
# Load JSON circuit
./target/release/injector load -f circuits/half-adder.json -x 100 -y 100

# Scan it back as ASCII
./target/release/scanner scan -x 100 -y 100 --width 30 --height 20 -o as-ascii.txt

# Now you have an ASCII version of the JSON circuit!
```

## Tips

**Keep it simple:**
- Start with small circuits (10x10)
- Test with `watch` before saving
- Use meaningful file names

**Debug with scan:**
- If a circuit doesn't work, scan it back
- Check if pixels were placed correctly
- Verify signal propagation

**Save your work:**
- Regular scans act as checkpoints
- Version control your ASCII files
- Share working circuits

## Advanced: Spatial Programming

Since ASCII characters map to opcodes, you can write programs spatially:

```
--*--*--*--
   |  |  |
   &  X  &
   |  |  |
   +--+--+
```

This is a 3-stage processor pipeline in visual form.

## Future: Visual Editor

Planned features:
- **Mouse drawing** — Click to place gates
- **Auto-routing** — Connect components automatically
- **Simulation** — Preview before injecting
- **Export** — ASCII, JSON, or image formats

## Files

```
gpu/
├── src/scanner.rs           — Scanner CLI (this tool)
├── circuits/ascii/          — ASCII circuit library
│   ├── half-adder.txt       — Simple example
│   ├── replicator-field.txt — Growth pattern
│   └── ...
└── /tmp/pixel-universe.mem  — Shared memory (runtime)
```

## See Also

- `INJECTOR.md` — Signal injection tool
- `CIRCUITS.md` — JSON circuit templates
- `pixel-agent-shader.wgsl` — Opcode implementations
