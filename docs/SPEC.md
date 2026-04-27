# Geometry OS — Specification

*Locked: 2026-04-27. Supersedes the "be like Linux/Windows/macOS" framing in `NORTH_STAR.md`.*

## What we are building

**A pixel-native computer on a bare-metal RISC-V substrate.**

The framebuffer is the canonical state of the machine. Programs are pixels that drive pixels. We use RISC-V because it is a real, proven ISA with a real toolchain — and we use it bare-metal, because the unique part of this project is the pixel substrate above the CPU, not yet another Linux distribution.

## Architecture

Three layers. Do not skip layers.

```
┌─────────────────────────────────────────────────────────────┐
│  Layer 3 — Userland: pixel programs + bare-metal C tools     │
│            programs/*.asm  •  examples/riscv-hello/*.c       │
├─────────────────────────────────────────────────────────────┤
│  Layer 2 — Kernel: small, pixel-aware, written in C          │
│            ecalls into SBI  •  framebuffer-as-syscall        │
├─────────────────────────────────────────────────────────────┤
│  Layer 1 — Hardware: RISC-V VM (interpreter + SBI + UART)    │
│            src/riscv/                                        │
└─────────────────────────────────────────────────────────────┘
```

**Current status (2026-04-27).** Layer 1 exists and is solid — interpreter, SBI dispatcher, UART RX/TX, ELF loader, virtio-blk. Layer 3 is starting — `programs/*.asm` is years deep on the pixel-VM side; `examples/riscv-hello/sh.c` is the first bare-metal C tool and it talks to Layer 1 directly via SBI ecalls. **Layer 2 does not exist yet.** Today, Layer 3 programs run on the M-mode bare-metal SBI surface with no kernel between them. Building Layer 2 (a small, pixel-aware C kernel) is future work; the diagram is the target, not the current state.

The Token → Pixel → GUI substrate model in [`GEMINI.md`](../GEMINI.md) is canonical for Layer 3 authoring conventions. Read it after this document.

## Locked decisions

We **will**:

- Keep RISC-V as the substrate. It gives us a real ISA, gcc/llvm, ELF tooling, and C as a sane authoring language for kernel + tools.
- Treat the framebuffer as canonical state. Persistence means checkpointing pixels, not serializing structs.
- Build the userland as a stack of small bare-metal programs that ecall into SBI for I/O.
- Earn opcodes (the Promotion Rule from `GEMINI.md`): pattern → macro → opcode, never the reverse.
- Keep `cargo test` green on every commit.

We **will not**:

- Boot upstream Linux as the target userland. The phase-160 path stays in the repo as research, but is demoted from priority 99 — it costs the system's identity and the boot path is not stable.
- Add Rust-side features that pixel programs could implement themselves. If a feature can be a pixel program, it must be one.
- Add opcodes without a program that needs them.
- Use POSIX abstractions (file descriptors, processes, signals) where pixel-native equivalents already exist or are obvious.

## First milestone — bare-metal interactive mini-shell ✅ shipped 2026-04-27

The first artifact that proves this thesis end-to-end. **Done.**

**What it is.** A ~418-line bare-metal C program (`examples/riscv-hello/sh.c`) that runs in the RISC-V interpreter and gives an interactive `geos>` prompt in the host terminal — no Linux involved. 1 MB VM, ~250K instructions per command, instant boot.

**Built-ins.** `help`, `echo TEXT`, `clear`, `peek <hex_addr>`, `poke <hex_addr> <hex_val>`, `mem <hex_addr>`, `hexdump <hex_addr>`, `regs`, `ver`, `shutdown` (also `exit`, `quit`).

Note: `peek`/`poke` are **memory** inspect/edit (address-based), not pixel-coordinate ops. Pixel-coordinate equivalents will come later as Layer 2 forms.

**Run it.**

```
$ cargo run --release --example sh_run
geos> help
…
geos> echo hello
hello
geos> poke 0x80004000 0xdeadbeef
geos> peek 0x80004000
0xdeadbeef
geos> shutdown
$
```

Stdin is set to raw mode so keypresses pass through immediately. Ctrl-C exits the runner.

**What shipped.** Four files touched:

1. `src/riscv/sbi.rs` — fixed SBI getchar (both legacy v0.1 and DBCN v0.2 paths) to drain `uart.rx_buf` instead of always returning -1.
2. `examples/riscv-hello/sh.c` — the shell.
3. `examples/riscv-hello/build.sh` — added `_zicsr` to `-march` for CSR instructions.
4. `examples/sh_run.rs` — runner that boots `sh.elf`, pipes stdin → `uart.receive_byte`, drains `uart.tx_buf` → stdout, until SBI shutdown.

**Reused infrastructure** (no new wheels):

- UART RX path with `rx_buf` + `LSR_DR` — `src/riscv/uart.rs:62`, `src/riscv/uart.rs:197`
- SBI dispatcher (putchar already wired) — `src/riscv/sbi.rs:144`
- ELF loader — `src/riscv/loader.rs`
- UartBridge for canvas/host I/O — `src/riscv/bridge.rs`
- Bare-metal C scaffolding (`crt0.S`, `hello.ld`, `build.sh`) — `examples/riscv-hello/`

**What this validated.** ELF load, interpreter loop, SBI putchar/getchar round-trip, UART RX wiring, raw-mode terminal pass-through, CSR instruction decode. All 30 SBI/UART/loader tests still green. Every later tool (file editor, hex viewer, pixel painter, asm REPL) reuses this scaffolding.

## Reading order

1. **`docs/SPEC.md`** (this file) — what we are building and why.
2. **`GEMINI.md`** — Token → Pixel → GUI layer model and authoring conventions for Layer 3.
3. **`docs/NORTH_STAR.md`** — priority hierarchy and "DO / DON'T" rules. The "be like Linux" framing is retired but the hierarchy still holds.
4. **`roadmap.yaml`** — concrete deliverables. Phase-160 (Linux boot to userspace) is now research-only; new work hangs off the mini-shell milestone above.
