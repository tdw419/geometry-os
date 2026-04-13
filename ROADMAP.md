# Geometry OS Roadmap

**Ultimate Goal:** Build Geometry OS to modern OS standards -- a real operating system with memory protection, a filesystem, proper scheduling, IPC, device abstraction, and a shell. Linux/Windows/macOS started somewhere. This is our somewhere.

v1.0.0 shipped 22 phases of VM construction. Now the real work begins.

## Current State

- 71 opcodes, 32 registers, 64K RAM, 256x256 framebuffer
- 195 tests (all passing, 2 ignored), 38 programs, all green
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
- Device drivers (device file convention, IOCTL, screen/keyboard/audio/net drivers)
- Shell (shell.asm, pipe operator, redirection, built-in commands, READLN/WAITPID/EXECP/CHDIR/GETCWD)

**What's missing for a real OS:**
- No init/boot sequence (hardcoded startup)
- No standard library for programs
- No signal handling or proper process lifecycle

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

### Phase 33: RISC-V RV32I Core

**Goal:** Implement a pure software RISC-V RV32I interpreter as a new module. This is the foundation for running real operating systems inside Geometry OS.

| Deliverable | Description | Scope |
|---|---|---|
| riscv/ module | New `src/riscv/` directory with mod.rs, cpu.rs, memory.rs, decode.rs | ~50 lines mod.rs |
| Register file | x[0..32] (x0=zero), PC, 32-bit registers | ~30 lines cpu.rs |
| Instruction decode | Decode all RV32I opcodes from 32-bit instruction words | ~200 lines decode.rs |
| R-type ALU | ADD, SUB, SLL, SLT, SLTU, XOR, SRL, SRA, OR, AND | ~80 lines cpu.rs |
| I-type immediate | ADDI, SLTI, SLTIU, XORI, ORI, ANDI, SLLI, SRLI, SRAI | ~60 lines cpu.rs |
| Upper immediate | LUI, AUIPC | ~20 lines cpu.rs |
| Jumps | JAL, JALR (link register x1 or rd) | ~30 lines cpu.rs |
| Branches | BEQ, BNE, BLT, BGE, BLTU, BGEU | ~40 lines cpu.rs |
| Memory load | LB, LH, LW, LBU, LHU (sign/zero extend) | ~50 lines cpu.rs |
| Memory store | SB, SH, SW | ~30 lines cpu.rs |
| FENCE, ECALL, EBREAK | NOP-like for now (ECALL traps in Phase 34) | ~20 lines cpu.rs |
| Guest RAM | Vec<u8> separate from host RAM, configurable size (default 128MB) | ~60 lines memory.rs |
| Test suite | One test per instruction, verification against known encodings | ~300 lines tests |
| riscv_simple.asm | Demo: compute fibonacci in RISC-V assembly, run in interpreter | programs/ |

**Why first:** RV32I is the base integer ISA. Every RISC-V program uses these 40 instructions. Getting decode+execute right with full test coverage is the non-negotiable foundation. Everything else (privilege modes, virtual memory, device emulation, guest OS boot) layers on top.

---

### Phase 34: RISC-V Privilege Modes

**Goal:** Implement Machine/Supervisor/User privilege levels, CSR registers, and trap handling. This is what allows a guest OS kernel to manage its own processes.

| Deliverable | Description | Scope |
|---|---|---|
| Privilege enum | M-mode (3), S-mode (1), U-mode (0) in CPU state | ~20 lines cpu.rs |
| CSR register bank | mstatus, mtvec, mepc, mcause, mtval, sstatus, stvec, sepc, scause, stval, satp, mie, mip, sie, sip, mcounteren, scounteren | ~80 lines csrs.rs |
| CSR read/write | CSRRW, CSRRS, CSRRC, CSRRWI, CSRRSI, CSRRCI opcodes (SYSTEM type) | ~100 lines cpu.rs |
| ECALL trap | ECALL from U->S or S->M: saves PC to mepc/sepc, jumps to stvec/mtvec, sets cause | ~60 lines cpu.rs |
| MRET instruction | Return from M-mode trap: restore PC from mepc, adjust mstatus | ~30 lines cpu.rs |
| SRET instruction | Return from S-mode trap: restore PC from sepc, adjust sstatus | ~30 lines cpu.rs |
| Timer interrupt | mtime/mtimecmp MMIO, fires interrupt when mtime >= mtimecmp | ~50 lines clint.rs |
| Software interrupt | msip/ssip registers, software-triggered interrupts | ~30 lines clint.rs |
| Trap delegation | medeleg/mideleg CSRs: delegate traps from M to S mode | ~40 lines csrs.rs |
| Privilege transition tests | U->S via ECALL, S->M via ECALL, MRET returns to S, SRET returns to U | ~150 lines tests |

**Why here:** Linux can't boot without privilege modes. The kernel runs in S-mode, user programs run in U-mode, and the hypervisor (us) runs in M-mode. Trap handling is how the kernel gets control back from user programs.

---

### Phase 35: RISC-V Virtual Memory

**Goal:** Implement SV32 page tables so a guest OS can manage virtual address spaces for its own processes.

| Deliverable | Description | Scope |
|---|---|---|
| satp CSR | Mode (off/SV32), ASID, root page table physical address | ~20 lines csrs.rs |
| SV32 page table walk | 2-level lookup: VPN[1]->PT1->VPN[0]->PT2->PPN+offset | ~120 lines mmu.rs |
| Page table entry flags | V, R, W, X, U, G, A, D bits in PTE | ~30 lines mmu.rs |
| Address translation | Virtual address -> physical address through page tables | ~80 lines mmu.rs |
| TLB cache | 64-entry TLB with ASID-aware invalidation | ~80 lines mmu.rs |
| Page fault traps | Store/AMO page fault, Load page fault, Instruction page fault with mtval/stval | ~40 lines mmu.rs |
| SFENCE.VMA | TLB flush instruction (privileged) | ~20 lines cpu.rs |
| Bare mode (satp=0) | Direct physical addressing, no translation (default for Phase 33) | ~10 lines mmu.rs |
| Memory test | Guest creates page tables, maps virtual to physical, reads/writes through translations | ~100 lines tests |

**Why here:** After privilege modes, virtual memory is the next piece Linux needs. The kernel sets up page tables (satp) for each user process. Without SV32, the guest kernel can't isolate its own processes.

---

### Phase 36: RISC-V Device Emulation

**Goal:** Emulate the devices a real OS needs: serial console, block storage, network, and timers. Linux needs these to boot and do anything useful.

| Deliverable | Description | Scope |
|---|---|---|
| UART 16550 | Serial port emulation at MMIO 0x10000000, THR/RBR/LSR/IER registers | ~150 lines uart.rs |
| UART to canvas | Guest UART output rendered as text on Geometry OS canvas (TEXT opcode) | ~60 lines bridge.rs |
| UART from keyboard | Geometry OS keyboard input forwarded to guest UART RBR | ~40 lines bridge.rs |
| CLINT (Core Local Interruptor) | mtime at 0x200BFF8, mtimecmp at 0x2004000, timer interrupts | ~80 lines clint.rs |
| PLIC (Platform Level Interrupt Controller) | Interrupt priority, enable, threshold, claim/complete for external interrupts | ~120 lines plic.rs |
| Virtio block device | Virtio MMIO transport at 0x10001000, disk image from VFS file | ~200 lines virtio_blk.rs |
| Virtio network | Virtio MMIO transport at 0x10002000, TAP interface or loopback | ~200 lines virtio_net.rs |
| Device Tree Blob | Generate DTB describing memory layout, UART, virtio devices, CPU count | ~150 lines dtb.rs |
| Disk image loader | Load raw/qcow2 disk image from host filesystem via Geometry OS VFS | ~60 lines loader.rs |
| Device test | Guest writes to UART THR, verify output appears on canvas | ~80 lines tests |

**Why here:** Linux can't boot without a console (UART), can't persist without storage (virtio-blk), and can't network without a NIC (virtio-net). The device tree tells the kernel what hardware exists. These are the minimum viable devices.

---

### Phase 37: Guest OS Boot

**Goal:** Load and boot a real Linux RISC-V kernel inside Geometry OS. The guest kernel boots, starts init, and provides a console on the canvas.

| Deliverable | Description | Scope |
|---|---|---|
| ELF loader | Parse ELF64 RISC-V kernel images, load segments into guest RAM | ~120 lines loader.rs |
| Raw binary loader | Load flat binary images at specified entry point (0x80000000) | ~40 lines loader.rs |
| DTB passthrough | Pass device tree blob to kernel in a1 register at boot | ~30 lines cpu.rs |
| Boot console | Guest UART output streams to Geometry OS canvas as scrolling text | ~80 lines bridge.rs |
| Keyboard forwarding | Geometry OS keypresses injected into guest UART RBR | ~40 lines bridge.rs |
| HYPERVISOR opcode | New Geometry OS opcode that spawns a RISC-V VM instance, loads kernel from VFS path | ~60 lines vm.rs |
| Boot script | One-command: `HYPERVISOR "linux/rv32.img"` from shell.asm | programs/ |
| Verified boot | Boot a known-good Linux RISC-V kernel (tinyconfig or OpenSBI) and verify console output matches expected string | ~100 lines tests |

**Why here:** This is the moment everything comes together. Phases 33-36 built the interpreter, privilege modes, virtual memory, and devices. Phase 37 wires them up to boot a real kernel. After this, Geometry OS isn't just an OS -- it's a hypervisor that can run other operating systems.

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
- [x] Phase 29: Shell -- shell.asm, pipe operator, redirection, built-in commands (ls, cd, cat, echo, ps, kill, help)
- [x] Phase 30: Boot Sequence -- boot ROM, init process (PID 1), graceful shutdown
- [ ] Phase 31: Standard Library -- lib/stdlib.asm, lib/math.asm, heap allocator, linking convention
- [ ] Phase 32: Signals & Lifecycle -- SIGNAL syscall, signal handlers, EXIT/WAIT syscalls, zombie cleanup

---

## Design Principles

- **Pixels are the truth.** Everything visual should be expressible as pixel operations. The screen isn't an afterthought -- it's the primary interface.
- **The screen IS the state.** Programs should be able to read the screen (PEEK) and react. Visual output isn't separate from computation.
- **Everything is a file.** Device access, IPC, configuration -- all through the filesystem interface.
- **Programs prove the need.** No speculative opcodes. Every new feature ships with a program that needs it.
- **Small steps, always green.** Every phase is a series of commits where `cargo test` passes.
