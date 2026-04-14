# Self-Modifying Programs Guide

Geometry OS has a unique capability: programs can write new programs onto the canvas
grid, compile them at runtime, and execute them — all without human intervention.
This is the **pixel-driving-pixels** paradigm: the display IS the source code IS the
executable.

## Memory Map

| Region | Addresses | Size | Purpose |
|--------|-----------|------|---------|
| Canvas Grid | 0x8000 - 0x8FFF | 4096 cells | 128 rows × 32 cols of editable text |
| Canvas Bytecode | 0x1000 - 0x1FFF | 4096 cells | Where ASMSELF writes compiled code |
| Screen Buffer | 0x10000 - 0x1FFFF | 65536 cells | 256×256 pixel display |
| ASM Status | 0xFFD | 1 cell | Assembly result (word count or error) |

## The Three Opcodes

### ASMSELF (0x73) — Compile Canvas Text

Reads the canvas buffer as text, preprocesses it, assembles it, and writes
the resulting bytecode to `0x1000`.

- **Input:** Current canvas buffer contents (32 columns × 128 rows)
- **Output:** Bytecode at `0x1000`, status at `RAM[0xFFD]`
- **Status codes:**
  - `RAM[0xFFD] > 0` — Success. Value is the bytecode word count.
  - `RAM[0xFFD] == 0xFFFFFFFF` — Assembly error (invalid syntax, unknown opcode, etc.)

```asm
ASMSELF             ; compile canvas -> bytecode at 0x1000
```

### RUNNEXT (0x74) — Execute New Code

Sets PC to `0x1000` and continues execution from the newly assembled bytecode.

- **Preserves:** All registers (r0–r26), return stack, halted flag is cleared
- **Resets:** PC only

```asm
RUNNEXT             ; jump to bytecode at 0x1000
```

### STORE / LOAD to Canvas (0x8000+)

Standard STORE/LOAD opcodes, but targeting the canvas address range. Each cell
holds one ASCII character value (as u32).

```asm
LDI r1, 0x8000      ; canvas row 0, col 0
LDI r2, 72          ; 'H'
STORE r1, r2        ; write 'H' to top-left canvas cell
ADD r1, r6          ; advance to next cell
LDI r2, 69          ; 'I'
STORE r1, r2        ; write 'I' next to it
```

## Pattern 1: Canvas STORE (Writing Code to the Grid)

The foundation of self-modification. A program writes assembly source text
character by character into canvas cells using STORE.

**Key constants:**
- Canvas base: `0x8000`
- Row offset: `row × 32` (each row is 32 columns)
- Newline: ASCII `10` (0x0A)

```asm
; Write "LDI r0, 42" to canvas row 0, then "HALT" to row 1
LDI r8, 0x8000      ; canvas start
LDI r6, 1           ; increment

; Character by character
LDI r7, 76          ; 'L'
STORE r8, r7
ADD r8, r6
LDI r7, 68          ; 'D'
STORE r8, r7
ADD r8, r6
LDI r7, 73          ; 'I'
STORE r8, r7
ADD r8, r6
LDI r7, 32          ; ' '
STORE r8, r7
ADD r8, r6
LDI r7, 114         ; 'r'
STORE r8, r7
ADD r8, r6
LDI r7, 48          ; '0'
STORE r8, r7
ADD r8, r6
LDI r7, 44          ; ','
STORE r8, r7
ADD r8, r6
LDI r7, 32          ; ' '
STORE r8, r7
ADD r8, r6
LDI r7, 52          ; '4'
STORE r8, r7
ADD r8, r6
LDI r7, 50          ; '2'
STORE r8, r7
ADD r8, r6
LDI r7, 10          ; newline
STORE r8, r7
ADD r8, r6

; Second line: "HALT"
LDI r7, 72          ; 'H'
STORE r8, r7
ADD r8, r6
LDI r7, 65          ; 'A'
STORE r8, r7
ADD r8, r6
LDI r7, 76          ; 'L'
STORE r8, r7
ADD r8, r6
LDI r7, 84          ; 'T'
STORE r8, r7
ADD r8, r6
LDI r7, 10          ; newline
STORE r8, r7
```

**Tip:** Use a helper loop when writing long strings. Load the string address
into a register and loop through characters, storing each to canvas:

```asm
LDI r1, 0x8000      ; canvas destination
LDI r2, string      ; source data address
LDI r3, 1           ; increment

loop:
  LOAD r4, r2       ; load next char
  JZ r4, done       ; null terminator?
  STORE r1, r4      ; write to canvas
  ADD r1, r3        ; advance canvas pointer
  ADD r2, r3        ; advance source pointer
  JMP loop

done:
  HALT

.org 0x100
string:
  .db "LDI r0, 42", 10
  .db "HALT", 10, 0
```

**Note:** The `.db` directive stores one byte per u32 word. The assembler
converts `.db "string"` into a sequence of u32 values where each holds one
ASCII character. This is useful for string data but not for general data.

## Pattern 2: ASMSELF + RUNNEXT (Compile and Execute)

The write-compile-execute cycle. Write code to canvas, compile it, then run it.

```asm
; (Assuming code has been written to canvas at 0x8000+)
ASMSELF             ; compile canvas text -> bytecode at 0x1000

; Check for assembly errors before running
LDI r1, 0xFFD
LOAD r2, r1
LDI r3, 0xFFFFFFFF
CMP r2, r3          ; is it an error?
JEQ r2, error       ; yes, don't run bad code

RUNNEXT             ; success! jump to newly compiled code at 0x1000
HALT                ; safety fallback

error:
  ; Handle error -- maybe write a message to canvas
  HALT
```

**Always check `RAM[0xFFD]` after ASMSELF** before calling RUNNEXT. If the
canvas contained invalid assembly, running the garbage bytecode at 0x1000
could crash or loop forever.

## Pattern 3: Register Passing Between Generations

RUNNEXT preserves all registers. The new program inherits the complete register
state from its parent. This is how programs pass data to their successors.

```asm
; Generation A: compute a value, pass it to Generation B
LDI r10, 99         ; data to pass
LDI r5, 0x8000      ; canvas base

; Write Gen B source that reads r10
; "ADD r0, r10\nHALT\n"
; (character-by-character STORE omitted for brevity)
; ... write "ADD r0, r10\nHALT\n" to canvas ...

ASMSELF
RUNNEXT             ; Gen B runs. r0 = 0 + 99 = 99
HALT
```

**Register convention for generational chains:**
- `r10`–`r15`: Use for passing data between generations (safe from common opcodes)
- `r0`: Commonly used as accumulator/return value
- `r1`–`r9`: Available but may be clobbered by loops and helpers

**Verified behavior (from tests):**
- `test_self_writer_registers_inherited_across_generations`: r5=100 survives
  RUNNEXT, Gen B reads r5=100 and computes r0=101
- `test_runnext_registers_inherited_by_new_code`: All register values preserved

## Pattern 4: Self-Reading (Inspecting Your Own Source)

A program can LOAD from canvas addresses to read what's on the grid — including
its own source code. This enables programs that analyze or modify themselves.

```asm
; Read the first character of the current canvas row
LDI r1, 0x8000      ; canvas start
LOAD r2, r1         ; r2 = first character of canvas
```

This is useful for:
- **Conditional evolution:** Check if a specific value is on the grid before
  deciding what to write next
- **Checksum verification:** Verify that your successor source was written
  correctly before calling ASMSELF
- **Introspection:** A program can read what code it previously wrote

## Pattern 5: Chained Generations (Multi-Step Evolution)

Programs can write successors that themselves write successors. Each generation
can be different from the last.

```asm
; Generation A writes Generation B to canvas
; Gen B source: writes Gen C, compiles, runs
; Gen C: simple computation, then HALT

; Gen A writes this to canvas:
;   LDI r8, 0x8100        ; canvas row 32 (skip Gen A's code)
;   LDI r7, 76             ; 'L'
;   STORE r8, r7
;   ... (write Gen C source) ...
;   ASMSELF
;   RUNNEXT

; Then Gen A calls ASMSELF + RUNNEXT to become Gen B
; Gen B writes Gen C to a different canvas row, then ASMSELF + RUNNEXT
; Gen C runs and halts

; Verified: test_self_writer_two_generation_chain passes
```

**Three-generation chain (A → B → C):**
1. Gen A writes Gen B source to canvas row 0, calls ASMSELF + RUNNEXT
2. Gen B writes Gen C source to canvas row 32 (offset 0x8200), calls ASMSELF + RUNNEXT
3. Gen C executes and halts

**Important:** Each ASMSELF call overwrites bytecode at 0x1000. The new generation
must be fully written to canvas BEFORE calling ASMSELF, otherwise the previous
bytecode may be partially overwritten.

## Common Pitfalls

### 1. Forgetting to Check ASMSELF Status

```asm
; WRONG -- no error check
ASMSELF
RUNNEXT             ; could execute garbage!

; RIGHT -- check first
ASMSELF
LDI r1, 0xFFD
LOAD r2, r1
LDI r3, 0xFFFFFFFF
CMP r2, r3
JEQ r2, error
RUNNEXT
```

### 2. Infinite Self-Modification Loop

A program that writes a copy of itself and runs it will loop forever:

```asm
; DANGEROUS: This loops infinitely
; (writes "ASMSELF\nRUNNEXT\nHALT\n" to canvas, then runs it)
; The successor is identical, so it repeats forever

; FIX: Add a counter or termination condition
LDI r10, 3          ; max generations
; ... write successor ...
; Successor source starts with:
;   SUB r10, r6    ; decrement counter
;   JZ r10, done   ; stop if counter is 0
;   ... rest of code ...
; done: HALT
```

### 3. Corrupting Your Own Code

When writing to canvas, be careful not to overwrite the region where your
currently running bytecode lives. Bytecode is at `0x1000`, canvas starts at
`0x8000` — these don't overlap, so writing to canvas is safe. But if you use
`.org` to relocate code, or if the assembler places data near the canvas range,
you could corrupt yourself.

### 4. Canvas Row Boundary

Each canvas row is exactly 32 cells. Row N starts at `0x8000 + N × 32`.
If you write past column 31, you'll overwrite the start of the next row.
Use `ADD r8, r6` (increment by 1) and track column position, or use `MOD`
to wrap.

### 5. Null Bytes in Canvas

Canvas cell value `0` is treated as a newline by the ASMSELF text converter.
If you store `0` to a canvas cell, it won't produce a character — it'll break
the line. This is fine for ending lines (same as newline), but don't use `0`
as a meaningful character value.

### 6. Colons in Comments

The assembler's label parser checks for `:` before stripping comments. A comment
containing a colon (e.g., `; scratch: use r0`) will be misinterpreted as a label.
**Use dashes or parens in comments instead:** `; scratch -- use r0`

### 7. Immediate vs Register Arguments

Most opcodes that take a single register argument (like SLEEP, GETPID) will fail
if you pass an immediate value. `SLEEP 60` gives "invalid register: 60". Use
`LDI r10, 60; SLEEP r10` instead.

## Quick Reference

| Operation | Opcode | Description |
|-----------|--------|-------------|
| ASMSELF | 0x73 | Compile canvas text → bytecode at 0x1000 |
| RUNNEXT | 0x74 | Jump to 0x1000 (preserves registers) |
| STORE reg, reg | 0x12 | Write to RAM/canvas/screen |
| LOAD reg, reg | 0x11 | Read from RAM/canvas/screen |

| Address | Purpose |
|---------|---------|
| 0x1000 | Canvas bytecode region (ASMSELF output) |
| 0x8000 - 0x8FFF | Canvas grid (128 × 32 cells) |
| 0xFFD | ASM status port (word count or error) |

## See Also

- `docs/CANVAS_TEXT_SURFACE.md` — Full system specification
- `docs/PIXEL_DRIVING_PIXELS.md` — Original design document
- `programs/self_writer.asm` — Working demo of the write-compile-execute cycle
- `programs/canvas_grid_writer.asm` — Demo of writing text to the canvas grid
- `programs/canvas_counter.asm` — Demo of live canvas state updates
- `programs/game_of_life.asm` — Conway's Game of Life in pure assembly
- `programs/code_evolution.asm` — Multigenerational self-writing program
