# Circuit Templates

Pre-defined logic circuits that can be "pasted" onto the pixel grid.

## Available Circuits

### Half Adder (`circuits/half-adder.json`)
Adds two single-bit numbers, outputs sum and carry.

**Inputs:**
- A — First input bit
- B — Second input bit

**Outputs:**
- SUM — XOR of inputs
- CARRY — AND of inputs

**Usage:**
```bash
./target/release/injector load -f circuits/half-adder.json -x 50 -y 50
```

---

### SR Flip-Flop (`circuits/sr-flipflop.json`)
Set-Reset latch — basic 1-bit memory element.

**Inputs:**
- S — Set (store 1)
- R — Reset (store 0)

**Outputs:**
- Q — Stored value
- Q' — Inverted value

**Usage:**
```bash
./target/release/injector load -f circuits/sr-flipflop.json -x 100 -y 100
```

---

### Clock Oscillator (`circuits/clock-oscillator.json`)
Generates a periodic pulse signal.

**Outputs:**
- CLK — Clock signal (toggles continuously)

**Usage:**
```bash
./target/release/injector load -f circuits/clock-oscillator.json -x 200 -y 50
```

---

## Circuit Template Format

```json
{
  "name": "Circuit Name",
  "description": "What it does",
  "author": "Your Name",
  "version": "1.0",
  "size": { "width": 20, "height": 15 },
  "inputs": [
    { "name": "A", "x": 0, "y": 5, "description": "Input A" }
  ],
  "outputs": [
    { "name": "OUT", "x": 19, "y": 5, "description": "Output" }
  ],
  "pixels": [
    { "x": 0, "y": 5, "opcode": "MOVE_RIGHT", "r": 0, "g": 255, "b": 0, "comment": "Wire" },
    { "x": 10, "y": 5, "opcode": "AND", "r": 255, "g": 255, "b": 0, "comment": "Gate" }
  ]
}
```

## Creating Custom Circuits

1. **Design your circuit** on paper or using the interactive mode
2. **Create a JSON file** in `circuits/` directory
3. **Test it** with the injector:
   ```bash
   ./target/release/injector load -f circuits/my-circuit.json -x 0 -y 0
   ```

### Example: 4-Bit Counter

```json
{
  "name": "4-Bit Counter",
  "description": "Counts from 0 to 15",
  "size": { "width": 100, "height": 40 },
  "inputs": [
    { "name": "CLK", "x": 0, "y": 20, "description": "Clock input" }
  ],
  "outputs": [
    { "name": "Q0", "x": 99, "y": 10, "description": "Bit 0" },
    { "name": "Q1", "x": 99, "y": 20, "description": "Bit 1" },
    { "name": "Q2", "x": 99, "y": 30, "description": "Bit 2" },
    { "name": "Q3", "x": 99, "y": 40, "description": "Bit 3" }
  ],
  "pixels": [
    // Define your counter circuit here
  ]
}
```

## Circuit Library

Standard components you can use:

**Gates:**
- AND gate — `opcode: "AND"`
- XOR gate — `opcode: "XOR"`
- (Future: OR, NOT, NAND, NOR)

**Wires:**
- Horizontal — `opcode: "MOVE_RIGHT"` or `MOVE_LEFT`
- Vertical — `opcode: "MOVE_UP"` or `MOVE_DOWN"`

**Signals:**
- Emitter — `opcode: "EMIT_SIGNAL"`
- Replicator — `opcode: "REPLICATE"`

**Memory:**
- Flip-flop — Use SR flip-flop template

## Composing Circuits

You can load multiple circuits and connect them:

```bash
# Load a clock
./target/release/injector load -f circuits/clock-oscillator.json -x 0 -y 50

# Load a counter (connects to clock output at x=29)
./target/release/injector load -f circuits/counter.json -x 30 -y 50

# Load a display (connects to counter outputs)
./target/release/injector load -f circuits/display.json -x 80 -y 50
```

## Sharing Circuits

Circuits are just JSON files — share them via:

1. **GitHub** — Create a repo of circuit templates
2. **Discord** — Paste JSON in #circuits channel
3. **pxOS Hub** — Upload to circuit marketplace (future)

## Best Practices

1. **Document inputs/outputs** — Others need to know how to connect
2. **Use comments** — Explain what each section does
3. **Test thoroughly** — Verify the circuit works before sharing
4. **Keep it modular** — Small, reusable components work best
5. **Color code** — Use consistent colors for wire types

## Example Workflow

```bash
# 1. Start the universe
./start-universe.sh

# 2. Load a clock
./target/release/injector load -f circuits/clock-oscillator.json -x 10 -y 100

# 3. Load a half-adder
./target/release/injector load -f circuits/half-adder.json -x 50 -y 80

# 4. Connect them manually (or design circuit to auto-connect)
./target/release/injector inject -x 39 -y 105 -o MOVE_RIGHT

# 5. Watch the computation unfold!
```

## Future: Visual Circuit Editor

Planned features:
- **Drag-and-drop** — Build circuits visually
- **Auto-routing** — Wires connect automatically
- **Simulation preview** — See results before injecting
- **Export to JSON** — Save your designs

## Troubleshooting

**Circuit doesn't work:**
1. Check wire connections (use MOVE opcodes)
2. Verify gate positions sense the correct neighbors
3. Make sure there's space for the circuit to grow

**Pixels collide:**
- Circuits need empty space around them
- Use offset parameters: `-x 100 -y 100`

**No output:**
- Check if outputs are being monitored
- Some circuits need input signals to trigger
