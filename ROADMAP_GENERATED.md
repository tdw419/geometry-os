# Geometry OS Roadmap

Pixel-art virtual machine with built-in assembler, debugger, and live GUI.
33 opcodes, 32 registers, 64K RAM, 256x256 framebuffer. Write assembly in
the built-in text editor, press F5, watch it run.


**Progress:** 9/13 phases complete, 1 in progress

**Deliverables:** 39/48 complete

## Scope Summary

| Phase | Status | Deliverables | LOC Target | Tests |
|-------|--------|-------------|-----------|-------|
| phase-1 Visual Programs | COMPLETE | 12/12 | 500 | 12 |
| phase-2 Interactive Programs | COMPLETE | 2/2 | 1,500 | 14 |
| phase-3 VM Extensions | COMPLETE | 4/4 | 2,500 | 16 |
| phase-4 Canvas & Editor | COMPLETE | 4/4 | 3,000 | 18 |
| phase-5 Terminal Mode | COMPLETE | 2/2 | 3,500 | 20 |
| phase-6 Animation & Games | COMPLETE | 5/5 | 4,200 | 22 |
| phase-7 Random & Snake | COMPLETE | 2/2 | 4,600 | 24 |
| phase-8 TICKS & Throttling | COMPLETE | 1/1 | 4,800 | 24 |
| phase-9 Sound | COMPLETE | 4/4 | 5,100 | 24 |
| phase-10 Debug Tools | IN PROGRESS | 3/5 | 5,500 | 26 |
| phase-11 Extended Graphics | PLANNED | 0/2 | 5,800 | 28 |
| phase-12 Advanced Games | PLANNED | 0/3 | 6,200 | 30 |
| phase-13 Self-Hosting | FUTURE | 0/2 | 6,800 | 32 |

## [x] phase-1: Visual Programs (COMPLETE)

**Goal:** Draw static images on the 256x256 screen using pixel opcodes

Sprint A: programs that produce static visual output

### Deliverables

- [x] **fill_screen.asm** -- Fill screen with a solid color
  - [x] Fills entire 256x256 canvas
    _Validation: cargo test test_fill_screen_
- [x] **diagonal.asm** -- Diagonal line from (0,0) to (255,255)
  - [x] Draws pixel-by-pixel diagonal
- [x] **border.asm** -- Colored border around screen edges
  - [x] Uses RECTF for 4 edges
- [x] **gradient.asm** -- Horizontal color gradient via nested loops
  - [x] Smooth gradient across full width
- [x] **stripes.asm** -- Alternating horizontal stripes
  - [x] Red and blue stripes
- [x] **nested_rects.asm** -- Concentric colored rectangles
  - [x] Multiple RECTF calls with different colors
- [x] **checkerboard.asm** -- Checkerboard pattern
- [x] **colors.asm** -- Color palette display
- [x] **lines.asm** -- Star burst using LINE opcode
- [x] **circles.asm** -- Concentric circles with cycling colors
- [x] **rings.asm** -- Concentric rings
- [x] **rainbow.asm** -- Rainbow stripes

## [x] phase-2: Interactive Programs (COMPLETE)

**Goal:** Programs that respond to keyboard input via IKEY

Sprint B: keyboard-driven programs

### Deliverables

- [x] **blink.asm** -- Toggle a pixel on/off with keyboard
- [x] **calculator.asm** -- 4-function calculator with text display

## [x] phase-3: VM Extensions (COMPLETE)

**Goal:** Shift ops, stack ops, branch comparisons, modulo

Sprint C: extended instruction set

### Deliverables

- [x] **SHL/SHR opcodes** -- Bit-shift left and right
  - [x] shift_test.asm passes
- [x] **PUSH/POP opcodes** -- Stack operations via r30 (SP)
  - [x] push_pop_test.asm passes
- [x] **BLT/BGE opcodes** -- Branch on less-than / greater-or-equal after CMP
- [x] **MOD opcode** -- Modulo operation

## [x] phase-4: Canvas & Editor (COMPLETE)

**Goal:** Clipboard paste, file loading, scroll/pan, syntax highlighting

Sprint D: text editor improvements

### Deliverables

- [x] **Clipboard paste** -- Ctrl+V to paste text onto the grid
- [x] **File load** -- Ctrl+F8 to load .asm files with Tab completion
- [x] **Scroll/pan** -- Support programs larger than 32x32 characters
- [x] **Syntax highlighting** -- Color opcodes, registers, numbers differently on canvas

## [x] phase-5: Terminal Mode (COMPLETE)

**Goal:** CLI mode with geo> prompt and 11 commands

Sprint E: headless operation mode

### Deliverables

- [x] **Terminal mode** -- geo> prompt with help, list, load, run, edit, regs, peek, poke, reset, clear, quit
- [x] **Mode switching** -- Escape toggles Editor/Terminal

## [x] phase-6: Animation & Games (COMPLETE)

**Goal:** FRAME opcode for 60fps animation loop, plus interactive games

v0.3.0: animation, keyboard input, real games

### Deliverables

- [x] **FRAME opcode** -- Yield to renderer, enable animation loops at 60fps
- [x] **IKEY opcode** -- Read keyboard port RAM[0xFFF] and clear it
- [x] **fire.asm** -- Scrolling fire animation using FRAME + SCROLL
- [x] **ball.asm** -- Bouncing ball with WASD control
- [x] **scroll_demo.asm** -- Horizontal bar scrolling upward

## [x] phase-7: Random & Snake (COMPLETE)

**Goal:** RAND opcode for procedural content, full Snake game

v0.3.0 continuation: randomness and the flagship game

### Deliverables

- [x] **RAND opcode** -- LCG pseudo-random u32 (seed 0xDEADBEEF)
  - [x] test_rand_opcode passes
- [x] **snake.asm** -- Snake game with WASD, random apples, growing tail, self-collision

## [x] phase-8: TICKS & Throttling (COMPLETE)

**Goal:** Frame counter register for game speed control

v0.3.1: TICKS port at 0xFFE

### Deliverables

- [x] **TICKS register** -- RAM[0xFFE] frame counter, incremented each FRAME opcode, wraps at u32 max
  - [x] frame_count field in Vm struct
  - [x] Snake throttled via TICKS & 7 (~7.5 moves/sec)

## [x] phase-9: Sound (COMPLETE)

**Goal:** BEEP opcode for audio feedback in games

v0.4.0: sine-wave tones via aplay, zero new deps

### Deliverables

- [x] **BEEP opcode** -- BEEP freq_reg, dur_reg -- sine-wave via aplay (20-20000 Hz, 1-5000 ms)
  - [x] Generates 16-bit mono WAV, pipes to aplay
- [x] **Snake sounds** -- 880Hz ping on apple eat, 110Hz thud on death
- [x] **Ball sounds** -- 330Hz click on wall bounce
- [x] **painter.asm** -- Freehand drawing program with cursor keys

## [~] phase-10: Debug Tools (IN PROGRESS)

**Goal:** Breakpoints, instruction trace, and save/load improvements

### Deliverables

- [x] **Save/load state** -- F7 saves full RAM to geometry_os.sav, restore on startup
  - [x] test_vm_save_load_roundtrip passes
- [x] **Disassembly panel** -- Show bytecode alongside source text in GUI
- [x] **Single-step mode** -- F6 steps one instruction when paused
- [ ] **Breakpoints** -- Mark PC addresses to pause at during execution
  - [ ] User can set breakpoint at an address
  - [ ] VM halts when PC hits breakpoint address
  _~80 LOC_
- [ ] **Instruction trace** -- Log PC + decoded instruction for first N steps
  - [ ] CLI mode logs each instruction with register state
  _~60 LOC_

## [ ] phase-11: Extended Graphics (PLANNED)

**Goal:** Sprite blitting and screenshot export

### Deliverables

- [ ] **SPRITE opcode** -- Copy a block of RAM to screen at (x,y) -- sprite blit
  - [ ] Copy NxM pixels from RAM to screen buffer
  _~120 LOC_
- [ ] **Screenshot export** -- Dump 256x256 canvas to PNG file
  - [ ] F9 or CLI command saves PNG
  _~40 LOC_

## [ ] phase-12: Advanced Games (PLANNED)

**Goal:**  richer games using sprites and extended features

### Deliverables

- [ ] **breakout.asm** -- Breakout game with paddle, ball, and brick rows
- [ ] **tetris.asm** -- Tetris with piece rotation and line clearing
- [ ] **maze.asm** -- Randomly generated maze with player navigation

## [?] phase-13: Self-Hosting (FUTURE)

**Goal:** VM can assemble and run its own programs from within the VM

The ultimate goal: the text editor types assembly, the assembler built
into the VM compiles it, and the VM runs it. Already partially there
(F8 assembles canvas text). The missing piece is making the assembler
callable as a subroutine from within VM bytecode.


### Deliverables

- [ ] **Assembler syscall** -- VM opcode that triggers the assembler on canvas text, loading bytecode at 0x1000
  _~150 LOC_
- [ ] **Self-hosting demo** -- Program that writes assembly to canvas, assembles it, then runs the output

## Global Risks

- Opcode space collision (0x00-0x61 used, gaps filling up)
- Scope creep -- adding opcodes is easy, keeping the VM simple is hard
- Performance -- WAV generation per BEEP could stack up if misused

## Conventions

- Every new opcode gets a test in tests/program_tests.rs
- Every new program gets assembled by test_all_programs_assemble
- README.md updated when opcodes or features change
- Semantic versioning: minor bump for new opcodes, patch for fixes
