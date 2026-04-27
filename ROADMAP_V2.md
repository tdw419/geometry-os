# Geometry OS — Post-SPEC Pixel-Native Roadmap

Roadmap for the pixel-native RISC-V hypervisor layer in Geometry OS. Covers toolchain hygiene, GUI bridge, pixel VM convergence, libgeos extraction, and legacy roadmap reconciliation. SPEC = thesis. roadmap_v2 = arc. OpenSpec = per-change diff.


**Progress:** 1/5 phases complete, 0 in progress

**Deliverables:** 2/12 complete

**Tasks:** 3/20 complete

## Scope Summary

| Phase | Status | Deliverables | LOC Target | Tests |
|-------|--------|-------------|-----------|-------|
| phase-A Toolchain Hygiene | COMPLETE | 2/4 | - | - |
| phase-B GUI Bridge — Live Pixel Display | PLANNED | 0/3 | - | - |
| phase-C Pixel VM Convergence | FUTURE | 0/1 | - | - |
| phase-D Layer 2 — libgeos and Primitives | FUTURE | 0/3 | - | - |
| phase-E Legacy Roadmap Reconciliation | FUTURE | 0/1 | - | - |

## Dependencies

| From | To | Type | Reason |
|------|----|------|--------|
| phase-A | phase-B | hard | Need verified 50+ MIPS before GUI bridge is worth building |
| phase-B | phase-C | hard | Need the GUI bridge working before we can converge the surfaces |
| phase-B | phase-D | soft | Animation primitives need the GUI bridge running |
| phase-A | phase-E | soft | Should audit after toolchain is clean, not before |

## [x] phase-A: Toolchain Hygiene (COMPLETE)

**Goal:** Eliminate software-math overhead, fix bus routing, establish rv32imac baseline

The interpreter supports RV32IMAC but examples were being compiled with rv32i, forcing software division loops even for power-of-two constants. This phase fixes the build defaults, audits all programs, and patches the half-word bus routing gap.


### Deliverables

- [x] **RV32IMAC default for all examples** -- All C/ASM programs compiled with -march=rv32imac_zicsr. build.sh already correct; life.c and painter2.c rebuilt with standard flags. Stale comment in build.sh updated.

  - [x] `a.1.1` Rebuild life.elf with rv32imac + O2
    > Built with build.sh-style flags. 118M instrs in 2.1s = 56 MIPS.
    _Files: examples/riscv-hello/life.c_
  - [x] `a.1.2` Rebuild painter2.elf with rv32imac + O2
    > Full 256x256 scene in 46ms (26 MIPS).
    _Files: examples/riscv-hello/painter2.c_
  - [x] `a.1.3` Audit all programs for softmath shims
    > grep found zero shims across all examples.
  - [x] life.c compiles with rv32imac_zicsr
    _Validation: riscv64-linux-gnu-gcc -march=rv32imac_zicsr ... life.c succeeds_
  - [x] No __udivsi3/__umodsi3 shims in any example
    _Validation: grep -r __udivsi3 examples/riscv-hello/ returns nothing_
- [x] **Power-of-two math optimizations in life.c** -- Replaced x%256 with x&0xFF, y*256 with y<<8, idx/8 with idx>>3. Result: 85x speedup (0.6 MIPS -> 56 MIPS).

  - [x] No modulo or multiply by FB_WIDTH in hot loops
    _Validation: grep -n '% FB_WIDTH\|* FB_WIDTH' life.c returns nothing_
  - [x] Benchmarks at 50+ MIPS
    _Validation: time sh_run life.elf shows 2-3s for 10 generations_
- [ ] **Half-word bus routing for MMIO framebuffer** -- Half-word (16-bit) writes to 0x6000_0000 currently fall through to RAM instead of routing to the framebuffer. Will silently corrupt if guest uses memcpy or RGBA565 packing.

  - [ ] `a.3.1` Add half-word routing in bus.rs for framebuffer range
    > In bus.rs write_word path, add framebuffer half-word routing similar to existing word routing. Also add byte-level write routing. Check Framebuffer::write() handles sub-word offsets.
    _Files: src/riscv/bus.rs, src/riscv/framebuf.rs_
  - [ ] `a.3.2` Add unit tests for half-word and byte framebuffer access
    > Test 16-bit and 8-bit writes/reads to MMIO framebuffer.
    _Files: src/riscv/tests.rs_
  - [ ] 16-bit write to framebuffer address stored correctly
    _Validation: Unit test: write 0x1234 to FB_BASE+0, read back matches_
  - [ ] Byte writes to framebuffer also work
    _Validation: Unit test: write 0xFF to FB_BASE+1, read back upper byte is 0xFF_
- [ ] **Update build.sh stale comment** -- Comment says 'Geometry OS CPU is RV32I' but interpreter supports RV32IMAC.
  - [ ] `a.4.1` Fix comment in build.sh
    > Change 'IMPORTANT: Geometry OS CPU is RV32I' to RV32IMAC.
    _Files: examples/riscv-hello/build.sh_
  - [ ] Comment in build.sh mentions RV32IMAC
    _Validation: grep RV32IMAC build.sh returns match_

### Technical Notes

The 85x speedup was the combination of two fixes: (1) rv32im gives hardware MUL/DIV/REM, (2) power-of-two constants (256) let the compiler use shift/mask instead of calling division routines at all. Both were necessary -- the compiler can't optimize division by constants into shifts if there's no hardware divide instruction in the target ISA.


## [ ] phase-B: GUI Bridge — Live Pixel Display (PLANNED)

**Goal:** Watch RISC-V programs paint in real-time on the Geometry OS display

Bridge the MMIO framebuffer to the actual Geometry OS display so programs render live instead of dumping PNGs. The present callback architecture is already in place -- this phase swaps the PNG-dump callback for a real screen blit, runs the VM on its own thread, and delivers the experiential payoff of the pixel-native thesis.


### Deliverables

- [ ] **Off-thread VM execution** -- Spawn RiscvVm on its own thread. Present callback pushes frame-ready signal (or buffer copy) over a channel. GUI thread blits on render tick. The current synchronous-callback-in-bus-write bug becomes structurally impossible.

  - [ ] `b.1.1` Create RiscvVmThread struct with channel-based present
    > New module (src/riscv/live.rs?) wrapping RiscvVm in a thread. Uses mpsc::channel: VM sends (Vec<u32> or Arc<[u32]>) on present, GUI thread recv()s on its tick. Include pause/resume/reset controls.
    _Files: src/riscv/live.rs, src/riscv/mod.rs_
  - [ ] `b.1.2` Replace synchronous PNG callback in sh_run with channel
    > Update sh_run to use the new threaded VM. Channel recv writes PNGs (keep as debug tool) but doesn't block the VM.
    _Files: examples/sh_run.rs_
  - [ ] VM runs on separate thread from GUI
    _Validation: Code review: std::thread::spawn for VM loop_
  - [ ] Present callback does not block interpreter
    _Validation: Channel send is non-blocking or bounded_
- [ ] **Framebuffer blit to Geometry OS display** -- Find where the main app renders the pixel VM canvas. Inject the RISC-V framebuffer (256x256 RGBA) as a surface in that pipeline. May need scaling (256x256 -> 512x512 display) or window integration via WINSYS.

  - [ ] `b.2.1` Identify main app render loop and injection point
    > Find the pixel VM canvas render path in main.rs or render.rs. Determine how to add a RISC-V framebuffer surface alongside existing canvas. Consider WINSYS window vs direct surface blit.
    _Files: src/main.rs, src/render.rs_
  - [ ] `b.2.2` Implement framebuffer-to-display blit
    > Wire the channel output from RiscvVmThread into the display pipeline. Scale 256x256 RGBA to whatever the display expects. Frame-rate limit to avoid spinning.
    _Files: src/main.rs, src/render.rs, src/riscv/live.rs_
  - [ ] `b.2.3` Add launch control in Geometry OS UI
    > Add ability to load and run a .elf from the Geometry OS UI (MCP command, keyboard shortcut, or menu item).
    _Files: src/main.rs, src/cli.rs_
  - [ ] RISC-V guest pixels appear on the Geometry OS display
    _Validation: Launch life.elf via GUI, see cells moving on screen_
  - [ ] Frame rate is at least 5 fps for 64x64 life
    _Validation: Visual confirmation of smooth animation_
- [ ] **Default demo: Life at 64x64** -- Life at 256x256 runs at ~5 gen/sec. Life at 64x64 should clear 20+ fps and look alive. Create a 64x64 variant as the default GUI demo. Moving cells, no ambiguity, proves read+compute+write in motion.

  - [ ] `b.3.1` Create life64.c variant
    > 64x64 grid in a 256x256 framebuffer (each cell = 4x4 pixel block). Toroidal. Same color gradient as life.c. Higher density seed (40%).
    _Files: examples/riscv-hello/life64.c_
  - [ ] `b.3.2` Benchmark life64.elf
    > Run via sh_run, measure gen/sec. Target 20+.
  - [ ] life64.elf runs at 20+ gen/sec
    _Validation: Benchmark with sh_run, divide wall time by gen count_
  - [ ] life64.elf launches from Geometry OS UI
    _Validation: UI action loads and runs life64.elf, visible on display_

### Technical Notes

The present callback architecture from commit 355ae7f is the right shape. The bug is that it runs synchronously inside bus.write(). Channel-based off-thread fix makes this structurally impossible.


### Risks

- Main app render loop may not have an easy injection point for external surfaces
- WINSYS window approach adds complexity vs direct surface blit

## [?] phase-C: Pixel VM Convergence (FUTURE)

**Goal:** Bridge MMIO framebuffer to the canonical pixel VM screen bidirectionally

The pixel VM (Geometry OS's native 32x16 tile canvas with 512x512 display) and the RISC-V MMIO framebuffer (256x256 RGBA) are currently separate surfaces. This phase makes them the same thing -- RISC-V guest programs draw to what IS the pixel VM screen, not a separate buffer. This is the true "pixel-native" convergence.


### Deliverables

- [ ] **Unified pixel surface** -- RISC-V framebuffer writes go directly to the pixel VM's display surface. Pixel VM opcodes can also read what the RISC-V program drew. One surface, two access paths (opcodes and MMIO). The 256x256 -> 512x512 scaling happens in the display pipeline, not in the guest's mental model.

  - [ ] `c.1.1` Map MMIO framebuffer writes to pixel VM canvas buffer
    > When guest writes to 0x6000_0000 + offset, update the same buffer that the pixel VM reads from. May need coordinate transform if the pixel VM uses a different pixel format or stride.
    _Files: src/riscv/framebuf.rs, src/vm/types.rs, src/main.rs_
  - [ ] `c.1.2` Integration test for bidirectional pixel access
    > Test that RISC-V MMIO writes are visible through pixel VM opcodes and vice versa.
    _Files: src/riscv/tests.rs_
  - [ ] RISC-V write at (x,y) appears as pixel VM read at same logical position
    _Validation: Integration test: write via MMIO, read via pixel VM opcode_
  - [ ] No duplicate buffer -- single source of truth for pixels
    _Validation: Memory audit: one Vec<u32> for the display surface_

### Risks

- Pixel VM may use a different pixel format (RGBA vs ARGB vs indexed)
- 512x512 display vs 256x256 guest resolution requires scaling decisions

## [?] phase-D: Layer 2 — libgeos and Primitives (FUTURE)

**Goal:** Extract shared C primitives into libgeos.c, add animation and input primitives

When a third tool (beyond sh and life) needs shared primitives (puts, tokenizer, fb_present, etc.), extract them into a shared library. Also add animation timing primitives and bidirectional input (GUI events -> RISC-V guest).


### Deliverables

- [ ] **libgeos.c shared library** -- Extracted when third program needs shared code. Contains: puts, put_dec, put_hex, fb_present, fb_pixel, rgb, sbi_console_putchar, sbi_shutdown. Compiled once, linked by all guest programs.

  - [ ] `d.1.1` Create libgeos.c and Makefile rule
    > Extract shared functions from life.c/painter2.c/sh.c into libgeos.c. Build as libgeos.a. Update build.sh to link against it.
    _Files: examples/riscv-hello/libgeos.c, examples/riscv-hello/build.sh_
  - [ ] Three or more programs link against libgeos.a
    _Validation: ls examples/riscv-hello/*.c | wc -l >= 3 and all link -lgeos_
- [ ] **Animation / frame timing primitives** -- SBI extension or MMIO register for frame timing. Guest can wait for next frame, query elapsed time, sync to display refresh. Enables smooth animation loops instead of spin-paint.

  - [ ] `d.2.1` Add frame timing SBI extension or MMIO register
    > Expose CLINT mtime to guest at a known MMIO address, or add SBI call that blocks until next display refresh. Let guest programs do frame-synced animation.
    _Files: src/riscv/sbi.rs, src/riscv/bus.rs_
  - [ ] Guest can sync to display refresh rate
    _Validation: Demo program maintains steady frame rate_
- [ ] **GUI-to-RISC-V input bridge** -- Keyboard/mouse events from the Geometry OS GUI flow into the RISC-V guest via UART RX or a dedicated input MMIO region. Enables interactive painters, shells, and games.

  - [ ] `d.3.1` Route GUI keyboard events to UART RX
    > When the RISC-V display window has focus, forward keystrokes to uart.receive_byte(). Guest reads via SBI getchar or UART MMIO.
    _Files: src/main.rs, src/riscv/uart.rs_
  - [ ] Keystroke in Geometry OS window arrives in RISC-V guest
    _Validation: Type 'A' in GUI, guest reads 'A' from UART_

### Risks

- Premature extraction -- only extract when the third program actually needs it

## [?] phase-E: Legacy Roadmap Reconciliation (FUTURE)

**Goal:** Triage roadmap.yaml (163 phases) against SPEC, demote/retire phases that don't fit

The legacy roadmap.yaml has 163 phases spanning the full Geometry OS history. Many are complete, some are superseded by the SPEC direction, some are still relevant. This phase audits the legacy roadmap against the post-SPEC direction and reconciles the two documents.


### Deliverables

- [ ] **Legacy roadmap audit** -- Go through each phase in roadmap.yaml. Mark complete where code exists. Mark superseded where SPEC direction changed. Mark relevant where still needed. Produce a reconciliation report.

  - [ ] `e.1.1` Audit roadmap.yaml against codebase
    > Use roadmap audit workflow (see skill). Compare meta block against actual opcode count, test count, LOC, program count. Update statuses.
    _Files: roadmap.yaml_
  - [ ] `e.1.2` Produce reconciliation report
    > Summary: how many phases complete, how many superseded, how many still relevant. Identify gaps between roadmap.yaml and roadmap_v2.yaml.
    _Files: ROADMAP.md_
  - [ ] Every phase in roadmap.yaml has an accurate status
    _Validation: roadmap validate passes, statuses match codebase_

## Global Risks

- Interpreter performance ceiling: 56 MIPS may not scale to complex guest programs
- Main app render loop coupling: hard to inject external surfaces without refactoring
- Premature abstraction: libgeos.c should only be extracted when truly needed

## Conventions

- Build all RISC-V examples with -march=rv32imac_zicsr -mabi=ilp32
- Use build.sh-style flags (ffreestanding, nostdlib, O2, medany)
- Power-of-two constants: use bit masks (x & 0xFF) not modulo (x % 256)
- Present callback must be channel-based, never synchronous in bus write
- SPEC = thesis. roadmap_v2 = arc. OpenSpec = per-change diff. Three docs, three jobs.
