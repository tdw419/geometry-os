# RISC-V on GPU — Improvement Roadmap

## Current State

| Component | File | Status |
|-----------|------|--------|
| Single-core shader | `riscv-shader.wgsl` | Working. 1 instruction/frame. |
| Single-core host | `src/bin/riscv.rs` | Working. Hand-assembled programs. |
| Multicore shader | `riscv-multicore.wgsl` | Compiles. Not run-tested. |
| Multicore host | `src/bin/multi_tile_ignition.rs` | Compiles. Not run-tested. |
| Executive Commander (RISC-V) | `riscv-cartridges/executive_commander/` | Compiles to riscv32im ELF. Not run-tested on GPU. |
| Executive Commander (host) | `src/bin/executive_commander.rs` | Compiles. Not run-tested. |
| Automated tests | — | **None.** |

### Known Bugs

1. **Signed comparisons missing.** BLT/BGE use unsigned `<`/`>=` in both shaders. Signed comparison needs `(a ^ 0x80000000) < (b ^ 0x80000000)` since WGSL has no signed integers.
2. **LB/LH don't sign-extend.** LB should sign-extend bit 7, LH should sign-extend bit 15. Only LBU/LHU are correct.
3. **SRAI not implemented.** SRLI and SRAI share funct3=0x5 — SRAI (arithmetic shift) needs funct7 check to preserve sign bit.
4. **SB/SH store full word.** The STORE handler writes `regs[rs2]` as a full u32. Byte/halfword stores should mask and merge.
5. **Two diverged shaders.** `riscv-shader.wgsl` and `riscv-multicore.wgsl` duplicate the entire ISA decoder. Bugs fixed in one won't propagate.
6. **`select()` args are swapped for SLT/SLTI.** `select(1u, 0u, a < b)` returns 1 when false, 0 when true — backwards. Should be `select(0u, 1u, a < b)`.

---

## Phase 1: Foundation (make what exists actually work)

### 1.1 Run-test everything
- [ ] Run `multi-tile-ignition` on actual GPU, capture output
- [ ] Run `executive-commander` on actual GPU, verify UART output
- [ ] Run `executive-commander -- --cmd ping`, verify "PONG"
- [ ] Document actual throughput numbers (not projected)

### 1.2 Fix the ISA bugs ✅ (2026-03-24)
- [x] Fix signed comparison (BLT/BGE) with XOR-flip trick
- [x] Fix `select()` argument order for SLT/SLTI/SLTU/SLTIU
- [x] Implement SRAI (check funct7 bit 30, arithmetic right shift)
- [x] Implement LB/LH sign extension
- [x] Implement SB/SH (byte/halfword store with read-modify-write)
- [x] Proper signed DIV/REM (absolute value + sign correction)
- [x] MULHU via 16-bit split multiplication
- [x] Naga validation passes

### 1.3 Unify the shaders (partial)
- [x] `riscv-multicore.wgsl` is the canonical shader with all fixes
- [x] `riscv-shader.wgsl` marked DEPRECATED
- [ ] Update `riscv.rs` host to use multicore shader
- [ ] Delete `riscv-shader.wgsl` once migration complete

### 1.4 Add a test suite
- [ ] CPU-side reference RISC-V interpreter in Rust (~200 lines)
- [ ] Test each instruction: compare reference vs GPU output
- [ ] riscv-tests compliance subset (at minimum: rv32ui-p-* for base integer)
- [ ] CI-runnable without GPU (reference interpreter only)

---

## Phase 2: Correctness (pass real programs)

### 2.1 Byte-addressable memory
Currently `mem_read`/`mem_write` operate on aligned u32 words. Real RISC-V needs:
- [ ] Unaligned LB/LH/LBU/LHU (extract byte/halfword from containing word)
- [ ] Unaligned SB/SH (read-modify-write the containing word)
- [ ] Misaligned LW/SW (either trap or handle split across words)

### 2.2 ELF loader improvements
- [ ] Handle .bss zeroing (currently just unmapped = zero, but fragile)
- [ ] Handle .data section in RAM region
- [ ] Respect entry point from ELF header (don't hardcode 0x1000)
- [ ] Validate ELF magic / architecture before loading

### 2.3 Compile real `no_std` Rust and run it
- [ ] Verify `executive_commander` ELF actually boots on GPU
- [ ] Send PING command, verify PONG response
- [ ] Send ASSIGN + STATUS sequence, verify tile state readback
- [ ] Profile: how many GPU dispatches to complete boot + command cycle

---

## Phase 3: Performance (make it fast)

### 3.1 Multi-step execution
The single-core shader runs 1 instruction per GPU dispatch (round trip to CPU each time). The multicore shader has `max_steps` but needs tuning.
- [ ] Benchmark: instructions/second vs `max_steps` (1, 8, 32, 128, 512)
- [ ] Find the sweet spot where GPU occupancy saturates
- [ ] Add inner loop unrolling hint if WGSL supports it

### 3.2 Memory layout optimization
Current: each tile = 4096 contiguous u32. This may cause bank conflicts when all tiles access the same local offset simultaneously.
- [ ] Benchmark: interleaved vs contiguous layout
- [ ] Consider: text region shared (read-only) across tiles running same program
- [ ] Consider: separate storage buffers for text vs RAM vs registers

### 3.3 Workgroup sizing
Currently `@workgroup_size(1)`. Each tile = 1 invocation = 1 workgroup.
- [ ] Experiment: `@workgroup_size(64)` with 64 tiles per workgroup
- [ ] Use `workgroup` memory for shared text segments
- [ ] Benchmark at 100, 1K, 10K, 100K tiles

---

## Phase 4: Inter-tile Communication

### 4.1 Shared mailbox region
Tiles currently have no way to communicate. Add a global shared region:
- [ ] Global mailbox buffer: `mailbox[tile_a][tile_b]` = message word
- [ ] MMIO address 0x5000+ maps to mailbox send/recv
- [ ] Atomic operations or double-buffered to avoid races

### 4.2 Tile-to-tile signals
- [ ] Define a minimal IPC protocol (send word, poll for response)
- [ ] Executive Commander can write to other tiles' mailboxes
- [ ] Test: Commander sends ASSIGN, worker tile reads it

### 4.3 Spatial addressing
Tiles should know their neighbors:
- [ ] MMIO register at 0x5000: own tile_id
- [ ] MMIO register at 0x5004-0x5010: neighbor tile IDs (N/E/S/W)
- [ ] Conway-style cellular automaton as a test case

---

## Phase 5: Developer Experience

### 5.1 Cartridge build system
- [ ] `Makefile` or `build.rs` that builds all cartridges in `riscv-cartridges/`
- [ ] Auto-extract .text/.rodata from ELF into flat binary
- [ ] Cartridge manifest: `cartridge.toml` with name, entry point, memory requirements

### 5.2 Debugging tools
- [ ] Register dump per tile (already in multi_tile_ignition, extract to library)
- [ ] Instruction trace mode: log PC + instruction for first N steps
- [ ] Breakpoint support: halt on specific PC value
- [ ] UART log viewer: aggregate output from all tiles

### 5.3 Shared library for host code
`riscv.rs`, `executive_commander.rs`, and `multi_tile_ignition.rs` duplicate GPU setup, buffer creation, readback. Extract:
- [ ] `src/riscv_gpu.rs` — shared GPU init, buffer management, dispatch, readback
- [ ] `src/elf_loader.rs` — ELF parsing, tile initialization
- [ ] `src/uart_reader.rs` — UART output extraction

### 5.4 Hot-reload
- [ ] Watch `.wgsl` file for changes, recompile pipeline without restart
- [ ] Watch cartridge source, rebuild ELF + re-upload on change

---

## Phase 6: Real Workloads

### 6.1 Port geometry-os modules
The original goal. Requires Phases 1-2 complete.
- [ ] `logic_gates.rs` — truth table verification on GPU
- [ ] `neural_kernel.rs` — weight update math (needs MUL/DIV = M extension ✅)
- [ ] `executive_commander` — full mailbox loop (needs byte-addressable memory)

### 6.2 Multi-program tiles
Currently all tiles run the same program. Real system needs:
- [ ] Different ELFs loaded into different tiles
- [ ] Tile type field in config (Commander, Worker, Neural, Logic)
- [ ] Host-side orchestrator that assigns programs to tiles

### 6.3 Visualization
- [ ] Render tile grid: color = state (running/halted/waiting)
- [ ] Zoom into tile: show registers, PC, UART output
- [ ] Connect to existing `sovereign_shell` or `spatial_swarm` visualizers

---

## Recommended Execution Order

**Week 1: Make it real**
- 1.1 (run-test) → 1.2 (ISA bugs) → 1.4 (reference interpreter + tests)

**Week 2: Correctness**
- 1.3 (unify shaders) → 2.1 (byte memory) → 2.3 (run executive_commander for real)

**Week 3: Performance + DX**
- 3.1 (multi-step benchmark) → 5.3 (shared library) → 3.3 (workgroup tuning)

**Week 4: Communication + Workloads**
- 4.1 (shared mailbox) → 6.1 (port logic_gates) → 6.2 (multi-program)

The critical path is **1.2 → 2.1 → 2.3**. Until byte-addressable memory and signed comparisons work, compiled Rust code will silently produce wrong results.
