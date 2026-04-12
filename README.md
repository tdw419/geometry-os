# Geometry OS v0.3.0

A pixel-art VM with a built-in assembler, debugger, and live GUI.

## What It Does

Geometry OS is a 256x256 pixel virtual machine with 32 registers, 32 opcodes,
memory-mapped I/O, and a real-time animation loop. You write assembly programs
in a text editor built into the GUI, hit run, and watch them execute live.

## Opcodes (32)

### Control
| Opcode | Args | Description |
|--------|------|-------------|
| HALT   |      | Stop execution |
| NOP    |      | No operation |
| FRAME  |      | Yield to renderer (animation tick) |

### Data
| Opcode | Args | Description |
|--------|------|-------------|
| LDI    | reg, imm | Load immediate |
| LOAD   | reg, [reg] | Load from memory |
| STORE  | [reg], reg | Store to memory |

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
| CALL   | addr  | Call subroutine |
| RET    |       | Return from subroutine |

### Graphics
| Opcode | Args | Description |
|--------|------|-------------|
| PSET   | xr, yr, cr | Pixel set (registers) |
| PSETI  | x, y, c | Pixel set (immediates) |
| FILL   | cr     | Fill screen with color |
| RECTF  | xr,yr,wr,hr,cr | Filled rectangle |
| TEXT   | xr, yr, ar | Draw null-terminated string from RAM[ar] |
| LINE   | x0r,y0r,x1r,y1r,cr | Bresenham line |
| CIRCLE | xr, yr, rr, cr | Midpoint circle |
| SCROLL | nr     | Scroll screen up by regs[nr] pixels |

### I/O
| Opcode | Args | Description |
|--------|------|-------------|
| IKEY   | reg   | Read keyboard port (RAM[0xFFF]), clear it |
| RAND   | reg   | Pseudo-random u32 (LCG) into reg |
| CMP    | rd, rs | Compare: r0 = -1/0/1 (lt/eq/gt) |
| PUSH   | reg   | Push to stack |
| POP    | reg   | Pop from stack |

## Memory-Mapped I/O

| Port | Address | Description |
|------|---------|-------------|
| TICKS | 0xFFE | Frame counter (read-only, incremented each FRAME) |
| KEY | 0xFFF | Keyboard input (read via IKEY) |

## Quick Start

```bash
cargo run --release
```

### CLI Mode
```bash
cargo run --release -- --cli
geo> load hello
geo> run
```

## Demo Programs

| Program | Description |
|---------|-------------|
| hello.asm | Hello world text |
| gradient.asm | Color gradient via nested loops |
| lines.asm | Star burst using LINE opcode |
| circles.asm | Concentric circles with cycling colors |
| fire.asm | Scrolling fire animation (FRAME + SCROLL) |
| ball.asm | Bouncing ball with WASD keyboard control |
| snake.asm | Snake game -- WASD control, random apples, growing tail |
| scroll_demo.asm | Horizontal bar that scrolls upward |
| checkerboard.asm | Checkerboard pattern |
| rainbow.asm | Rainbow stripes |
| rings.asm | Concentric rings |
| painter.asm | Paint program (cursor keys) |
| calculator.asm | Simple 4-function calculator |

## Animation Pattern

Any program can be an animation by replacing HALT with a loop:

```
loop:
  FILL r_black       ; clear screen
  ; ... draw scene ...
  FRAME              ; display + yield to renderer
  JMP loop
```

## Interactive Pattern

Read keyboard input with IKEY inside the animation loop:

```
loop:
  FILL r_black
  IKEY r10           ; read key press
  ; ... handle input + physics + draw ...
  FRAME
  JMP loop
```

## Throttling Pattern

Use the TICKS port (0xFFE) to control game speed independently of frame rate:

```
loop:
  FILL r_black
  IKEY r10
  ; ... handle input ...

  ; Throttle: only move every 8 frames (~7.5 moves/sec at 60fps)
  LDI r4, 0xFFE
  LOAD r8, r4           ; r8 = TICKS
  LDI r9, 7
  AND r8, r9            ; r8 = TICKS & 7
  JNZ r8, skip_move     ; skip if not a move frame
  ; ... update game state ...
skip_move:
  ; ... draw ...
  FRAME
  JMP loop
```

## GUI Controls

| Key | Action |
|-----|--------|
| F5 | Run program |
| F6 | Single-step |
| F7 | Save state |
| F8 | Load .asm file |
| Escape | Toggle editor/terminal |

## Architecture

- **VM**: 32 registers (r0-r31), 65536-word RAM, 256x256x32bit screen buffer
- **Assembler**: Two-pass with labels, hex/dec/bin literals
- **GUI**: minifb window with canvas text editor, VM screen, register display, disassembly panel
- **CLI**: Headless mode for testing and scripting
- **Memory map**: 0x0000-0x0FFF bytecode, 0x1000-0x1FFF canvas text, 0xFFE TICKS port (frame counter), 0xFFF keyboard port

## Stats

- ~5,100 lines of Rust (main.rs, vm.rs, assembler.rs, font.rs)
- 32 opcodes
- 23 demo programs
- 24 unit tests

## License

MIT
