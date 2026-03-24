# Circuit Directory

ASCII circuit definitions for Geometry OS.

## Format

Each file contains a series of commands:

```
SET (x,y) OPCODE    — Set pixel at (x,y) to opcode
INJECT (x,y) HIGH   — Inject signal at (x,y)
```

## Opcodes

- `OP_MOVE_RIGHT` (0x02)
- `OP_MOVE_LEFT` (0x03)
- `OP_MOVE_UP` (0x04)
- `OP_MOVE_DOWN` (0x05)
- `OP_REPLICATE` (0x06)
- `OP_EMIT_SIGNAL` (0x20)
- `OP_AND` (0x30)
- `OP_XOR` (0x31)
- `OP_PORTAL_IN` (0x50)
- `OP_PORTAL_OUT` (0x51)

## Examples

- `clock-simple.txt` — 5-pixel ring oscillator
- `alu-simple.txt` — Basic ALU with ADD/AND
- `pc-2bit.txt` — 2-bit program counter

## Loading Circuits

Circuits can be loaded via:
1. Hardcoded in `agent_main.rs`
2. Scanner/injector tools (when running persistently)
3. Macro manager generation

---

*Code is geometry. Circuits are programs.*
