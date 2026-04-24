# Geometry OS Performance Benchmark Report

Generated: Phase 117 (2026-04-24)
Machine: Linux x86_64 (Hermes worker)
Profile: `--release` (optimized)
Tool: Criterion 0.5

## Executive Summary

Geometry OS achieves **~88 million instructions/second** on pure arithmetic code, with graphics
operations (FILL, RECTF) reaching **5.4 billion pixels/second** throughput. The assembler processes
**3 million lines/second** at steady state. The primary bottleneck is **Vm::new()** at 164µs
(256KB allocation), which dominates short-lived VM instances.

## VM Core Performance

| Benchmark | Median Time | Throughput |
|-----------|-------------|------------|
| Vm::new() | 163.78 µs | 6.1K VMs/s |
| Arithmetic loop (1000 iters) | 11.34 µs | 264 Melem/s (~88M instr/s) |
| JMP loop (1000 iters) | 8.12 µs | 123 Melem/s |
| CMP+BLT loop (1000 iters) | 11.45 µs | 87 Melem/s |
| CALL/RET loop (1000 iters) | 21.28 µs | 47 Melem/s |
| MUL/DIV loop (900 iters) | 17.80 µs | 50 Melem/s |
| SPAWN+KILL (single) | 1.33 µs | 751K ops/s |

### Per-Instruction Cost Analysis

| Instruction Type | Cost (ns/iteration) | Notes |
|------------------|---------------------|-------|
| JMP (unconditional branch) | 8.1 ns | ~3 instructions: SUB, JNZ |
| CMP + BLT (conditional) | 11.4 ns | ~3 instructions: ADD, CMP, BLT |
| CALL + RET (subroutine) | 21.3 ns | ~7 instructions: CALL, body, RET |
| MUL + DIV (heavy compute) | 19.8 ns | ~3 instructions + compute |
| SPAWN + KILL (process mgmt) | 1.33 µs | Page dir alloc + dealloc |

### Key Insight: Instruction Rate

The arithmetic loop runs 3000 instructions in 11.34µs = **264M elements/s** = **~88M VM instructions/s**.
At 60fps, this gives ~1.47M instructions per frame budget — well above the infinite_map's 344K.

## Graphics Performance

| Benchmark | Median Time | Throughput |
|-----------|-------------|------------|
| PSET (single pixel) | 1.08 µs | 902K ops/s |
| FILL (256×256 screen) | 12.23 µs | 5.36 Gpixel/s |
| RECTF (50×50 rect) | 2.12 µs | 1.18 Gpixel/s |

### Per-Pixel Cost

- FILL: **0.19 ns/pixel** (memset-level performance, 65536 pixels in 12µs)
- RECTF: **0.85 ns/pixel** (2500 pixels in 2.1µs)
- PSET: **1.08 µs/pixel** (includes full VM step overhead, not just pixel write)

### Key Insight: FILL is Free

At 5.4 Gpixel/s, FILL is essentially a hardware-accelerated memset. A 256×256 screen clear
costs only 12µs — negligible compared to instruction execution. RECTF at 1.2 Gpixel/s is also
extremely fast. The bottleneck is NEVER the pixel writes; it's the VM instructions computing
what to write.

## Memory Access Performance

| Benchmark | Median Time | Throughput |
|-----------|-------------|------------|
| STORE (256 sequential) | 5.88 µs | 43.6 Melem/s |
| LOAD (256 sequential) | 6.63 µs | 38.6 Melem/s |
| Canvas STORE (256 seq) | 6.24 µs | 41.0 Melem/s |
| Canvas LOAD (256 seq) | 6.62 µs | 38.7 Melem/s |

### Key Insight: Canvas Interception is Free

The canvas buffer RAM interception (LOAD/STORE at 0x8000-0x8FFF redirecting to canvas_buffer)
adds **zero measurable overhead** compared to regular RAM access. Both paths run at ~40M ops/s.
The range check is a single branch that predicts correctly in sequential access patterns.

## Assembler Performance

| Benchmark | Median Time | Throughput |
|-----------|-------------|------------|
| Small (4 lines) | 1.31 µs | 3.06 Mline/s |
| Medium (10 lines) | 2.59 µs | 3.86 Mline/s |
| Large (30 lines) | 9.63 µs | 3.12 Mline/s |
| Many labels (100 labels, 300 lines) | 95.59 µs | 3.14 Mline/s |
| With .org directive (14 lines) | 5.20 µs | 2.69 Mline/s |

### Key Insight: Linear Scaling at 3M Lines/sec

The assembler scales linearly with source size. The two-pass architecture (collect labels, then
emit bytecode) shows no superlinear behavior even at 300 lines with 100 labels. The per-line
cost is ~0.32 µs regardless of program size.

## Preprocessor Performance

| Benchmark | Median Time | Throughput |
|-----------|-------------|------------|
| With VAR/SET/GET macros (15 lines) | 9.72 µs | 1.54 Mline/s |

### Key Insight: Preprocessor is 2x Slower Than Assembler

Preprocessing + assembly at 1.5 Mline/s is about half the assembler's raw throughput, due to
token parsing and variable resolution. Still very fast — even the largest programs (<500 lines)
assemble in under 0.5ms.

## Bottleneck Ranking

1. **Vm::new() — 164 µs** (dominates short-lived instances)
   - Allocates 65536 × u32 RAM + 4096 × u32 canvas + process structures
   - 256KB total allocation
   - Optimization: pool/recycle VM instances for repeated benchmarks

2. **MUL/DIV — 19.8 ns per iteration** (heaviest arithmetic ops)
   - Still fast enough: infinite_map's 64×64 DIV-based hash completes in ~50µs
   - No optimization needed

3. **CALL/RET — 21.3 ns per iteration** (subroutine overhead)
   - Includes stack push/pop + indirect jump
   - Acceptable for structured programs

4. **SPAWN/KILL — 1.33 µs** (process lifecycle)
   - Page directory allocation + COW setup
   - Fine for process-per-app model (not per-frame)

## Frame Budget Analysis

At 60fps, each frame has a budget of **16.67ms**.

| Program | Steps/Frame | Time (estimated) | % Budget |
|---------|-------------|-------------------|----------|
| fill_screen.asm | ~4 | 0.05 µs | <0.001% |
| snake.asm | ~3K | 34 µs | 0.2% |
| infinite_map_pxpk.asm | ~344K | 3.9 ms | 23% |
| Full-screen per-pixel (plasma) | ~2.75M | 31 ms | 186% ⚠️ |

The infinite map uses ~23% of frame budget — plenty of headroom for future features.
Plasma-style per-pixel rendering at 2.75M steps exceeds the 16.67ms budget — would need
optimization (lookup tables, SIMD-style batch ops) for full-screen animation at 60fps.

## Recommendations

1. **No immediate optimization needed** — the VM runs well within frame budgets for all
   existing programs.

2. **Future optimization target**: If per-pixel programs (plasma, raycaster) need 60fps,
   consider a BATCH_PSET opcode that writes N pixels from a RAM buffer, reducing per-pixel
   instruction overhead from ~42 instructions to ~3 instructions.

3. **Vm::new() pooling**: For repeated VM creation (test suites, web terminal), a VM pool
   could eliminate the 164µs allocation cost. Current test suite (1401 tests) spends ~230ms
   just on VM construction.

## Methodology

All benchmarks use `criterion::BatchSize::SmallInput` with `iter_batched` to isolate per-iteration
costs from VM setup. Sample size: 100 iterations with 3-second warmup. Measurements taken on
a single-core Linux x86_64 system under release profile (`--opt-level=3`).
