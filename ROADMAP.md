File unchanged since last read. The content from the earlier read_file result in this conversation is still current — refer to that instead of re-reading.
---

## Part II: Hypervisor (Running Other Operating Systems)

Geometry OS is an OS. Now it becomes a hypervisor.

Two paths, ordered by value:

1. **QEMU Bridge** (Phase 33) -- fast path. Wrap QEMU subprocess, render guest
   console on the canvas. Boot Linux THIS WEEK. Proves the concept and
   teaches us what the RISC-V interpreter actually needs.

2. **RISC-V Interpreter** (Phases 34-37) -- deep path. Pure Rust, no external
   dependencies, runs in WASM, testable down to individual instructions.
   Built with knowledge gained from the QEMU bridge.

---

### Phase 33: QEMU Bridge

**Goal:** Spawn QEMU as a subprocess, pipe its serial console I/O through the Geometry OS canvas text surface. Boot Linux (or any OS) on day one.

| Deliverable | Description | Scope |
|---|---|---|
| qemu.rs module | New `src/qemu.rs` -- QEMU subprocess management, stdin/stdout pipes | ~60 lines qemu.rs |
| QEMU spawn | Launch `qemu-system-*` with `-nographic -serial mon:stdio`, capture stdin/stdout | ~80 lines qemu.rs |
| Output to canvas | Read QEMU stdout bytes, write to canvas_buffer as u32 chars, auto-scroll | ~60 lines qemu.rs |
| Input from keyboard | Geometry OS keypresses -> key_to_ascii_shifted() -> write to QEMU stdin | ~40 lines qemu.rs |
| ANSI escape handling | Parse basic ANSI sequences (cursor movement, clear screen) for proper terminal rendering | ~100 lines qemu.rs |
| HYPERVISOR opcode (0x54) | `HYPERVISOR config_addr_reg` -- reads config string from host RAM, spawns QEMU | ~60 lines vm.rs |
| Config format | String at config_addr: `"arch=riscv64 kernel=linux.img [ram=256M] [disk=rootfs.ext4]"` | docs/ |
| Shell command | `hypervisor arch=riscv64 kernel=linux.img` from shell.asm | programs/hypervisor.asm |
| Process lifecycle | QEMU runs as host OS child process. F5 kills it, HYPERVISOR spawns new one | ~40 lines qemu.rs |
| Download helper | Script to fetch pre-built RISC-V Linux kernel + rootfs for testing | scripts/ |
| Integration test | Spawn QEMU with known kernel, verify "Linux version" appears in canvas output | ~60 lines tests |

**Why first:** QEMU gives us a working hypervisor in days. Every architecture QEMU supports (x86, ARM, RISC-V, MIPS) works immediately. We learn exactly what the canvas text surface needs to handle (ANSI sequences, scroll speed, buffer size). This is the prototype that teaches us what the RISC-V interpreter needs to reimplement.

**QEMU serial output -> canvas pipeline:**
```
QEMU stdout (raw bytes)
  -> read into Vec<u8> buffer (non-blocking)
  -> parse ANSI escape sequences
  -> for each printable char: canvas_buffer[row * 32 + col] = char as u32
  -> existing pixel font rendering renders the character
  -> scroll when cursor passes row 128
```

**Keyboard -> QEMU stdin pipeline:**
```
minifb key event
  -> key_to_ascii_shifted(key, shift)
  -> write byte to QEMU stdin pipe
  -> guest OS receives character via serial driver
```

**Supported QEMU architectures out of the box:**
- `qemu-system-riscv64 -kernel Image` -- Linux RISC-V 64-bit
- `qemu-system-x86_64 -kernel bzImage` -- Linux x86
- `qemu-system-aarch64 -kernel Image` -- Linux ARM64
- `qemu-system-mipsel -kernel vmlinux` -- Linux MIPS
- Any QEMU-supported OS with serial console

---

### Phase 34: RISC-V RV32I Core

**Goal:** Implement a pure software RISC-V RV32I interpreter. This is the owned stack -- no QEMU dependency, runs in WASM, testable with `cargo test`.

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
| FENCE, ECALL, EBREAK | NOP-like for now (ECALL traps in Phase 35) | ~20 lines cpu.rs |
| Guest RAM | Vec<u8> separate from host RAM, configurable size (default 128MB) | ~60 lines memory.rs |
| Test suite | One test per instruction, verification against known encodings | ~300 lines tests |
| riscv_simple.asm | Demo: compute fibonacci in RISC-V assembly, run in interpreter | programs/ |

**Why here (not Phase 33):** QEMU already proved what works. We know which devices matter, what the boot sequence needs, how UART output looks. Now we rebuild it owned -- pure Rust, no subprocess, portable to WASM and embedded. RV32I is the non-negotiable foundation: 40 instructions, every RISC-V program uses them.

---

### Phase 35: RISC-V Privilege Modes

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

### Phase 36: RISC-V Virtual Memory & Devices

**Goal:** Implement SV32 page tables and the minimum device emulation (UART, CLINT, PLIC, virtio-blk) needed for a guest OS to boot.

| Deliverable | Description | Scope |
|---|---|---|
| satp CSR | Mode (off/SV32), ASID, root page table physical address | ~20 lines csrs.rs |
| SV32 page table walk | 2-level lookup: VPN[1]->PT1->VPN[0]->PT2->PPN+offset | ~120 lines mmu.rs |
| Page table entry flags | V, R, W, X, U, G, A, D bits in PTE | ~30 lines mmu.rs |
| Address translation | Virtual address -> physical address through page tables | ~80 lines mmu.rs |
| TLB cache | 64-entry TLB with ASID-aware invalidation | ~80 lines mmu.rs |
| Page fault traps | Store/AMO page fault, Load page fault, Instruction page fault with mtval/stval | ~40 lines mmu.rs |
| SFENCE.VMA | TLB flush instruction (privileged) | ~20 lines cpu.rs |
| UART 16550 | Serial port emulation at MMIO 0x10000000, THR/RBR/LSR/IER registers | ~150 lines uart.rs |
| UART to canvas | Guest UART output rendered on canvas (reuses Phase 33 bridge pattern) | ~60 lines bridge.rs |
| CLINT | mtime at 0x200BFF8, mtimecmp at 0x2004000, timer interrupts | ~80 lines clint.rs |
| PLIC | Interrupt priority, enable, threshold, claim/complete | ~120 lines plic.rs |
| Virtio block device | Virtio MMIO transport at 0x10001000, disk image from VFS | ~200 lines virtio_blk.rs |
| Device Tree Blob | Generate DTB describing memory, UART, virtio devices | ~150 lines dtb.rs |
| MMU + device test | Guest sets up page tables, writes to UART, verify output | ~150 lines tests |

**Why here:** Virtual memory + devices in one phase because the RISC-V interpreter needs both before it can boot anything. QEMU already proved these are the minimum devices (we watched Linux use them through the Phase 33 bridge).

---

### Phase 37: Guest OS Boot (Native RISC-V)

**Goal:** Boot a real Linux RISC-V kernel using our own interpreter instead of QEMU. Geometry OS now has two hypervisor modes: QEMU (fast, any arch) and native RISC-V (owned stack, portable).

| Deliverable | Description | Scope |
|---|---|---|
| ELF loader | Parse ELF64 RISC-V kernel images, load segments into guest RAM | ~120 lines loader.rs |
| Raw binary loader | Load flat binary images at specified entry point (0x80000000) | ~40 lines loader.rs |
| DTB passthrough | Pass device tree blob to kernel in a1 register at boot | ~30 lines cpu.rs |
| Boot console | Guest UART output streams to canvas (same bridge as Phase 33) | ~80 lines bridge.rs |
| Keyboard forwarding | Key presses -> guest UART RBR (same bridge as Phase 33) | ~40 lines bridge.rs |
| HYPERVISOR mode flag | `HYPERVISOR` opcode detects "native" vs "qemu" from config string | ~30 lines vm.rs |
| Boot script | `hypervisor native kernel=linux.img dtb=rv32.dtb ram=128M` | programs/ |
| Verified boot | Boot OpenSBI + Linux tinyconfig, verify "Linux version" on canvas | ~100 lines tests |
| Performance benchmark | Measure MIPS, compare interpreter vs QEMU, document results | docs/ |

**Why here:** This is the payoff. Phases 33-36 built both paths. Now Geometry OS has:
- **QEMU mode** (Phase 33): any architecture, battle-tested, fast
- **Native RISC-V mode** (Phases 34-37): owned stack, no subprocess, WASM-portable

Two ways to run guest OSes, same canvas interface. The QEMU bridge taught us what the interpreter needed. The interpreter proves we understand every layer of the stack.

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
11. Phase 33 (QEMU Bridge) -- fast path: boot any OS via QEMU subprocess on canvas
12. Phase 34 (RISC-V Core) -- RV32I interpreter, the owned stack foundation
13. Phase 35 (RISC-V Privilege) -- M/S/U modes, CSRs, traps, ECALL/MRET/SRET
14. Phase 36 (RISC-V Memory & Devices) -- SV32 page tables, UART, CLINT, PLIC, virtio-blk
15. Phase 37 (Guest OS Boot Native) -- boot Linux with our own interpreter, two hypervisor modes

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
- [ ] Phase 33: QEMU Bridge -- QEMU subprocess, serial I/O to canvas, ANSI parsing, HYPERVISOR opcode (0x54), keyboard forwarding, multi-arch support
- [ ] Phase 34: RISC-V RV32I Core -- instruction decode, register file, ALU ops, branches, LUI/AUIPC/JAL/JALR, memory load/store, test suite for all 40 base instructions
- [ ] Phase 35: RISC-V Privilege Modes -- M/S/U modes, CSR registers, ECALL/MRET/SRET, trap entry/return, privilege transitions, TIMER/SOFTWARE interrupts
- [ ] Phase 36: RISC-V Virtual Memory & Devices -- SV32 page table walk, satp, TLB cache, page fault traps, UART 16550, CLINT, PLIC, virtio-blk, DTB generation
- [ ] Phase 37: Guest OS Boot (Native RISC-V) -- ELF/binary loader, DTB passthrough, boot console on canvas, HYPERVISOR native mode, verified boot of Linux RISC-V, performance benchmark

---

## Design Principles

- **Pixels are the truth.** Everything visual should be expressible as pixel operations. The screen isn't an afterthought -- it's the primary interface.
- **The screen IS the state.** Programs should be able to read the screen (PEEK) and react. Visual output isn't separate from computation.
- **Everything is a file.** Device access, IPC, configuration -- all through the filesystem interface.
- **Programs prove the need.** No speculative opcodes. Every new feature ships with a program that needs it.
- **Small steps, always green.** Every phase is a series of commits where `cargo test` passes.
