# Geometry OS — Specification

*Locked: 2026-04-27. Supersedes the "be like Linux/Windows/macOS" framing in `NORTH_STAR.md`.*

## What we are building

**A pixel-native computer on a bare-metal RISC-V substrate.**

The framebuffer is the canonical state of the machine. Programs are pixels that drive pixels. We use RISC-V because it is a real, proven ISA with a real toolchain — and we use it bare-metal, because the unique part of this project is the pixel substrate above the CPU, not yet another Linux distribution.

## Why this stack

Pixel computing — pixels-driving-pixels, framebuffer-as-canonical-state, programs-as-pixels — is genuinely **unproven technology**. Nobody has shipped a useful computer built this way. That is the bet.

Everything else in the stack is **proven, load-bearing, and deliberately boring**:

| Layer | Status | Why this choice |
|---|---|---|
| RISC-V ISA, gcc/llvm, ELF, C, SBI, UART | Proven (decades old, exhaustively documented, free toolchain) | Lets us write tools in C against a real ISA without inventing a compiler or debugger |
| **The shim between proven and unproven** | **Tiny on purpose** (~10 lines per syscall today: SBI putchar/getchar + interpreter loop) | This is the surface area we're betting won't crack |
| Pixel substrate (the framebuffer-as-state, pixels-driving-pixels model) | Unproven, the actual experiment | The thing nobody else has built — the only part of the system that should consume novelty budget |

**The design rule that follows:** every time we're tempted to add proven complexity (a kernel, a libc, a Linux compatibility shim, an X server), ask first whether it grows the shim or shrinks it. The reason `phase-160` (boot upstream Linux) was demoted is that Linux is a 30M-line shim sitting between our proven CPU and our unproven pixel substrate — it would have inverted the ratio: huge shim, tiny pixel surface poking through. Bare-metal RISC-V + a few hundred lines of C keeps the shim collapsed and lets the pixel layer be the thing the system is actually about.

The same rule rejects pivots like "use a CRT simulation instead of pixels" — that swaps one unproven model for a *more* unproven one (beam timing, phosphor decay, scanline ordering) without removing any of the existing pixel work. CRT *aesthetic* (scanlines, glow, persistence trails) belongs as a Layer 3 program over the pixel substrate, not as a substrate replacement.

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

## Canonical framebuffer

`vm.screen` (256×256, format `0x00RRGGBB`) is the single canonical pixel surface. All visible pixel state lives there. `render.rs` reads `vm.screen` and blits it to the display — it knows nothing about RISC-V or any other pixel source.

**Writers.** GeOS bytecode programs write `vm.screen` directly via PSET and other pixel opcodes. RISC-V guests write to their own MMIO framebuffer (`framebuf.pixels`, format `0xRRGGBBAA`, mapped at `0x6000_0000`); on `fb_present` the host composites that buffer into `vm.screen` with alpha-keyed transparency (pixels with alpha=0 are skipped). From render's perspective, both are just pixel writers.

**Readers.** A GeOS PEEK reads `vm.screen`. A RISC-V load from `0x6000_0000` reads `framebuf.pixels` — the guest's own MMIO buffer, not `vm.screen`. Cross-system reads (a RISC-V program reading pixels that a GeOS program drew, or vice versa) do not currently work. That unification (U3 — shared buffer with locking or same-thread execution) is gated on a real use case. No program needs it today.

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
