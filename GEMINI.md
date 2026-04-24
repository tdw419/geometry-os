# Architectural Philosophy: Token -> Pixel -> GUI

Geometry OS follows a strict layered substrate. Do not skip layers.

- **Layer 1: Token (Instruction Stream):** The raw u32 bytecode. All logic must decompose to these.
  - **Subtle Trap:** `CMP` and `CMPI` write results to **r0**, not `rd`. Always branch based on `r0` after comparison.
- **Layer 2: Pixel (Framebuffer Primitives):** The primary visual interface.
  - **Output:** `PSET`, `PSETI`, `FILL`, `RECTF`, `TEXT`, `LINE`, `CIRCLE`, `SCROLL`, `SPRITE`, `TILEMAP`, `PEEK`.
  - **Input:** `FRAME` (yield), `IKEY` (keyboard), `HITSET`/`HITQ` (regions), `MOUSEQ` (cursor).
  - **AI Advantage:** Coordinate math is free for AI; prefer explicit pixel composition over rigid GUI abstractions.
- **Layer 3: GUI (Conventions):** Widgets (buttons, windows) are **patterns** of Layer 2 ops, not VM opcodes.

### The Promotion Rule
New opcodes are **earned**, not guessed:
1. Implement as a raw pixel pattern.
2. If reused, promote to a macro or subroutine.
3. If stable and high-performance requirement exists, promote to a VM opcode (e.g., `HITSET`).

**Mandate:** Do not propose new GUI opcodes (e.g., `DRAW_BUTTON`) unless the pattern is proven across multiple programs.

# Visual Authoring: Canvas Text Surface

The grid IS the text editor and the source of truth.

- **TEXT Mode (Default):** 32-column grid. "The letter IS the colored pixels."
- **F8 Assembly Pipeline:**
  - Read grid (`0x000-0x3FF`) -> Preprocessor (Macro expansion) -> Assembler -> Bytecode at `0x1000`.
  - Use `.org <addr>` to layout multi-process programs in one file.
- **Preprocessor Macros:** Use to reduce boilerplate (Reserved registers: `r27`, `r28`, `r29`):
  - `VAR name addr`: Define variable.
  - `SET var, val`: `LDI r28, val` / `LDI r29, addr` / `STORE r29, r28`.
  - `GET reg, var`: `LDI r29, addr` / `LOAD reg, r29`.
  - `INC`/`DEC var`: Atomic math on RAM addresses.
- **Input & Navigation:**
  - `Ctrl+V`: Paste text to grid.
  - `PageUp`/`PageDown`: Scroll the 32x128 logical grid.
  - `Ctrl+F8`: Load `.asm` files from disk onto the grid.

# Memory Map & Protocols

| Range | Purpose |
|-------|---------|
| `0x0000 - 0x03FF` | Visible Source Grid (1024 cells) |
| `0x1000 - 0x1FFF` | Canvas Bytecode Output (Invisible to grid) |
| `0x8000 - 0x8FFF` | RAM-mapped mirror of Canvas Grid |
| `0x10000 - 0x1FFFF`| RAM-mapped Screen Buffer (256x256 pixels) |
| `0xF000 - 0xF003` | Window Bounds Protocol (`win_x`, `win_y`, `win_w`, `win_h`) |
| `0xF010 - 0xF01F` | System Clipboard (Ownership, Len, Data/Path) |
| `0xFFFB - 0xFFFF` | Hardware Ports (Keys, Net, ASM, Ticks, Keyboard) |
