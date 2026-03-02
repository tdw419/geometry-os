# Project Research Summary

**Project:** Geometry OS Font Toolkit
**Domain:** GPU-native operating systems with font-based execution
**Researched:** 2026-03-02
**Confidence:** MEDIUM (novel domain with no direct precedent; recommendations based on GPU systems research, WebGPU/Vulkan documentation, and existing codebase analysis)

## Executive Summary

Geometry OS is a novel GPU-native operating system where fonts are executable programs and the GPU serves as the primary compute substrate. Unlike traditional architectures that treat the GPU as an accelerator, this system runs entirely on GPU compute shaders, with SPIR-V bytecode interpreted directly by a WGSL kernel. The recommended stack combines WebGPU (browser) + wgpu-native (desktop) for cross-platform GPU compute, WGSL for kernel shaders with naga cross-compilation, and Python (fonttools + Pillow + NumPy) for font encoding and asset generation.

The key architectural insight is that all OS state lives in WebGPU storage buffers, read/written by compute shaders. The kernel uses cooperative multitasking (not preemptive) to iterate through process control blocks in a single dispatch. Memory-mapped I/O via specific RAM addresses provides GPU-to-CPU communication, with CPU-side polling to trigger side effects like audio output. The font-as-program pattern encodes semantic operations in glyph color channels (R=category, G=operation, B=value).

Critical risks include cooperative multitasking starvation (long-running processes block everyone), GPU memory race conditions in shared RAM (no hardware synchronization between workgroups), and CPU-GPU synchronization latency (WebGPU is inherently async, no synchronous readback). Mitigation requires strict instruction limits per time slice, atomic flags for mailbox ownership, and designing for 1-2 frame latency in all CPU-GPU communication.

## Key Findings

### Recommended Stack

The stack combines browser-native WebGPU for accessibility with optional wgpu-native for desktop performance. Use WGSL as the primary kernel language (not raw SPIR-V generation) because WebGPU tooling is mature in 2025 and naga can cross-compile to SPIR-V. Python tooling handles font atlas generation with Hilbert curve traversal for spatial locality.

**Core technologies:**
- **WebGPU API** (Chrome 113+, Safari 17.4+): Browser GPU compute -- production-ready, native compute shader support
- **WGSL** (WebGPU Shading Language): Kernel/OS shader language -- Rust-like syntax, strongly typed, naga compiles to SPIR-V
- **naga** (wgpu-naga 24.x): Shader cross-compiler -- compiles WGSL to SPIR-V, better DX than glslang
- **fonttools** (4.55+): TTF/OTF generation -- industry standard, full OpenType spec support
- **Vitest + Vite** (3.x/6.x): Testing and build -- Vite-native, browser mode for WebGPU tests
- **spirv-tools** (1.4.304+): SPIR-V optimization -- strip debug, validate, disassemble for debugging

**Avoid:** Raw SPIR-V generation (use WGSL + naga), OpenGL/WebGL (no compute shaders), CUDA/OpenCL (not web-compatible), CPU-side interpreters (defeats GPU-native purpose).

### Expected Features

The feature set is derived from embedded OS patterns and visual programming environments since no direct competitor exists. MVP must validate the core concept: "a GPU-native OS where fonts are executable programs."

**Must have (table stakes):**
- **Process spawning** -- Load SPIR-V, create PCB entry, dispatch kernel
- **Memory management** -- Allocate/deallocate RAM sectors, prevent corruption
- **Basic shell/CLI** -- Process list, spawn commands, kill, status
- **Program execution** -- SPIR-V interpreter in GPU compute shader (kernel.wgsl)
- **Error handling** -- Catch GPU errors, report back, clean up PCB
- **State visibility** -- PCB inspection, memory browser, process states
- **File loading** -- Drag-drop .spv files
- **Memory-mapped I/O** -- At minimum: sound output to prove I/O works

**Should have (competitive):**
- **Font-as-program** -- Glyphs encode opcodes/operands in color channels
- **Visual programming IDE** -- Draw programs instead of typing code
- **GPU-native execution** -- Entire OS runs on GPU via compute shaders
- **Hilbert curve memory** -- Spatial locality preserved, visual RAM coherent
- **Multi-process on GPU** -- Cooperative multitasking without CPU intervention
- **Agent system** -- Pre-built system processes (audio, network, etc.)

**Defer (v2+):**
- **Self-hosting compiler** -- Compiler written in Geometry OS itself
- **Network stack** -- IPC to network agent, socket API
- **Process isolation / security** -- Hardware-enforced protection
- **Desktop native build** -- wgpu-native for desktop distribution

### Architecture Approach

Geometry OS follows a layered architecture where the GPU IS the execution environment. All state lives in WebGPU storage buffers (program, stack, RAM, PCB table), accessed by compute shaders. The kernel uses cooperative multitasking to iterate through processes in a single dispatch. Memory-mapped I/O provides GPU-to-CPU communication via specific RAM addresses polled by CPU-side handlers.

**Major components:**
1. **kernel.wgsl** -- SPIR-V bytecode interpreter, multi-process scheduling, IPC mailbox handling (THE HEART)
2. **executor.wgsl** -- Single-process SPIR-V execution (simpler testing case)
3. **GeometryKernel.js** -- Process lifecycle management, PCB buffer management, shared memory allocation
4. **SpirvRunner.js** -- Per-process state persistence, memory-mapped I/O handling
5. **VisualCompiler.js** -- Grid-to-SPIR-V compilation, Hilbert curve traversal, opcode emission
6. **VisualIDE.js** -- Interactive glyph grid editor, PNG import/export
7. **VisualShell.js** -- Process spawning UI, PCB inspection, dashboard metrics
8. **GeometryOS.js** -- Unified desktop substrate, window management, agent coordination

**Key patterns:**
- Storage Buffer State Machine: All OS state in GPU buffers, no CPU-GPU sync during compute
- Cooperative Multitasking: Kernel iterates PCBs in single dispatch, each process gets time slice
- Hilbert Curve Memory Layout: 2D grid -> 1D linear with spatial locality preserved
- Memory-Mapped I/O: RAM addresses trigger CPU-side handlers (polling-based)
- Font-as-Program: Glyph color channels encode semantics (R=category, G=op, B=value)

### Critical Pitfalls

1. **Cooperative Multitasking Starvation** -- A single long-running process blocks all others because GPU dispatch is atomic. Avoid with strict `MAX_INST_PER_SLICE` limits (100 is safe), yield instructions in hot loops, and CPU-side watchdog timer.

2. **GPU Memory Race Conditions in Shared RAM** -- Multiple processes writing to shared memory (mailboxes, syscall buffers) produce corrupted data. Avoid with single-producer single-consumer pattern, atomic flags for mailbox ownership using `atomicStore`/`atomicLoad`.

3. **CPU-GPU Synchronization Latency Assumption** -- Code assumes GPU results are available immediately after `dispatchWorkgroups()`, but WebGPU is async. Always use `await` with `mapAsync()`, design for 1-2 frame latency, never assume dispatch completed.

4. **SPIR-V Bytecode Interpretation Errors** -- The kernel's interpreter mis-handles edge cases (variable-width instructions, operand ordering). Start with minimal opcode subset, add incrementally with tests, validate with `spirv-val` before loading.

5. **Shader Timeout on Long-Running Programs** -- Compute-intensive programs cause GPU shader to exceed browser timeout (~2-10 seconds), triggering device loss. Keep instruction limits conservative, split dispatches across frames, monitor `device.lost` promise.

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Core Kernel (Foundation)
**Rationale:** The SPIR-V interpreter (kernel.wgsl) is the heart of the system. Nothing can run without it. All other components depend on GPU execution working correctly.
**Delivers:** Functional SPIR-V interpreter, process control blocks, basic scheduling, memory protection
**Addresses:** Process spawning, program execution, error handling, memory management, state visibility
**Avoids:** Cooperative multitasking starvation (instruction limits), memory region overflow (bounds checking), SPIR-V interpretation errors (validation)
**Stack:** WGSL, WebGPU storage buffers, spirv-val
**Research needed:** LOW -- well-documented patterns, existing kernel.wgsl to extend

### Phase 2: Execution Environment
**Rationale:** With kernel working, need tooling to compile visual programs and run them persistently. This enables the core "font-as-program" value proposition.
**Delivers:** Grid-to-SPIR-V compilation, glyph loading/decoding, persistent process execution, memory-mapped I/O
**Addresses:** File loading, memory-mapped I/O (sound), font-as-program, visual programming IDE
**Uses:** VisualCompiler.js, GeometryFont.js, SpirvRunner.js, SoundSystem.js
**Implements:** Hilbert curve traversal, semantic color decoding, I/O trigger polling
**Avoids:** CPU-GPU sync latency (proper async patterns), polling overhead (dirty flags)
**Research needed:** MEDIUM -- Hilbert curve implementation has good references, but font encoding specifics need validation

### Phase 3: Visual Interface
**Rationale:** Users need a way to interact with the system. Visual Shell provides process management, Memory Browser shows state, Visual IDE enables program creation.
**Delivers:** Interactive glyph grid editor, process management UI, RAM visualization, drag-drop file loading
**Addresses:** Basic shell/CLI, visual programming IDE, state visibility
**Uses:** VisualIDE.js, VisualShell.js, MemoryBrowser.js
**Implements:** Canvas 2D rendering, PCB inspection, process spawn/kill/status
**Research needed:** LOW -- standard web UI patterns, existing components to extend

### Phase 4: Multi-Process and IPC
**Rationale:** True OS behavior requires multiple processes running concurrently and communicating. This differentiates from single-process compute shaders.
**Delivers:** Multi-process scheduler, IPC mailboxes, process groups, agent system foundation
**Addresses:** Multi-process on GPU, IPC, agent system
**Uses:** PCB table iteration, shared RAM mailboxes, atomic flags
**Avoids:** GPU memory race conditions (atomic operations), message corruption (sequence numbers)
**Research needed:** HIGH -- GPU synchronization patterns are complex, need deeper research on atomic operations in WGSL

### Phase 5: Desktop Integration
**Rationale:** Unified desktop composes all components into cohesive experience. 3D spatial placement in Hilbert landscape provides navigation.
**Delivers:** GeometryOS.js unified desktop, AgentManager, 3D unified desktop, window management
**Addresses:** 3D unified desktop, agent system
**Uses:** All prior components
**Research needed:** MEDIUM -- 3D spatial navigation patterns, agent coordination

### Phase Ordering Rationale

1. **Kernel first:** Nothing works without SPIR-V interpretation. All dependencies flow from this.
2. **Execution before UI:** Can't build UI for something that doesn't run.
3. **Visual Interface before Multi-process:** Simpler to debug single-process before adding concurrency complexity.
4. **Multi-process before Desktop:** Desktop composes processes; need IPC working first.
5. **Desktop last:** Integration layer that requires all underlying pieces.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 2:** Font encoding specifics (SPIR-V in font files, glyph atlas compression)
- **Phase 4:** GPU synchronization (atomic operations in WGSL, mailbox protocol, race condition prevention)

Phases with standard patterns (skip research-phase):
- **Phase 1:** SPIR-V interpretation is well-documented, existing kernel.wgsl to extend
- **Phase 3:** Standard web UI patterns, existing components to extend
- **Phase 5:** Integration layer, standard composition patterns

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | WebGPU/wgpu/fonttools are production-ready, official docs verified |
| Features | MEDIUM | Novel domain, table stakes derived from embedded OS and visual programming domains |
| Architecture | MEDIUM | No prior art for font-based SPIR-V OS, but GPU compute patterns well-documented |
| Pitfalls | MEDIUM | Based on GPU systems research and WebGPU docs, but timing thresholds may vary by hardware |

**Overall confidence:** MEDIUM

### Gaps to Address

- **SPIR-V in font files:** Can we embed .spv binaries in OpenType `glyf` tables, or need custom table? Needs experimentation during Phase 2.
- **Font atlas compression:** Is PNG atlas sufficient, or need ASTC/BC7 for GPU upload? Profile first during Phase 2.
- **Process isolation:** WebGPU has no process isolation -- how to safely run untrusted font-programs? Security research needed for v2.
- **GPU synchronization patterns:** Atomic operations in WGSL for mailbox management need validation during Phase 4.
- **Performance thresholds:** Specific timing (100 instructions/slice, 2-second timeout) may vary by hardware -- need real-world testing.

## Sources

### Primary (HIGH confidence)
- [Khronos SPIR-V Registry](https://registry.khronos.org/SPIR-V/) -- SPIR-V specification, instruction encoding
- [WebGPU Specification](https://www.w3.org/TR/webgpu/) -- WebGPU API, buffer mapping, async patterns
- [Vulkan SDK 1.4.309.0](https://www.lunarg.com/vulkan-sdk/) -- LunarG official release
- [SPIRV-Tools GitHub](https://github.com/KhronosGroup/SPIRV-Tools) -- Khronos Group
- [wgpu GitHub](https://github.com/gfx-rs/wgpu) -- gfx-rs project
- [fonttools GitHub](https://github.com/fonttools/fonttools) -- fonttools project
- [Vitest Official Docs](https://vitest.dev/) -- Testing framework

### Secondary (MEDIUM confidence)
- WebGPU browser support status (Chrome 113+, Safari 17.4+, Firefox 141+) -- Multiple sources
- spirv-opt optimization flags -- CSDN/Linux From Scratch guides
- Hilbert curve GPU applications -- CVPR 2024, arXiv 2025 papers
- Embedded OS process management -- Baidu Baike
- IPC mechanisms overview -- CSDN blog posts
- Visual programming environment features -- CSDN blogs
- GPU scheduling limitations -- SOSP 2024, OSDI 2025 papers
- NVIDIA GPU Preemption Research (OSDI'22) -- REEF: microsecond-scale preemption challenges
- Microsoft IOMMU GPU Isolation -- Hardware isolation limitations

### Tertiary (LOW confidence)
- iOS WebGPU support timeline (early 2026) -- Single source
- Specific performance benchmarks -- Blog posts, not official
- Specific timing thresholds -- May vary by hardware
- Font-based execution adoption -- Novel concept, no market data

---
*Research completed: 2026-03-02*
*Ready for roadmap: yes*
