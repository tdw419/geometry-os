# Geometry OS

A pixel-art virtual machine with a built-in assembler, text editor, debugger, and live GUI.

Write assembly. Press F5. Watch it run.

## What Is This?

Geometry OS is a from-scratch virtual machine: 32 registers, 65536 words of RAM, a 256x256 pixel framebuffer, and 40 opcodes. It has its own two-pass assembler, a real-time animation loop at 60fps, keyboard input, sound, sprite blitting, and an integrated text editor where you type assembly directly into the VM's memory and execute it live.

There is no compiler. No runtime. No garbage collector. You write the opcodes, the VM runs them. It's a computer small enough to hold in your head.

## Programs

28 programs included -- static art, animations, interactive games:

**Visual demos:** hello, gradient, diagonal, border, checkerboard, rainbow, rings, nested_rects, colors, circles, lines, fill_screen, stripes

**Animations:** fire (scrolling fire effect), scroll_demo

**Interactive:** blink, painter (freehand drawing), calculator (4-function)

**Games:** snake, ball (bouncing ball), breakout (4 rows of bricks, 3 lives), tetris (7 tetrominoes, rotation, line clearing), maze (randomly generated, WASD to navigate)

**Self-hosting:** self_host (writes assembly, assembles it, runs the output)

**Test helpers:** push_pop_test, shift_test, sprint_c_test (opcode verification)

**Demos:** sprite_demo (sprite blitting)

## Build & Run

**Prerequisites:** Rust (1.70+), Linux with `libasound2-dev` for sound

```bash
git clone https://github.com/tdw419/geometry-os.git
cd geometry-os
cargo run --release
```

**CLI mode** (headless, no GUI):
```bash
cargo run --release -- --cli
geo> load hello
geo> run
```

## The Instruction Set (40 opcodes)

### Control
| Opcode | Args | Description |
|--------|------|-------------|
| HALT   |      | Stop execution |
| NOP    |      | No operation |
| FRAME  |      | Yield to renderer (animation tick) |
| BEEP   | freq_reg, dur_reg | Play sine-wave tone (20-20000 Hz, 1-5000 ms) |

### Data
| Opcode | Args | Description |
|--------|------|-------------|
| LDI    | reg, imm | Load immediate value into register |
| LOAD   | reg, [reg] | Load from memory address |
| STORE  | [reg], reg | Store to memory address |

### Arithmetic
| Opcode | Args | Description |
|--------|------|-------------|
| ADD    | rd, rs | rd = rd + rs |
| SUB    | rd, rs | rd = rd - rs |
| MUL    | rd, rs | rd = rd * rs |
| DIV    | rd, rs | rd = rd / rs |
| MOD    | rd, rs | rd = rd % rs |
| NEG    | rd     | rd = -rd (two's complement) |

### Logic
| Opcode | Args | Description |
|--------|------|-------------|
| AND    | rd, rs | Bitwise AND |
| OR     | rd, rs | Bitwise OR |
| XOR    | rd, rs | Bitwise XOR |
| SHL    | rd, rs | Shift left |
| SHR    | rd, rs | Shift right |

### Branches
| Opcode | Args | Description |
|--------|------|-------------|
| JMP    | addr  | Unconditional jump |
| JZ     | reg, addr | Jump if zero |
| JNZ    | reg, addr | Jump if not zero |
| BLT    | reg, addr | Branch if r0 < 0 (after CMP) |
| BGE    | reg, addr | Branch if r0 >= 0 (after CMP) |
| CALL   | addr  | Call subroutine (return address in r31) |
| RET    |       | Return from subroutine |

### Graphics
| Opcode | Args | Description |
|--------|------|-------------|
| PSET   | xr, yr, cr | Set pixel (from registers) |
| PSETI  | x, y, c | Set pixel (immediates) |
| FILL   | cr     | Fill entire screen with color |
| RECTF  | xr,yr,wr,hr,cr | Filled rectangle |
| TEXT   | xr, yr, ar | Draw null-terminated string from RAM |
| LINE   | x0r,y0r,x1r,y1r,cr | Bresenham line |
| CIRCLE | xr, yr, rr, cr | Midpoint circle |
| SCROLL | nr     | Scroll screen up by N pixels |
| SPRITE | xr,yr,ar,wr,hr | Blit NxM sprite from RAM (0=transparent) |

### Stack & I/O
| Opcode | Args | Description |
|--------|------|-------------|
| PUSH   | reg   | Push to stack (r30 = SP) |
| POP    | reg   | Pop from stack |
| CMP    | rd, rs | Compare: r0 = -1/0/1 (lt/eq/gt) |
| IKEY   | reg   | Read keyboard port, clear it |
| RAND   | reg   | Pseudo-random u32 into register |
| ASM    | sr, dr | Assemble source text at RAM[sr], write bytecode to RAM[dr] |

## Memory-Mapped I/O

| Port  | Address | Description |
|-------|---------|-------------|
| ASM_RESULT | 0xFFD | Assembler output (word count or 0xFFFFFFFF on error) |
| TICKS | 0xFFE   | Frame counter (read-only, incremented each FRAME) |
| KEY   | 0xFFF   | Keyboard input (read via IKEY) |

## Writing Programs

**Animation loop** -- any program can animate by replacing HALT with a FRAME loop:

```
loop:
  FILL r_black       ; clear screen
  ; ... draw scene ...
  FRAME              ; display + yield
  JMP loop
```

**Keyboard input** -- read keys with IKEY inside the loop:

```
loop:
  FILL r_black
  IKEY r10           ; read key press
  ; ... handle input ...
  FRAME
  JMP loop
```

**Throttle game speed** with the TICKS port:

```
  LDI r4, 0xFFE
  LOAD r8, r4        ; r8 = current frame count
  LDI r9, 7
  AND r8, r9         ; r8 = TICKS & 7
  JNZ r8, skip_move  ; only move every 8th frame
  ; ... update game state ...
skip_move:
  FRAME
  JMP loop
```

## GUI Controls

| Key | Action |
|-----|--------|
| F5  | Run / resume program |
| F6  | Single-step (when paused) |
| F7  | Save VM state |
| F8  | Load .asm file |
| F9  | Screenshot (PNG) |
| Escape | Toggle editor / terminal |

**Terminal commands:** `help`, `load <name>`, `run`, `step`, `regs`, `peek <addr>`, `poke <addr> <val>`, `bp [addr]`, `bpc`, `trace [n]`, `screenshot`, `reset`, `quit`

## Architecture

```
┌─────────────────────────────────────────┐
│                 GUI Window              │
│  ┌──────────────┐  ┌────────────────┐  │
│  │ Text Editor  │  │   256x256      │  │
│  │ (32x32 grid) │  │   Screen       │  │
│  │              │  │                │  │
│  └──────────────┘  └────────────────┘  │
│  ┌──────────────┐  ┌────────────────┐  │
│  │ Registers    │  │  Disassembly   │  │
│  │ r0-r31       │  │  Panel         │  │
│  └──────────────┘  └────────────────┘  │
└─────────────────────────────────────────┘

VM: 32 registers, 65536-word RAM, 40 opcodes
Memory: 0x0000 bytecode | 0x1000 canvas text | 0xFFD ASM result | 0xFFE TICKS | 0xFFF KEY
```

## Stats

- 4,536 lines of Rust (main.rs, vm.rs, assembler.rs, font.rs)
- 40 opcodes
- 28 demo programs (visual demos, animations, interactive games)
- 41 tests

## License

MIT
