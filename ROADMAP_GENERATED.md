# Geometry OS Roadmap

Pixel-art virtual machine with built-in assembler, debugger, and live GUI.
40 opcodes, 32 registers, 64K RAM, 256x256 framebuffer. Write assembly in
the built-in text editor, press F5, watch it run.


**Progress:** 12/12 phases complete, 0 in progress

**Deliverables:** 66/66 complete

## Scope Summary

| Phase | Status | Deliverables | LOC Target | Tests |
|-------|--------|-------------|-----------|-------|
| phase-1 Core VM + Visual Programs | COMPLETE | 23/23 | 2,000 | 10 |
| phase-2 Extended Opcodes | COMPLETE | 10/10 | 2,800 | 16 |
| phase-3 Interactive Programs | COMPLETE | 4/4 | 3,200 | 20 |
| phase-4 Canvas & Editor | COMPLETE | 4/4 | 3,500 | 22 |
| phase-5 Terminal Mode | COMPLETE | 2/2 | 3,800 | 24 |
| phase-6 Animation | COMPLETE | 3/3 | 4,000 | 24 |
| phase-7 Random & Games | COMPLETE | 3/3 | 4,300 | 24 |
| phase-8 TICKS & Sound | COMPLETE | 5/5 | 4,322 | 46 |
| phase-9 Debug Tools | COMPLETE | 5/5 | 4,500 | 48 |
| phase-10 Extended Graphics | COMPLETE | 2/2 | 4,700 | 50 |
| phase-11 Advanced Games | COMPLETE | 3/3 | 5,100 | 56 |
| phase-12 Self-Hosting | COMPLETE | 2/2 | 5,500 | 54 |

## [x] phase-1: Core VM + Visual Programs (COMPLETE)

**Goal:** Working VM with pixel opcodes, static visual output

Foundational VM + programs that produce static images

### Deliverables

- [x] **HALT/NOP/FRAME opcodes** -- Control flow: stop, no-op, yield to renderer
- [x] **LDI/LOAD/STORE opcodes** -- Data movement: load immediate, load/store from RAM
- [x] **ADD/SUB/MUL/DIV opcodes** -- Arithmetic on registers
- [x] **AND/OR/XOR opcodes** -- Bitwise logic on registers
- [x] **JMP/JZ/JNZ opcodes** -- Unconditional and conditional branching
- [x] **CALL/RET opcodes** -- Subroutine call/return via r31
- [x] **PSET/PSETI opcodes** -- Pixel set from registers or immediates
- [x] **FILL opcode** -- Fill entire screen with a color
- [x] **RECTF opcode** -- Filled rectangle
- [x] **Two-pass assembler** -- Labels, comments, hex/dec/bin literals, .db directive
- [x] **hello.asm** -- Hello world text display
- [x] **fill_screen.asm** -- Fill screen with solid color
- [x] **diagonal.asm** -- Diagonal line from (0,0) to (255,255)
- [x] **border.asm** -- Colored border around screen edges
- [x] **gradient.asm** -- Horizontal color gradient via nested loops
- [x] **stripes.asm** -- Alternating horizontal stripes
- [x] **nested_rects.asm** -- Concentric colored rectangles
- [x] **checkerboard.asm** -- Checkerboard pattern
- [x] **colors.asm** -- Color palette display
- [x] **rainbow.asm** -- Rainbow stripes
- [x] **rings.asm** -- Concentric rings
- [x] **lines.asm** -- Star burst using LINE opcode
- [x] **circles.asm** -- Concentric circles using CIRCLE opcode

## [x] phase-2: Extended Opcodes (COMPLETE)

**Goal:** Shift, modulo, compare, stack, and signed negation opcodes

Instruction set extensions

### Deliverables

- [x] **SHL/SHR opcodes** -- Bit-shift left and right
  - [x] shift_test.asm passes
- [x] **MOD opcode** -- Modulo operation
- [x] **NEG opcode** -- Two's complement negation
- [x] **CMP opcode** -- Compare: r0 = -1/0/1 (lt/eq/gt)
- [x] **BLT/BGE opcodes** -- Branch on CMP result (less-than, greater-or-equal)
- [x] **PUSH/POP opcodes** -- Stack operations via r30 (SP)
  - [x] push_pop_test.asm passes
- [x] **TEXT opcode** -- Render null-terminated string from RAM at (x,y)
- [x] **LINE opcode** -- Bresenham line between two points
- [x] **CIRCLE opcode** -- Midpoint circle algorithm
- [x] **SCROLL opcode** -- Scroll screen up by N pixels

## [x] phase-3: Interactive Programs (COMPLETE)

**Goal:** Keyboard input via IKEY and interactive programs

### Deliverables

- [x] **IKEY opcode** -- Read keyboard port RAM[0xFFF], clear it
- [x] **blink.asm** -- Toggle a pixel on/off with keyboard
- [x] **calculator.asm** -- 4-function calculator with text display
- [x] **painter.asm** -- Freehand drawing with cursor keys

## [x] phase-4: Canvas & Editor (COMPLETE)

**Goal:** Text editor improvements: paste, load, scroll, syntax highlighting

### Deliverables

- [x] **Clipboard paste** -- Ctrl+V to paste text onto the grid
- [x] **File load** -- Ctrl+F8 to load .asm files with Tab completion
- [x] **Scroll/pan** -- Support programs larger than 32x32 characters
- [x] **Syntax highlighting** -- Color opcodes, registers, numbers differently on canvas

## [x] phase-5: Terminal Mode (COMPLETE)

**Goal:** CLI mode with geo> prompt

### Deliverables

- [x] **Terminal mode** -- geo> prompt with help, list, load, run, edit, regs, peek, poke, reset, clear, quit
- [x] **Mode switching** -- Escape toggles Editor/Terminal

## [x] phase-6: Animation (COMPLETE)

**Goal:** FRAME opcode for 60fps animation loop

### Deliverables

- [x] **FRAME opcode** -- Yield to renderer, enable animation loops
- [x] **fire.asm** -- Scrolling fire animation using FRAME + SCROLL
- [x] **scroll_demo.asm** -- Horizontal bar scrolling upward

## [x] phase-7: Random & Games (COMPLETE)

**Goal:** RAND opcode, Snake and Ball games

### Deliverables

- [x] **RAND opcode** -- LCG pseudo-random u32 (seed 0xDEADBEEF)
- [x] **snake.asm** -- Snake: WASD, random apples, growing tail, self-collision
- [x] **ball.asm** -- Bouncing ball with WASD control

## [x] phase-8: TICKS & Sound (COMPLETE)

**Goal:** Frame counter for throttling, BEEP opcode for audio

### Deliverables

- [x] **TICKS register** -- RAM[0xFFE] frame counter, incremented each FRAME, wraps at u32 max
- [x] **Snake throttle** -- Snake throttled via TICKS & 7 (~7.5 moves/sec)
- [x] **BEEP opcode** -- BEEP freq_reg, dur_reg -- sine-wave via aplay (20-20000 Hz, 1-5000 ms)
- [x] **Snake sounds** -- 880Hz ping on apple eat, 110Hz thud on death
- [x] **Ball sounds** -- 330Hz click on wall bounce

## [x] phase-9: Debug Tools (COMPLETE)

**Goal:** Breakpoints, instruction trace, and save/load improvements

### Deliverables

- [x] **Save/load state** -- F7 saves full RAM to geometry_os.sav, restore on startup
  - [x] test_vm_save_load_roundtrip passes
- [x] **Disassembly panel** -- Show bytecode alongside source text in GUI
- [x] **Single-step mode** -- F6 steps one instruction when paused
- [x] **Breakpoints** -- Mark PC addresses to pause at during execution
  - [x] User can set breakpoint at an address
  - [x] VM halts when PC hits breakpoint address
  _~80 LOC_
- [x] **Instruction trace** -- Log PC + decoded instruction for first N steps
  - [x] CLI mode logs each instruction with register state
  _~60 LOC_

## [x] phase-10: Extended Graphics (COMPLETE)

**Goal:** Sprite blitting and screenshot export

### Deliverables

- [x] **SPRITE opcode** -- Copy a block of RAM to screen at (x,y) -- sprite blit
  - [x] Copy NxM pixels from RAM to screen buffer
  - [x] Color 0 pixels are transparent (skip)
  - [x] test_sprite_transparent_skips_zero passes
  _~120 LOC_
- [x] **Screenshot export** -- Dump 256x256 canvas to PNG file
  - [x] F9 or CLI command saves PNG
  _~40 LOC_

## [x] phase-11: Advanced Games (COMPLETE)

**Goal:** Richer games using sprites and extended features

### Deliverables

- [x] **breakout.asm** -- Breakout with paddle, ball, 4 rows of colored bricks, score, lives
- [x] **tetris.asm** -- Tetris with piece rotation and line clearing
- [x] **maze.asm** -- Randomly generated maze with player navigation
  - [x] test_maze_assembles passes
  - [x] test_maze_initializes passes

## [x] phase-12: Self-Hosting (COMPLETE)

**Goal:** VM can assemble and run its own programs from within

The text editor types assembly, the assembler compiles it, and
the VM runs it. Missing piece: assembler callable as VM subroutine.


### Deliverables

- [x] **Assembler syscall** -- VM opcode that triggers the assembler on canvas text
  - [x] ASM opcode (0x4B) reads null-terminated source from RAM
  - [x] ASM writes bytecode to RAM at destination address
  - [x] RAM[0xFFD] holds result (word count or 0xFFFFFFFF on error)
  - [x] test_asm_opcode_basic passes
  - [x] test_asm_opcode_error passes
  _~150 LOC_
- [x] **Self-hosting demo** -- Program that writes assembly, assembles it, runs the output
  - [x] self_host.asm compiles and runs
  - [x] Generated code executes (screen filled with green)
  _~80 LOC_

## Global Risks

- Opcode space filling up (40 of ~256 slots used, but gaps in hex space)
- Scope creep -- adding opcodes is easy, keeping the VM simple is hard
- BEEP spawning aplay processes without rate limiting could exhaust FDs

## Conventions

- Every new opcode gets a test in tests/program_tests.rs
- Every new program gets assembled by test_all_programs_assemble
- README.md updated when opcodes or features change
- roadmap.yaml is the single source of truth for project state
- Semantic versioning: minor bump for new opcodes, patch for fixes
