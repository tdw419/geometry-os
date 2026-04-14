# The Pixel-Driving-Pixels Problem

## The Problem We Keep Having

We built a system where the screen is a grid of colored cells. Each cell is a
pixel. The pixel's color comes from its stored value. The value is also a
character, and the character is also an instruction. One value, three meanings.

This works beautifully in one direction:

```
keystroke -> cell value -> colored glyph on screen
                       -> text that assembles to bytecode
```

But the moment we want a running program to change what's on screen, we always
end up writing Rust code. Or Python. Or JavaScript. The host language reaches
into the pixel grid and changes it. The pixels themselves never get to drive
other pixels.

We keep arriving at the same gap:

```
what we want:  pixel on screen -> changes pixel on screen
what we get:   pixel on screen -> Rust code changes pixel on screen
```

The pixel is the visible thing. The pixel is the data. The pixel is the
program. But the pixel can't act on another pixel without leaving the pixel
world and asking the host language to do it.

---

## Why This Keeps Happening

Geometry OS has two separate realities that never touch each other:

### Reality 1: The Canvas (visible, editable, human-facing)

The canvas is a 32x128 grid of cells. Each cell holds a u32. The value
determines:

- The glyph shape (via font.rs -- 8x8 bitmap)
- The glyph color (via syntax_highlight -- token-aware coloring)
- The text meaning (ASCII character for assembly source)

You type on it. You see it. F8 reads it as text, assembles it, puts bytecode
at 0x1000. F5 runs it. The grid stays visible showing your source while the
VM runs invisible bytecode underneath.

The canvas lives in a Vec<u32> called canvas_buffer. It is NOT in VM RAM.

### Reality 2: The VM (invisible, running, machine-facing)

The VM executes bytecode from RAM. It has registers, a stack, memory-mapped I/O.
It can DRAW pixels to the screen buffer (PIXEL opcode). It can READ pixels back
(PEEK opcode). It can do math, branch, loop, spawn child processes.

The VM lives in Rust structs. Its RAM is a Vec<u32> separate from the canvas.

### The Gap

These two realities never connect:

- The canvas can't run. It's dead text until F8 compiles it and F5 starts a
  separate VM process.
- The VM can't write to the canvas. It can DRAW to the screen buffer, but it
  can't modify the grid cells that hold the source text.
- A running program can't change its own source. It can't write new assembly
  onto the grid. It can't modify the program it's running.

The consequence: every time we want dynamic behavior on the grid, we write it
in the host language (Rust) rather than in the pixel language (Geometry OS
assembly).

---

## The Pattern In Practice

This has happened repeatedly across the project:

### Drawing opcodes (PIXEL, RECT, LINE)

We wanted pixels to appear on screen. We wrote Rust code (the VM runtime) that
interprets PIXEL/RECT/LINE opcodes and draws to the framebuffer. The pixel on
screen is the *output* of Rust code, not the *action* of another pixel.

### Syntax highlighting

We wanted the grid to show colored text. We wrote Rust code (syntax_highlight)
that reads the canvas buffer, tokenizes it, and assigns colors. The color is
determined by Rust, not by the cell value itself.

### F8 assembly

We wanted text on the grid to become a running program. We wrote Rust code
(assembler.rs) that reads the canvas, parses it, emits bytecode. The program
is compiled by Rust, not by the grid.

### The preprocessor (VAR, SET, GET)

We wanted higher-level abstractions. We wrote Rust code (preprocessor.rs) that
expands macros before assembly. The abstraction is implemented in Rust, not in
the pixel language.

In every case, the *result* appears on the pixel grid, but the *mechanism* is
Rust code interpreting or manipulating the grid from outside.

---

## What "Pixels Driving Pixels" Actually Means

A system where pixels drive pixels would have this property:

```
A cell's value changes -> adjacent cell's value changes -> visible effect
```

Without any host-language code in the loop. The grid is not just a display --
it's the compute substrate. The arrangement of values IS the program. Changing
a value IS executing an instruction.

Examples of what this looks like:

### Cellular automata

In Conway's Game of Life, each cell looks at its neighbors and decides whether
to live or die. The rule is simple. No compiler. No assembler. No host
language. Each cell reads nearby cells and writes to itself. The pattern IS
the computation.

### Spreadsheet cells

Cell A1 = 5, Cell B1 = A1 * 2. Change A1 and B1 updates. The cell formula
reacts to other cells. No separate program needed.

### Self-modifying code

A program writes new instructions into its own code space and then executes
them. The output of one instruction IS the next instruction.

### The Smalltalk model

Everything is an object. The screen is an object. Objects send messages to
other objects. The debugger is an object that can modify running code. The
code browser IS the running system. Nothing is "compiled" in a separate step.

---

## What Geometry OS Already Has

Not everything is missing. Several pieces are already in place:

### PEEK (opcode 0x4F)

`PEEK rx, ry, rd` reads a pixel from the screen buffer into a register.
A running program CAN read the screen. This is half of the loop -- pixels
can be read.

### PIXEL (opcode 0x0D)

`PIXEL rx, ry, rc` writes a colored pixel to the screen. A running program
CAN write to the screen. This is the other half -- pixels can be written.

### STORE / LOAD

The VM can read and write RAM. If the canvas buffer were mapped into RAM,
programs could read and write the grid directly.

### SPAWN / multi-process

Programs can spawn child processes. The grid could be partitioned so each
process owns a region and they communicate through shared RAM.

### The canvas IS text

The grid already holds readable assembly. If a program could write to the
grid, it could write new assembly. F8 would compile it. F5 would run it.
A program writing its own successor.

---

## The Missing Piece

The canvas buffer is not addressable from the VM. The VM has RAM (0x0000 to
whatever). The canvas has canvas_buffer (Vec<u32>, separate). They don't
connect.

Right now:

```
VM RAM:    [bytecode | data | stack | I/O ports]
Canvas:    [source text, separate Vec<u32>]
Screen:    [framebuffer, separate Vec<u32>]
```

What's needed:

```
VM RAM:    [bytecode | data | stack | I/O ports | CANVAS | SCREEN]
```

If the canvas buffer were mapped into the VM's address space (say 0x8000),
then any STORE to 0x8000+row*32+col would change the grid. Any LOAD from
that range would read the grid. The program could:

1. Read its own source from the grid
2. Modify the source
3. Write new source elsewhere on the grid
4. You press F8 to compile the new source
5. You press F5 to run it

Or, going further -- add a self-assembly opcode so the VM can trigger its
own F8/F5 cycle without human intervention. Then a program can write a new
program, compile it, and run it, entirely from within the pixel world.

---

## Why This Matters

The whole point of Geometry OS is that the pixel grid is the primary interface.
You type on it. You read it. You see the program AND the output on the same
surface. The canvas text surface was built to make this true for input.

But for output -- for a running program to express itself -- we keep falling
back to the host language. We write Rust to interpret opcodes that draw pixels.
We write Rust to parse text and compile it. We write Rust to implement
macros.

Each of these is correct engineering. But each one moves the intelligence
out of the pixel grid and into the host. The grid becomes a dumb display for
a smart runtime. The pixels don't drive anything -- they're driven.

The fix isn't to remove the Rust runtime. The VM needs a runtime. The fix is
to make the canvas addressable from inside the VM so that the running program
can reach back and modify its own source. The runtime stays. But the locus of
control shifts from "Rust code that manipulates pixels" to "pixel values that
manipulate other pixel values."

That's the difference between a display and a living system.

---

## Concrete Steps

1. **Map the canvas buffer into VM RAM** at a fixed address range. STORE/LOAD
   to that range reads/writes the grid cells directly. No new opcodes needed.

2. **Map the screen buffer into VM RAM** at another range. This already
   partially exists (PEEK reads, PIXEL writes). Make it addressable via normal
   LOAD/STORE so the same mechanism works for both canvas and screen.

3. **Add a self-assembly opcode** (ASSEMBLE or RECOMPILE) that takes the
   current canvas text, runs it through the preprocessor and assembler, and
   loads the resulting bytecode. Now a program can write a new program onto
   the grid and compile it without human intervention.

4. **Add a self-run opcode** (RUN_SELF) that starts executing the newly
   assembled bytecode. Combined with step 3, a program can write, compile,
   and execute its own replacement.

5. **Build the first self-modifying demo**: a program that counts something,
   writes the count onto the grid as visible digits, and those digits are
   readable by a human watching the screen. The program's state IS the
   display. No separate output step.

After step 1, the canvas becomes programmable from within. After step 4, the
system can evolve its own code. That's when pixels start driving pixels.

---

## See Also

- **CANVAS_TEXT_SURFACE.md** -- How the canvas text surface works today
- **KEYSTROKE_TO_PIXELS.md** -- The foundational keystroke-to-RAM pipeline
- **PIXELC_GUIDE.md** -- The Python-to-bytecode compiler (current abstraction
  layer, implemented in Python, an example of the host-language problem)
