# Geometry OS Roadmap

v1.0.0 -- 41 opcodes, 32 registers, 64K RAM, 256x256 framebuffer, 61 tests, 28 programs.

Phases 1-15 are **complete**. This document covers what comes next.

## Current State

**What works:**
- Full VM with arithmetic, control flow, graphics, audio, sprites, self-hosting ASM
- GUI (minifb) + CLI mode + Hermes agent loop
- 28 programs: static art, animations, interactive games, self-hosting demo
- 61 tests all green
- Breakpoints, single-step, save/load, PNG screenshot (F9), GIF recording (F10)
- TICKS throttle, BEEP audio, SPRITE blit, ASM self-hosting
- Assembler constants (#define), signed arithmetic (SAR), multi-key input (0xFFB bitmask)

**What's rough:**
- GitHub issues #29-54 are all GPU-tile parallelism work (separate track)

---

## Phase 13: Close the Gaps (done)

Goal: Every program tested, every error traceable, no regressions.

| Deliverable | Scope | Acceptance |
|---|---|---|
| Tests for untested programs (ball, fire, hello, circles, lines, scroll_demo, rainbow, rings, colors, checkerboard, painter) | ~220 lines in tests/program_tests.rs | `cargo test` all green, each test: assembles + first-frame sanity |
| Assembler error line numbers | ~30 lines in assembler.rs | Error message says `line N: unknown opcode: XYZ` |
| Version string audit | 3 places: banner, CLI, Cargo.toml | All say same version, single source of truth |


**Why first:** Without test coverage, any future change is a coin flip. The assembler error improvement is tiny and disproportionately helpful.

---

## Phase 14: Developer Experience (done)

Goal: Make the VM pleasant to program.

| Deliverable | Scope | Acceptance |
|---|---|---|
| Assembler constants (`#define NAME value`) | ~80 lines in assembler.rs | `#define TILE 8` resolves in LDI and other immediate contexts |
| programs/README.md | ~60 lines | One-line description + controls + opcodes demonstrated per program |
| Disassembler panel in GUI | ~50 lines in main.rs | Pane shows current PC ± 10 instructions, updates each step |
| GIF/video capture (F10 record toggle) | ~20 lines in main.rs | Writes numbered PNGs to /tmp/geo_frames/, documented ffmpeg command |

**Why next:** The assembler is the main interface. Constants eliminate half the magic numbers. The disassembler panel makes single-step actually usable.

---

## Phase 15: VM Capability Gaps (done)

Goal: Fix the rough edges that make game programming harder than it needs to be.

| Deliverable | Scope | Acceptance |
|---|---|---|
| SAR opcode (arithmetic shift right, 0x2B) | ~10 lines in vm.rs | Two's-complement division works for negative numbers |
| Multi-key input (bitmask port at 0xFFB) | ~20 lines in vm.rs + main.rs | Two simultaneous keys register in same frame |
| BEEP in more programs | ~5 lines each in tetris, breakout, maze, sprite_demo | Sound effects on game events |
| Signed arithmetic audit | Documentation | SUB/ADD/MUL sign contract documented, CMP semantics clear |

**Why here:** Physics games (gravity, velocity) silently break with unsigned-only shifts. Multi-key enables diagonal movement. These are small VM changes that unlock better programs.

---

## Phase 16: Showcase Shipping

Goal: Make tetris a complete game, make the repo presentable.

| Deliverable | Scope | Acceptance |
|---|---|---|
| Complete tetris: scoring, levels, sound, game-over | ~100 lines in tetris.asm | Playable start-to-finish with visible score |
| TILEMAP opcode (grid blit from tile index array) | ~60 lines in vm.rs | snake, tetris, maze each 3x shorter |
| Persistent save slots (4 named) | ~30 lines in vm.rs/main.rs | `save slot1` / `load slot1` from terminal |
| GitHub release v1.0.1 | Tag + release notes | Prebuilt binary attached, Substack link |

**Why here:** A complete game is worth more than 10 half-working ones. TILEMAP makes grid games competitive with hand-drawn loops. Shipping something public creates momentum.

---

## Phase 17: Platform Growth

Goal: Geometry OS as a target platform, not just a toy VM.

| Deliverable | Scope | Acceptance |
|---|---|---|
| GlyphLang compiler backend (emit .geo bytecode) | ~200 lines in glyphlang | `glyphlang compile --target geo program.gl` runs in VM |
| Browser port via WASM | ~200 lines new crate | VM runs in browser with canvas rendering |
| Network port (0xFFB UDP send/recv) | ~40 lines in vm.rs | Two VM instances exchange messages |

**Why last:** These are speculative and large. They depend on Phases 13-16 being solid. The WASM port is the highest-leverage (instant cross-platform, no install) but requires the most design work.

---

## Priority Order

1. Phase 13 (tests + error lines) -- defensive, low risk
2. Phase 14 (constants + README + disassembler) -- developer joy
3. Phase 16 (tetris completion + TILEMAP + ship) -- public momentum
4. Phase 15 (SAR + multi-key + sound) -- quality of life
5. Phase 17 (WASM + GlyphLang + network) -- ambitious, optional

Phase 15 and 16 can be parallelized. Phase 13 should land before anything else touches vm.rs.

---

## Risks

- **Opcode space:** 38 of ~256 slots used, plenty of room, but the hex layout has gaps. New opcodes should fill gaps sequentially.
- **Scope creep:** Every opcode is easy to add. The VM's value is its simplicity. New opcodes need a program that needs them.
- **BEEP subprocess:** spawns `aplay` per beep. Rapid beeps could exhaust FDs. Consider a ring buffer or direct ALSA.
- **GPU tile issues (#29-54):** Parallel compute work is a separate track. Don't let it block the core VM.

---

## Not Planned (Explicitly)

These came up and were deliberately deferred:

- **Self-hosting ASM opcode (phase-12 revisited):** test_self_host_runs was reverted. The ASM opcode works for simple programs but the self-hosting demo proved fragile. Revisit when there's a concrete use case.
- **RISC-V cartridge system:** Issues #29-54. Interesting but orthogonal to core VM improvements.
- **Mobile port:** No current path. WASM port (Phase 17) would cover this indirectly.
