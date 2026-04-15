# Geometry OS Roadmap

Pixel-art virtual machine with built-in assembler, debugger, and live GUI.\n  144 opcodes, 32 registers, 64K RAM, 256x256 framebuffer. Write assembly in\n  the built-in text editor, press F5,  watch it run.

**Progress:** 49/50 phases complete, 1 in progress

**Deliverables:** 210/212 complete

**Tasks:** 81/83 complete

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
| phase-24 Memory Protection | COMPLETE | 6/6 | - | - |
| phase-25 Filesystem | COMPLETE | 5/5 | - | - |
| phase-26 Preemptive Scheduler | COMPLETE | 3/3 | - | - |
| phase-27 Inter-Process Communication | COMPLETE | 4/4 | - | - |
| phase-28 Device Driver Abstraction | COMPLETE | 3/3 | - | - |
| phase-29 Shell | COMPLETE | 6/6 | - | - |
| phase-30 Boot Sequence & Init | COMPLETE | 3/3 | - | - |
| phase-31 Standard Library | COMPLETE | 4/4 | - | - |
| phase-32 Signals & Process Lifecycle | COMPLETE | 4/4 | - | - |
| phase-33 QEMU Bridge | COMPLETE | 9/9 | - | - |
| phase-34 RISC-V RV32I Core | COMPLETE | 6/6 | - | - |
| phase-35 RISC-V Privilege Modes | COMPLETE | 5/5 | - | - |
| phase-36 RISC-V Virtual Memory & Devices | COMPLETE | 8/8 | - | - |
| phase-37 Guest OS Boot (Native RISC-V) | COMPLETE | 6/6 | - | - |
| phase-38 RISC-V M/A/C Extensions | COMPLETE | 3/3 | - | - |
| phase-39 Build Linux for RV32IMAC | COMPLETE | 3/3 | - | - |
| phase-40 Boot Linux in Geometry OS | IN PROGRESS | 0/2 | - | - |
| phase-41 Tracing and Instrumentation | COMPLETE | 4/4 | - | - |
| phase-42 Geometry OS Process Manager | COMPLETE | 3/3 | - | - |
| phase-43 Geometry OS VFS and Disk | COMPLETE | 2/2 | - | - |
| phase-44 Geometry OS Memory Management | COMPLETE | 3/3 | - | - |
| phase-45 RAM-Mapped Canvas Buffer | COMPLETE | 5/5 | 370 | 10 |
| phase-46 RAM-Mapped Screen Buffer | COMPLETE | 3/3 | 220 | 8 |
| phase-47 Self-Assembly Opcode (ASMSELF) | COMPLETE | 3/3 | 340 | 8 |
| phase-48 Self-Execution Opcode (RUNNEXT) | COMPLETE | 2/2 | 140 | 5 |
| phase-49 Self-Modifying Programs: Demos and Patterns | COMPLETE | 2/2 | 400 | - |
| phase-50 Reactive Canvas: Live Cell Formulas | COMPLETE | 3/3 | 800 | 10 |

## Dependencies

| From | To | Type | Reason |
|------|----|------|--------|
| phase-32 | phase-33 | soft | Signals not required for QEMU bridge, but nice to have |
| phase-33 | phase-34 | informs | QEMU bridge teaches us what the interpreter needs to reimplement |
| phase-34 | phase-35 | hard | Privilege modes layer on top of the base RV32I interpreter |
| phase-35 | phase-36 | hard | SV32 and devices need privilege modes for trap handling |
| phase-33 | phase-36 | informs | QEMU bridge proved which devices are the minimum needed |
| phase-36 | phase-37 | hard | Need MMU + devices before booting a kernel |
| phase-33 | phase-37 | informs | QEMU boot experience guides native boot requirements |
| phase-45 | phase-46 | hard | Screen mapping follows the same interception pattern as canvas mapping |
| phase-45 | phase-47 | hard | ASMSELF needs the canvas buffer to be addressable so programs can write to it first |
| phase-46 | phase-47 | soft | Screen mapping not required for ASMSELF, but useful for demos |
| phase-47 | phase-48 | hard | RUNNEXT is meaningless without ASMSELF to produce the bytecode |
| phase-45 | phase-49 | hard | Canvas RAM mapping needed for all demos |
| phase-46 | phase-49 | soft | Screen mapping needed for Game of Life demo |
| phase-47 | phase-49 | hard | ASMSELF needed for self-writing and code evolution demos |
| phase-48 | phase-49 | hard | RUNNEXT needed for self-writing and code evolution demos |
| phase-49 | phase-50 | soft | Reactive canvas builds on proven self-modifying patterns |

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

## [x] phase-24: Memory Protection (COMPLETE)

**Goal:** Each process gets its own address space. Processes can't trash each other.

### Deliverables

- [x] **Page tables** -- Simple 1-level paging: page_dir per process, maps virtual to physical
  - [x] translate_va maps virtual to physical via page directory
  - [x] Kernel mode uses identity mapping (no page directory)
- [x] **Address space per process** -- SPAWN creates new page table, not just new registers
  - [x] Each child gets 4 private physical pages
  - [x] Shared regions (page 3, page 63) identity-mapped
- [x] **SEGFAULT on illegal access** -- LOAD/STORE/fetch to unmapped page halts the process
  - [x] test_child_segfaults_on_unmapped_store passes
  - [x] test_child_segfaults_on_unmapped_load passes
  - [x] test_child_segfaults_on_unmapped_fetch passes
  - [x] RAM[0xFF9] tracks segfaulted PID
- [x] **Process memory isolation** -- Two processes can't corrupt each other's memory
  - [x] test_process_memory_isolation passes
  - [x] test_child_user_mode_blocks_hardware_port_write passes
  - [x] test_child_can_access_shared_window_bounds passes
- [x] **Memory protection tests** -- 9 tests covering segfault, isolation, page tables, kernel mode
  - [x] test_child_page_directory_has_shared_regions_mapped passes
  - [x] test_kernel_mode_identity_mapping passes
  - [x] test_kill_frees_physical_pages passes
  - [x] test_segfault_pid_tracking passes
- [x] **Process memory regions documentation** -- docs/MEMORY_PROTECTION.md -- code/heap/stack/shared segments

## [x] phase-25: Filesystem (COMPLETE)

**Goal:** Programs can create, read, write, and delete named files. Persistent storage.

### Deliverables

- [x] **Virtual filesystem layer** -- Abstract FS interface backed by host filesystem in .geometry_os/fs/
- [x] **OPEN/READ/WRITE/CLOSE/SEEK syscalls** -- Full file I/O through syscall interface
- [x] **LS syscall** -- Directory listing into RAM buffer
- [x] **Per-process fd table** -- Max 16 open files per process
- [x] **cat.asm** -- Test program that reads a file and displays it

## [x] phase-26: Preemptive Scheduler (COMPLETE)

**Goal:** Replace round-robin single-step with time-sliced priority scheduler.

### Deliverables

- [x] **Timer interrupt** -- VM fires tick every N instructions, triggers context switch
- [x] **Priority levels** -- Each process has priority 0-3, higher gets more slices
- [x] **Yield/Sleep syscalls** -- Voluntary yield and timed sleep

## [x] phase-27: Inter-Process Communication (COMPLETE)

**Goal:** Processes communicate through pipes and messages, not raw shared RAM.

### Deliverables

- [x] **PIPE syscall** -- Create unidirectional pipe with circular buffer (0x5D)
  - [x] PIPE r5, r6 creates read FD (0x8000|idx) and write FD (0xC000|idx)
  - [x] Pipe buffer holds 256 words
- [x] **MSGSND/MSGRCV syscalls** -- Send and receive fixed-size messages by PID (0x5E, 0x5F)
  - [x] MSGSND sends 4-word message to target PID
  - [x] MSGRCV receives message, returns sender PID in r0
  - [x] Per-process message queue holds 16 messages
- [x] **Blocking I/O** -- READ on empty pipe blocks until data arrives
  - [x] READ on empty pipe sets proc.blocked = true
  - [x] Scheduler skips blocked processes
  - [x] MSGRCV blocks if no message queued
- [x] **pipe_test.asm** -- Program demonstrating parent-child pipe communication

## [x] phase-28: Device Driver Abstraction (COMPLETE)

**Goal:** All hardware access through a uniform driver interface. Everything is a file.

### Deliverables

- [x] **Device file convention** -- /dev/screen, /dev/keyboard, /dev/audio, /dev/net
  - [x] OPEN /dev/screen returns fd 0xE000
  - [x] OPEN /dev/keyboard returns fd 0xE001
  - [x] OPEN /dev/audio returns fd 0xE002
  - [x] OPEN /dev/net returns fd 0xE003
- [x] **IOCTL syscall** -- Device-specific control operations
  - [x] IOCTL assembles to opcode 0x62
  - [x] Screen: get width/height via cmd 0/1
  - [x] Keyboard: get/set echo mode via cmd 0/1
  - [x] Audio: get/set volume via cmd 0/1
  - [x] Net: get status via cmd 0
- [x] **Screen/keyboard/audio/net drivers** -- Wrap existing hardware ports as device files
  - [x] WRITE to /dev/screen draws pixels from (x,y,color) triplets
  - [x] READ from /dev/keyboard reads RAM[0xFFF] and clears it
  - [x] WRITE to /dev/audio sets beep from (freq,dur) pair
  - [x] READ/WRITE to /dev/net uses RAM[0xFFC]
  - [x] device_test.asm demo program

## [x] phase-29: Shell (COMPLETE)

**Goal:** Proper command shell with pipes, redirection, environment variables.

### Deliverables

- [x] **shell.asm** -- Interactive command interpreter as user process
  - [x] shell.asm assembles without errors
  - [x] Built-in commands: ls, cd, cat, echo, ps, kill, help, pwd, clear, exit
- [x] **Pipe operator** -- prog1 | prog2 connects stdout to stdin
  - [x] EXECP opcode (0x6A) spawns with fd redirection
  - [x] shell.asm parses | operator and creates pipes
- [x] **Redirection** -- prog > file, prog < file, prog >> file
  - [x] shell.asm parses > operator and opens file for output
- [x] **Built-in commands** -- ls, cd, cat, echo, ps, kill, help
  - [x] ls lists VFS directory entries
  - [x] cd changes CWD via CHDIR opcode
  - [x] cat reads file and displays content
  - [x] echo prints arguments to screen
  - [x] ps lists process IDs
  - [x] kill terminates a process by PID
  - [x] help displays command list
- [x] **New opcodes** -- READLN, WAITPID, EXECP, CHDIR, GETCWD
  - [x] READLN (0x68) reads keyboard chars into line buffer
  - [x] WAITPID (0x69) waits for child process to halt
  - [x] EXECP (0x6A) spawns program with stdin/stdout fd redirection
  - [x] CHDIR (0x6B) changes current working directory
  - [x] GETCWD (0x6C) reads current working directory
- [x] **VFS dup_fd** -- Duplicate file descriptors across PID tables for pipe/redir

## [x] phase-30: Boot Sequence & Init (COMPLETE)

**Goal:** OS boots into known state, starts init, manages services.

### Deliverables

- [x] **Boot ROM** -- Fixed bytecode at 0x0000, initializes hardware, jumps to init
  - [x] boot() method assembles init.asm and spawns PID 1
  - [x] boot.cfg created with default configuration
- [x] **Init process** -- PID 1, reads boot.cfg, starts shell
  - [x] init.asm assembles without errors
  - [x] init process spawned with priority 2
  - [x] supervisor loop monitors shell and respawns if it dies
  - [x] environment variables set (SHELL, HOME, CWD, USER)
- [x] **Graceful shutdown** -- SHUTDOWN syscall stops all processes, flushes FS
  - [x] SHUTDOWN opcode 0x6E in kernel mode halts VM
  - [x] SHUTDOWN in user mode returns error (r0=0xFFFFFFFF)
  - [x] SHUTDOWN kills all child processes and frees pages
  - [x] SHUTDOWN clears pipes and closes file descriptors
  - [x] shutdown_requested flag set for host to check

## [x] phase-31: Standard Library (COMPLETE)

**Goal:** Reusable library of common operations for all programs.

### Deliverables

- [x] **lib/stdlib.asm** -- String ops, memory ops, formatted I/O
- [x] **lib/math.asm** -- sin, cos, sqrt via lookup tables
- [x] **Heap allocator** -- malloc/free for dynamic memory
  - [x] lib/heap.asm implements _lib_heap_alloc and _lib_heap_free
- [x] **Linking convention** -- .include or .lib directive in assembler
  - [x] .include directive resolves and inlines lib/*.asm files

## [x] phase-32: Signals & Process Lifecycle (COMPLETE)

**Goal:** Signals, exit codes, wait, and proper process lifecycle management.

### Deliverables

- [x] **SIGNAL opcode** -- Send signal to process by PID (SIGTERM=0, SIGKILL=1, SIGUSR=2, SIGALRM=3)
  - [x] SIGNAL opcode sends signal to target process
- [x] **Signal handlers (SIGSET)** -- Process sets handler address for each signal type via SIGSET opcode
  - [x] SIGSET registers handler address, signal delivery jumps to it
- [x] **EXIT/WAITPID opcodes** -- Exit with status code, parent waits for child via WAITPID
  - [x] EXIT opcode halts process with status code, sets zombie flag
  - [x] WAITPID reaps zombie and returns exit code
- [x] **Zombie cleanup** -- Exited processes cleaned up after parent WAITPID
  - [x] Zombie process freed after WAITPID, pages reclaimed

## [x] phase-33: QEMU Bridge (COMPLETE)

**Goal:** Spawn QEMU as a subprocess, pipe serial console I/O through the Geometry OS canvas text surface. Boot Linux on day one.

QEMU gives us a working hypervisor in days. Every architecture QEMU supports
(x86, ARM, RISC-V, MIPS) works immediately. We learn what the canvas text
surface needs to handle (ANSI sequences, scroll speed, buffer size).


### Deliverables

- [x] **qemu.rs module** -- QEMU subprocess management with stdin/stdout pipes
  - [x] `p33.d1.t1` Create src/qemu.rs with QemuBridge struct
    > Create QemuBridge struct with fields for Child process, stdin/stdout pipes,
    > and an output buffer. Implement Drop to kill child on cleanup.
    - QemuBridge struct compiles
    - Drop trait kills child process
    _Files: src/qemu.rs_
  _~60 LOC_
- [x] **QEMU spawn** -- Launch qemu-system-* with -nographic -serial mon:stdio, capture stdin/stdout
  - [x] `p33.d2.t1` Implement QemuBridge::spawn(config_str) -> Result (depends: p33.d1.t1)
    > Parse config string "arch=riscv64 kernel=linux.img ram=256M disk=rootfs.ext4"
    > into QEMU command. Construct qemu-system-{arch} with appropriate flags.
    > Use std::process::Command with stdin/stdout piped.
    - Config string parsed into arch, kernel, ram, disk fields
    - Correct qemu-system-{arch} binary selected
    - -nographic -serial mon:stdio flags always present
    - -machine virt for riscv/aarch64
    - -kernel, -m, -drive flags constructed from config
    _Files: src/qemu.rs_
  - [x] `p33.d2.t2` Implement architecture mapping (riscv64, riscv32, x86_64, aarch64, mipsel) (depends: p33.d2.t1)
    > Map arch config values to qemu-system binary names and machine types.
    - riscv64 -> qemu-system-riscv64 -machine virt
    - x86_64 -> qemu-system-x86_64
    - aarch64 -> qemu-system-aarch64 -machine virt
    - mipsel -> qemu-system-mipsel -machine malta
    - Unknown arch returns error
    _Files: src/qemu.rs_
  - [x] `p33.d2.t3` Test: spawn QEMU with --version, verify process starts and exits clean (depends: p33.d2.t1)
    > Unit test that spawns qemu-system-riscv64 --version and captures version string from stdout.
    - QEMU process starts and exits with code 0
    - Version string captured from stdout
    _Files: src/qemu.rs_
  _~80 LOC_
- [x] **Output to canvas** -- Read QEMU stdout bytes, write to canvas_buffer as u32 chars, auto-scroll
  - [x] `p33.d3.t1` Implement non-blocking stdout reader (depends: p33.d1.t1)
    > Set QEMU stdout to non-blocking mode. Each frame tick, read available
    > bytes into a Vec<u8> buffer. Return the bytes for processing.
    - Non-blocking read returns immediately even if no data
    - Bytes read are valid QEMU output
    _Files: src/qemu.rs_
  - [x] `p33.d3.t2` Implement stdout bytes -> canvas_buffer writer (depends: p33.d3.t1)
    > For each printable byte: write as u32 to canvas_buffer at cursor position.
    > Track virtual cursor (row, col). Auto-scroll when row >= 128.
    - Printable ASCII chars appear in canvas_buffer
    - Cursor advances correctly
    - Scrolling works when row exceeds 128
    _Files: src/qemu.rs, src/main.rs_
  - [x] `p33.d3.t3` Test: feed known bytes, verify canvas_buffer contents (depends: p33.d3.t2)
    > Unit test: write 'Hello\nWorld' bytes, verify canvas_buffer has correct chars at correct positions.
    - 'H' at position [0][0], 'e' at [0][1], etc.
    - 'W' starts at row 1 after newline
    _Files: src/qemu.rs_
  _~60 LOC_
- [x] **Input from keyboard** -- Geometry OS keypresses -> key_to_ascii_shifted() -> write to QEMU stdin
  - [x] `p33.d4.t1` Implement keyboard event -> QEMU stdin writer (depends: p33.d1.t1)
    > When hypervisor is active and a key is pressed, call key_to_ascii_shifted()
    > and write the resulting byte to QEMU's stdin pipe. Map Enter to \\r,
    > Backspace to 0x7F, Ctrl+C to 0x03.
    - Regular keys forwarded as ASCII bytes
    - Enter sends \r (carriage return)
    - Backspace sends 0x7F
    - Ctrl+C sends 0x03
    _Files: src/qemu.rs, src/main.rs_
  _~40 LOC_
- [x] **ANSI escape handling** -- Parse basic ANSI sequences (cursor movement, clear screen) for proper terminal rendering
  - [x] `p33.d5.t1` Implement ANSI escape state machine (depends: p33.d3.t2)
    > State machine: Normal -> Escape (0x1B) -> Csi ('[') -> params.
    > Handle: CSI A/B/C/D (cursor), CSI H (home), CSI 2J (clear),
    > CSI K (clear line), CSI m (color, can ignore), CSI ? 25 h/l (cursor show/hide).
    - ESC [ A moves cursor up
    - ESC [ B moves cursor down
    - ESC [ C moves cursor right
    - ESC [ D moves cursor left
    - ESC [ H moves cursor to 0,0
    - ESC [ 2 J clears canvas_buffer
    - ESC [ K clears from cursor to end of row
    - Unknown sequences ignored gracefully
    _Files: src/qemu.rs_
  - [x] `p33.d5.t2` Test: feed ANSI sequences, verify cursor state (depends: p33.d5.t1)
    > Unit tests for each supported ANSI sequence. Verify cursor position and buffer state.
    - Test for each cursor movement sequence
    - Test for clear screen
    - Test for clear line
    - Test for mixed text + escape sequences
    _Files: src/qemu.rs_
  _~100 LOC_
- [x] **HYPERVISOR opcode (0x72)** -- New opcode that reads config string from RAM and spawns QEMU
  - [x] `p33.d6.t1` Add HYPERVISOR opcode 0x72 to vm.rs execute (depends: p33.d2.t1, p33.d3.t2, p33.d4.t1)
    > Read config string from RAM at address in r0. Parse config.
    > Spawn QemuBridge. Store in VM state. F5 while active kills QEMU.
    - HYPERVISOR opcode triggers QEMU spawn
    - Config string read from VM RAM
    - VM state tracks active hypervisor
    _Files: src/vm.rs_
  - [x] `p33.d6.t2` Add HYPERVISOR to assembler mnemonic list (depends: p33.d6.t1)
    > Register HYPERVISOR in assembler.rs so it can be used in .asm programs.
    - 'HYPERVISOR r0' assembles to opcode 0x54
    - Disassembler outputs HYPERVISOR for 0x54
    _Files: src/assembler.rs_
  _~60 LOC_
- [x] **Shell command** -- hypervisor arch=riscv64 kernel=linux.img command in shell.asm
  - [x] `p33.d7.t1` Add hypervisor command to shell.asm (depends: p33.d6.t1)
    > Parse 'hypervisor <config>' from shell input, construct config string in RAM, execute HYPERVISOR opcode.
    - 'hypervisor arch=riscv64 kernel=linux.img' spawns QEMU
    - Error message on missing kernel file
    _Files: programs/shell.asm_
- [x] **Download helper** -- Script to fetch pre-built RISC-V Linux kernel + rootfs for testing
  - [x] `p33.d8.t1` Create scripts/download_riscv_linux.sh
    > Download pre-built RISC-V 64-bit Linux kernel (Image) and minimal rootfs
    > from a known URL. Place in .geometry_os/fs/linux/ directory.
    - Script downloads kernel Image and rootfs
    - Files placed in correct directory
    - QEMU can boot the downloaded kernel
    _Files: scripts/download_riscv_linux.sh_
- [x] **Integration test** -- Spawn QEMU with known kernel, verify Linux version appears in output
  - [x] `p33.d9.t1` Test: boot RISC-V Linux, verify console output (depends: p33.d2.t1, p33.d3.t1, p33.d5.t1)
    > Integration test (marked #[ignore] for CI without QEMU).
    > Spawn QEMU with RISC-V kernel, read stdout for 30 seconds,
    > verify "Linux version" string appears.
    - QEMU spawns and produces output
    - 'Linux version' detected in output within 30 seconds
    - QEMU process cleaned up after test
    _Files: src/qemu.rs, tests/qemu_boot_test.rs_
  _~60 LOC_

### Technical Notes

QEMU subprocess uses std::process::Command with piped stdin/stdout.
Non-blocking reads via set_nonblocking() on the ChildStdout.
Canvas rendering reuses existing pixel font pipeline from CANVAS_TEXT_SURFACE.md.


### Risks

- QEMU not installed on host -- need clear error message
- ANSI parsing incomplete -- Linux boot output may use obscure sequences
- Non-blocking pipe reads may miss data on fast output -- buffer management

## [x] phase-34: RISC-V RV32I Core (COMPLETE)

**Goal:** Pure software RISC-V RV32I interpreter. 40 base instructions, full test coverage, no QEMU dependency.

QEMU proved what works. Now rebuild it owned -- pure Rust, no subprocess,
portable to WASM and embedded. RV32I is the foundation.


### Deliverables

- [x] **riscv/ module** -- src/riscv/ with mod.rs, cpu.rs, memory.rs, decode.rs
  - [x] `p34.d1.t1` Create src/riscv/ directory with mod.rs, cpu.rs, memory.rs, decode.rs stubs
    - Files compile
    - mod.rs exports public structs
    _Files: src/riscv/mod.rs, src/riscv/cpu.rs, src/riscv/memory.rs, src/riscv/decode.rs_
  _~50 LOC_
- [x] **Register file** -- x[0..32] (x0=zero), PC, 32-bit registers
  - [x] `p34.d2.t1` Define RiscvCpu struct with x[32], pc, privilege fields
    - RiscvCpu struct with x: [u32; 32], pc: u32, privilege: u8
    - x[0] always reads as 0 (enforced on write)
    - new() initializes pc=0x80000000, privilege=3 (M-mode)
    _Files: src/riscv/cpu.rs_
  _~30 LOC_
- [x] **Guest RAM** -- Vec<u8> separate from host RAM, configurable size (default 128MB)
  - [x] `p34.d3.t1` Implement GuestMemory with read_byte/half/word and write_byte/half/word
    - GuestMemory with ram: Vec<u8>, ram_base: u64
    - read_word at 0x80000000 reads first 4 bytes little-endian
    - write_word followed by read_word returns same value
    - Out-of-range access returns error
    _Files: src/riscv/memory.rs_
  _~60 LOC_
- [x] **Instruction decode** -- Decode all RV32I opcodes from 32-bit instruction words
  - [x] `p34.d4.t1` Implement decode() returning Operation enum for all RV32I opcodes (depends: p34.d1.t1)
    - R-type: ADD, SUB, SLL, SLT, SLTU, XOR, SRL, SRA, OR, AND
    - I-type: ADDI, SLTI, SLTIU, XORI, ORI, ANDI, SLLI, SRLI, SRAI
    - Load: LB, LH, LW, LBU, LHU
    - Store: SB, SH, SW
    - Branch: BEQ, BNE, BLT, BGE, BLTU, BGEU
    - Upper: LUI, AUIPC
    - Jump: JAL, JALR
    - System: ECALL, EBREAK, FENCE
    _Files: src/riscv/decode.rs_
  _~200 LOC_
- [x] **Execute loop** -- CPU step() fetches, decodes, executes one instruction
  - [x] `p34.d5.t1` Implement RiscvCpu::step() and execute() for all RV32I instructions (depends: p34.d2.t1, p34.d3.t1, p34.d4.t1)
    - step() fetches word at PC, decodes, executes, advances PC by 4
    - JAL/JALR update PC to target and store return address
    - Branches conditionally update PC
    - x[0] always reads as 0 after any write
    _Files: src/riscv/cpu.rs_
  _~200 LOC_
- [x] **Test suite** -- One test per instruction, verification against known encodings
  - [x] `p34.d6.t1` Write tests for all R-type ALU operations (depends: p34.d5.t1)
    - ADD: 10 + 20 = 30
    - SUB: 30 - 10 = 20
    - SLL: 1 << 5 = 32
    - SLT: 5 < 10 = 1
    - SLTU: unsigned comparison
    - XOR, OR, AND: bitwise ops
    - SRL: logical right shift
    - SRA: arithmetic right shift (sign-preserving)
    _Files: src/riscv/cpu.rs_
  - [x] `p34.d6.t2` Write tests for I-type, load, store, branch, jump instructions (depends: p34.d5.t1)
    - ADDI: x1 = x2 + 100
    - LW/SW: store word, load same address, verify equal
    - LB/LBU: signed vs unsigned byte load
    - BEQ: branch taken when equal, not taken when not
    - JAL: jump and link, verify return address saved
    - JALR: indirect jump with register base
    _Files: src/riscv/cpu.rs_
  - [x] `p34.d6.t3` Write fibonacci test program that runs 20 iterations in RISC-V (depends: p34.d5.t1)
    - Fibonacci(10) = 55 computed by RISC-V code
    - Result stored in a register, verified by test
    _Files: src/riscv/cpu.rs_
  _~300 LOC_

## [x] phase-35: RISC-V Privilege Modes (COMPLETE)

**Goal:** M/S/U privilege levels, CSR registers, trap handling. Linux needs this to manage its own processes.

### Deliverables

- [x] **Privilege enum + CSR bank** -- M/S/U modes, mstatus, mtvec, mepc, mcause, sstatus, stvec, sepc, scause, satp
  _~80 LOC_
- [x] **CSR read/write** -- CSRRW, CSRRS, CSRRC and immediate variants
  _~100 LOC_
- [x] **ECALL/MRET/SRET** -- Trap entry saves PC, jumps to vector. MRET/SRET restore PC.
  _~120 LOC_
- [x] **Timer + software interrupts** -- mtime/mtimecmp, msip/ssip, interrupt pending/enable
  _~80 LOC_
- [x] **Privilege transition tests** -- U->S via ECALL, S->M via ECALL, MRET returns to S, SRET returns to U
  _~150 LOC_

## [x] phase-36: RISC-V Virtual Memory & Devices (COMPLETE)

**Goal:** SV32 page tables and minimum device emulation (UART, CLINT, PLIC, virtio-blk) for guest OS boot.

### Deliverables

- [x] **SV32 page table walk** -- 2-level lookup, PTE flags, address translation
  _~120 LOC_
- [x] **TLB cache** -- 64-entry TLB with ASID-aware invalidation
  _~80 LOC_
- [x] **Page fault traps** -- Instruction/Load/Store page faults with stval/mtval
  _~40 LOC_
- [x] **UART 16550** -- Serial port emulation, reuses Phase 33 bridge pattern to canvas
  _~150 LOC_
- [x] **CLINT + PLIC** -- Timer interrupt controller + platform interrupt controller
  _~200 LOC_
- [x] **Virtio block device** -- Virtio MMIO transport, disk image from VFS
  _~200 LOC_
- [x] **Device Tree Blob** -- Generate DTB describing memory, UART, virtio devices
  _~150 LOC_
- [x] **MMU + device integration test** -- Guest sets up page tables, writes to UART, verify output on canvas
  _~150 LOC_

## [x] phase-37: Guest OS Boot (Native RISC-V) (COMPLETE)

**Goal:** Boot real Linux RISC-V kernel using our own interpreter. Two hypervisor modes: QEMU and native.

### Deliverables

- [x] **ELF + binary loader** -- Parse ELF32 RISC-V kernel images, load segments into guest RAM
  _~160 LOC_
- [x] **DTB passthrough** -- Pass device tree blob to kernel in a1 register at boot
  _~30 LOC_
- [x] **Boot console** -- Guest UART output to canvas (same bridge as Phase 33)
  _~80 LOC_
- [x] **HYPERVISOR mode flag** -- Opcode detects 'native' vs 'qemu' from config string
  _~30 LOC_
- [x] **Verified boot** -- Boot synthetic RISC-V kernel, verify 'Linux version' on canvas via UART bridge
  _~100 LOC_
- [x] **Performance benchmark** -- Measure MIPS, compare interpreter vs QEMU, document results
  _~40 LOC_

## [x] phase-38: RISC-V M/A/C Extensions (COMPLETE)

**Goal:** Extend the interpreter from RV32I to RV32IMAC so it can run real Linux kernels.

Linux requires at minimum RV32IMAC: M (multiply/divide), A (atomics), C (compressed 16-bit instructions). Our interpreter currently only handles RV32I. These extensions are well-defined and mechanical to implement. M: 8 instructions. A: 11 instructions. C: ~40 compressed forms.

### Deliverables

- [x] **M extension (multiply/divide)** -- MUL, MULH, MULHU, MULHSU, DIV, DIVU, REM, REMU. All R-type, funct7=0b0000001.
  - [x] `p38.d1.t1` Add M-extension opcodes to decode.rs and execute in cpu.rs
    - MUL: rd = (rs1 * rs2)[31:0]
    - MULH: rd = (rs1 * rs2)[63:32] signed*signed
    - MULHU: rd = (rs1 * rs2)[63:32] unsigned*unsigned
    - MULHSU: rd = (rs1 * rs2)[63:32] signed*unsigned
    - DIV: rd = rs1 / rs2 signed
    - DIVU: rd = rs1 / rs2 unsigned
    - REM: rd = rs1 % rs2 signed
    - REMU: rd = rs1 % rs2 unsigned
    _Files: src/riscv/decode.rs, src/riscv/cpu.rs_
  - [x] All 8 M-extension opcodes decode and execute correctly
  - [x] Edge cases handled -- div by zero, overflow, signed/unsigned semantics
  _~80 LOC_
- [x] **A extension (atomics)** -- LR.W, SC.W, AMOSWAP, AMOADD, AMOXOR, AMOAND, AMOOR, AMOMIN, AMOMAX, AMOMINU, AMOMAXU
  - [x] `p38.d2.t1` Add A-extension atomic instructions with reservation set tracking
    - LR.W: load reserved, track address in reservation set
    - SC.W: store conditional, succeed only if reservation holds
    - AMOSWAP: atomically swap rs2 into memory, return old value
    - AMOADD/AMOAND/AMOOR/AMOXOR: atomic RMW operations
    - AMOMIN/AMOMAX/AMOMINU/AMOMAXU: atomic min/max
    _Files: src/riscv/decode.rs, src/riscv/cpu.rs_
  _~100 LOC_
- [x] **C extension (compressed instructions)** -- Decode 16-bit compressed instruction forms into equivalent 32-bit operations
  - [x] `p38.d3.t1` Implement C-extension decoder for all RV32C compressed instructions
    - C.LWSP, C.SWSP, C.LW, C.SW
    - C.ADDI, C.ADDI16SP, C.ADDI4SPN, C.LI, C.LUI
    - C.SRLI, C.SRAI, C.ANDI, C.SUB, C.XOR, C.OR, C.AND
    - C.BEQZ, C.BNEZ, C.J, C.JAL, C.JR, C.JALR, C.EBREAK
    - C.NOP, C.ADD, C.MV
    _Files: src/riscv/decode.rs, src/riscv/cpu.rs_
  _~200 LOC_

## [x] phase-39: Build Linux for RV32IMAC (COMPLETE)

**Goal:** Cross-compile a minimal Linux kernel and initramfs for riscv32 that boots in our interpreter.

Use Buildroot or direct kernel build to produce a vmlinux for riscv32. Tinyconfig + UART + CLINT + PLIC + virtio-blk + initramfs with busybox. Target: boot to shell in under 256MB RAM.

### Deliverables

- [x] **RV32 toolchain** -- riscv32 cross-compiler toolchain
  - [x] `p39.d1.t1` Install or build riscv32 cross-compiler toolchain
  - [x] riscv32 gcc compiles a hello world
  - [x] Can produce statically-linked ELF binaries for rv32imac
- [x] **Minimal kernel** -- Linux vmlinux for riscv32, defconfig + UART/CLINT/PLIC/virtio
  - [x] `p39.d2.t1` Build minimal Linux kernel for riscv32 with UART/CLINT/PLIC/virtio
  - [x] vmlinux ELF is valid ELF32 RISC-V binary
  - [x] Console output via UART
  - [x] Kernel loads in Geometry OS interpreter
  - [x] Kernel size under 20MB
- [x] **Initramfs** -- Busybox-based root filesystem in initramfs
  - [x] `p39.d3.t1` Create minimal initramfs with busybox for riscv32
  - [x] busybox statically linked for rv32imac
  - [x] /init script mounts proc/sys, spawns shell
  - [x] initramfs size under 4MB

## [~] phase-40: Boot Linux in Geometry OS (IN PROGRESS)

**Goal:** Boot the riscv32 Linux kernel inside our RISC-V interpreter and reach a shell prompt.

Load vmlinux + initramfs into the interpreter, boot to shell. This is the "QEMU bridge" moment -- running real Linux in our own emulator. Fix any interpreter bugs discovered during boot.

### Deliverables

- [~] **Linux boot** -- Linux boots to shell prompt in the interpreter
  - [~] `p40.d1.t1` Fix interpreter issues blocking Linux boot
    - vmlinux loads and begins executing
    - Kernel reaches console output (prints "Linux version...")
    - No unimplemented instruction panics
  _~200 LOC_
- [ ] **Shell access** -- Interactive shell via UART bridge to canvas
  - [ ] `p40.d2.t1` Get Linux to a working shell prompt through the UART canvas
    - Shell prompt appears on canvas
    - Can type commands and see output
  _~100 LOC_

## [x] phase-41: Tracing and Instrumentation (COMPLETE)

**Goal:** Add instruction-level tracing to the interpreter so we can watch exactly what Linux does.

Once Linux boots, instrument the interpreter to capture: every syscall, every page table walk, every context switch, every interrupt.

### Deliverables

- [x] **Instruction trace** -- Log every instruction executed with register state
  - [x] `p41.d1.t1` Add toggleable instruction-level tracing to step()
    - Can enable/disable trace at runtime
    - {'Each line': 'PC, opcode, register values, result'}
    - Trace output to file or ring buffer
    - Overhead under 2x when tracing enabled
  _~272 LOC_
- [x] **Syscall trace** -- Intercept ECALL and decode/record syscall name + args + return value
  - [x] `p41.d2.t1` Add syscall decoder mapping Linux riscv syscall numbers to names
    - Maps all ~400 Linux riscv syscalls by number
    - Logs syscall_name(arg0, arg1, ...) = return_value
  _~100 LOC_
- [x] **Page table trace** -- Trace SV32 page table walks, TLB fills, and page faults
  - [x] `p41.d3.t1` Add page table walk tracing to MMU
    - Logs every SATP write (new page table root)
    - Logs page table walks with VPN to PFN mappings
    - Logs page faults with faulting VA and reason
  _~80 LOC_
- [x] **Scheduler trace** -- Detect and log context switches and schedule decisions
  - [x] `p41.d4.t1` Infer context switches from register state changes
    - Detects task switches via SP/mhartid changes
    - Logs switch_from to switch_to with PC and SP
  _~60 LOC_

## [x] phase-42: Geometry OS Process Manager (COMPLETE)

**Goal:** Rebuild Geometry OS process management based on observed Linux scheduler behavior.

Using traces from Phase 41, understand how Linux creates processes, schedules them, and manages task state. Then build Geometry OS equivalents that follow the same patterns but simpler.

### Deliverables

- [x] **Process abstraction** -- Process struct with PID, state, page table, registers, kernel stack
  - [x] `p42.d1.t1` Design Process struct based on Linux task_struct observations
    - Process has PID, state, page table root, saved registers
    - Kernel stack per process
    - Parent/child relationship
  _~200 LOC_
- [x] **Context switching** -- Save/restore registers on timer interrupt, switch address space
  - [x] `p42.d2.t1` Implement context switch based on traced Linux switch_to pattern
    - Timer interrupt triggers schedule
    - callee-saved registers preserved
    - SATP updated on address space change
  _~150 LOC_
- [x] **Fork/exec/exit/wait** -- Process lifecycle syscalls matching Linux semantics
  - [x] `p42.d3.t1` Implement fork, exec, exit, wait syscalls
    - fork returns 0 in child, child PID in parent
    - exec replaces process image
    - exit marks zombie, wakes parent
    - wait blocks parent until child exits
  _~200 LOC_

## [x] phase-43: Geometry OS VFS and Disk (COMPLETE)

**Goal:** Build a virtual filesystem layer based on observed Linux VFS patterns.

Trace Linux VFS operations during boot and build Geometry OS equivalents.

### Deliverables

- [x] **Inode filesystem** -- In-memory inode-based filesystem with directory tree
  - [x] `p43.d1.t1` Implement inode structures and directory operations
    - {'Inode types': 'regular file, directory, device, pipe'}
    - Path resolution and read/write with offset tracking
    - FMKDIR, FSTAT, FUNLINK opcodes with assembler and disassembler support
    - 30+ unit tests for inode operations
  _~300 LOC_
- [x] **File descriptor table** -- Per-process fd table with pipe support
  - [x] `p43.d2.t1` Implement fd table with open/close/dup2/pipe
    - stdin/stdout/stderr per process
    - pipe creates connected read/write fds
    - dup2 for shell redirects
  _~100 LOC_

## [x] phase-44: Geometry OS Memory Management (COMPLETE)

**Goal:** Rebuild Geometry OS memory management based on observed Linux SV32 paging.

Trace Linux page table setup during boot and build Geometry OS equivalents.

### Deliverables

- [x] **Page allocator** -- Physical page allocator for 4KB pages
  - [x] `p44.d1.t1` Implement physical page allocator
    - Allocates/frees 4KB pages
    - Tracks used/free pages
  _~150 LOC_
- [x] **Virtual memory areas** -- Per-process VMA list for code, heap, stack, mmap
  - [x] `p44.d2.t1` Implement VMA tracking and page fault handler
    - VMA list per process
    - Page fault allocates on demand
    - Stack grows downward, heap via brk
  _~150 LOC_
- [x] **Copy-on-write fork** -- Fork shares physical pages, copies only on write
  - [x] `p44.d3.t1` Implement COW fork based on observed Linux fork behavior
    - fork marks pages read-only in child
    - Write fault copies page
    - Reference counting on physical pages
  _~100 LOC_

## [x] phase-45: RAM-Mapped Canvas Buffer (COMPLETE)

**Goal:** Make the canvas grid addressable from VM RAM via STORE/LOAD

The canvas buffer (128 rows x 32 cols = 4096 cells) currently lives in a separate Vec<u32> outside VM RAM. Map it into the VM address space at 0x8000-0x8FFF so that existing STORE and LOAD opcodes can read and write grid cells directly. No new opcodes needed -- just intercept the address range in the VM's memory access path.


### Deliverables

- [x] **Canvas memory region constant and address mapping** -- Define CANVAS_RAM_BASE = 0x8000, CANVAS_RAM_SIZE = 4096 (128*32). Document the mapping: address 0x8000 + row*32 + col corresponds to canvas_buffer[row * 32 + col]. Add to memory map docs.

  - [x] `p45.d1.t1` Define CANVAS_RAM_BASE and CANVAS_RAM_SIZE constants
    > Add `pub const CANVAS_RAM_BASE: usize = 0x8000;` and `pub const CANVAS_RAM_SIZE: usize = 4096;` to vm.rs (or main.rs if canvas_buffer ownership stays there). These are the address range [0x8000, 0x8FFF] that maps to the canvas grid.
    - Constants defined and visible to both vm.rs and main.rs
    _Files: src/vm.rs_
  - [x] `p45.d1.t2` Update CANVAS_TEXT_SURFACE.md memory map with 0x8000 range (depends: p45.d1.t1)
    > Add a row to the memory map table in CANVAS_TEXT_SURFACE.md: 0x8000-0x8FFF | 4096 | Canvas grid (RAM-mapped mirror of canvas_buffer)
    - Memory map shows 0x8000-0x8FFF as canvas region
    _Files: docs/CANVAS_TEXT_SURFACE.md_
  - [x] CANVAS_RAM_BASE constant defined in vm.rs or main.rs
    _Validation: grep CANVAS_RAM_BASE src/vm.rs src/main.rs_
  - [x] Memory map documentation updated in CANVAS_TEXT_SURFACE.md
    _Validation: grep 0x8000 docs/CANVAS_TEXT_SURFACE.md_
  _~20 LOC_
- [x] **Intercept LOAD for canvas address range** -- In the LOAD opcode handler (0x11 in vm.rs), when the translated physical address falls in [CANVAS_RAM_BASE, CANVAS_RAM_BASE + 4095], read from canvas_buffer instead of self.ram. The VM needs a reference or copy of the canvas buffer. Easiest approach: the canvas_buffer is passed to the VM (or VM holds a reference) so LOAD can index into it.

  - [x] `p45.d2.t1` Add canvas_buffer reference to VM struct (depends: p45.d1.t1)
    > The VM struct needs access to the canvas buffer for both LOAD and STORE interception. Add a field like `pub canvas_buffer: Vec<u32>` to the VM struct (a copy that gets synced back to main.rs canvas_buffer each frame) OR pass it as a mutable reference through the execute method. The copy approach is simpler and avoids lifetime issues.
    - VM struct has access to canvas buffer data
    - cargo build succeeds
    _Files: src/vm.rs, src/main.rs_
  - [x] `p45.d2.t2` Intercept LOAD opcode for canvas range (depends: p45.d2.t1)
    > In the LOAD handler (opcode 0x11), after page translation produces a physical address, check if it falls in [CANVAS_RAM_BASE, CANVAS_RAM_BASE + CANVAS_RAM_SIZE). If so, read from the canvas buffer at (addr - CANVAS_RAM_BASE) instead of self.ram[addr]. The canvas buffer index maps directly: canvas_buffer[addr - 0x8000].
    - LOAD from canvas addr returns the glyph value stored there
    - LOAD from normal RAM addr is unchanged
    _Files: src/vm.rs_
  - [x] `p45.d2.t3` Sync canvas_buffer to VM before execution (depends: p45.d2.t1)
    > Before each frame's VM execution, copy the current canvas_buffer contents into the VM's canvas mirror (or set up the reference). This ensures the VM sees the latest grid state including human-typed text.
    - VM canvas mirror matches main.rs canvas_buffer at start of each frame
    _Files: src/main.rs_
  - [x] LOAD from 0x8000+row*32+col returns canvas_buffer value
    _Validation: Write test program: STORE to canvas addr, LOAD back, verify_
  - [x] LOAD from addresses outside 0x8000-0x8FFF still works normally
    _Validation: Existing tests pass without modification_
  _~80 LOC_
- [x] **Intercept STORE for canvas address range** -- In the STORE opcode handler (0x12 in vm.rs), when the translated physical address falls in [CANVAS_RAM_BASE, CANVAS_RAM_BASE + 4095], write to canvas_buffer instead of self.ram. After the store, mark the canvas as dirty so the renderer picks up the change on the next frame.

  - [x] `p45.d3.t1` Intercept STORE opcode for canvas range (depends: p45.d2.t1)
    > In the STORE handler (opcode 0x12), after page translation, check if the address is in the canvas range. If so, write to the canvas buffer at (addr - CANVAS_RAM_BASE) instead of self.ram[addr]. Bypass the user-mode protection for this range (canvas is not I/O).
    - STORE to canvas addr writes to canvas buffer
    - User-mode programs can write to canvas (no segfault)
    _Files: src/vm.rs_
  - [x] `p45.d3.t2` Sync VM canvas mutations back to main canvas_buffer (depends: p45.d3.t1)
    > After each frame's VM execution, copy any changed canvas cells from the VM's mirror back to main.rs's canvas_buffer. This ensures the renderer displays the VM's writes. A simple full-copy each frame is fine (4096 u32 values = 16KB).
    - Changes made by VM via STORE appear on the visible canvas grid
    _Files: src/main.rs_
  - [x] `p45.d3.t3` Handle User-mode access to canvas region (depends: p45.d3.t1)
    > The STORE handler currently blocks User-mode writes to addr >= 0xFF00. The canvas range (0x8000) is below this threshold, so User-mode should work by default. But verify and add a comment clarifying that canvas writes are permitted in User mode. If any page translation or protection logic would block it, add an explicit exception.
    - User-mode programs can STORE to canvas range without segfault
    _Files: src/vm.rs_
  - [x] STORE to 0x8000+row*32+col writes value to canvas_buffer
    _Validation: Write test: STORE 0x8000 with 'H', see 'H' appear on grid_
  - [x] Stored values appear as glyphs on the canvas grid
    _Validation: Visual test: program writes ASCII chars, grid shows them_
  - [x] STORE to addresses outside canvas range still works
    _Validation: Existing tests pass_
  _~60 LOC_
- [x] **Test suite for RAM-mapped canvas** -- Write tests that verify STORE/LOAD to canvas addresses work correctly. Test read-after-write, boundary conditions, interaction with normal RAM, and multi-process canvas access.

  - [x] `p45.d4.t1` Test: LOAD reads canvas buffer values (depends: p45.d2.t2, p45.d3.t1)
    > Write a test that pre-fills canvas_buffer cells with known values, runs a program that LOADs from 0x8000+offset, and checks the register contains the expected value.
    - Test asserts register value matches canvas cell content
    _Files: src/vm.rs_
  - [x] `p45.d4.t2` Test: STORE writes appear in canvas buffer (depends: p45.d3.t1)
    > Write a test that runs a program storing values to canvas addresses, then checks the canvas buffer contains those values.
    - Test asserts canvas_buffer has stored values after execution
    _Files: src/vm.rs_
  - [x] `p45.d4.t3` Test: boundary conditions (first/last cell, row boundaries) (depends: p45.d3.t1)
    > Test STORE/LOAD at 0x8000 (first cell), 0x8FFF (last cell), and at row boundaries (e.g. end of row 0, start of row 1). Verify no off-by-one errors.
    - All boundary addresses read/write correctly
    _Files: src/vm.rs_
  - [x] `p45.d4.t4` Test: canvas access does not corrupt normal RAM (depends: p45.d3.t1)
    > Write a test that stores to both normal RAM and canvas addresses, then verifies the normal RAM values are unchanged and the canvas values are correct. Ensures the two memory spaces don't overlap.
    - RAM values unchanged after canvas writes
    - Canvas values unchanged after RAM writes
    _Files: src/vm.rs_
  - [x] `p45.d4.t5` Test: page translation works with canvas addresses (depends: p45.d2.t2, p45.d3.t1)
    > Verify that LOAD/STORE to canvas addresses still go through the page translation mechanism. A process with a page table that maps 0x8000 to a different physical address should see the translated result. Or if canvas is identity-mapped, verify that works.
    - Canvas LOAD/STORE respects page translation
    _Files: src/vm.rs_
  - [x] At least 5 tests covering canvas LOAD/STORE behavior
    _Validation: cargo test passes with new tests_
  - [x] All existing tests still pass
    _Validation: cargo test --no-fail-fast 2>&1 | tail -5_
  _~150 LOC_
- [x] **Demo program: canvas grid writer** -- Write an assembly program that writes ASCII characters to the canvas grid using STORE. The program fills the grid with a visible pattern -- for example, writing "HELLO WORLD" across the top row, or filling the grid with sequential ASCII values. The human sees the text appear on the grid while the program runs.

  - [x] `p45.d5.t1` Write canvas_grid_writer.asm demo (depends: p45.d3.t1)
    > Create programs/canvas_grid_writer.asm. The program uses LDI to load ASCII values and STORE to write them to 0x8000+ addresses. Writes "PIXELS DRIVE PIXELS" across the first visible row. Uses a loop with an index register incrementing through the string.
    - Program assembles without errors
    - Running the program shows text on the canvas grid
    _Files: programs/canvas_grid_writer.asm_
  - [x] `p45.d5.t2` Write canvas_counter.asm demo (depends: p45.d3.t1)
    > Create programs/canvas_counter.asm. A loop that increments a counter and writes the digit (as ASCII) to a specific canvas cell each iteration. The human sees a digit ticking up on the grid in real time.
    - Counter digit visibly changes on the grid each frame
    _Files: programs/canvas_counter.asm_
  - [x] Program writes visible text to canvas grid via STORE
    _Validation: Load program, F8 assemble, F5 run, see text on grid_
  - [x] Demo program added to programs/ directory
    _Validation: ls programs/canvas_*.asm_
  _~60 LOC_

### Technical Notes

The VM's RAM is 0x10000 (65536 cells). The canvas buffer is 4096 cells. Mapping at 0x8000 leaves plenty of headroom (0x9000-0xFFFF still available). The screen buffer (256x256 = 65536 pixels) is too large for a contiguous RAM mapping -- that's addressed in phase 46.
Canvas buffer sync strategy: copy main's canvas_buffer into VM before execution, copy VM's canvas writes back after execution. 4096 * 4 bytes = 16KB per frame, negligible cost.
The page translation layer (translate_va_or_fault) must be considered. For kernel-mode processes (the default for canvas-assembled programs), virtual address == physical address. For user-mode child processes, the page table may remap things. The canvas range should work through the normal translation path.


### Risks

- Page translation might block canvas access for user-mode processes
- Canvas buffer ownership between main.rs and vm.rs needs careful handling
- STORE handler's user-mode protection (addr >= 0xFF00 check) must not block canvas writes

## [x] phase-46: RAM-Mapped Screen Buffer (COMPLETE)

**Goal:** Make the 256x256 screen buffer addressable from VM RAM

The screen buffer (256x256 = 65536 pixels) is currently only accessible via PIXEL (write) and PEEK (read) opcodes. Map it into the VM address space at 0x9000-0x13FFF (a 64K region) so that normal LOAD/STORE can read and write screen pixels. This unifies all three memory spaces (RAM, canvas, screen) under one addressing scheme.


### Deliverables

- [x] **Screen memory region mapping** -- Define SCREEN_RAM_BASE = 0x9000. The screen is 256x256 = 65536 cells, so it spans 0x9000-0x18FFF. However, VM RAM is only 0x10000 total. Options: (a) expand RAM_SIZE to 0x20000, (b) use a sparse/aliased mapping where only low-res access works, (c) map screen at a higher address with extended RAM. Simplest: expand RAM to 0x20000 (128K) and map screen at 0x10000.

  - [x] `p46.d1.t1` Determine screen mapping strategy and expand RAM if needed (depends: p45.d3.t1)
    > Evaluate options for mapping the 64K screen buffer. The simplest approach: expand RAM_SIZE from 0x10000 to 0x20000 (128K) and map the screen buffer at 0x10000. This keeps everything in one flat address space. Alternative: use a windowed mapping at 0x9000 where only a 4K window is visible at a time (controlled by a register). Recommend the flat mapping for simplicity.
    - Decision documented with address range and RAM size
    _Files: src/vm.rs_
  - [x] `p46.d1.t2` Implement screen buffer LOAD interception (depends: p46.d1.t1)
    > In the LOAD handler, check if the translated address falls in the screen buffer range. If so, read from self.screen[addr - SCREEN_RAM_BASE] instead of self.ram[addr]. The screen buffer already exists on the VM struct as `pub screen: Vec<u32>`.
    - LOAD from screen addr returns pixel color value
    _Files: src/vm.rs_
  - [x] `p46.d1.t3` Implement screen buffer STORE interception (depends: p46.d1.t1)
    > In the STORE handler, check if the translated address falls in the screen buffer range. If so, write to self.screen[addr - SCREEN_RAM_BASE]. The renderer will pick up the change on the next frame automatically since it reads from self.screen.
    - STORE to screen addr changes the visible pixel
    _Files: src/vm.rs_
  - [x] Screen buffer is LOAD/STORE accessible at a defined address range
    _Validation: LOAD from screen addr returns same value as PEEK_
  - [x] Existing PIXEL and PEEK opcodes still work
    _Validation: cargo test passes_
  _~100 LOC_
- [x] **Tests for screen buffer mapping** -- Verify that LOAD/STORE to screen addresses correctly read and write pixels. Cross-validate against PEEK and PIXEL opcodes.

  - [x] `p46.d2.t1` Test: LOAD from screen matches PEEK (depends: p46.d1.t2)
    > Write a test that draws a pixel with PIXEL opcode, then reads it with both PEEK and LOAD (via screen-mapped address). Verify both return the same color value.
    - PEEK and LOAD return identical values
    _Files: src/vm.rs_
  - [x] `p46.d2.t2` Test: STORE to screen matches PIXEL (depends: p46.d1.t3)
    > Write a test that writes a pixel via both PIXEL opcode and STORE to screen-mapped address. Read back with PEEK and verify both wrote the same value.
    - Both methods produce identical pixel values on screen
    _Files: src/vm.rs_
  - [x] LOAD from screen address matches PEEK result
    _Validation: Test program: PEEK and LOAD same pixel, compare registers_
  - [x] STORE to screen address matches PIXEL result
    _Validation: Test program: STORE and PIXEL write same location, compare_
  _~80 LOC_
- [x] **Unified memory map documentation** -- Update all memory map documentation to show the complete unified address space: RAM (0x0000-0x7FFF), canvas (0x8000-0x8FFF), screen (0x10000+). Add a new doc section showing the full map.

  - [x] `p46.d3.t1` Write UNIFIED_MEMORY_MAP section in docs (depends: p46.d1.t3)
    > Add a section to CANVAS_TEXT_SURFACE.md (or create UNIFIED_MEMORY_MAP.md) showing the complete address space: 0x0000-0x0FFF: bytecode/data, 0x1000-0x1FFF: canvas bytecode, 0x8000-0x8FFF: canvas grid (mirror), 0x10000-0x1FFFF: screen buffer. Explain the design: one address space, three backing stores, LOAD/STORE as the universal access method.
    - Document shows all regions with address ranges and purposes
    _Files: docs/CANVAS_TEXT_SURFACE.md_
  - [x] All three regions documented in one place
    _Validation: grep 'canvas\|screen\|RAM' docs/CANVAS_TEXT_SURFACE.md shows unified map_
  _~40 LOC_

### Technical Notes

The screen buffer (self.screen) is already a field on the VM struct, unlike canvas_buffer which lives in main.rs. This makes interception simpler -- no sync step needed.
RAM_SIZE expansion from 0x10000 to 0x20000 adds 256KB of memory (64K u32 cells). At current RAM usage this is fine. The screen mapping at 0x10000 means screen pixels are at screen[y * 256 + x], accessed as RAM[0x10000 + y * 256 + x].
Alternative: don't expand RAM, instead use a separate mapping that redirects LOAD/STORE at 0x9000-0xFFFF to the screen buffer. But this creates an address collision with I/O ports (0xFFB-0xFFF). Expanding RAM is cleaner.


### Risks

- RAM_SIZE expansion may affect fuzzer or existing test assumptions about address space
- Screen buffer is 256x256=64K which exactly fills the expansion -- no room for growth
- Page translation for screen addresses may need special handling

## [x] phase-47: Self-Assembly Opcode (ASMSELF) (COMPLETE)

**Goal:** Add an opcode that lets a running program assemble canvas text into bytecode

Add the ASMSELF opcode (or RECOMPILE) that reads the current canvas text, runs it through the preprocessor and assembler, and stores the resulting bytecode at 0x1000. This lets a program write new assembly onto the canvas grid (using STORE to the canvas range from phase 45) and then compile it without human intervention. Combined, a program can generate its own replacement.


### Deliverables

- [x] **ASMSELF opcode implementation** -- New opcode (suggest 0x52 or next available). When executed: 1. Read the canvas buffer as a text string (same logic as F8 handler) 2. Run through preprocessor::preprocess() 3. Run through assembler::assemble() 4. If success: write bytecode to 0x1000, set a flag 5. If failure: set an error register/port with the error info The VM needs access to the preprocessor and assembler modules.

  - [x] `p47.d1.t1` Add ASMSELF opcode constant and handler skeleton (depends: p45.d3.t1)
    > Reserve the next available opcode number for ASMSELF. Add a stub handler in the VM's execute loop that reads the canvas buffer, converts to text string, and logs "ASMSELF called" for now.
    - Opcode constant defined in vm.rs
    - Handler appears in execute match arm
    _Files: src/vm.rs_
  - [x] `p47.d1.t2` Implement canvas-to-text conversion in VM context (depends: p47.d1.t1)
    > Extract the canvas-to-text conversion logic from the F8 handler in main.rs into a reusable function. This function takes a &[u32] (canvas buffer slice) and returns a String. The F8 handler and the ASMSELF opcode both call this function. Place it in a shared module (e.g., preprocessor.rs or a new canvas.rs).
    - Function exists and is callable from both vm.rs and main.rs
    - F8 handler refactored to use the shared function
    _Files: src/vm.rs, src/main.rs_
  - [x] `p47.d1.t3` Wire preprocessor and assembler into ASMSELF handler (depends: p47.d1.t2)
    > In the ASMSELF handler, after getting the text string from the canvas: call preprocessor::preprocess(), then assembler::assemble(). On success, write bytecode bytes to self.ram starting at CANVAS_BYTECODE_ADDR (0x1000). On failure, write the error string to a memory-mapped error port or a designated RAM region. The VM will need to import/use the preprocessor and assembler modules.
    - ASMSELF produces valid bytecode at 0x1000
    - Invalid assembly writes error info without crashing
    _Files: src/vm.rs_
  - [x] `p47.d1.t4` Add ASMSELF to disassembler (depends: p47.d1.t1)
    > Add the ASMSELF opcode to the disassemble() method in vm.rs so it appears correctly in trace output and disassembly views.
    - Disassembler shows ASMSELF with correct operand count
    _Files: src/vm.rs_
  - [x] `p47.d1.t5` Add ASMSELF to assembler mnemonic list (depends: p47.d1.t1)
    > Add "ASMSELF" to the OPCODES list in preprocessor.rs and the assembler in assembler.rs. It takes no operands (just the opcode byte). Update the opcode count in docs and meta.
    - Can type ASMSELF in assembly source and it assembles
    - Opcode count incremented in documentation
    _Files: src/assembler.rs, src/preprocessor.rs_
  - [x] ASMSELF assembles canvas text into bytecode at 0x1000
    _Validation: Program writes text to canvas, calls ASMSELF, then LOADs from 0x1000 to verify bytecode_
  - [x] Assembly errors are reported without crashing the VM
    _Validation: Write invalid text to canvas, call ASMSELF, VM continues running_
  _~200 LOC_
- [x] **Assembly status port** -- Define a memory-mapped port (e.g., 0xFFE or 0xFFA) where the ASMSELF opcode writes its result: success (bytecode length) or failure (0xFFFFFFFF). Programs poll this port after calling ASMSELF to check if assembly succeeded.

  - [x] `p47.d2.t1` Define ASM_STATUS port and write logic (depends: p47.d1.t3)
    > Use existing RAM[0xFFD] (ASM result port) which already exists for this purpose (bytecode word count, or 0xFFFFFFFF on error). Ensure ASMSELF writes to this port identically to how F8 assembly does.
    - RAM[0xFFD] contains result after ASMSELF
    _Files: src/vm.rs_
  - [x] Port shows bytecode length on success
    _Validation: LOAD from status port after ASMSELF returns positive number_
  - [x] Port shows 0xFFFFFFFF on failure
    _Validation: LOAD from status port after bad ASMSELF returns 0xFFFFFFFF_
  _~20 LOC_
- [x] **Test suite for ASMSELF** -- Test that ASMSELF correctly assembles canvas text, handles errors, and the resulting bytecode is executable.

  - [x] `p47.d3.t1` Test: ASMSELF assembles valid canvas text (depends: p47.d1.t3)
    > Pre-fill canvas buffer with "LDI r0, 42\nHALT\n". Execute ASMSELF. Verify RAM[0xFFD] contains a positive byte count. Verify RAM at 0x1000 contains expected bytecode for LDI r0, 42.
    - Bytecode at 0x1000 matches hand-assembled LDI r0, 42; HALT
    _Files: src/vm.rs_
  - [x] `p47.d3.t2` Test: ASMSELF handles invalid assembly gracefully (depends: p47.d1.t3)
    > Pre-fill canvas with garbage text. Execute ASMSELF. Verify RAM[0xFFD] contains 0xFFFFFFFF. Verify VM did not crash and continues executing.
    - Error port set, VM still running
    _Files: src/vm.rs_
  - [x] `p47.d3.t3` Test: program writes code to canvas then assembles it (depends: p47.d1.t3)
    > Full integration test: a program uses STORE to write "LDI r0, 99\nHALT\n" to the canvas address range, calls ASMSELF, then jumps to 0x1000 (or uses RUNNEXT from phase 48). Verify r0 ends up as 99.
    - Self-written program executes correctly after ASMSELF
    _Files: src/vm.rs_
  - [x] ASMSELF assembles and the result runs correctly
    _Validation: Test program: write simple ASM to canvas, ASMSELF, jump to 0x1000, verify behavior_
  _~120 LOC_

### Technical Notes

The assembler and preprocessor are currently called from main.rs. The VM (vm.rs) will need to import them. Since vm.rs is a separate module, this means adding `use crate::assembler;` and `use crate::preprocessor;` to vm.rs.
The canvas-to-text conversion currently lives in the F8 handler in main.rs. It reads 4096 cells, converts each u32 to a char, collapses newlines. This logic needs to be extracted into a shared function. The function should be in a neutral module (preprocessor.rs is a good candidate since it already handles text processing).
ASMSELF takes no operands (1-byte instruction). The assembled bytecode always goes to 0x1000 (CANVAS_BYTECODE_ADDR), same as F8. This means calling ASMSELF overwrites whatever bytecode is currently running. The program should use RUNNEXT (phase 48) to jump to the new bytecode.


### Risks

- ASMSELF during execution replaces the running bytecode -- program must jump to new code carefully
- Preprocessor/assembler errors in a running VM context need careful error handling
- Self-assembly is inherently dangerous (infinite loops, corrupting own code)

## [x] phase-48: Self-Execution Opcode (RUNNEXT) (COMPLETE)

**Goal:** Add an opcode that starts executing the newly assembled bytecode

Add the RUNNEXT opcode that sets PC to 0x1000 (the canvas bytecode region) and continues execution. Combined with ASMSELF, a program can write new code onto the canvas, compile it, and run it -- all from within the VM. This closes the loop: pixels write pixels, pixels assemble pixels, pixels execute pixels.


### Deliverables

- [x] **RUNNEXT opcode implementation** -- New opcode (next available after ASMSELF). When executed: 1. Set PC = CANVAS_BYTECODE_ADDR (0x1000) 2. Reset halted flag 3. Clear any error state 4. Execution continues from the new bytecode on the next fetch cycle
This is essentially JMP 0x1000 but with awareness that the bytecode at 0x1000 was just assembled from canvas text. Could be implemented as a simple PC set, or as JMP with an implicit operand.

  - [x] `p48.d1.t1` Implement RUNNEXT opcode handler (depends: p47.d1.t1)
    > Add RUNNEXT opcode in vm.rs execute match. Handler sets self.pc = CANVAS_BYTECODE_ADDR (0x1000). No operands needed (1-byte instruction). Register file is preserved. The VM continues fetching from the new PC on the next cycle.
    - PC set to 0x1000 after RUNNEXT
    - Execution continues from new bytecode
    _Files: src/vm.rs_
  - [x] `p48.d1.t2` Add RUNNEXT to disassembler and assembler (depends: p48.d1.t1)
    > Add RUNNEXT to the mnemonic list in assembler.rs, the OPCODES list in preprocessor.rs, and the disassemble() method in vm.rs. No operands.
    - RUNNEXT appears in trace output correctly
    - Can type RUNNEXT in assembly source
    _Files: src/vm.rs, src/assembler.rs, src/preprocessor.rs_
  - [x] RUNNEXT starts executing bytecode at 0x1000
    _Validation: Program writes code, ASMSELF, RUNNEXT, verify new code runs_
  - [x] Register state preserved across RUNNEXT
    _Validation: r0-r26 retain their values after RUNNEXT_
  _~40 LOC_
- [x] **Test suite for RUNNEXT** -- Test the full write-compile-execute cycle. A program writes new code, assembles it, runs it, and the new code's effects are visible.

  - [x] `p48.d2.t1` Test: RUNNEXT executes newly assembled code (depends: p47.d1.t3, p48.d1.t1)
    > Write a test program that: (1) stores "LDI r0, 77\nHALT" to canvas addresses, (2) calls ASMSELF, (3) checks RAM[0xFFD] for success, (4) calls RUNNEXT, (5) verify r0 == 77 after execution.
    - r0 == 77 after RUNNEXT
    _Files: src/vm.rs_
  - [x] `p48.d2.t2` Test: registers preserved across RUNNEXT (depends: p48.d1.t1)
    > Set r5 = 12345. Write code to canvas that reads r5 and adds 1. ASMSELF, RUNNEXT. Verify r5 is still 12345 in the new program's context, and that the new program can read it.
    - Register values survive the transition
    _Files: src/vm.rs_
  - [x] `p48.d2.t3` Test: chained self-modification (depends: p48.d1.t1)
    > Program A writes Program B to canvas. ASMSELF. RUNNEXT. Program B writes Program C to canvas. ASMSELF. RUNNEXT. Program C HALTs. Verify all three ran in sequence. This is the generational self-modification test.
    - Three generations of code execute in sequence
    _Files: src/vm.rs_
  - [x] Full write-compile-execute cycle works end-to-end
    _Validation: Test program writes LDI r0, 77 to canvas, ASMSELF, RUNNEXT, verify r0=77_
  _~100 LOC_

### Technical Notes

RUNNEXT is intentionally simple: it just sets PC = 0x1000. The complexity is in ASMSELF (phase 47). RUNNEXT could alternatively be implemented as a JMP to a label at 0x1000, but having a dedicated opcode makes the intent clear and enables future extensions (e.g., RUNNEXT with a timeout, RUNNEXT in a sandboxed context).
Register preservation: RUNNEXT does NOT reset registers. The new program inherits all register state. This is by design -- it allows data passing between program generations. If a clean slate is needed, the new program can zero registers itself.
Stack preservation: the return stack is NOT reset. This means the new program can RET back to the caller if the caller used CALL before RUNNEXT. This is a feature, not a bug -- it enables coroutines.


### Risks

- Infinite self-modification loops (program rewrites itself forever)
- Assembler errors in a running context could leave the VM in a bad state

## [x] phase-49: Self-Modifying Programs: Demos and Patterns (COMPLETE)

**Goal:** Build demonstration programs that showcase the pixel-driving-pixels capability

With phases 45-48 complete, write programs that demonstrate the full self-modifying capability: programs that write their own code, programs whose state IS the display, programs that evolve over time. These demos prove that the pixel-driving-pixels problem is solved.


### Deliverables

- [x] **Demo: Self-writing program** -- A program that writes another program onto the canvas grid using STORE to canvas addresses, calls ASMSELF to compile it, and RUNNEXT to execute it. The generated program is different from the original -- it's a true successor. The human watches text appear on the grid, then sees the new program run.

  - [x] `p49.d1.t1` Write programs/self_writer.asm (depends: p48.d1.t1)
    > A program that uses STORE to canvas addresses (0x8000+) to write "LDI r0, 42\nLDI r1, 1\nADD r0, r1\nHALT\n" onto the grid. The text becomes visible as typed glyphs. Then calls ASMSELF and RUNNEXT. The successor runs and r0 = 43.
    - Text appears on grid before assembly
    - Successor program executes correctly
    _Files: programs/self_writer.asm_
  - [x] `p49.d1.t2` Write programs/evolving_counter.asm (depends: p45.d3.t1)
    > A program that counts frames (via TICKS port 0xFFE) and writes the count as ASCII digits directly onto the canvas grid. The grid becomes a live dashboard. The count digits are the program's visible state -- no separate output. The digit changes each frame. This demonstrates that the grid IS the display.
    - Digits visibly increment on the canvas grid
    _Files: programs/evolving_counter.asm_
  - [x] `p49.d1.t3` Write programs/game_of_life.asm (depends: p46.d1.t3)
    > Conway's Game of Life implemented entirely in Geometry OS assembly. Uses PEEK to read the screen, POKE (or STORE to screen-mapped RAM) to write the next generation. The screen IS the cellular automaton. No Rust code involved in the logic -- pure pixel-driven-pixels. Initialize with a glider or blinker pattern.
    - Cells evolve according to Conway's rules
    - Gliders move, blinkers blink
    _Files: programs/game_of_life.asm_
  - [x] `p49.d1.t4` Write programs/code_evolution.asm (depends: p48.d1.t1)
    > The crown jewel demo. A program that writes increasingly complex versions of itself to the canvas grid. Generation 0 just halts. Generation 1 writes generation 2 which adds a counter. Generation 2 writes generation 3 which adds a screen effect. Each generation writes its successor, compiles, and runs it. The human watches the code evolve on the grid in real time.
    - At least 3 generations of code evolution
    - Each generation visibly different from the last
    _Files: programs/code_evolution.asm_
  - [x] Program generates a visually different successor and runs it
    _Validation: Load demo, F5, watch grid change, see new program execute_
  _~300 LOC_
- [x] **Documentation: pixel-driving-pixels patterns** -- Write a guide for building self-modifying programs. Document the patterns: canvas STORE for writing code, ASMSELF for compiling, RUNNEXT for executing, register passing between generations, and common pitfalls (infinite loops, corrupting your own code).

  - [x] `p49.d2.t1` Write docs/SELF_MODIFYING_GUIDE.md (depends: p48.d1.t1)
    > Create a guide covering: (1) Canvas STORE pattern -- how to write text to canvas cells, (2) ASMSELF + RUNNEXT pattern -- compile and execute, (3) Register passing -- sharing state between generations, (4) Self-reading -- using LOAD from canvas to inspect your own source, (5) Pitfalls -- infinite loops, corruption, error handling. Include code snippets for each pattern.
    - Guide covers all 5 topics with working code examples
    _Files: docs/SELF_MODIFYING_GUIDE.md_
  - [x] Guide document exists with at least 3 documented patterns
    _Validation: ls docs/SELF_MODIFYING_GUIDE.md_
  _~100 LOC_

### Technical Notes

Demo programs should be small enough to fit on the canvas grid (32 columns, 128 rows). Complex programs may need to use the .org directive for layout. The code_evolution demo is the most ambitious -- it may need careful tuning to keep each generation's code within grid size limits.
The game_of_life.asm demo is the purest expression of pixels-driving-pixels. It needs the screen buffer mapping from phase 46 to work optimally, but could also work with just PEEK and PIXEL opcodes.


### Risks

- Demo programs may be too complex to write in raw assembly within grid size limits
- Code evolution demo may be too ambitious for initial implementation

## [x] phase-50: Reactive Canvas: Live Cell Formulas (COMPLETE)

**Goal:** Make canvas cells react to changes in other cells automatically

Extend the canvas with an optional formula layer. A cell can have a formula instead of a static value. When a referenced cell changes, the formula cell recalculates. This is the spreadsheet model applied to the pixel grid. It makes the canvas reactive without explicit STORE/LOAD loops.
This is a future phase -- it depends on phases 45-48 being stable and is a natural evolution of the pixel-driving-pixels concept. Not required for the initial self-modifying capability.


### Deliverables

- [x] **Formula cell type and evaluation engine** -- Add a parallel buffer (formula_buffer, same size as canvas_buffer) where each cell can optionally hold a formula instead of a value. Formulas reference other cells by address. When a STORE writes to a cell, the engine checks if any formula depends on that cell and recalculates.

- [x] **Formula syntax in preprocessor** -- Extend the preprocessor to recognize formula syntax in canvas text. A line like `= r0 + r1` means "this cell displays the value of r0 + r1". The preprocessor generates the reactive update hooks.

- [x] **Demo: live register dashboard** -- A program where the canvas grid shows live register values. As the program runs, the grid cells update to show r0, r1, r2 etc. as changing digits. The display IS the debug view. No separate inspector.


### Technical Notes

This phase explores the spreadsheet model. Each cell can be: - A literal value (current behavior) - A formula that references other cells or registers - A formula that references screen pixels (PEEK-equivalent)
The dependency graph needs cycle detection to prevent infinite recalculation. Simple approach: single-pass topological sort of formula dependencies, recalculate in order after any STORE to the canvas.
This is marked "future" because it's a significant new feature. The core pixel-driving-pixels capability (phases 45-48) does not require this.


## Global Risks

- Opcode space: 101 of ~256 slots used, plenty of room
- Scope creep -- adding features is easy, keeping the OS coherent is hard
- Kernel boundary breaks existing programs -- need a compatibility mode
- Memory protection removes shared RAM -- IPC now in place (Phase 27), window_manager tests passing
- Filesystem persistence needs host directory -- WASM port needs different backing
- Phase 24 memory protection resolved: page tables + segfaults working, IPC replaces shared-RAM for multiprocess
- Phase 28 device drivers: IOCTL opcode 0x62, 4 device files at fds 0xE000-0xE003
- Self-modifying code is inherently hard to debug -- need good error reporting
- Assembly inside a running VM may be slow for large programs -- may need optimization
- The concept of a program rewriting itself challenges test design -- how do you unit test a program that changes?
- RAM size expansion (phase 46) affects the fuzzer which generates random addresses

## Conventions

- Every new opcode gets a test in tests/program_tests.rs
- Every new program gets assembled by test_all_programs_assemble
- README.md updated when opcodes or features change
- roadmap.yaml is the single source of truth for project state
- Semantic versioning: minor bump for new opcodes, patch for fixes
- New opcodes need a program that needs them (no speculative opcodes)
- All new opcodes added to assembler.rs, preprocessor.rs OPCODES list, and vm.rs disassembler
- Opcode numbers assigned sequentially from next available
- Canvas and screen mappings use LOAD/STORE interception, not new opcodes
- ASMSELF and RUNNEXT take no operands (1-byte instructions)
- Error reporting via RAM[0xFFD] (existing ASM result port)
