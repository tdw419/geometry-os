# Canvas Text Surface: How to Write Programs by Typing Text on the Pixel Grid

This document explains the Canvas Text Surface feature in Geometry OS. It is
written for AI agents who need to understand, use, or extend this system.

Read this alongside KEYSTROKE_TO_PIXELS.md (the foundational document) and
PIXELC_GUIDE.md (the Python-to-bytecode compiler).

---

## What It Is

The canvas grid IS a text editor. Each cell holds one ASCII character. You type
assembly source code directly onto the 32x32 grid, press F8 to assemble it, and
F5 to run it. The grid reads like a text file rendered in colored pixels.

The VM does not change. The assembler does not change. Only the canvas input
and display model changed.

---

## The Core Chain

```
keystroke -> ASCII byte value -> stored in vm.ram[cell] as u32
                                    |
                          +---------+---------+
                          |                   |
                     rendering            assembly (F8)
                          |                   |
                   pixel font glyph     read grid as text string
                   colored by           -> assembler::assemble()
                   palette_color()      -> bytecode at 0x1000
                          |                   |
                   the letter IS        F5 runs from 0x1000
                   the colored pixels
```

A single keystroke produces one ASCII value. That value:
- Determines the pixel color via `palette_color(val)` -- the ASCII byte maps
  directly to an HSV hue
- Determines the glyph shape via `font::GLYPHS[byte]` -- an 8x8 bitmap
- Gets stored in RAM at the cursor position

The letter shape and the color both come from the same source value. No
overlays. No separate rendering layer. The text IS pixels.

---

## Two Modes: TEXT and DIRECT

The backtick key (`) toggles between TEXT mode (default) and DIRECT mode.

### TEXT Mode (default)

- Keystrokes write ASCII characters to grid cells
- `key_to_ascii_shifted(key, shift)` handles the mapping
- Shift+letter = uppercase, no shift = lowercase
- Enter = newline (writes `\n` and advances to next row)
- Space = 0x20 (space character)
- F8 = reads the grid as a text string, assembles it, stores bytecode at 0x1000
- F5 = runs the VM from 0x1000 (after F8 assembly)
- The grid displays characters rendered as pixel-font glyphs

### DIRECT Mode

- Keystrokes write raw byte values to grid cells
- `key_to_pixel(key, hex_mode)` handles the mapping
- Single-char opcode encoding: `A` = ADD, `I` = LDI, etc.
- F8 = loads programs/boot.asm from disk
- F5 = runs from PC=0
- The grid displays solid colored cells (no glyph rendering)
- Tab toggles hex mode for entering raw nibbles

---

## How TEXT Mode Input Works

When `text_surface_mode == true` and the VM is stopped and not in any special
mode (editor/REPL/ASM), keystrokes go through this path in main.rs:

```
Keypress
  |
  v
Is it Enter?
  YES -> write '\n' (0x0A) to current cell, advance cursor to start of next row
  NO -> Is it Space?
    YES -> write 0x20 to current cell, advance cursor one cell right
    NO -> key_to_ascii_shifted(key, shift)?
      Some(ch) -> write ch as u32 to current cell, advance cursor one cell right
      None -> key not recognized, ignore
```

The cursor wraps at column 32 (next row) and at row 32 (back to row 0).

- Backspace clears the current cell and moves the cursor back one position.

Arrow keys move the cursor without modifying cells.

**Ctrl+V** pastes text from the system clipboard onto the grid at the cursor
position. Newlines advance to the next row. Text wraps at column 32 and stops
at the bottom of the grid (row 32). Carriage returns (`\r`) are stripped. The
status bar shows how many characters were pasted, or an error if clipboard
access fails.

Uses the `arboard` crate for cross-platform clipboard access (X11 on Linux,
Win32 on Windows, NSPasteboard on macOS).

---

## How F8 Assembly Works (TEXT Mode)

When you press F8 in TEXT mode (without Ctrl held):

1. Read all 1024 cells (32x32 grid) from `vm.ram[0..1024]`
2. Convert each u32 to a character:
   - `0` (null) becomes `\n` (line break)
   - `0x0A` (explicit newline) stays as `\n`
   - Any other value becomes `(val & 0xFF) as u8 as char`
3. Collapse consecutive newlines to avoid blank lines
4. Pass the resulting string to `assembler::assemble(&source)`
5. On success:
   - Clear the bytecode region at `CANVAS_BYTECODE_ADDR` (0x1000) for 4096 cells
   - Write assembled bytecode bytes to `vm.ram[0x1000..]`
   - Set `canvas_assembled = true`
   - Set `vm.pc = 0x1000`
   - Set `vm.halted = false`
   - The source text on the grid is NOT modified -- it stays visible
6. On error:
   - Display the assembler error message in the status bar

The key insight: your source text at 0x000-0x3FF stays intact. Bytecode lives
at 0x1000+. They don't overlap. You can always see what you wrote.

### Ctrl+F8 in TEXT Mode

Holding Ctrl while pressing F8 enters **file input mode**. A prompt appears in the
status bar: `[load file: | Tab=complete, Enter=load, Esc=cancel]`.

Type a file path (absolute or relative). Press **Tab** to cycle through `.asm`
files in the `programs/` directory. Press **Enter** to load the file onto the
grid (clears the grid first, like the command-line argument). Press **Escape** to
cancel.

If a file was previously loaded (via command-line or Ctrl+F8), the path is
pre-populated so you can just press Enter to reload it.

After loading, the file path is remembered so the next Ctrl+F8 starts with it
pre-filled. The source text appears on the grid ready for F8 assembly.

---

## How F5 Runs (After Canvas Assembly)

When `canvas_assembled == true`:
- F5 starts execution at `vm.pc = CANVAS_BYTECODE_ADDR` (0x1000)
- The VM fetches and executes bytecode from that address
- The canvas grid continues showing your source text

When `canvas_assembled == false`:
- F5 starts execution at `vm.pc = 0` (standard behavior)

---

## How Pixel Font Rendering Works

In TEXT mode, each non-empty cell with a printable ASCII value (0x20-0x7F) is
rendered using the pixel font method. The rendering pipeline:

```
For cell at (row, col):
  1. val = vm.ram[row * 32 + col]
  2. ascii_byte = val & 0xFF
  3. fg = palette_color(val)           // HSV hue from the byte value
  4. glyph = font::GLYPHS[ascii_byte]  // 8x8 bitmap from font.rs
  5. For each pixel (dx, dy) in the 16x16 cell:
       a. Map to glyph coordinates (gx, gy) at 2x scale
       b. If glyph bit is ON:  pixel color = fg (colored letter pixel)
       c. If glyph bit is OFF: pixel color = GRID_BG (dark background)
       d. Cell border (right/bottom edge): GRID_LINE color
       e. Cursor/PC highlight: CYAN/MAGENTA border override
```

The result: each character appears as its letter shape built from colored
pixels. Adjacent characters with different byte values have different colors.
The color is NOT arbitrary -- it derives directly from the ASCII value:

```
palette_color(val):
  t = (val - 32) / 94            // normalize printable ASCII to 0..1
  hue = t * 360                   // spread across full color wheel
  saturation = 0.8, value = 1.0
  return HSV(hue, 0.8, 1.0) as RGB u32
```

This means:
- Uppercase letters (opcodes: A-Z) cluster in the green-blue hue range
- Lowercase letters cluster in the blue-magenta range
- Digits (0-9) cluster in the yellow-green range
- Symbols are scattered across the rest

The color gives you structural information at a glance. You can scan a grid
and see opcodes, registers, and numbers as different color groups.

---

## Memory Map for Text Surface Mode

```
Address        Size    Purpose
---------------------------------------------------------------
0x000-0x3FF   1024    Canvas grid (source text in TEXT mode)
                       Each cell = one ASCII character as u32
                       Visible on the 32x32 grid
0x400-0x7FF   1024    Text input buffer (micro-asm)
0x800-0xBFF   1024    VM-resident micro-assembler
0xC00-0xFFF   1024    Label table
0xFFF          1       Keyboard port (memory-mapped I/O)
0x1000-0x1FFF 4096    Canvas bytecode output
                       F8 assembles grid text here
                       F5 runs VM from here when canvas_assembled=true
0x2000-0xFEFF ~60K    General purpose RAM
0xFF00-0xFFFF 256     Hardware registers
---------------------------------------------------------------
Total: 65536 (0x10000) u32 cells
```

The source text and the assembled bytecode live in different memory regions.
Source at 0x000 stays visible on the grid. Bytecode at 0x1000 is invisible but
fully addressable by the VM.

---

## Step-by-Step: Writing and Running a Program

1. Launch Geometry OS (TEXT mode is the default)
2. Type your assembly program on the grid:
   ```
   LDI r0, 10
   LDI r1, 20
   ADD r0, r1
   HALT
   ```
   Each character appears as a colored pixel glyph in its own cell.
   Press Enter after each line to advance to the next row.

3. Press F8 to assemble
   - Status bar shows: `[OK: N bytes at 0x1000]`
   - If there's a syntax error, the error message appears instead

4. Press F5 to run
   - VM starts executing from 0x1000
   - The grid still shows your source text
   - The VM screen (256x256 panel) shows program output

5. Press F5 again to pause
6. Press F5 again to resume, or modify the source and press F8 to reassemble

---

## Practical Examples

### Example 1: Simple Add

Type on the grid (TEXT mode):
```
LDI r0, 10
LDI r1, 20
ADD r0, r1
HALT
```
F8 -> F5. After running, r0 = 30.

### Example 2: Loading an Existing Program

To load an .asm file from disk onto the grid:
- Press Ctrl+F8 to enter file input mode
- Type a path or press Tab to cycle through `programs/*.asm` files
- Press Enter to load it onto the grid
- Then F8 to assemble, F5 to run

### Example 3: Converting Existing Code

Any existing .gasm or .asm file can be "typed" onto the grid. The text
representation on the grid is identical to the file contents. Feed each
character to the grid cells and it renders the same source. F8 assembles it
exactly as if it came from a file.

---

## What AI Agents Need to Know

### If you're writing programs for the grid:

1. Use standard assembly mnemonics (LDI, ADD, HALT, etc.) -- NOT single-char
   codes (I, A, H). TEXT mode reads full assembly text.

2. Each cell holds one character. A line like `LDI r0, 10` takes 10 cells.

3. The 32x32 grid has 32 cells per row, 32 rows = 1024 characters max.
   That's about 30-40 lines of average assembly, depending on line length.

4. Enter newlines explicitly (press Enter / write 0x0A). The assembler reads
   the grid as one big text string with embedded newlines.

5. Null cells (value 0) are treated as newlines during assembly. Unwritten
   cells at the end of a line don't matter.

6. Source text stays at 0x000-0x3FF. Bytecode assembles to 0x1000. They
   never overlap.

### If you're modifying the codebase:

1. The toggle state is `text_surface_mode: bool` in main.rs
2. Input handling branches at the `text_surface_mode` check (~line 770)
3. F8 assembly is at ~line 1049 (the `if text_surface_mode && !ctrl` branch)
4. Pixel font rendering is at ~line 1833 (the `if use_pixel_font` branch)
5. The rendering condition:
   ```rust
   let use_pixel_font = text_surface_mode
       && !pending_here        // not mid-hex-entry
       && val != 0             // not empty cell
       && ascii_byte >= 0x20   // printable ASCII
       && ascii_byte < 0x80;
   ```
6. The font data is in `font.rs` -- the `GLYPHS` array, 128 entries of 8 u8
   rows each (8x8 VGA/CP437-style bitmaps). Bit test uses `(7 - col)`.

### Key constants:

```rust
const CANVAS_COLS: usize = 32;           // grid width
const CANVAS_ROWS: usize = 32;           // grid height
const CANVAS_SCALE: usize = 16;          // pixels per cell on screen
const CANVAS_BYTECODE_ADDR: usize = 0x1000; // where assembled bytecode goes
```

### Key functions:

| Function | Purpose |
|----------|---------|
| `key_to_ascii_shifted(key, shift)` | TEXT mode input: key -> ASCII with shift awareness |
| `key_to_pixel(key, hex_mode)` | DIRECT mode input: key -> raw byte value |
| `key_to_ascii(key)` | Runtime input: key -> ASCII (no shift) |
| `palette_color(val)` | ASCII value -> HSV -> RGB u32 color |
| `font::GLYPHS[byte]` | 8x8 bitmap for the character |
| `assembler::assemble(&text)` | Text -> bytecode (shared by all modes) |

---

## Relationship to Other Features

| Feature | How it relates to text surface |
|---------|-------------------------------|
| KEYSTROKE_TO_PIXELS.md | The foundational document. Text surface builds on the same keystroke-to-RAM path. |
| DIRECT mode | The original mode. Toggle with backquote. Raw bytes, no text rendering. |
| Editor (F9) | A separate text editor with its own buffer. Both feed the same assembler. |
| REPL (F6) | Live single-instruction execution. Uses the assembler but not the grid. |
| pixelc compiler | Python-to-.gasm compiler. Output can be typed onto the grid or loaded via Ctrl+F8. |
| font.rs | The 8x8 VGA bitmap font data. Used for pixel font rendering in TEXT mode. |

---

## Design Rationale

Why "keystroke -> pixel" and not "keystroke -> framebuffer":

The framebuffer (buffer[1024x768]) is the final host-side pixel buffer that
minifb renders to the OS window. It's a rendering concern, not a semantic one.
The meaningful unit is the RAM cell -- it holds the character value that is
simultaneously the display color, the glyph shape, and (after assembly) the
program instruction.

Why TEXT mode is the default:

The original DIRECT mode (single-char opcodes: `I` = LDI, `A` = ADD) requires
memorizing a mapping table. TEXT mode is self-documenting: `LDI r0, 10` on the
grid reads as `LDI r0, 10`. Anyone who can read assembly can read the grid.

Why source and bytecode are separate:

Keeping source text at 0x000-0x3FF and bytecode at 0x1000 means you always
see what you wrote. You can reassemble after editing without losing your
source. The grid IS the source file.
