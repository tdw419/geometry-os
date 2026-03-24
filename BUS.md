# Universal Bus - Motherboard Backplane for Circuits

Standard ASCII template for connecting multiple circuits.

## What It Does

The Universal Bus provides a standardized "backplane" that allows separate circuit files to communicate. Think of it like a motherboard's data bus - circuits plug into it and share signals.

## Bus Template

```ascii
════════════════════════════════════════════════════════════════════
                            UNIVERSAL BUS
════════════════════════════════════════════════════════════════════

   SLOT A          SLOT B          SLOT C          SLOT D
   (0,50)          (120,50)        (240,50)        (360,50)
      │               │               │               │
      │               │               │               │
──────┼───────────────┼───────────────┼───────────────┼──────  BUS 0 (DATA)
      │               │               │               │
──────┼───────────────┼───────────────┼───────────────┼──────  BUS 1 (ADDR)
      │               │               │               │
──────┼───────────────┼───────────────┼───────────────┼──────  BUS 2 (CTRL)
      │               │               │               │
──────┼───────────────┼───────────────┼───────────────┼──────  BUS 3 (CLK)
      │               │               │               │
      │               │               │               │

════════════════════════════════════════════════════════════════════
```

## ASCII Bus File

Save as `circuits/ascii/bus-universal.txt`:

```
────────────────────────────────────────────────────────────────────
────────────────────────────────────────────────────────────────────
────────────────────────────────────────────────────────────────────
────────────────────────────────────────────────────────────────────
```

Each `─` is a horizontal wire (MOVE_RIGHT). Signals flow left-to-right.

## Slot System

The bus defines 4 standard slots where circuits can plug in:

| Slot | Position | Size | Purpose |
|------|----------|------|---------|
| A | (0, 50) | 100×100 | Input stage |
| B | (120, 50) | 100×100 | Processing |
| C | (240, 50) | 100×100 | Processing |
| D | (360, 50) | 100×100 | Output stage |

## Connecting Circuits

### Method 1: Bus Taps

Create a "tap" file that connects a circuit to the bus:

```ascii
      │
      │  (vertical wire down from bus)
      │
      *  (splitter - REPLICATE)
    ┌─┼─┐
    │   │  (to circuit inputs)
```

### Method 2: Direct Connection

Position your circuit so its outputs align with bus wires:

```ascii
half-adder.txt at (120, 50):
  Input A ──→ & ──→ Carry ──→ (connects to bus at x=220)
  Input B ──→ X ──→ Sum   ──→ (connects to bus at x=220)
```

## Example: Multi-Stage Pipeline

### positions.json

```json
{
  "bus-main": { "x": 0, "y": 100, "width": 480, "height": 4 },
  "input-stage": { "x": 0, "y": 50, "width": 100, "height": 100 },
  "process-a": { "x": 120, "y": 50, "width": 100, "height": 100 },
  "process-b": { "x": 240, "y": 50, "width": 100, "height": 100 },
  "output-stage": { "x": 360, "y": 50, "width": 100, "height": 100 }
}
```

### Circuit Files

**bus-main.txt** (the bus):
```
────────────────────────────────────────────────────────────────────────────────────────────────
────────────────────────────────────────────────────────────────────────────────────────────────
────────────────────────────────────────────────────────────────────────────────────────────────
────────────────────────────────────────────────────────────────────────────────────────────────
```

**input-stage.txt** (sends to bus):
```
  │
  *───┐
  │   │
  └───┼────→ (to bus)
```

**process-a.txt** (receives from bus, processes, sends back):
```
  ←───┼─── (from bus)
      │
      &  (process)
      │
  ────┼───→ (to bus)
```

## Bus Protocol

### Data Encoding

Use signal intensity (green channel) to encode data:

| Signal | Meaning |
|--------|---------|
| 0-50 | LOW (0) |
| 50-150 | MEDIUM (control) |
| 150-255 | HIGH (1) |

### Timing

- **1 pixel = 1 frame** (33ms at 30 FPS)
- Signals propagate left-to-right on bus
- Circuits should account for propagation delay

### Handshake

For reliable communication:

```
SENDER                          BUS                           RECEIVER
  │                              │                               │
  ├── DATA (signal high) ────────┼───────────────────────────────┤
  │                              │                               │
  ├── READY (signal high) ───────┼───────────────────────────────┤
  │                              │                               │
  │                              │     (propagation delay)       │
  │                              │                               │
  │                              │     ├── ACK (signal high) ────┤
  │                              │     │                         │
  │                              │     │                         │
```

## Bus Types

### BUS 0 - Data Bus
Primary data transmission channel.

### BUS 1 - Address Bus
Selects which circuit to read/write.

### BUS 2 - Control Bus
Control signals (read/write, interrupt, etc.).

### BUS 3 - Clock Bus
Global clock signal for synchronization.

## Multi-Instance Bus

For distributed systems, the bus can span multiple GPU instances:

```
Instance A (GPU 1)           Instance B (GPU 2)
┌────────────────┐           ┌────────────────┐
│ BUS ───────────┼──bridge───┼─────────────BUS│
│ 0-3            │           │ 0-3            │
└────────────────┘           └────────────────┘
```

Use the network bridge to extend the bus across machines.

## Advanced: Hierarchical Bus

For complex systems, use multiple bus levels:

```
        MAIN BUS (480 wide)
            │
    ┌───────┼───────┐
    │       │       │
 SUB-BUS  SUB-BUS  SUB-BUS
  (A)      (B)      (C)
    │       │       │
  Circuit Circuit Circuit
```

## Collision Detection

Before adding circuits to the bus, run:

```bash
./circuit-check.js circuits/ascii
```

This will verify no circuits overlap on the bus.

## Example: 4-Bit Adder on Bus

```ascii
positions.json:
{
  "bus-main": { "x": 0, "y": 100, "width": 480, "height": 4 },
  "bit0": { "x": 0, "y": 50, "width": 100, "height": 50 },
  "bit1": { "x": 120, "y": 50, "width": 100, "height": 50 },
  "bit2": { "x": 240, "y": 50, "width": 100, "height": 50 },
  "bit3": { "x": 360, "y": 50, "width": 100, "height": 50 }
}

Each bit circuit:
  - Reads inputs from bus
  - Performs 1-bit addition
  - Writes sum to bus
  - Passes carry to next bit via bus
```

## Troubleshooting

**Signals not reaching circuits:**
- Check bus wire continuity (no gaps)
- Verify circuit position aligns with bus
- Account for propagation delay

**Data corruption:**
- Run collision detector
- Ensure only one circuit writes to each bus line
- Use handshake protocol for critical data

**Timing issues:**
- Add repeaters (*) every 50 pixels
- Use clock bus for synchronization
- Account for cumulative delay

## Files

```
circuits/ascii/
├── bus-universal.txt      — Main bus template
├── bus-tap.txt            — Tap connector
├── positions.json         — Circuit positions
└── ...

gpu/
├── circuit-check.js       — Collision detector
└── circuit-watcher.js     — Auto-reload
```

## See Also

- `WATCHER.md` — Hot-reload circuits
- `CIRCUITS.md` — Circuit templates
- `README.md` — Complete system guide
