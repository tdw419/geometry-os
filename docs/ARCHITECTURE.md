# Geometry OS Architecture

System-level documentation for the full Geometry OS stack.
Read alongside CANVAS_TEXT_SURFACE.md (editor/assembly pipeline) and
SIGNED_ARITHMETIC.md (arithmetic semantics).

---

## Full Opcode Reference (77 opcodes)

### Control Flow
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x00 | HALT     |      | Stop execution |
| 0x01 | NOP      |      | No operation |
| 0x02 | FRAME    |      | Yield to renderer, increment TICKS |
| 0x03 | BEEP     | freq_reg, dur_reg | Sine-wave tone (20-20000 Hz, 1-5000 ms) |

### Data Movement
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x10 | LDI      | reg, imm | Load immediate |
| 0x11 | LOAD     | reg, [reg] | Load from memory |
| 0x12 | STORE    | [reg], reg | Store to memory |
| 0x51 | MOV      | rd, rs | Register copy |

### Arithmetic
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x20 | ADD      | rd, rs | rd = rd + rs |
| 0x21 | SUB      | rd, rs | rd = rd - rs |
| 0x22 | MUL      | rd, rs | rd = rd * rs |
| 0x23 | DIV      | rd, rs | rd = rd / rs |
| 0x24 | AND      | rd, rs | Bitwise AND |
| 0x25 | OR       | rd, rs | Bitwise OR |
| 0x26 | XOR      | rd, rs | Bitwise XOR |
| 0x27 | SHL      | rd, rs | Shift left |
| 0x28 | SHR      | rd, rs | Shift right |
| 0x29 | MOD      | rd, rs | Modulo |
| 0x2A | NEG      | rd     | Two's complement negation |
| 0x2B | SAR      | rd, rs | Arithmetic shift right (sign-preserving) |

### Compare & Branches
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x50 | CMP      | rd, rs | Compare: r0 = -1/0/1 (lt/eq/gt) |
| 0x30 | JMP      | addr  | Unconditional jump |
| 0x31 | JZ       | reg, addr | Jump if zero |
| 0x32 | JNZ      | reg, addr | Jump if not zero |
| 0x33 | CALL     | addr  | Subroutine call (return addr in r31) |
| 0x34 | RET      |       | Return from subroutine |
| 0x35 | BLT      | reg, addr | Branch if r0 < 0 (after CMP) |
| 0x36 | BGE      | reg, addr | Branch if r0 >= 0 (after CMP) |

### Graphics
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x40 | PSET     | xr, yr, cr | Set pixel (registers) |
| 0x41 | PSETI    | x, y, c | Set pixel (immediates) |
| 0x42 | FILL     | cr     | Fill screen with color |
| 0x43 | RECTF    | xr,yr,wr,hr,cr | Filled rectangle |
| 0x44 | TEXT     | xr, yr, ar | Draw null-terminated string from RAM |
| 0x45 | LINE     | x0r,y0r,x1r,y1r,cr | Bresenham line |
| 0x46 | CIRCLE   | xr, yr, rr, cr | Midpoint circle |
| 0x47 | SCROLL   | nr     | Scroll screen up by N pixels |
| 0x4A | SPRITE   | xr,yr,ar,wr,hr | Blit NxM sprite from RAM (0=transparent) |
| 0x4C | TILEMAP  | xr,yr,mr,tr,gwr,ghr,twr,thr | Grid blit from tile index array |
| 0x4F | PEEK     | rx, ry, rd | Read screen pixel at (rx,ry) into rd |

### Stack & I/O
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x60 | PUSH     | reg   | Push to stack (r30 = SP) |
| 0x61 | POP      | reg   | Pop from stack |
| 0x48 | IKEY     | reg   | Read keyboard port, clear it |
| 0x49 | RAND     | reg   | Pseudo-random u32 (LCG, seed 0xDEADBEEF) |

### Meta-Programming
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x4B | ASM      | src_reg, dest_reg | Assemble source text from RAM, write bytecode to RAM |

### Multi-Process
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x4D | SPAWN    | addr_reg | Create child process at address, PID in RAM[0xFFA] |
| 0x4E | KILL     | pid_reg | Terminate child process by PID |

### Kernel Mode (Syscalls)
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x52 | SYSCALL  |       | Trap into kernel mode, dispatch by number in r0 |
| 0x53 | RETK     |       | Return from kernel mode to user mode |

### Filesystem
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x54 | OPEN     | path_reg, mode_reg | Open file, fd in r0 |
| 0x55 | READ     | fd_reg, buf_reg, count_reg | Read from file into RAM |
| 0x56 | WRITE    | fd_reg, buf_reg, count_reg | Write from RAM to file |
| 0x57 | CLOSE    | fd_reg | Close file descriptor |
| 0x58 | SEEK     | fd_reg, offset_reg | Seek in file |
| 0x59 | LS       | buf_reg | Directory listing into RAM buffer |

### Process Management
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x5A | YIELD    |       | Voluntary context switch |
| 0x5B | SLEEP    | ticks_reg | Sleep for N frames |
| 0x5C | SETPRIORITY | prio_reg | Set process priority (0-3) |
| 0x65 | GETPID   |       | Get current process ID, result in r0 |
| 0x66 | EXEC     | addr_reg | Execute program at address (in-kernel) |
| 0x6F | EXIT     | status_reg | Exit process with status code |

### Inter-Process Communication
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x5D | PIPE     | r5, r6 | Create pipe: read FD in r5, write FD in r6 |
| 0x5E | MSGSND   | pid_reg | Send 4-word message to process |
| 0x5F | MSGRCV   |       | Receive message, sender PID in r0 |

### Device I/O
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x62 | IOCTL    | fd_reg, cmd_reg, val_reg | Device-specific control |
| 0x63 | GETENV   | key_reg, buf_reg | Read environment variable |
| 0x64 | SETENV   | key_reg, val_reg | Set environment variable |
| 0x67 | WRITESTR | fd_reg, buf_reg | Write null-terminated string |
| 0x68 | READLN   | buf_reg | Read keyboard line into buffer |
| 0x6D | SCREENP  | xr, yr, cr | Draw pixel to screen via fd |

### Shell & Execution
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x69 | WAITPID  | pid_reg | Wait for child process, exit code in r0 |
| 0x6A | EXECP    | addr_reg, stdin_fd, stdout_fd | Spawn with fd redirection |
| 0x6B | CHDIR    | path_reg | Change working directory |
| 0x6C | GETCWD   | buf_reg | Get current working directory |

### System
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x6E | SHUTDOWN |       | Halt all processes, flush filesystem |
| 0x70 | SIGNAL   | pid_reg, sig_reg | Send signal to process |
| 0x71 | SIGSET   | sig_reg, handler_reg | Register signal handler |

### Hypervisor
| Hex  | Mnemonic | Args | Description |
|------|----------|------|-------------|
| 0x72 | HYPERVISOR | config_reg | Read config from RAM, spawn guest OS |

---

## Memory Map

```
Address         Size     Purpose
──────────────────────────────────────────────────────────────
0x000-0x3FF     1024     Canvas grid (legacy, separate buffer in TEXT mode)
0x400-0xEFF     ~4K      Multi-process bytecode (via .org directive)
0xF00-0xF03     4        Window Bounds Protocol (win_x, win_y, win_w, win_h)
0x1000-0x1FFF   4096     Canvas bytecode output (F8 assembles here)
0x2000-0xFFA    ~60K     General purpose RAM
0xFFB           1        Key bitmask port (bits 0-5, read-only)
0xFFC           1        Network port (UDP)
0xFFD           1        ASM result port (word count or 0xFFFFFFFF on error)
0xFFE           1        TICKS port (frame counter, read-only)
0xFFF           1        Keyboard port (cleared on IKEY read)
──────────────────────────────────────────────────────────────
Total: 65536 (0x10000) u32 cells
```

---

## Kernel Mode Architecture

Geometry OS has two execution modes:

- **Kernel mode**: Full access to all opcodes, hardware ports, and system resources.
- **User mode**: Restricted -- cannot directly access hardware ports (0xFFF, 0xFFB, etc.) or use privileged opcodes.

Programs spawned via `SPAWN` start in user mode. The `SYSCALL` opcode traps into kernel mode, dispatches based on the syscall number in r0, and `RETK` returns to user mode.

### Syscall Convention

1. Set r0 to syscall number
2. Set argument registers as needed
3. Execute `SYSCALL`
4. Kernel handler runs, sets r0 to return value
5. Execute `RETK` to return to user mode

---

## Multi-Process Architecture

Geometry OS supports up to 8 concurrent processes sharing the same 64K RAM.

### Scheduler

Processes are scheduled with a priority-based preemptive scheduler:
- **Priority levels**: 0 (lowest) to 3 (highest)
- **Timer interrupt**: Fires every N instructions, triggers context switch
- **YIELD**: Voluntary context switch
- **SLEEP**: Timed sleep, process wakes after N frames
- **Blocking I/O**: Processes block on empty pipe reads or message receives

### Process Lifecycle

- **SPAWN** (0x4D): Creates a child process with its own register file, page table, and fd table. PID stored in RAM[0xFFA].
- **KILL** (0x4E): Terminates a child by PID.
- **EXIT** (0x6F): Exits current process with status code, becomes zombie.
- **WAITPID** (0x69): Parent reaps zombie, gets exit code.
- **SIGNAL** (0x70) / **SIGSET** (0x71): POSIX-like signal handling.

### Window Bounds Protocol

For spatial coordination between processes, RAM[0xF00..0xF03] is a shared convention:

| Address | Field | Who Writes |
|---------|-------|------------|
| 0xF00   | win_x | Primary |
| 0xF01   | win_y | Primary |
| 0xF02   | win_w | Primary |
| 0xF03   | win_h | Primary |

### Multi-Process Assembly

Use `.org <addr>` in a single assembly file to place child process code:

```
  LDI r0, child
  SPAWN r0          ; spawn child at label
  ; ... primary loop ...

.org 0x400
child:
  ; ... child process code ...
```

---

## Memory Protection

Each process gets its own page table mapping virtual addresses to physical RAM.

- **Kernel mode**: Identity mapping (no translation)
- **User mode**: Page table translation via 1-level paging
- **SEGFAULT**: Access to unmapped page halts the offending process
- **RAM[0xFF9]**: Tracks which PID caused the last segfault

Each child process receives 4 private physical pages. Shared regions (page 3, page 63) are identity-mapped for inter-process communication.

See `docs/MEMORY_PROTECTION.md` for full details.

---

## Virtual Filesystem (VFS)

Programs access files through syscall opcodes. Backed by the host filesystem at `.geometry_os/fs/`.

| Opcode | Syscall | Description |
|--------|---------|-------------|
| 0x54   | OPEN    | Open file, returns fd |
| 0x55   | READ    | Read bytes into RAM |
| 0x56   | WRITE   | Write bytes from RAM |
| 0x57   | CLOSE   | Close file descriptor |
| 0x58   | SEEK    | Seek to offset |
| 0x59   | LS      | Directory listing |

Each process has up to 16 open file descriptors.

### Device Files

Hardware accessed through the filesystem interface:

| Path          | FD        | Description |
|---------------|-----------|-------------|
| /dev/screen   | 0xE000    | Screen pixel output |
| /dev/keyboard | 0xE001    | Keyboard input |
| /dev/audio    | 0xE002    | Audio output |
| /dev/net      | 0xE003    | Network (UDP) |

### IOCTL (0x62)

Device-specific control operations:
- **Screen**: cmd 0 = get width, cmd 1 = get height
- **Keyboard**: cmd 0 = get echo mode, cmd 1 = set echo mode
- **Audio**: cmd 0 = get volume, cmd 1 = set volume
- **Net**: cmd 0 = get status

---

## Inter-Process Communication

### Pipes (0x5D)

Unidirectional byte streams with circular buffer (256 words). Created with `PIPE r5, r6` which returns read FD (0x8000|idx) and write FD (0xC000|idx).

### Messages (0x5E/0x5F)

Fixed-size (4-word) messages between processes. `MSGSND` sends to target PID, `MSGRCV` receives and returns sender PID. Per-process message queue holds 16 messages. `MSGRCV` blocks if no message is queued.

---

## Shell

`shell.asm` is an interactive command interpreter running as a user process. It supports:
- **Built-in commands**: ls, cd, cat, echo, ps, kill, help, pwd, clear, exit
- **Pipe operator**: `prog1 | prog2` connects stdout to stdin
- **Redirection**: `prog > file`, `prog < file`, `prog >> file`
- **Environment variables**: SHELL, HOME, CWD, USER set by init

---

## Boot Sequence

1. VM initializes hardware (screen, keyboard, timer)
2. Boot ROM assembles `init.asm` from boot.cfg
3. Init process (PID 1) spawns with priority 2
4. Init reads boot.cfg, sets environment variables
5. Init spawns shell process
6. Init enters supervisor loop, respawns shell if it dies
7. `SHUTDOWN` (0x6E) halts all processes, flushes filesystem

---

## Hypervisor

Geometry OS has two hypervisor modes for running guest operating systems:

### QEMU Bridge (Phase 33)

Spawns QEMU as a subprocess, pipes serial console I/O through the canvas text surface.
Supports any QEMU architecture: riscv64, x86_64, aarch64, mipsel.

- ANSI escape sequence parsing for terminal rendering
- Keyboard forwarding to QEMU stdin
- Auto-scrolling canvas output

### Native RISC-V Interpreter (Phases 34-37)

Pure Rust RISC-V RV32I interpreter with no external dependencies.

**RISC-V module** (`src/riscv/`):
- `mod.rs` -- public interface, bridge integration
- `cpu.rs` -- register file, instruction execute, privilege modes, CSRs
- `memory.rs` -- guest RAM (up to 128MB), load/store
- `decode.rs` -- instruction decode for all RV32I opcodes
- `mmu.rs` -- SV32 page table walk, TLB cache, page fault traps
- `uart.rs` -- UART 16550 serial port emulation
- `clint.rs` -- CLINT timer interrupt controller
- `plic.rs` -- PLIC platform interrupt controller
- `virtio_blk.rs` -- Virtio MMIO block device
- `dtb.rs` -- Device Tree Blob generation

**Features**:
- 40 RV32I base instructions
- M/S/U privilege modes with ECALL/MRET/SRET
- CSR register bank (mstatus, mtvec, mepc, sstatus, stvec, satp, etc.)
- SV32 2-level page table walk with 64-entry ASID-aware TLB
- Timer and software interrupts
- ELF and raw binary kernel loader
- Device tree blob generation for guest kernel boot

---

## Standard Library

Located in `lib/`:
- `lib/stdlib.asm` -- String operations, memory operations, formatted I/O
- `lib/math.asm` -- sin, cos, sqrt via lookup tables
- `lib/heap.asm` -- malloc/free dynamic memory allocator

Loaded via `.include` directive in the assembler.

---

## Preprocessor (Abstraction Layer)

The preprocessor (`preprocessor.rs`) sits between the canvas text and the assembler.

### Macros

| Macro | Syntax | Expansion | Temp Registers |
|-------|--------|-----------|----------------|
| VAR   | `VAR name addr` | Defines variable | none |
| SET   | `SET var, val` | LDI r28, val / LDI r29, addr / STORE r29, r28 | r28, r29 |
| GET   | `GET reg, var` | LDI r29, addr / LOAD reg, r29 | r29 |
| INC   | `INC var` | LDI r29, addr / LOAD r28, r29 / ADD r28, r27 / STORE r29, r28 | r27, r28, r29 |
| DEC   | `DEC var` | LDI r29, addr / LOAD r28, r29 / SUB r28, r27 / STORE r29, r28 | r27, r28, r29 |

### #define Constants

```
#define TILE 8
#define MAX_X 255
```

Replaced before instruction parsing. Works in immediate contexts.

---

## VM Instrumentation

### Access Log Buffer

Tracks LOAD, STORE, SPRITE, and TILEMAP memory accesses per frame. Each access records the RAM address and type (read/write). Consumed by the visual debugger overlay.

### Instruction Fetch Logging

Every PC value is logged to a circular buffer. Used by the visual debugger to trace execution flow.

---

## Visual Debugger

### Memory Heatmap

Compact 256x256 view of the entire 64K RAM. Each pixel represents one word.

- **Cyan**: Recent read
- **Magenta**: Recent write
- **White**: Current PC position

Intensity decay fades highlights over ~10 frames.

### Canvas Cell Tinting

Active RAM addresses flash with colored borders on the canvas grid.

### PC Trail

Fading white glow follows the program counter, showing execution path.

### RAM Inspector Panel

Second 32x32 grid at the bottom of the window visualizes a scrollable region of RAM. PageUp/PageDown in Terminal mode scrolls through different regions.

---

## Audio

The BEEP opcode generates sine-wave tones via `aplay` (Linux). Requires `libasound2-dev`.
- Frequency: 20-20000 Hz
- Duration: 1-5000 ms

---

## Platform Ports

### WASM (Web)

Compiles to WebAssembly via `wasm-pack`. Full opcode set works in WASM mode.

```bash
cd wasm && wasm-pack build --target web
```

### Network (UDP)

RAM[0xFFC] is a bidirectional network port. Two VM instances exchange messages via UDP.

### GlyphLang Backend

`src/glyph_backend.rs` compiles GlyphLang source to Geometry OS bytecode.

---

## Build & Run

```bash
# GUI mode
cargo run --release

# CLI mode (headless)
cargo run --release -- --cli

# WASM build
cd wasm && wasm-pack build --target web

# Run tests
cargo test
```

### Key Bindings (GUI)

| Key | Action |
|-----|--------|
| F5  | Run / resume |
| F6  | Single-step |
| F7  | Save state |
| F8  | Assemble canvas text |
| Ctrl+F8 | Load .asm file |
| F9  | Screenshot (PNG) |
| F10 | Toggle frame capture |
| Escape | Toggle editor/terminal |

### CLI Commands

`help`, `load <name>`, `run`, `step`, `regs`, `peek <addr>`, `poke <addr> <val>`,
`bp [addr]`, `bpc`, `trace [n]`, `screenshot`, `save [slot]`, `load-slot [slot]`,
`reset`, `quit`

---

## Stats

- 10,023 lines of Rust (core VM, assembler, main, preprocessor, font, glyph backend, QEMU bridge)
- 5,739 lines of Rust (RISC-V interpreter)
- 77 opcodes (Geometry OS VM)
- 40 programs + 5 library modules
- 697 tests
- 15,762 total LOC
