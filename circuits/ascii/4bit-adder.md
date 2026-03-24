# 4-Bit Adder Design

A 4-bit adder adds two 4-bit numbers using cascaded full-adders.

## Structure

```
   A3 B3        A2 B2        A1 B1        A0 B0
    │  │         │  │         │  │         │  │
    └──┘         └──┘         └──┘         └──┘
     FA3          FA2          FA1          FA0
      │            │            │            │
  C4 ─┤       C3 ─┤       C2 ─┤       C1 ─┤
      │            │            │            │
     S3           S2           S1           S0
```

## Full Adder Logic

Each full-adder:
- **Inputs:** A, B, Cin (carry in)
- **Outputs:** S (sum), Cout (carry out)
- **Logic:** 
  - S = A XOR B XOR Cin
  - Cout = (A AND B) OR (Cin AND (A XOR B))

## ASCII Implementation

We'll use:
- `&` = AND gates
- `X` = XOR gates
- `-` = horizontal wires
- `|` = vertical wires
- `*` = signal splitter

## Layout

Position: (50, 50)
Size: 150×80

```
BIT 3          BIT 2          BIT 1          BIT 0
  │  │           │  │           │  │           │  │
  │  │           │  │           │  │           │  │
  └──┤           └──┤           └──┤           └──┤
     │              │              │              │
     X              X              X              X
     │              │              │              │
     &──┐           &──┐           &──┐           &──┐
     │  │           │  │           │  │           │  │
     │  │           │  │           │  │           │  │
  ───┼──┼───────────┼──┼───────────┼──┼───────────┼──┼──
     │  │           │  │           │  │           │  │
     │  │           │  │           │  │           │  │
     S3             S2             S1             S0
```

## Signal Flow

1. **Input A** enters from top left
2. **Input B** enters from top right
3. **Carry** propagates left-to-right
4. **Sum** outputs at bottom

## Testing

```bash
# Load the adder
./target/release/scanner load -f circuits/ascii/4bit-adder.txt -x 50 -y 50

# Inject test values
./target/release/injector inject -x 50 -y 50 -o MOVE_DOWN -r 255 -g 0 -b 0
./target/release/injector inject -x 60 -y 50 -o MOVE_DOWN -r 255 -g 0 -b 0

# Watch computation
./target/release/heatmap -f circuits/ascii/4bit-adder.txt --offset-x 50 --offset-y 50
```

## Expected Results

Input: A=5 (0101), B=3 (0011)
Output: S=8 (1000)

Signal propagation:
- Frame 0: Inputs injected
- Frame 1-5: Signals propagate through XOR gates
- Frame 6-10: Carry chain activates
- Frame 11+: Final sum visible
