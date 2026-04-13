# Geometry OS Architecture

System-level documentation for features beyond the canvas text surface.
Read alongside CANVAS_TEXT_SURFACE.md (editor/assembly pipeline) and
SIGNED_ARITHMETIC.md (arithmetic semantics).

---

## Full Opcode Reference (44 opcodes)

### Control Flow
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x00 | HALT     |      | Stop execution |
| 0x01 | NOP      |      | No operation |
| 0x02 | FRAME    |      | Yield to renderer, increment TICKS |
| 0x03 | BEEP     | freq_reg, dur_reg | Sine-wave tone (20-20000 Hz, 1-5000 ms) |

### Data Movement
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x10 | LDI      | reg, imm | Load immediate |
| 0x11 | LOAD     | reg, [reg] | Load from memory |
| 0x12 | STORE    | [reg], reg | Store to memory |
| 0x13 | MOV      | rd, rs | Register copy |

### Arithmetic
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x20 | ADD      | rd, rs | rd = rd + rs |
| 0x21 | SUB      | rd, rs | rd = rd - rs |
| 0x22 | MUL      | rd, rs | rd = rd * rs |
| 0x23 | DIV      | rd, rs | rd = rd / rs |
| 0x24 | AND      | rd, rs | Bitwise AND |
| 0x25 | OR       | rd, rs | Bitwise OR |
| 0x26 | XOR      | rd, rs | Bitwise XOR |
| 0x27 | SHL      | rd, rs | Shift left |
| 0x28 | SHR      | rd, rs | Shift right |
| 0x29 | MOD      | rd, rs | Modulo |
| 0x2A | NEG      | rd     | Two's complement negation |
| 0x2B | SAR      | rd, rs | Arithmetic shift right (sign-preserving) |

### Branches
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x30 | JMP      | addr  | Unconditional jump |
| 0x31 | JZ       | reg, addr | Jump if zero |
| 0x32 | JNZ      | reg, addr | Jump if not zero |
| 0x33 | CALL     | addr  | Subroutine call (return addr in r31) |
| 0x34 | RET      |       | Return from subroutine |
| 0x35 | BLT      | reg, addr | Branch if r0 < 0 (after CMP) |
| 0x36 | BGE      | reg, addr | Branch if r0 >= 0 (after CMP) |

### Graphics
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x40 | PSET     | xr, yr, cr | Set pixel (registers) |
| 0x41 | PSETI    | x, y, c | Set pixel (immediates) |
| 0x42 | FILL     | cr     | Fill screen with color |
| 0x43 | RECTF    | xr,yr,wr,hr,cr | Filled rectangle |
| 0x44 | TEXT     | xr, yr, ar | Draw null-terminated string from RAM |
| 0x45 | LINE     | x0r,y0r,x1r,y1r,cr | Bresenham line |
| 0x46 | CIRCLE   | xr, yr, rr, cr | Midpoint circle |
| 0x47 | SCROLL   | nr     | Scroll screen up by N pixels |
| 0x4A | SPRITE   | xr,yr,ar,wr,hr | Blit NxM sprite from RAM (0=transparent) |
| 0x4C | TILEMAP  | xr,yr,mr,tr,gwr,ghr,twr,thr | Grid blit from tile index array |
| 0x4F | PEEK     | rx, ry, rd | Read screen pixel at (rx,ry) into rd |

### Stack & I/O
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x60 | PUSH     | reg   | Push to stack (r30 = SP) |
| 0x61 | POP      | reg   | Pop from stack |
| 0x50 | CMP      | rd, rs | Compare: r0 = -1/0/1 (lt/eq/gt) |
| 0x48 | IKEY     | reg   | Read keyboard port, clear it |
| 0x49 | RAND     | reg   | Pseudo-random u32 (LCG, seed 0xDEADBEEF) |

### Meta-Programming
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x4B | ASM      | src_reg, dest_reg | Assemble source text from RAM, write bytecode to RAM |

### Multi-Process
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x4D | SPAWN    | addr_reg | Create child process at address, PID in RAM[0xFFA] |
| 0x4E | KILL     | pid_reg | Terminate child process by PID |

---

## Memory Map

```
Address         Size     Purpose
──────────────────────────────────────────────────────────────
0x000-0x3FF     1024     Canvas grid (legacy, separate buffer in TEXT mode)
0x400-0xEFF     ~4K      Multi-process bytecode (via .org directive)
0xF00-0xF03     4        Window Bounds Protocol (win_x, win_y, win_w, win_h)
0x1000-0x1FFF   4096     Canvas bytecode output (F8 assembles here)
0x2000-0xFFA    ~60K     General purpose RAM
0xFFB           1        Key bitmask port (bits 0-5, read-only)
0xFFC           1        Network port (UDP)
0xFFD           1        ASM result port (word count or 0xFFFFFFFF on error)
0xFFE           1        TICKS port (frame counter, read-only)
0xFFF           1        Keyboard port (cleared on IKEY read)
──────────────────────────────────────────────────────────────
Total: 65536 (0x10000) u32 cells
```

---

## Multi-Process Architecture

Geometry OS supports up to 8 concurrent processes sharing the same 64K RAM.

### How It Works

Each process has its own register file (r0-r31) and program counter. The VM
scheduler cycles through all active processes, executing one instruction per
process per tick (round-robin).

- **SPAWN** (0x4D): Creates a child process. The parent provides an entry address
  via a register. The child starts with a fresh register file (all zeros) and
  shares the same RAM. The PID is written to RAM[0xFFA].
- **KILL** (0x4E): Terminates a child process by PID.
- **MAX_PROCESSES**: 8 (including the primary). Attempting to spawn beyond this
  limit silently fails.

### Window Bounds Protocol

For spatial coordination between processes, RAM[0xF00..0xF03] is a shared
convention:

| Address | Field | Who Writes |
|---------|-------|------------|
| 0xF00   | win_x | Primary |
| 0xF01   | win_y | Primary |
| 0xF02   | win_w | Primary |
| 0xF00   | win_h | Primary |

The primary process sets these values each frame. Child processes read them to
clamp their rendering within the allocated window area. This is a convention,
not enforced by hardware -- cooperative multitasking.

### Multi-Process Assembly

Use `.org <addr>` in a single assembly file to place child process code:

```
  LDI r0, child
  SPAWN r0          ; spawn child at label
  ; ... primary loop ...

.org 0x400
child:
  ; ... child process code ...
```

The assembler resolves `child` to its actual address (0x400 in this case),
so `LDI r0, child` loads the correct entry point.

---

## VM Instrumentation

### Access Log Buffer

The VM tracks LOAD, STORE, SPRITE, and TILEMAP memory accesses per frame.
Each access records the RAM address and type (read/write). The buffer wraps
and is consumed by the visual debugger overlay.

### Instruction Fetch Logging

Every PC value is logged to a circular buffer. Used by the visual debugger
to trace execution flow.

---

## Visual Debugger

### Memory Heatmap

A compact 256x256 view of the entire 64K RAM. Each pixel represents one word.
Colors pulse based on access patterns:

- **Cyan**: Recent read
- **Magenta**: Recent write
- **White**: Current PC position

The heatmap uses intensity decay -- highlights fade over ~10 frames.

### Canvas Cell Tinting

Active RAM addresses flash with colored borders on the canvas grid:

- **Cyan border**: Read access
- **Magenta border**: Write access

### PC Trail

A fading white glow follows the program counter across the canvas, showing
execution path.

### RAM Inspector Panel

A second 32x32 grid at the bottom of the window visualizes a scrollable
region of RAM (default 0x2000-0x23FF). PageUp/PageDown in Terminal mode
scrolls through different regions. Access intensities are shown as color tints.

---

## Audio

The BEEP opcode generates sine-wave tones by piping WAV data to `aplay` (Linux).
Requires `libasound2-dev`. Parameters:

- Frequency: 20-20000 Hz (from register)
- Duration: 1-5000 ms (from register)

Each BEEP spawns an `aplay` process. Rapid beeps can exhaust file descriptors
if not throttled.

---

## Platform Ports

### WASM (Web)

The VM compiles to WebAssembly via `wasm-pack`. Located in `wasm/`.

```bash
cd wasm
wasm-pack build --target web
```

The demo page (`wasm-demo/`) provides a browser-based interface with canvas
rendering. Full opcode set works in WASM mode, with the exception of BEEP
(audio uses Web Audio API instead of aplay).

### Network (UDP)

RAM[0xFFC] is a network port. Two VM instances can exchange messages via UDP.
The port is bidirectional -- writes send, reads receive.

### GlyphLang Backend

`src/glyph_backend.rs` emits Geometry OS bytecode from GlyphLang source.
This lets you write programs in a higher-level language and compile them
down to VM bytecode.

---

## Preprocessor (Abstraction Layer)

The preprocessor (`preprocessor.rs`) sits between the canvas text and the
assembler. It uses the same tokenizer that drives syntax highlighting.

### Macros

| Macro | Syntax | Expansion | Temp Registers |
|-------|--------|-----------|----------------|
| VAR   | `VAR name addr` | Defines variable | none |
| SET   | `SET var, val` | LDI r28, val / LDI r29, addr / STORE r29, r28 | r28, r29 |
| GET   | `GET reg, var` | LDI r29, addr / LOAD reg, r29 | r29 |
| INC   | `INC var` | LDI r29, addr / LOAD r28, r29 / LDI r27, 1 / ADD r28, r27 / STORE r29, r28 | r27, r28, r29 |
| DEC   | `DEC var` | LDI r29, addr / LOAD r28, r29 / LDI r27, 1 / SUB r28, r27 / STORE r29, r28 | r27, r28, r29 |

### #define Constants

```
#define TILE 8
#define MAX_X 255
```

The assembler replaces defined names with their values before instruction
parsing. Works in immediate contexts (LDI, PSETI, etc.).

### Register Safety

The preprocessor uses r27, r28, r29 as temporaries. Programs should avoid
relying on these across macro calls.

---

## Build & Run

```bash
# GUI mode
cargo run --release

# CLI mode (headless)
cargo run --release -- --cli

# WASM build
cd wasm && wasm-pack build --target web

# Run tests
cargo test
```

### Key Bindings (GUI)

| Key | Action |
|-----|--------|
| F5  | Run / resume |
| F6  | Single-step |
| F7  | Save state |
| F8  | Assemble canvas text |
| Ctrl+F8 | Load .asm file |
| F9  | Screenshot (PNG) |
| F10 | Toggle frame capture |
| Escape | Toggle editor/terminal |
| Backtick | (no longer toggles DIRECT mode) |

### CLI Commands

`help`, `load <name>`, `run`, `step`, `regs`, `peek <addr>`, `poke <addr> <val>`,
`bp [addr]`, `bpc`, `trace [n]`, `screenshot`, `save [slot]`, `load-slot [slot]`,
`reset`, `quit`

---

## Stats

- 5,623 lines of Rust (main.rs, vm.rs, assembler.rs, preprocessor.rs, font.rs, glyph_backend.rs)
- 44 opcodes
- 32 demo programs
- 113 tests
