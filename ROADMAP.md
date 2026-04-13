# Geometry OS Roadmap

**Ultimate Goal:** Build Geometry OS to modern OS standards -- a real operating system with memory protection, a filesystem, proper scheduling, IPC, device abstraction, and a shell. Linux/Windows/macOS started somewhere. This is our somewhere.

v1.0.0 shipped 22 phases of VM construction. Now the real work begins.

## Current State

- 61 opcodes, 32 registers, 64K RAM, 256x256 framebuffer
- 167 tests (all passing, 2 ignored), 37 programs, all green
- Multi-process (SPAWN/KILL), shared RAM, Window Bounds Protocol
- Self-hosting assembler, browser port (WASM), network port (UDP)
- Visual debugger, heatmaps, RAM inspector
- PEEK (screen readback), MOV, CMP/BLT/BGE
- Text editor (F8/F5 assemble-and-run workflow)
- Kernel boundary (SYSCALL/RETK, user/kernel mode, restricted opcodes)
- Memory protection (page tables, address space per process, SEGFAULT)
- Filesystem (VFS, OPEN/READ/WRITE/CLOSE/SEEK/LS, per-process fd table, cat.asm)
- Preemptive scheduler (priority levels 0-3, time quantum, YIELD/SLEEP/SETPRIORITY opcodes)
- IPC (PIPE/MSGSND/MSGRCV opcodes, blocking I/O, per-process message queues, pipe_test.asm)

**What's missing for a real OS:**
- ~~No memory protection (any process can trash any RAM)~~ (Phase 24 done)
- ~~No syscall boundary (programs directly access hardware)~~ (Phase 23 done)
- ~~No proper scheduler (round-robin single-step, no priorities)~~ (Phase 26 done)
- ~~No IPC beyond shared RAM (no pipes, no messages)~~ (Phase 27 done)
- No device driver model (hardware ports are hardcoded)
- No shell (just a REPL, no pipes/redirection)
- No init/boot sequence (hardcoded startup)
- No standard library for programs

---

## The Road to a Real OS

### Phase 23: Kernel Boundary (Syscall Mode)

**Goal:** Establish user mode vs kernel mode. Programs can't directly access hardware -- they go through syscalls.

| Deliverable | Description | Scope |
|---|---|---|
| CPU mode flag | `vm.mode: User/Kernel` bit in VM state | ~20 lines vm.rs |
| SYSCALL opcode (0x52) | `SYSCALL number` -- trap into kernel mode, dispatch by number | ~40 lines vm.rs |
| RETK opcode (0x53) | Return from kernel mode to user mode | ~10 lines vm.rs |
| Syscall table | RAM region (0xFE00..0xFEFF) mapping syscall numbers to kernel entry points | ~30 lines vm.rs |
| Restricted opcodes in user mode | IKEY, PEEK (hardware ports), STORE to 0xFF00+ blocked in user mode | ~30 lines vm.rs |
| Kernel call convention | Document: r0=syscall#, r1-r5=args, r0=return value | docs/ |

**Why first:** Every modern OS starts here. Without a kernel boundary, there's no protection, no stability, no trust. Programs that crash should crash themselves, not the system.

---

### Phase 24: Memory Protection

**Goal:** Each process gets its own address space. A process can't read/write another process's memory.

| Deliverable | Description | Scope |
|---|---|---|
| Page tables | Simple 1-level paging: page_dir in each process, maps virtual→physical | ~80 lines vm.rs |
| Address space per process | SPAWN creates a new page table, not just new registers | ~60 lines vm.rs |
| SEGFAULT on illegal access | LOAD/STORE to unmapped page halts the process with an error | ~30 lines vm.rs |
| Process memory regions | Each process gets code segment, heap segment, stack segment | docs/ |
| Memory test | Two processes, one tries to write to the other's memory, gets SEGFAULT | ~40 lines tests |

**Why here:** Right after the kernel boundary. Without memory protection, a buggy program takes down the whole system. This is what separates an OS from a bare-metal monitor.

---

### Phase 25: Filesystem

**Goal:** Programs can create, read, write, and delete named files. Persistent storage across reboots.

| Deliverable | Description | Scope |
|---|---|---|
| Virtual filesystem (VFS) layer | Abstract filesystem interface in the host (Rust) | ~100 lines new fs.rs |
| Host-backed storage | Files stored on the host filesystem in a `.geometry_os/fs/` directory | ~60 lines fs.rs |
| OPEN syscall | `OPEN path_reg, mode_reg` -- returns file descriptor | ~40 lines vm.rs |
| READ syscall | `READ fd_reg, buf_addr_reg, len_reg` -- reads bytes into RAM | ~40 lines vm.rs |
| WRITE syscall | `WRITE fd_reg, buf_addr_reg, len_reg` -- writes bytes from RAM | ~40 lines vm.rs |
| CLOSE syscall | `CLOSE fd_reg` -- release file descriptor | ~20 lines vm.rs |
| SEEK syscall | `SEEK fd_reg, offset_reg` -- set file position | ~20 lines vm.rs |
| Directory listing | LS syscall returns directory entries into a RAM buffer | ~40 lines vm.rs |
| File descriptor table | Per-process fd table (max 16 open files) | ~30 lines vm.rs |
| File test program | `cat.asm` -- reads a file and displays it on screen | programs/cat.asm |

**Why here:** After memory protection, programs need persistent storage. A filesystem is what turns a demo into a platform. Without files, programs can't save data, share data, or load configuration.

---

### Phase 26: Preemptive Scheduler

**Goal:** Replace the naive round-robin single-step with a proper time-sliced scheduler with priorities.

| Deliverable | Description | Scope |
|---|---|---|
| Timer interrupt | VM fires a timer tick every N instructions, triggers context switch | ~40 lines vm.rs |
| Priority levels | Each process has a priority (0-3). Higher priority gets more time slices | ~30 lines vm.rs |
| Time quantum | Each process gets N instructions per slice before preemption | ~30 lines vm.rs |
| Yield syscall | Process can voluntarily yield remaining time slice | ~10 lines vm.rs |
| Sleep syscall | Process sleeps for N frames, removed from run queue until then | ~30 lines vm.rs |
| Scheduler test | Three processes with different priorities, verify higher priority runs more | ~40 lines tests |

**Why here:** The current scheduler is cooperative (single-step round-robin). Real OSes are preemptive. A runaway process shouldn't starve everything else.

---

### Phase 27: Inter-Process Communication

**Goal:** Processes can communicate through proper channels, not just raw shared RAM.

| Deliverable | Description | Scope |
|---|---|---|
| PIPE syscall | `PIPE read_fd_reg, write_fd_reg` -- create a unidirectional pipe | ~60 lines vm.rs |
| Pipe buffer | Circular buffer in kernel memory, 256 words per pipe | ~40 lines vm.rs |
| Blocking reads | READ on empty pipe blocks the process until data arrives | ~30 lines vm.rs |
| MSGSND syscall | Send a fixed-size message to a process by PID | ~40 lines vm.rs |
| MSGRCV syscall | Receive a message (blocks if none pending) | ~40 lines vm.rs |
| Message queue | Per-process queue of pending messages (max 16) | ~30 lines vm.rs |
| Pipe test | `pipe_test.asm` -- one process writes, another reads through a pipe | programs/ |

**Why here:** With memory protection (Phase 24), shared RAM goes away. Processes need a proper way to talk. Pipes and messages are the UNIX way.

---

### Phase 28: Device Driver Abstraction

**Goal:** All hardware access goes through a uniform driver interface. Programs don't know or care what device they're talking to.

| Deliverable | Description | Scope |
|---|---|---|
| Device file convention | Everything is a file: `/dev/screen`, `/dev/keyboard`, `/dev/audio` | docs/ |
| Driver registration | Kernel maintains a driver table mapping device names to handler functions | ~60 lines vm.rs |
| IOCTL syscall | `IOCTL fd_reg, cmd_reg, arg_reg` -- device-specific control operations | ~30 lines vm.rs |
| Screen driver | `/dev/screen` -- write pixels, read dimensions, set mode | ~40 lines drivers/ |
| Keyboard driver | `/dev/keyboard` -- read key events as a stream | ~30 lines drivers/ |
| Audio driver | `/dev/audio` -- write frequency/duration pairs | ~30 lines drivers/ |
| Network driver | `/dev/net` -- read/write UDP packets (wraps existing 0xFFC port) | ~30 lines drivers/ |

**Why here:** After filesystem + IPC, we have the pieces to make hardware access uniform. "Everything is a file" is the UNIX philosophy and it's proven for 50 years.

---

### Phase 29: Shell

**Goal:** A proper command shell with pipes, redirection, environment variables, and job control.

| Deliverable | Description | Scope |
|---|---|---|
| Shell process | `shell.asm` -- interactive command interpreter running as a user process | ~200 lines programs/shell.asm |
| Command parsing | Parse `cmd arg1 arg2` format from keyboard input | ~80 lines |
| Pipe operator | `prog1 | prog2` -- connect stdout of prog1 to stdin of prog2 | ~60 lines |
| Redirection | `prog > file`, `prog < file`, `prog >> file` | ~40 lines |
| Environment variables | GETENV/SETENV syscalls, inherited by child processes | ~40 lines vm.rs |
| Path resolution | Search PATH for executables, resolve relative paths | ~50 lines |
| Built-in commands | `ls`, `cd`, `cat`, `echo`, `ps`, `kill`, `help` | programs/ |

**Why here:** The shell is the user's primary interface to the OS. Without it, you have a bunch of system calls but no way to compose them interactively.

---

### Phase 30: Boot Sequence & Init

**Goal:** The OS boots into a known state, starts an init process, and manages system services.

| Deliverable | Description | Scope |
|---|---|---|
| Boot ROM | Fixed bytecode at 0x0000 that initializes hardware and jumps to init | ~40 lines |
| Init process | First user process (PID 1), reads config, starts shell | programs/init.asm |
| Boot configuration | `boot.cfg` in filesystem -- defines init program, default shell, services | ~30 lines |
| Service manager | Init can spawn background services (drivers, daemons) | docs/ |
| Graceful shutdown | SHUTDOWN syscall cleanly stops all processes, flushes filesystem | ~30 lines vm.rs |

**Why here:** Every real OS has a boot sequence. It's the difference between "I typed a command and it ran" and "I turned on the machine and it's ready."

---

### Phase 31: Standard Library

**Goal:** A reusable library of common operations that all programs can link against.

| Deliverable | Description | Scope |
|---|---|---|
| lib/stdlib.asm | String operations (strlen, strcmp, strcpy, strcat) | ~60 lines |
| lib/math.asm | sin, cos, sqrt, abs using lookup tables or iterative methods | ~80 lines |
| lib/stdio.asm | printf-like formatted output to screen or file descriptor | ~100 lines |
| lib/stdlib.asm (cont.) | malloc/free using a heap region (simple bump or free-list allocator) | ~80 lines |
| lib/time.asm | Get current tick, convert to seconds, delay for N ms | ~40 lines |
| Linking convention | `.include "lib/stdlib.asm"` or `.lib stdlib` directive in assembler | ~40 lines assembler.rs |
| Library test | Program that uses 3+ library functions | programs/ |

**Why here:** Programs shouldn't reinvent string comparison every time. A standard library raises the floor for what programs can do.

---

### Phase 32: Signal Handling & Process Lifecycle

**Goal:** Processes can receive signals, handle errors gracefully, and have a proper lifecycle.

| Deliverable | Description | Scope |
|---|---|---|
| SIGNAL syscall | Send a signal to a process by PID | ~40 lines vm.rs |
| Signal handler registration | Process sets a handler address for each signal type | ~30 lines vm.rs |
| Signal types | SIGTERM (terminate), SIGSEGV (bad memory), SIGKILL (unblockable kill), SIGUSR (user-defined) | docs/ |
| WAIT syscall | Parent waits for child to exit, gets exit code | ~30 lines vm.rs |
| EXIT syscall | Process exits with a status code | ~20 lines vm.rs |
| Zombie process cleanup | Exited processes stay as zombies until parent calls WAIT | ~30 lines vm.rs |
| Fork-like mechanism | Process can duplicate itself (or equivalent: spawn self with shared state) | ~50 lines vm.rs |

**Why here:** Signals are how the OS tells a process "something happened." Without them, you can't gracefully shut down, handle errors, or coordinate parent-child relationships.

---

## Priority Order

1. Phase 23 (Kernel Boundary) -- foundation for everything
2. Phase 24 (Memory Protection) -- can't have an OS without it
3. Phase 25 (Filesystem) -- programs need persistent storage
4. Phase 26 (Preemptive Scheduler) -- stability under load
5. Phase 27 (IPC) -- processes need to talk after memory protection removes shared RAM
6. Phase 28 (Device Drivers) -- uniform hardware access
7. Phase 29 (Shell) -- user interface to the OS
8. Phase 30 (Boot/Init) -- proper startup sequence
9. Phase 31 (Standard Library) -- raise the programming floor
10. Phase 32 (Signals/Lifecycle) -- production-grade process management

---

## Priority Order for Automated Development

- [x] Phase 23: Kernel Boundary -- CPU mode flag, SYSCALL opcode (0x52), RETK opcode (0x53), syscall dispatch table, restricted opcodes in user mode
- [x] Phase 24: Memory Protection -- page tables, address space per process, SEGFAULT on illegal access
- [x] Phase 25: Filesystem -- VFS layer, OPEN/READ/WRITE/CLOSE/SEEK syscalls, LS syscall, per-process fd table, cat.asm
- [x] Phase 26: Preemptive Scheduler -- timer interrupt, priority levels, yield/sleep syscalls
- [x] Phase 27: IPC -- PIPE syscall, MSGSND/MSGRCV syscalls, blocking I/O
- [x] Phase 28: Device Drivers -- device file convention, IOCTL syscall, screen/keyboard/audio/net drivers
- [ ] Phase 29: Shell -- shell.asm, pipe operator, redirection, built-in commands (ls, cd, cat, echo, ps, kill, help)
- [ ] Phase 30: Boot Sequence -- boot ROM, init process (PID 1), graceful shutdown
- [ ] Phase 31: Standard Library -- lib/stdlib.asm, lib/math.asm, heap allocator, linking convention
- [ ] Phase 32: Signals & Lifecycle -- SIGNAL syscall, signal handlers, EXIT/WAIT syscalls, zombie cleanup

---

## Design Principles

- **Pixels are the truth.** Everything visual should be expressible as pixel operations. The screen isn't an afterthought -- it's the primary interface.
- **The screen IS the state.** Programs should be able to read the screen (PEEK) and react. Visual output isn't separate from computation.
- **Everything is a file.** Device access, IPC, configuration -- all through the filesystem interface.
- **Programs prove the need.** No speculative opcodes. Every new feature ships with a program that needs it.
- **Small steps, always green.** Every phase is a series of commits where `cargo test` passes.
