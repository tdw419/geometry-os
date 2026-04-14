# Geometry OS Roadmap

Pixel-art virtual machine with built-in assembler, debugger, and live GUI.\n  114 opcodes, 32 registers, 64K RAM, 256x256 framebuffer. Write assembly in\n  the built-in text editor, press F5,  watch it run.

**Progress:** 39/44 phases complete, 3 in progress

**Deliverables:** 185/194 complete

**Tasks:** 34/43 complete

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
| phase-41 Tracing and Instrumentation | IN PROGRESS | 2/4 | - | - |
| phase-42 Geometry OS Process Manager | IN PROGRESS | 3/3 | - | - |
| phase-43 Geometry OS VFS and Disk | PLANNED | 0/2 | - | - |
| phase-44 Geometry OS Memory Management | PLANNED | 0/3 | - | - |

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

## [~] phase-41: Tracing and Instrumentation (IN PROGRESS)

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
- [ ] **Page table trace** -- Trace SV32 page table walks, TLB fills, and page faults
  - [ ] `p41.d3.t1` Add page table walk tracing to MMU
    - Logs every SATP write (new page table root)
    - Logs page table walks with VPN to PFN mappings
    - Logs page faults with faulting VA and reason
  _~80 LOC_
- [ ] **Scheduler trace** -- Detect and log context switches and schedule decisions
  - [ ] `p41.d4.t1` Infer context switches from register state changes
    - Detects task switches via SP/mhartid changes
    - Logs switch_from to switch_to with PC and SP
  _~60 LOC_

## [~] phase-42: Geometry OS Process Manager (IN PROGRESS)

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

## [ ] phase-43: Geometry OS VFS and Disk (PLANNED)

**Goal:** Build a virtual filesystem layer based on observed Linux VFS patterns.

Trace Linux VFS operations during boot and build Geometry OS equivalents.

### Deliverables

- [ ] **Inode filesystem** -- In-memory inode-based filesystem with directory tree
  - [ ] `p43.d1.t1` Implement inode structures and directory operations
    - {'Inode types': 'regular file, directory, device, pipe'}
    - Path resolution and read/write with offset tracking
  _~300 LOC_
- [ ] **File descriptor table** -- Per-process fd table with pipe support
  - [ ] `p43.d2.t1` Implement fd table with open/close/dup2/pipe
    - stdin/stdout/stderr per process
    - pipe creates connected read/write fds
    - dup2 for shell redirects
  _~100 LOC_

## [ ] phase-44: Geometry OS Memory Management (PLANNED)

**Goal:** Rebuild Geometry OS memory management based on observed Linux SV32 paging.

Trace Linux page table setup during boot and build Geometry OS equivalents.

### Deliverables

- [ ] **Page allocator** -- Physical page allocator for 4KB pages
  - [ ] `p44.d1.t1` Implement physical page allocator
    - Allocates/frees 4KB pages
    - Tracks used/free pages
  _~150 LOC_
- [ ] **Virtual memory areas** -- Per-process VMA list for code, heap, stack, mmap
  - [ ] `p44.d2.t1` Implement VMA tracking and page fault handler
    - VMA list per process
    - Page fault allocates on demand
    - Stack grows downward, heap via brk
  _~150 LOC_
- [ ] **Copy-on-write fork** -- Fork shares physical pages, copies only on write
  - [ ] `p44.d3.t1` Implement COW fork based on observed Linux fork behavior
    - fork marks pages read-only in child
    - Write fault copies page
    - Reference counting on physical pages
  _~100 LOC_

## Global Risks

- Opcode space: 114 of ~256 slots used, plenty of room
- Scope creep -- adding features is easy, keeping the OS coherent is hard
- Kernel boundary breaks existing programs -- need a compatibility mode
- Memory protection removes shared RAM -- IPC now in place (Phase 27), window_manager tests passing
- Filesystem persistence needs host directory -- WASM port needs different backing
- Phase 24 memory protection resolved: page tables + segfaults working, IPC replaces shared-RAM for multiprocess
- Phase 28 device drivers: IOCTL opcode 0x62, 4 device files at fds 0xE000-0xE003

## Conventions

- Every new opcode gets a test in tests/program_tests.rs
- Every new program gets assembled by test_all_programs_assemble
- README.md updated when opcodes or features change
- roadmap.yaml is the single source of truth for project state
- Semantic versioning: minor bump for new opcodes, patch for fixes
- New opcodes need a program that needs them (no speculative opcodes)
