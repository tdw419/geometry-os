# Geometry OS Roadmap

Pixel-art virtual machine with built-in assembler, debugger, and live GUI.
46 opcodes, 32 registers, 64K RAM, 256x256 framebuffer. Write assembly in
the built-in text editor, press F5, watch it run.


**Progress:** 23/32 phases complete, 0 in progress

**Deliverables:** 102/134 complete

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
| phase-13 Close the Gaps | COMPLETE | 3/3 | - | - |
| phase-14 Developer Experience | COMPLETE | 4/4 | - | - |
| phase-15 VM Capability Gaps | COMPLETE | 4/4 | - | - |
| phase-16 Showcase Shipping | COMPLETE | 4/4 | - | - |
| phase-17 Platform Growth | COMPLETE | 3/3 | - | - |
| phase-18 VM Instrumentation | COMPLETE | 2/2 | - | - |
| phase-19 Visual Debugger | COMPLETE | 3/3 | - | - |
| phase-20 High RAM Visualization | COMPLETE | 2/2 | - | - |
| phase-21 Spatial Program Coordinator (Native Windowing) | COMPLETE | 3/3 | - | - |
| phase-22 Screen Readback & Collision Detection | COMPLETE | 3/3 | - | - |
| phase-23 Kernel Boundary (Syscall Mode) | COMPLETE | 5/5 | - | - |
| phase-24 Memory Protection | PLANNED | 0/3 | - | - |
| phase-25 Filesystem | PLANNED | 0/5 | - | - |
| phase-26 Preemptive Scheduler | PLANNED | 0/3 | - | - |
| phase-27 Inter-Process Communication | PLANNED | 0/3 | - | - |
| phase-28 Device Driver Abstraction | PLANNED | 0/3 | - | - |
| phase-29 Shell | PLANNED | 0/4 | - | - |
| phase-30 Boot Sequence & Init | PLANNED | 0/3 | - | - |
| phase-31 Standard Library | PLANNED | 0/4 | - | - |
| phase-32 Signals & Process Lifecycle | PLANNED | 0/4 | - | - |

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

## [x] phase-13: Close the Gaps (COMPLETE)

**Goal:** Every program tested, every error traceable, no regressions

### Deliverables

- [x] **Tests for untested programs** -- ball, fire, hello, circles, lines, scroll_demo, rainbow, rings, colors, checkerboard, painter -- assemble + first-frame sanity
  - [ ] cargo test all green
  - [ ] Each untested program has at least one test
  _~220 LOC_
- [x] **Assembler error line numbers** -- Error messages include source line number
  - [ ] Error message format: 'line N: unknown opcode: XYZ'
  _~30 LOC_
- [x] **Version string audit** -- Single source of truth for version across banner, CLI, Cargo.toml
  _~10 LOC_

## [x] phase-14: Developer Experience (COMPLETE)

**Goal:** Make the VM pleasant to program

### Deliverables

- [x] **Assembler #define constants** -- #define NAME value -- eliminates magic numbers
  - [x] #define TILE 8 resolves in LDI and other immediate contexts
  _~80 LOC_
- [x] **programs/README.md** -- One-line description + controls + opcodes per program
  _~60 LOC_
- [x] **Disassembler panel in GUI** -- Shows PC +/- 10 instructions, updates each step
  _~50 LOC_
- [x] **GIF/video capture** -- F10 toggle writes numbered PNGs, ffmpeg command documented
  _~20 LOC_

## [x] phase-15: VM Capability Gaps (COMPLETE)

**Goal:** Fix rough edges in game programming

### Deliverables

- [x] **SAR opcode (arithmetic shift right, 0x2B)** -- Two's-complement division for negative numbers
  - [x] Two's-complement division works for negative numbers
  _~10 LOC_
- [x] **Multi-key input (bitmask at 0xFFB)** -- Two simultaneous keys in same frame
  - [x] Two simultaneous keys register in same frame
  _~20 LOC_
- [x] **BEEP in more programs** -- Sound effects for tetris, breakout, maze, sprite_demo
  - [x] Sound effects on game events
  _~20 LOC_
- [x] **Signed arithmetic audit** -- Document SUB/ADD/MUL sign contract, CMP semantics
  _~30 LOC_

## [x] phase-16: Showcase Shipping (COMPLETE)

**Goal:** Complete game, presentable repo, public release

### Deliverables

- [x] **Complete tetris** -- Scoring, levels, sound, game-over screen
  - [x] Playable start-to-finish with visible score
  _~100 LOC_
- [x] **TILEMAP opcode** -- Grid blit from tile index array -- makes grid games 3x shorter
  - [x] snake, tetris, maze each 3x shorter
  _~60 LOC_
- [x] **Persistent save slots** -- 4 named save slots accessible from terminal
  - [x] save/load slot1 works
  _~30 LOC_
- [x] **GitHub release v1.0.1** -- Tag, release notes, prebuilt binary
  - [x] Release notes prepared

## [x] phase-17: Platform Growth (COMPLETE)

**Goal:** Geometry OS as a target platform

### Deliverables

- [x] **GlyphLang compiler backend** -- Emit .geo bytecode from GlyphLang source
  _~200 LOC_
- [x] **Browser port via WASM** -- VM runs in browser with canvas rendering
  - [x] wasm-pack build succeeds
  - [x] Demo page with built-in programs runs in browser
  - [x] Full opcode set works in WASM
  _~200 LOC_
- [x] **Network port (0xFFC UDP)** -- Two VM instances exchange messages
  _~40 LOC_

## [x] phase-18: VM Instrumentation (COMPLETE)

**Goal:** Telemetry for memory access and execution flow

### Deliverables

- [x] **Access log buffer** -- Track LOAD/STORE/SPRITE/TILEMAP RAM hits per frame
  _~50 LOC_
- [x] **Instruction fetch logging** -- Track PC addresses for execution trail
  _~10 LOC_

## [x] phase-19: Visual Debugger (COMPLETE)

**Goal:** Live heat-map and PC trail overlays on canvas

### Deliverables

- [x] **Intensity decay buffer** -- Fade memory highlights over ~10 frames
  _~30 LOC_
- [x] **Canvas cell tinting** -- Cyan (Read) and Magenta (Write) flashes on active RAM addresses
  _~40 LOC_
- [x] **PC trail visualization** -- Fading white glow follows the execution pointer
  _~20 LOC_

## [x] phase-20: High RAM Visualization (COMPLETE)

**Goal:** Deep observability into game state and sprite memory

### Deliverables

- [x] **RAM inspector panel** -- Second 32x32 grid visualizing 0x2000-0x23FF or scrollable range
  - [x] 32x32 grid renders at bottom of window
  - [x] PageUp/PageDown scrolls through RAM in Terminal mode
  - [x] Access intensities shown as color tints
  _~60 LOC_
- [x] **Global heatmap** -- Compact 256x256 view of entire 64K RAM access patterns
  - [x] 256x256 pixel grid shows all 64K words
  - [x] Read/Write access shown as cyan/magenta pulses
  - [x] PC position highlighted in white
  _~80 LOC_

## [x] phase-21: Spatial Program Coordinator (Native Windowing) (COMPLETE)

**Goal:** Eliminate CPU-side compositor dependency by running multiple autonomous Glyph programs concurrently within the GPU/WGPU substrate.

### Deliverables

- [x] **Multi-Process VM Scheduler** -- Modify the core VM to support multiple concurrent execution contexts (window instances) within the same 64K RAM.
  - [x] SpawnedProcess struct with isolated registers and PC
  - [x] step_all_processes() with swap-in/step/swap-out pattern
  - [x] MAX_PROCESSES = 8 cap
- [x] **SPAWN/KILL Opcodes (0x4D/0x4E)** -- SPAWN addr_reg creates child process; KILL pid_reg halts it. PID stored in RAM[0xFFA].
  - [x] test_spawn_creates_child_process passes
  - [x] test_spawn_max_processes passes
  - [x] test_kill_halts_child_process passes
- [x] **Window Manager Demo** -- window_manager.asm -- primary draws animated window border, child bounces ball inside via shared RAM bounds protocol.
  - [x] window_manager.asm assembles and runs
  - [x] test_window_manager_spawns_child passes
  - [x] Ball stays within window bounds

## [x] phase-22: Screen Readback & Collision Detection (COMPLETE)

**Goal:** Let programs read pixel values from the framebuffer for collision detection, pick-color, and window compositing.

### Deliverables

- [x] **PEEK opcode (0x4F)** -- PEEK rx, ry, rd -- read screen pixel at (rx,ry) into rd. Enables collision detection.
  - [x] PEEK reads screen buffer value into destination register
  - [x] Out-of-bounds returns 0
  - [x] test_peek_reads_screen_pixel passes
  _~20 LOC_
- [x] **Collision detection demo** -- peek_bounce.asm -- white ball bounces off drawn obstacles using PEEK. No RAM collision map.
  _~100 LOC_
- [x] **MOV instruction everywhere** -- MOV rd, rs documented and used across programs

## [x] phase-23: Kernel Boundary (Syscall Mode) (COMPLETE)

**Goal:** Establish user mode vs kernel mode. Programs can't directly access hardware.

### Deliverables

- [x] **CPU mode flag** -- vm.mode: User/Kernel bit in VM state
- [x] **SYSCALL opcode (0x52)** -- Trap into kernel mode, dispatch by syscall number
- [x] **RETK opcode (0x53)** -- Return from kernel mode to user mode
- [x] **Syscall dispatch table** -- RAM region 0xFE00..0xFEFF mapping syscall numbers to kernel handlers
- [x] **Restricted opcodes in user mode** -- IKEY, hardware STORE blocked in user mode

## [ ] phase-24: Memory Protection (PLANNED)

**Goal:** Each process gets its own address space. Processes can't trash each other.

### Deliverables

- [ ] **Page tables** -- Simple 1-level paging: page_dir per process, maps virtual to physical
- [ ] **Address space per process** -- SPAWN creates new page table, not just new registers
- [ ] **SEGFAULT on illegal access** -- LOAD/STORE to unmapped page halts the process

## [ ] phase-25: Filesystem (PLANNED)

**Goal:** Programs can create, read, write, and delete named files. Persistent storage.

### Deliverables

- [ ] **Virtual filesystem layer** -- Abstract FS interface backed by host filesystem in .geometry_os/fs/
- [ ] **OPEN/READ/WRITE/CLOSE/SEEK syscalls** -- Full file I/O through syscall interface
- [ ] **LS syscall** -- Directory listing into RAM buffer
- [ ] **Per-process fd table** -- Max 16 open files per process
- [ ] **cat.asm** -- Test program that reads a file and displays it

## [ ] phase-26: Preemptive Scheduler (PLANNED)

**Goal:** Replace round-robin single-step with time-sliced priority scheduler.

### Deliverables

- [ ] **Timer interrupt** -- VM fires tick every N instructions, triggers context switch
- [ ] **Priority levels** -- Each process has priority 0-3, higher gets more slices
- [ ] **Yield/Sleep syscalls** -- Voluntary yield and timed sleep

## [ ] phase-27: Inter-Process Communication (PLANNED)

**Goal:** Processes communicate through pipes and messages, not raw shared RAM.

### Deliverables

- [ ] **PIPE syscall** -- Create unidirectional pipe with circular buffer
- [ ] **MSGSND/MSGRCV syscalls** -- Send and receive fixed-size messages by PID
- [ ] **Blocking I/O** -- READ on empty pipe blocks until data arrives

## [ ] phase-28: Device Driver Abstraction (PLANNED)

**Goal:** All hardware access through a uniform driver interface. Everything is a file.

### Deliverables

- [ ] **Device file convention** -- /dev/screen, /dev/keyboard, /dev/audio, /dev/net
- [ ] **IOCTL syscall** -- Device-specific control operations
- [ ] **Screen/keyboard/audio/net drivers** -- Wrap existing hardware ports as device files

## [ ] phase-29: Shell (PLANNED)

**Goal:** Proper command shell with pipes, redirection, environment variables.

### Deliverables

- [ ] **shell.asm** -- Interactive command interpreter as user process
- [ ] **Pipe operator** -- prog1 | prog2 connects stdout to stdin
- [ ] **Redirection** -- prog > file, prog < file, prog >> file
- [ ] **Built-in commands** -- ls, cd, cat, echo, ps, kill, help

## [ ] phase-30: Boot Sequence & Init (PLANNED)

**Goal:** OS boots into known state, starts init, manages services.

### Deliverables

- [ ] **Boot ROM** -- Fixed bytecode at 0x0000, initializes hardware, jumps to init
- [ ] **Init process** -- PID 1, reads boot.cfg, starts shell
- [ ] **Graceful shutdown** -- SHUTDOWN syscall stops all processes, flushes FS

## [ ] phase-31: Standard Library (PLANNED)

**Goal:** Reusable library of common operations for all programs.

### Deliverables

- [ ] **lib/stdlib.asm** -- String ops, memory ops, formatted I/O
- [ ] **lib/math.asm** -- sin, cos, sqrt via lookup tables
- [ ] **Heap allocator** -- malloc/free for dynamic memory
- [ ] **Linking convention** -- .include or .lib directive in assembler

## [ ] phase-32: Signals & Process Lifecycle (PLANNED)

**Goal:** Signals, exit codes, wait, and proper process lifecycle management.

### Deliverables

- [ ] **SIGNAL syscall** -- Send signal to process by PID
- [ ] **Signal handlers** -- Process sets handler address for each signal type
- [ ] **EXIT/WAIT syscalls** -- Exit with status code, parent waits for child
- [ ] **Zombie cleanup** -- Exited processes cleaned up after parent WAIT

## Global Risks

- Opcode space: 48 of ~256 slots used, plenty of room
- Scope creep -- adding features is easy, keeping the OS coherent is hard
- Kernel boundary breaks existing programs -- need a compatibility mode
- Memory protection removes shared RAM -- need IPC first or programs break
- Filesystem persistence needs host directory -- WASM port needs different backing

## Conventions

- Every new opcode gets a test in tests/program_tests.rs
- Every new program gets assembled by test_all_programs_assemble
- README.md updated when opcodes or features change
- roadmap.yaml is the single source of truth for project state
- Semantic versioning: minor bump for new opcodes, patch for fixes
- New opcodes need a program that needs them (no speculative opcodes)
