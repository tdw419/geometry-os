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

**Current status (2026-04-27).** Layer 1 exists and is solid — interpreter, SBI dispatcher, UART RX/TX, ELF loader, virtio-blk. Layer 3 is active — `examples/riscv-hello/` contains 10+ bare-metal C programs including `sh.c` (mini-shell) and `paint.c` (interactive pixel painter), all linked against `libgeos.a` (a static support library with SBI wrappers, framebuffer primitives, timing, and input — not a kernel, just shared C code). **Layer 2 does not exist yet.** Today, Layer 3 programs run on the M-mode bare-metal SBI surface with no kernel between them. Building Layer 2 (a small, pixel-aware C kernel) is future work; the diagram is the target, not the current state.

The Token → Pixel → GUI substrate model in [`GEMINI.md`](../GEMINI.md) is canonical for Layer 3 authoring conventions. Read it after this document.

## Locked decisions

We **will**:

- Keep RISC-V as the substrate. It gives us a real ISA, gcc/llvm, ELF tooling, and C as a sane authoring language for kernel + tools.
- Treat the framebuffer as canonical state. Persistence means checkpointing pixels, not serializing structs. (Phase G shipped in-session checkpoint/restore via the VFS pixel surface. Phase H shipped cross-session persistence — canvas survives VM restart via `.geometry_os/fs/canvas.raw`.)
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

## Capstone demonstration — interactive pixel painter ✅ shipped 2026-04-27

The mini-shell proved the input→compute→output loop. `paint.c` proves the pixel thesis: a bare-metal C program where the framebuffer *is* the program state, not a side effect.

**What it is.** A ~200-line bare-metal C program (`examples/riscv-hello/paint.c`) that provides a pixel-art editor running in the RISC-V interpreter. Cursor movement (WASD), color selection (0-9), fill mode (F), paint (space), clear (C). 10-color palette bar at the bottom. No Linux, no windowing system, no file descriptors — just MMIO writes to the framebuffer.

**What shipped.** The painter went through visual verification via `riscv_fb_dump` (a new socket command that snapshots the RISC-V framebuffer to PNG). Three bugs were caught by reading actual pixel values that no unit test would have caught:

1. **Channel bit-leaking**: `(color >> 8) >> 1` without masking `& 0xFF` shifted bits across byte boundaries, producing wrong arm colors.
2. **Paint-overwrite**: 3x3 crosshair drawn after painting overwrote the painted pixel. Redesigned to save/restore single pixel cursor.
3. **Fill-mode position**: painting at new cursor position instead of old. Off-by-one in move-then-paint vs paint-then-move ordering.

**Infrastructure added.** `riscv_fb_dump <path>` socket command triggers a `TriggerSnapshot` VM control message that clones the current framebuffer pixels and writes them as a valid PNG. Also fixed `encode_png` in `vision.rs` to produce valid zlib-wrapped PNGs (was raw deflate without header/checksum). This makes visual verification scriptable — automated tests can launch a guest, inject inputs, dump the framebuffer, and assert on pixel values.

**Verified output.** Rectangle drawn with fill mode: 10 red + 10 blue + 10 green + 11 yellow pixels at expected coordinates. Adjacent pixels are background (10,10,20). No spill. Palette bar renders correctly with all 10 colors and white border on selection.

## Capstone demonstration — in-session canvas checkpoint ✅ shipped 2026-04-27

`paint.c` now supports save/load of its canvas state via `geos_save_canvas` and `geos_load_canvas` in libgeos. This proves the "checkpointing pixels" half of the persistence thesis.

**What shipped.** Two new libgeos functions that copy the framebuffer to/from the VFS pixel surface at `0x7000_0000` (a 256x256 MMIO region). The save writes framebuffer rows 0-254 to VFS rows 1-255 (preserving the VFS directory index in row 0) and sets a `CANV` marker. The load checks the marker and copies back. Word-by-word MMIO — no serialization, no structs, just pixels copying pixels. Paint.c binds P=save and O=load (S/L conflict with WASD movement keys).

**Scope and limits.** This is in-session checkpoint/restore only. The VFS surface is in-memory; when the VM process exits, the saved canvas is lost. This is still useful — undo, snapshots, scratchpad within a painting session. Cross-session persistence (save today, load tomorrow) requires either flushing the raw canvas region to disk in `VfsSurface::Drop` or routing through the virtio-blk device. That's Phase H.

**Round-trip verification.** `persistence_roundtrip_test.sh` draws a colored pattern (red/green/blue fill trails), dumps the framebuffer, saves, clears, loads, dumps again, and compares pixel-by-pixel. Result: all 62,464 canvas pixels match exactly (tolerance=0). The palette bar area (y=244-255) is saved and restored as part of the framebuffer but paint.c redraws it on top after load — the round-trip test only asserts on the canvas area (y<244) to avoid false positives from cursor position changes.

**Infrastructure reused.** `geos_test_lib.sh` from Phase F provided boot/run/inject/dump/assert primitives. No new test harness needed.

## Capstone demonstration — cross-session canvas persistence ✅ shipped 2026-04-27

Phase G proved in-session save/load. Phase H proves the full persistence thesis: save today, kill the VM, restart tomorrow, load. The canvas survives across process boundaries.

**What shipped.** `VfsSurface::Drop` now detects the CANV marker (set by `geos_save_canvas()` at pixel (0,255)) and writes the raw pixel region (rows 1-255, 255×256×4 = 261,120 bytes) to `.geometry_os/fs/canvas.raw` as little-endian u32 words. On startup, `Bus::new()` calls `VfsSurface::restore_canvas()` after `load_files()`, which checks for canvas.raw, decodes the pixel data back into rows 1-255, and sets the CANV marker so that `geos_load_canvas()` in paint.c can find it.

**Why load_files() first.** `load_files()` clears all pixels except Row 0. If canvas restoration happened before file loading, the file data would overwrite the canvas. The correct order is: create surface → load host files → restore canvas from disk. The canvas data sits in rows 1-255 which are separate from the file directory index in Row 0. If a host file occupies the same row range as the canvas, the canvas wins (it runs last) — this is intentional since the canvas represents user state, not system state.

**Implementation details.** Three methods added to VfsSurface:
- `restore_canvas()` — reads `.geometry_os/fs/canvas.raw`, decodes LE u32 pixels into rows 1-255, sets CANV marker
- `new_with_base(path)` — test helper that creates surface with custom base_dir and calls restore_canvas()
- Modified `Drop::drop()` — calls flush() for file entries, then checks CANV marker and writes canvas.raw

**Unit tests.** Four tests covering the complete lifecycle:
1. `test_canvas_persist_on_drop` — verifies canvas.raw is created with correct size and pixel data when CANV marker is present
2. `test_canvas_restore_on_new` — verifies manually created canvas.raw is loaded correctly
3. `test_canvas_round_trip_persistence` — full two-session simulation: session 1 writes pattern + drops, session 2 restores and verifies all pixels match
4. `test_no_canvas_raw_without_marker` — verifies no file is written when CANV marker is absent

All 11 VFS surface tests pass.

## Verification

Visual verification is now scriptable via `riscv_fb_dump` and `vision::encode_png` (which now produces valid PNGs). New milestones close by asserting on pixel values, not by looking at a screenshot. The pattern: boot guest program → inject known input sequence → dump framebuffer → assert pixel values at specific coordinates.

## Reading order

1. **`docs/SPEC.md`** (this file) — what we are building and why.
2. **`GEMINI.md`** — Token → Pixel → GUI layer model and authoring conventions for Layer 3.
3. **`docs/NORTH_STAR.md`** — priority hierarchy and "DO / DON'T" rules. The "be like Linux" framing is retired but the hierarchy still holds.
4. **`roadmap_v2.yaml`** — concrete deliverables. Phase-160 (Linux boot to userspace) is now research-only; new work hangs off the mini-shell milestone above. Phase E (legacy reconciliation) is hygiene. Phases F (verification), G (in-session persistence), and H (cross-session persistence) are done. Phase E (legacy reconciliation) remains as future hygiene.
