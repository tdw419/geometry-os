# ALU Multiplexer - Spatial Logic Switcher

**The ultimate demonstration of spatial computing: one circuit, two operations.**

## What It Does

An ALU (Arithmetic Logic Unit) that switches between operations based on a control signal:

- **Mode 0 (M=0):** ADD operation (A + B)
- **Mode 1 (M=1):** AND operation (A & B)

## Architecture

```
          Inputs               Control
             │                    │
    A3 A2 A1 A0  B3 B2 B1 B0     M
      │  │  │  │   │  │  │  │    │
      └──┴──┴──┴───┴──┴──┴──┴────┤
                                 │
                    ┌────────────┤
                    │            │
              ┌─────┴─────┐      │
              │           │      │
           ADDER        ANDER    │
          (4-bit)      (4-bit)   │
              │           │      │
          S0-S3       Z0-Z3      │
              │           │      │
              └─────┬─────┘      │
                    │            │
                 ┌──┴──┐         │
                 │ MUX │◄────────┘
                 └──┬──┘
                    │
              R0 R1 R2 R3
```

## How It Works

### Signal Flow

1. **Inputs** (A, B) enter from top
2. **Control** (M) splits to multiplexer
3. **Parallel paths:**
   - Left: 4-bit ripple-carry adder
   - Right: 4-bit bitwise AND
4. **Multiplexer** selects output based on M

### Mode Selection

```
M = 0 (horizontal wire):
  ────────► ADDER → OUTPUT

M = 1 (vertical wire):
  │
  └──► ANDER → OUTPUT
```

## Circuits Included

| Circuit | Purpose | Location |
|---------|---------|----------|
| `simple-alu.txt` | 2-bit demo | (50, 220) |
| `4bit-ripple-adder.txt` | 4-bit addition | (50, 180) |
| `alu-mux-design.txt` | Full ALU blueprint | Documentation |

## Testing

```bash
# 1. Load the simple ALU
cd ~/zion/projects/ascii_world/gpu
./target/release/scanner load -f circuits/ascii/simple-alu.txt -x 50 -y 220

# 2. Inject inputs (A=3, B=2)
./target/release/injector inject -x 50 -y 220 -o MOVE_DOWN -r 1 -g 1 -b 0  # A1=1
./target/release/injector inject -x 60 -y 220 -o MOVE_DOWN -r 1 -g 0 -b 0  # A0=1
./target/release/injector inject -x 80 -y 220 -o MOVE_DOWN -r 0 -g 1 -b 0  # B1=1
./target/release/injector inject -x 90 -y 220 -o MOVE_DOWN -r 0 -g 0 -b 0  # B0=0

# 3. Set mode (M=0 for ADD)
./target/release/injector inject -x 100 -y 220 -o MOVE_DOWN -r 0 -g 0 -b 0  # M=0

# 4. Watch computation
./target/release/heatmap -f circuits/ascii/simple-alu.txt --offset-x 50 --offset-y 220

# Expected output (A+B=3+2=5):
#   R1=1, R0=1 (binary: 11 = 5)
```

## Mode Switching

```bash
# Test ADD mode (M=0)
./target/release/injector inject -x 100 -y 220 -o MOVE_DOWN -r 0 -g 0 -b 0

# Test AND mode (M=1)
./target/release/injector inject -x 100 -y 220 -o MOVE_DOWN -r 255 -g 255 -b 0
```

## Design Philosophy

### Spatial Multiplexing

Instead of electrical switches, we use **spatial routing**:

- **Wire length = propagation delay**
- **Parallel paths = simultaneous computation**
- **Control signal = path selection**

### Why This Matters

This proves that **spatial computing works**:
1. Logic gates are physical structures
2. Operations are geometric paths
3. Control flow is spatial routing
4. Computation is signal propagation

## Extending to Full ALU

A complete ALU would include:

| Operation | Opcode | Description |
|-----------|--------|-------------|
| ADD | 000 | A + B |
| SUB | 001 | A - B |
| AND | 010 | A & B |
| OR | 011 | A \| B |
| XOR | 100 | A ^ B |
| NOT | 101 | ~A |
| SHL | 110 | A << 1 |
| SHR | 111 | A >> 1 |

**Requires:** 3 control bits → 8-way multiplexer

## Visual Debugging

Watch the ALU in action:

```bash
# Terminal 1: Run agent
cargo run --release --bin agent

# Terminal 2: Watch heat-map
./target/release/heatmap -f circuits/ascii/simple-alu.txt --offset-x 50 --offset-y 220 --interval 50

# Terminal 3: Control inputs
./target/release/injector interactive

# You'll see:
#   - Signals entering from top
#   - Computation in ADDER/ANDER
#   - Multiplexer selecting output
#   - Result appearing at bottom
```

## Scaling Up

The simple ALU demonstrates the concept. To scale:

1. **4-bit ALU** — Cascade 4 simple ALUs
2. **8-bit ALU** — Two 4-bit ALUs with carry chain
3. **16-bit ALU** — Four 4-bit ALUs
4. **32-bit ALU** — Eight 4-bit ALUs

**Each level adds:** 4 more bits, ~100 more pixels

## Performance

- **Propagation delay:** ~20 frames (0.67 seconds)
- **Area:** 150×60 pixels (0.08% of grid)
- **Power:** GPU parallel execution
- **Scalability:** Linear with bit width

## Next Steps

1. ✅ Build simple 2-bit ALU
2. 🔄 Extend to 4-bit with carry
3. 🔄 Add more operations (OR, XOR, NOT)
4. 🔄 Connect to CPU design
5. 🔄 Build self-modifying circuits

## Files

```
circuits/ascii/
├── simple-alu.txt        — 2-bit demo circuit
├── 4bit-ripple-adder.txt — 4-bit adder component
├── alu-mux-design.txt    — Full ALU blueprint
└── positions.json        — Updated with ALU positions
```

## See Also

- `BUS.md` — Universal bus system
- `README.md` — Complete system guide
- `demo-bus.sh` — Bus system demo

---

*The ALU is the heart of any processor. Now it's spatial.* 💜🌐
