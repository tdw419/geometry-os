# Authoring Paths: Canvas Text Surface vs. Token-to-Pixel GUI

Two ways to produce a running Geometry OS app. They look different on the
surface but land at the same place: bytecode in RAM that the VM executes in a
FRAME loop. This document explains what each path is, when to use it, and why
neither is "better" than the other -- they're complementary on-ramps to the
same runtime.

Read this alongside:
- `CANVAS_TEXT_SURFACE.md` -- the human-authoring pipeline
- `PIXEL_DRIVING_PIXELS.md` -- the philosophical frame
- `programs/counter.asm` -- the reference persistent token-to-pixel app

---

## TL;DR

```
Canvas Text Surface      Token-to-Pixel GUI
-------------------      ------------------
human keystrokes    →    LLM / tool emissions
↓                         ↓
colored pixel glyphs     assembler text (or raw u32s)
on a 32x128 grid          ↓
↓                         bytecode in RAM
F8 reads grid as text     (no source view kept)
↓
bytecode at 0x1000        bytecode at 0x0000 (or anywhere)
↓                         ↓
        F5 / load → VM runs FRAME loop → persistent app
```

Both paths end at *bytecode in RAM executed by the VM*. The VM does not know
or care which path produced it. The difference is **who authored it** and
**whether the source stays visible**.

---

## Path A: Canvas Text Surface (human-authored)

### The chain

```
keystroke
  → ASCII byte stored in canvas_buffer[cell]
  → rendered as pixel-font glyph, colored by token type
  → (on F8) grid read back as text string
  → preprocessor expands macros (VAR/SET/GET/INC/DEC)
  → assembler produces bytecode
  → bytecode written to RAM[0x1000..]
  → (on F5) VM runs from 0x1000
```

### Who types

A human at the keyboard. Each keystroke produces one ASCII value that is
simultaneously:

1. The glyph shape (via `font::GLYPHS[byte]`)
2. The pixel color (via `syntax_highlight_color()` -- cyan=opcode,
   green=register, orange=number, yellow=label, gray=comment)
3. The source character the assembler will later read

The letter IS the colored pixels. The source IS the grid.

### Why this path exists

To close the "I want to see what I wrote" gap. Any human writing assembly
wants to see `LDI r0, 10` as `LDI r0, 10` -- not as a hex blob. The canvas
guarantees the source stays visible at 0x000-0x3FF (or in the separate
canvas_buffer) even after F8 assembles it to 0x1000. The two regions never
overlap.

### What it optimizes for

- **Reading** -- syntax coloring, persistent source view
- **Editing** -- WYSIWYG, in-place re-assembly (F8 → F5 cycle)
- **Discoverability** -- the grid teaches you the language by coloring it

### What it does NOT help with

- An LLM cannot type. It emits text one token at a time. Feeding 500 tokens
  through `key_to_ascii_shifted()` to populate a grid cell-by-cell is an
  absurd pipeline when the LLM could just emit bytecode directly.
- Large apps don't fit on a 32x128 grid (4096 cells). Something like
  `counter.asm` is ~180 lines; fine. Something like a full GUI framework
  is not.

---

## Path B: Token-to-Pixel GUI (LLM-authored)

### The chain

```
LLM (or tool) emission
  → assembly text (or raw u32 bytecode)
  → assembler produces bytecode (if text)
  → bytecode written to RAM[...]
  → VM runs FRAME loop
  → RECTF / TEXT draws pixels
  → HITSET registers semantic regions (x, y, w, h, id)
  → HITQ reads cursor → returns region id
  → app loop branches on the id, updates state, redraws
```

### Who emits

An LLM, or any bytecode-producing tool (a compiler, a code generator, a
script). The emitter writes the app *once*; it persists in RAM. There is no
continuous token stream while the app runs -- the FRAME loop just re-executes
the bytecode that's already there.

### The HITSET / HITQ additions (commit 90606c9fc)

Before these two opcodes, the VM could draw pixels (RECTF, TEXT) but had no
notion of *semantic regions*. A blue rectangle was just colored cells -- no
way to say "this is the OK button with id=1." HITSET fills that gap:

| Opcode | Encoding | What it does |
|--------|---------|-------------|
| HITSET 0x37 | xr, yr, wr, hr, id | Register a hit-region from register values + immediate id |
| HITQ 0x38 | rd | Write the id of the region containing the cursor (or 0) into rd |

Plus the host method `vm.push_mouse(x, y)` which the runtime calls on mouse
events. HITQ reads `vm.mouse_x / mouse_y` and tests the cursor against every
registered region.

### Why this path exists

To let an LLM (or any tool) ship a *persistent* app in one emission. The LLM
doesn't re-stream tokens every frame -- it writes the bytecode once, and the
VM runs it forever. This is the sovereignty-preserving property: the app's
execution does not depend on an ongoing connection to the model.

### What it optimizes for

- **Execution density** -- the app is a blob of u32s, not a source file
- **One-shot emission** -- write once, run indefinitely
- **Machine readability** -- no font rendering, no syntax coloring, no grid
  layout constraints. Just opcodes.

### What it does NOT help with

- Humans can't read raw bytecode. If you want to modify the app, you need
  the source (which lives wherever the LLM's transcript went, or in
  `programs/*.asm` if you saved it).
- There's no live re-edit loop. You reload the bytecode to change anything.

---

## Side-by-Side

| Aspect | Canvas Text Surface | Token-to-Pixel GUI |
|--------|---------------------|---------------------|
| Primary author | Human | LLM / tool |
| Source visibility | Always on grid | In `.asm` file (if saved) |
| Compile trigger | F8 | Host loads bytecode |
| Bytecode location | 0x1000 | 0x0000 (or wherever loaded) |
| Pixels encode | Token type (opcode / register / number) | Semantic region (button / label) |
| Edit loop | F8 → F5, in-place | Reload bytecode |
| Good for | Learning, authoring, debugging | Generated apps, one-shot emission |
| Persistence after load | ✅ | ✅ |
| FRAME loop model | ✅ | ✅ |
| Uses same VM / opcodes | ✅ | ✅ |

The bottom three rows are the point: **the runtime contract is identical**.
What changes is the authoring ergonomics and what the pixels represent
*before* they become bytecode.

---

## The Convergence: Pixels as the Semantic Unit

Both paths treat pixels as the universal medium, just at different scales:

- **CTS**: one pixel per *source token* (at 16x scaling, each character cell
  is 16x16 pixels -- but conceptually each cell = one token). The color IS
  the token type. The grid IS the source file.
- **Token-to-pixel**: one pixel (or rectangular block of pixels) per
  *semantic region*. The rectangle IS the button. The hit-region entry IS
  the event binding.

In both cases: **the pixel carries meaning, not just appearance.** The
difference is what kind of meaning.

```
Canvas Text Surface:     pixel = (source token)
Token-to-pixel GUI:      pixel = (app region semantic)

Both:                    the pixel is the first-class semantic unit,
                         not a byproduct of some higher-level model.
```

This is the same thesis as `PIXEL_DRIVING_PIXELS.md`: we want a world where
pixels act on pixels without leaving the pixel layer. CTS does it for source
(coloring = tokenization). HITSET does it for runtime (rectangles =
interactive regions). They're two applications of the same idea.

---

## When to Use Which

### Use Canvas Text Surface when:
- A human is writing the program
- You want to see + edit + re-assemble in a tight loop
- The program fits in 4096 cells (~30 lines of assembly with comments)
- You're teaching, debugging, or exploring the ISA
- Syntax highlighting and WYSIWYG matter more than emission speed

### Use Token-to-Pixel GUI when:
- An LLM (or any tool) is generating the program
- The program should run without ongoing tool involvement
- You want apps that persist in RAM and reuse bytecode across sessions
- The program needs interactive regions (buttons, clickable areas) and
  cursor routing -- i.e., real GUI semantics, not just pixel art
- You're shipping to many users where the compiled blob is smaller than
  the source text

### The apps we've shipped

| Program | Path | What it proves |
|---------|------|----------------|
| `programs/snake.asm` | CTS (human-written) | Games fit the CTS path |
| `programs/fire.asm` | CTS | Animation via SCROLL + FRAME |
| `programs/hello_window.asm` | Token-to-pixel | Minimal static UI + hit region |
| `programs/counter.asm` | Token-to-pixel | Full event loop: click → arithmetic → redraw |

---

## Future: Unifying the Paths

The interesting next step is making an LLM emit onto the canvas grid instead
of emitting bare assembly. The LLM would write colored token-pixels (cyan
LDI, green r0, orange 10) directly into `canvas_buffer[]`. Then F8
assembles as usual. This gives:

1. The LLM's output becomes *human-readable source* (the grid shows it)
2. The human can edit and re-assemble with F8 -- same loop
3. The bytecode runtime contract stays unchanged
4. The LLM and the human share one authoring surface

At that point, the two on-ramps merge. The canvas is the IDE for both
humans and LLMs, and pixels are the universal medium for both source and
runtime.

Until then, the two paths run in parallel: humans use F8, LLMs use HITSET.

---

## See Also

- `docs/CANVAS_TEXT_SURFACE.md` -- full specification of the human path
- `docs/PIXEL_DRIVING_PIXELS.md` -- the philosophy behind pixel-as-unit
- `docs/TEXT_AS_PIXEL_IMAGE.md` -- how text becomes image
- `programs/counter.asm` -- reference persistent app (token-to-pixel)
- `programs/hello_window.asm` -- minimal HITSET/HITQ example
- `src/vm/ops_graphics.rs` -- HITSET/HITQ handler source (0x37, 0x38)
- commit `90606c9fc` -- where HITSET/HITQ + quote-aware assembler landed
