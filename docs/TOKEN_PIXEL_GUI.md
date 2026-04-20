# Token -> Pixel -> GUI

The layered stack Geometry OS builds on, what each layer contributes, and why
this ordering is the right one for an AI-programmed system.

Read this alongside:
- `NORTH_STAR.md` -- why we build what we build
- `PIXEL_DRIVING_PIXELS.md` -- the pixel-as-unit thesis
- `AUTHORING_PATHS.md` -- how a program gets *into* RAM (orthogonal axis)
- `programs/counter.asm`, `programs/paint.asm`, `programs/terminal.asm` --
  reference apps at different points on the stack

---

## TL;DR

```
Layer 3  GUI     widgets (button, textbox, menu, list, window)
                 -- not opcodes yet; patterns of pixel + input ops
                     ^
                     | composition + naming
                     |
Layer 2  Pixel   framebuffer primitives: PSET/PSETI, RECTF, LINE,
                 CIRCLE, TEXT, FILL, SCROLL, SPRITE, TILEMAP, PEEK
                 input primitives: IKEY, HITSET, HITQ, MOUSEQ, FRAME
                     ^
                     | opcode side-effects
                     |
Layer 1  Token   instruction stream: MOV, ADD, CMP, JNZ, CALL, RET,
                 LDI, STO, LOD, PUSH, POP, etc.
```

Each layer is a small, regular vocabulary that composes into the next. The
VM runs tokens; tokens move pixels; pixels form widgets. You can always
step one layer down to debug; nothing is hidden.

---

## Layer 1: Token

**What it is.** The instruction stream the VM consumes. In Geometry OS this
is a dense u32 array where each word is an opcode or an operand. Tokens are
the only thing the CPU actually "sees."

**What it provides.**

- Control flow: `JMP`, `JZ`, `JNZ`, `BLT`, `BGE`, `CALL`, `RET`, `HALT`
- Data movement: `MOV`, `LDI`, `LOD`, `STO`, `PUSH`, `POP`
- Arithmetic / logic: `ADD`, `SUB`, `MUL`, `AND`, `OR`, `XOR`, `CMP`, `CMPI`
- Side-effect opcodes that reach the next layer (graphics, input, timers)

**What it does not provide.** Anything visible. A pure-token program that
computes primes and never touches graphics is valid and invisible. Tokens
are the nervous system, not the face.

**A subtle trap.** `CMP rd, rs` and `CMPI rd, imm` write their result to
**r0**, not to `rd`. Every conditional branch reads **r0**. This caught
`terminal.asm` on its first draft and is the single easiest mistake for an
LLM to make at this layer. Always branch on r0 after a compare.

---

## Layer 2: Pixel

**What it is.** The narrowest possible visual substrate: a framebuffer of
`(x, y) -> u32 color`, plus a handful of opcodes that write to it and read
from it, plus a handful that deliver input events into registers.

### Output primitives

| Opcode | Hex  | Role                                            |
|--------|------|-------------------------------------------------|
| PSET   | 0x40 | Plot one pixel at (xr, yr) in color cr          |
| PSETI  | 0x41 | Plot one pixel at (x, y) immediate with color   |
| FILL   | 0x42 | Fill the whole framebuffer with color cr        |
| RECTF  | 0x43 | Filled rectangle (x, y, w, h, color)            |
| TEXT   | 0x44 | Draw string at (x, y) from address register     |
| LINE   | 0x45 | Line from (x0, y0) to (x1, y1) in color         |
| CIRCLE | 0x46 | Circle at (x, y) radius r in color              |
| SCROLL | 0x47 | Scroll framebuffer by n rows                    |
| SPRITE | 0x4A | Blit w*h pixel block from address               |
| TILEMAP| 0x4C | Render a tile grid from a map + tile sheet      |
| PEEK   | 0x4F | Read framebuffer pixel at (x, y) into register  |

### Input primitives

| Opcode | Hex  | Role                                             |
|--------|------|--------------------------------------------------|
| FRAME  | 0x02 | Yield until next frame (the persistent-app loop) |
| IKEY   | 0x48 | Pop keyboard ring buffer into register           |
| HITSET | 0x37 | Register hit region (x, y, w, h, id)             |
| HITQ   | 0x38 | Report which region contains cursor -> register  |
| MOUSEQ | 0x85 | Report current mouse (x, y) into registers       |

**Properties that matter.**

- **Uniform.** Every op has the same shape: coordinates and/or color in,
  a side-effect out. The vocabulary is small enough to hold in one page.
- **Compositional.** Every higher visual concept decomposes into these
  primitives. Nothing visible exists that cannot be built from them.
- **Verifiable byte-exact.** Tests assert on framebuffer contents via
  `PEEK` or direct buffer inspection. `assert pixel(10, 10) == RED` is
  the tightest possible feedback loop.
- **Input semantically thin.** `IKEY` is "a keystroke"; `HITQ` is "a
  region id"; `MOUSEQ` is "a cursor position." None of them know what a
  button, a textbox, or a menu is. That knowledge lives in Layer 3.

### Apps that live at Layer 2 today

| Program              | Exercises                                  |
|----------------------|--------------------------------------------|
| `hello_window.asm`   | RECTF + TEXT + one HITSET, no input loop   |
| `pulse.asm`          | FRAME-driven animation, pure pixel output  |
| `counter.asm`        | RECTF + TEXT + HITSET + HITQ + state       |
| `paint.asm`          | MOUSEQ + PSET (drag-to-draw)               |
| `terminal.asm`       | IKEY + TEXT rows + SCROLL + command dispatch |

These are not "Layer 2 apps because they are primitive." They are Layer 2
apps because they chose to hand-roll their visuals. That's the right
default right now -- the vocabulary above is expressive enough that
widgets do not need to exist yet.

---

## Layer 3: GUI

**What it is today.** A convention, not a set of opcodes. A "widget" is a
named pattern of Layer 2 ops, plus input bindings, wrapped in a subroutine,
macro, or file template.

### Widgets as pixel decompositions

| Widget     | Decomposition                                         |
|------------|-------------------------------------------------------|
| Button     | `RECTF` (body) + `TEXT` (label) + `HITSET` (region)   |
|            | + branch on `HITQ` returning the id                   |
| Textbox    | `RECTF` (border) + cursor state + `IKEY` loop +       |
|            | `TEXT` redraw of the buffer each frame                |
| List row   | `TEXT` at `(x, y + i*row_h)` + `HITSET` per row       |
| Scroll view| Offset var applied to every nested draw + clip rect + |
|            | arrow-key or drag handler that updates the offset     |
| Menu       | List of rows + selected-index var + up/down keys +    |
|            | Enter fires the row's action                          |
| Window     | Outer `RECTF` + title-bar `RECTF` + `TEXT` title +    |
|            | close-button `HITSET` + inner region with offset      |

**None of these require a new opcode.** They are pure compositions of
pixel + input primitives. A new widget is a new subroutine, not a new
VM feature.

### The promotion rule

The stack is additive, and the ratchet goes one way:

```
raw pixel pattern  --(used in several apps)-->  macro / subroutine
     macro         --(stable, high call count)-->  opcode
```

`HITSET`, `HITQ`, and `MOUSEQ` all arrived this way. They were not
designed up front. They were promoted from repeated pixel patterns in
`counter.asm` and `paint.asm`. That is the signal an opcode has *earned*
its place: real apps demanded it, and the replaced pattern collapsed.

**Don't pre-ship widgets as opcodes.** A GUI layer you invent before
real apps exercise it is a guess; a GUI layer that emerges from usage is
a measurement.

---

## Why the ordering matters

### You cannot skip Token

Tokens are what the VM consumes. There is no stack without them.

### You should not skip Pixel

Three things die if you do:

1. **Visual diversity.** Paint, games, charts, animations, waveforms,
   sprites, custom glyphs -- none of them express cleanly as widgets.
   Pre-shipped GUI primitives cap the possibility space at whatever
   widgets were imagined on day one.
2. **Debuggability.** Pixel output is byte-exact verifiable. Widget
   output is "the widget rendered." The first gives tests something
   deterministic to hold onto; the second asks you to trust the widget
   implementation.
3. **Evidence-driven growth.** Without Layer 2 in the middle, there is
   no place for patterns to live and prove themselves before becoming
   opcodes. The promotion ratchet needs a floor.

### You can defer GUI indefinitely

GUI does not need to exist as a formal layer until the evidence forces
it. Right now, every "widget" in `terminal.asm`, `counter.asm`, and
`paint.asm` is hand-rolled pixel composition. When the same 30-line
sequence appears in its fifth app, that is the signal. Until then, the
absence of a GUI layer is a feature, not a gap.

---

## Why this ordering fits an AI programmer

These five reasons compound. Together they argue that Token -> Pixel -> GUI
is not merely a valid progression, but the one most aligned with how an
AI actually works.

**1. Small uniform opcode surfaces are learned deeply; sprawling APIs
are learned shallowly.** The pixel layer is ~15 primitives. A widget
catalog is open-ended (button, textbox, dropdown, menu, tabs, tree,
table, dialog, slider, toggle, tooltip, ... each with variants). Every
widget opcode is a new thing to remember, version, and debug -- and
the catalog is never done.

**2. Coordinate math is free for AIs, expensive for humans.** The
historical reason high-level GUI toolkits exist is that humans hate
writing nested pixel loops. AIs don't share that cost; arithmetic is
just tokens. The DX argument for GUI-first was built around a
constraint AIs don't have.

**3. Pixel composes; GUI enumerates.** With pixel primitives an AI can
*invent* widgets -- a radial menu, a waveform view, a chess board, a
paint canvas. With GUI primitives it is capped at whatever catalog
ships. Pixel is a floor with unlimited ceiling; widgets pre-shipped as
opcodes are a ceiling with fixed floor.

**4. Byte-exact verification is a tighter loop than widget-render
trust.** The test suite already asserts on framebuffer contents. Every
pixel op has a deterministic, inspectable outcome. Widget-level testing
("a button appeared") obscures the failure mode and removes the thread
the AI would pull to debug.

**5. Opaque primitives terminate the debugging loop; transparent ones
don't.** If a widget opcode has a bug, the AI cannot fix it -- it's a
black box inside the VM; the only move is to swap widgets or complain.
If a pixel-composed widget has a bug, the AI reads the pixel math, sees
the off-by-one, fixes it. Transparent primitives keep the whole stack
in reach.

---

## When would Token -> GUI (skipping Pixel) be right?

Not never. It would be right if:

- The target is only line-of-business apps (forms, lists, CRUD)
- Development speed dominates and visual diversity is out of scope
- The AI author has tight context budgets and cannot afford pixel math

Geometry OS is none of those. It is a substrate, not a line-of-business
toolkit. Paint, games, charts, custom visuals are first-class goals.
For this target, Pixel-first is structurally correct.

---

## What this gives us

- **One substrate, infinite ceiling.** Any visual is expressible.
- **Every layer testable at its own resolution.** Token ops have VM
  tests. Pixel ops have framebuffer assertions. Widgets, when they
  arrive, will be tested against composed pixel patterns.
- **No black boxes.** Descend one layer to debug anything.
- **Evidence-driven vocabulary growth.** New opcodes exist because
  real apps demanded them, not because someone guessed they would be
  useful. The HITSET / HITQ / MOUSEQ trio is the template.

Token -> Pixel -> GUI treats the AI as a capable low-level programmer
that can build its own abstractions. Token -> GUI treats it as a human
who needs pre-built widgets to be productive. The first assumption is
the one that matches what AIs actually are.

---

## See Also

- `docs/NORTH_STAR.md` -- priority hierarchy and drift guardrails
- `docs/PIXEL_DRIVING_PIXELS.md` -- the pixel-as-unit thesis
- `docs/AUTHORING_PATHS.md` -- Canvas Text Surface vs. Token-to-Pixel GUI
  (how programs *enter* RAM; orthogonal to the layer stack described here)
- `src/vm/ops_graphics.rs` -- Layer 2 output and input opcode handlers
- `src/vm/disasm.rs` -- canonical opcode table with mnemonics
- `programs/counter.asm`, `programs/paint.asm`, `programs/terminal.asm` --
  reference apps that live at Layer 2 today
