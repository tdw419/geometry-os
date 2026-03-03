# Feature Research

**Domain:** GPU-native operating systems with font-based execution
**Researched:** 2026-03-02
**Confidence:** MEDIUM (novel domain, limited direct precedent)

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Process spawning** | "OS" implies running programs | MEDIUM | Load SPIR-V, create PCB entry, dispatch kernel |
| **Memory management** | All programs need memory | HIGH | Allocate/deallocate RAM sectors, prevent corruption |
| **Basic shell/CLI** | Interaction with running system | MEDIUM | Process list, spawn commands, kill, status |
| **Program execution** | Core promise: fonts run programs | HIGH | SPIR-V interpreter in GPU compute shader |
| **Error handling** | Programs crash; OS should recover | MEDIUM | Catch GPU errors, report back, clean up PCB |
| **State visibility** | Debugging requires seeing what's happening | LOW | PCB inspection, memory browser, process states |
| **File loading** | Need to get programs into the system | LOW | Drag-drop .spv files, browser File API |
| **I/O of some kind** | Programs that can't output are useless | MEDIUM | Memory-mapped I/O to CPU-side handlers |

### Differentiators (Competitive Advantage)

Features that set the product apart. Not required, but valuable.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Font-as-program** | Programs are visual, editable as images, self-describing | HIGH | Glyph color channels encode opcodes/operands |
| **Visual programming IDE** | Draw programs instead of typing code | HIGH | Grid editor with semantic color meaning |
| **GPU-native execution** | True parallelism, no CPU bottleneck | HIGH | Entire OS runs on GPU via compute shaders |
| **Hilbert curve memory** | Spatial locality preserved, visual RAM coherent | MEDIUM | 2D grid -> 1D linear with locality |
| **Multi-process on GPU** | Cooperative multitasking without CPU | HIGH | Kernel iterates PCBs in single dispatch |
| **Memory-mapped I/O** | Clean GPU/CPU separation, same instruction set | MEDIUM | Specific RAM addresses trigger CPU handlers |
| **3D unified desktop** | Processes exist in spatial Hilbert landscape | HIGH | Windows as 3D objects in memory space |
| **Agent system** | Pre-built system processes (audio, network, etc.) | MEDIUM | SPIR-V binaries providing OS services |
| **Self-hosting potential** | Kernel can compile its own tools | VERY HIGH | Long-term goal, not V1 |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Preemptive multitasking** | "Real OS" has preemption | WebGPU has no interrupts; GPU dispatch is atomic | Cooperative multitasking with time slices |
| **Virtual memory / paging** | Unlimited process memory | Complex, slow on GPU, defeats visual RAM concept | Fixed-size sectors, Hilbert-partitioned |
| **File system on disk** | "OS needs files" | Browser sandbox prevents real FS; adds complexity | Drag-drop .spv, memory-only filesystem |
| **User accounts / security** | Multi-user expected | GPU has no isolation; WebGPU shares device | Single-user, trusted code only (V1) |
| **Network stack** | Modern OS needs network | Massive scope, socket API complex | IPC to network agent (future) |
| **GUI toolkit** | Users want buttons/windows | Out of scope for kernel-focused OS | Canvas rendering, simple overlays |
| **Real-time guarantees** | Deterministic timing | WebGPU is not real-time; browser schedules | Best-effort, visualize timing in shell |
| **POSIX compliance** | Run existing software | Defeats novel architecture; SPIR-V != ELF | Custom ABI, write new programs |

## Feature Dependencies

```
[Process Execution]
    └──requires──> [SPIR-V Interpreter (kernel.wgsl)]
                       └──requires──> [WebGPU Device + Storage Buffers]

[Visual IDE]
    └──requires──> [Visual Compiler]
                       └──requires──> [GeometryFont (semantic decoding)]
                                          └──requires──> [Font Atlas]

[Multi-process]
    └──requires──> [PCB Table in GPU Memory]
                       └──requires──> [Process Scheduler]
                                          └──requires──> [Cooperative Multitasking Pattern]

[IPC]
    └──requires──> [Shared RAM Mailboxes]
                       └──requires──> [Multi-process Support]

[Memory-mapped I/O]
    └──requires──> [CPU Polling Loop]
                       └──requires──> [Staging Buffer Readback]

[Visual Shell]
    └──requires──> [PCB Reader]
    └──requires──> [Memory Browser]
    └──enhances──> [Process Management]

[3D Desktop]
    └──requires──> [Hilbert Sector Map]
    └──enhances──> [All Components (spatial placement)]

[Sound System]
    └──requires──> [Memory-mapped I/O]
    └──conflicts──> [None (orthogonal)]
```

### Dependency Notes

- **Process Execution requires SPIR-V Interpreter:** Cannot run programs without the kernel.wgsl compute shader that interprets opcodes.
- **Visual IDE requires Visual Compiler:** Drawing glyphs is useless without compilation to executable SPIR-V.
- **Multi-process requires PCB Table:** Process state must be stored in GPU-accessible memory for scheduling.
- **IPC requires Shared RAM:** Processes cannot share memory without a common RAM buffer and mailbox protocol.
- **Memory-mapped I/O requires CPU Polling:** GPU cannot interrupt CPU; CPU must poll RAM for trigger addresses.
- **3D Desktop enhances All Components:** Spatial placement is optional but dramatically improves UX when present.
- **Sound System conflicts with None:** Audio is orthogonal; it uses memory-mapped I/O but doesn't block other features.

## MVP Definition

### Launch With (v1)

Minimum viable product -- what's needed to validate the concept.

- [x] **kernel.wgsl SPIR-V interpreter** -- Core execution engine; nothing works without this
- [x] **executor.wgsl single-process variant** -- Simpler testing, proves interpreter works
- [x] **GeometryKernel.js** -- CPU-side controller for kernel dispatch and PCB management
- [x] **SpirvRunner.js** -- Persistent process execution with state readback
- [x] **VisualCompiler.js** -- Grid-to-SPIR-V compilation; enables visual programming
- [x] **GeometryFont.js** -- Glyph loading and semantic color decoding
- [x] **Basic Visual Shell** -- Process list, spawn, kill, status display
- [x] **Memory Browser** -- Visualize GPU RAM contents
- [x] **Memory-mapped I/O (basic)** -- At minimum: sound output to prove I/O works
- [x] **File loading (drag-drop .spv)** -- Get programs into the system

**Why these:** Without any of these, you cannot demonstrate "a GPU-native OS where fonts are executable programs." These form the minimal proof-of-concept.

### Add After Validation (v1.x)

Features to add once core is working.

- [ ] **Multi-process scheduler** -- Currently single-process; add PCB table iteration
- [ ] **IPC mailboxes** -- Inter-process communication via shared RAM
- [ ] **Agent system (pre-built services)** -- Audio agent, memory agent, etc.
- [ ] **3D unified desktop (GeometryOS.js)** -- Spatial window management
- [ ] **Sound System (full)** -- Multiple voices, envelopes, samples
- [ ] **Visual IDE (full grid editor)** -- Interactive glyph editing with compilation
- [ ] **Syscall interface** -- Formalized syscall numbers and conventions
- [ ] **Process groups / hierarchies** -- Parent-child relationships

**Triggers for adding:**
- Multi-process: When you need to demonstrate IPC or concurrent tasks
- Agents: When kernel services need to be user-accessible programs
- 3D Desktop: When shell feels cramped and navigation needs improvement
- Sound System: When demo needs audio beyond basic tones
- Visual IDE: When users want to create programs without external tools

### Future Consideration (v2+)

Features to defer until product-market fit is established.

- [ ] **Self-hosting compiler** -- Compiler written in Geometry OS itself
- [ ] **Network stack** -- IPC to network agent, socket API
- [ ] **File system (in-memory)** -- Hierarchical file organization
- [ ] **Persistence** -- Save/load state between sessions
- [ ] **Desktop native build** -- wgpu-native for desktop distribution
- [ ] **Process isolation / security** -- Protection between processes
- [ ] **Debugging tools (breakpoints, step)** -- Developer experience
- [ ] **Performance profiling** -- GPU timing, memory usage stats
- [ ] **Multi-user support** -- User accounts, permissions

**Why defer:**
- Self-hosting: Requires complete toolchain; massive effort
- Network: Complex, many edge cases, not core value proposition
- File system: Browser sandbox limits; memory-only is simpler
- Persistence: LocalStorage is sufficient for MVP; real FS is v2
- Desktop native: WebGPU works; desktop is optimization, not requirement
- Security: Trusted code assumption for V1; security is v2+
- Debugging: Nice to have but not required for proof-of-concept
- Profiling: Performance is good enough for demo; optimize later
- Multi-user: Single-user is acceptable for experimental OS

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| SPIR-V interpreter (kernel.wgsl) | HIGH | HIGH | P1 |
| Process spawning/execution | HIGH | MEDIUM | P1 |
| Basic visual shell | HIGH | MEDIUM | P1 |
| Memory-mapped I/O (sound) | MEDIUM | LOW | P1 |
| File loading (.spv) | HIGH | LOW | P1 |
| Visual compiler | HIGH | HIGH | P1 |
| Multi-process scheduler | MEDIUM | HIGH | P2 |
| IPC mailboxes | MEDIUM | MEDIUM | P2 |
| Agent system | MEDIUM | MEDIUM | P2 |
| 3D unified desktop | LOW | HIGH | P3 |
| Full sound system | LOW | MEDIUM | P3 |
| Visual IDE (full) | MEDIUM | HIGH | P2 |
| Network stack | LOW | VERY HIGH | P3 |
| Self-hosting | LOW | VERY HIGH | P3 |

**Priority key:**
- P1: Must have for launch (MVP)
- P2: Should have, add when possible (v1.x)
- P3: Nice to have, future consideration (v2+)

## Competitor Feature Analysis

Note: There are no direct competitors for "GPU-native OS with font-based execution." This is a novel architecture. Competitors are partial overlaps.

| Feature | Traditional OS (Linux) | Visual Programming (LabVIEW) | Compute Shaders (Unity) | Geometry OS |
|---------|------------------------|------------------------------|-------------------------|-------------|
| Execution substrate | CPU | CPU | GPU (accelerator) | GPU (native) |
| Programming model | Text (C, etc.) | Visual dataflow | Text (HLSL) | Visual glyphs |
| Memory model | Virtual memory, paging | Flat arrays | GPU buffers | Hilbert-partitioned visual RAM |
| Multitasking | Preemptive | Cooperative (dataflow) | N/A (single dispatch) | Cooperative (GPU-scheduled) |
| I/O model | System calls, drivers | Nodes/wires | CPU callbacks | Memory-mapped I/O |
| Process isolation | Hardware-enforced | None | None | None (V1) |
| Self-hosting | Yes | No | No | Future goal |

**Key differentiator:** Geometry OS is the only system where:
1. GPU is the primary compute substrate (not accelerator)
2. Programs are encoded as visual glyphs (fonts are executable)
3. Memory has spatial locality (Hilbert curve)
4. Visual RAM is a first-class concept

## Sources

**HIGH Confidence (Official/Verified):**
- [Khronos SPIR-V Registry](https://registry.khronos.org/SPIR-V/) - SPIR-V specification
- [WebGPU Specification](https://www.w3.org/TR/webgpu/) - WebGPU API
- [Vulkan SDK Documentation](https://www.lunarg.com/vulkan-sdk/) - GPU tooling

**MEDIUM Confidence (WebSearch verified):**
- [Embedded OS Process Management](https://baike.baidu.com/item/%E8%BF%9B%E7%A8%8B%E7%AE%A1%E7%90%86) - Minimum OS features
- [IPC Mechanisms Overview](https://blog.csdn.net/) - Inter-process communication patterns
- [Visual Programming Environment Features](https://m.blog.csdn.net/) - Visual programming expectations
- [Shell Features](https://baike.baidu.com/item/shell) - CLI expectations
- [Esoteric Programming Languages](https://arxiv.org/html/2505.15327v1) - Novel programming paradigms

**LOW Confidence (Needs validation):**
- Specific GPU OS patterns (no direct precedent exists)
- Font-based execution adoption (novel concept, no market data)
- User expectations for visual OS (assumed from visual programming tools)

**Domain-specific insights:**
- Novel architecture means no direct competitors; table stakes derived from embedded OS and visual programming domains
- GPU-native execution is the key differentiator; all features should support this
- Font-as-program is the "wow factor" that makes this memorable; prioritize visual aspects

---
*Feature research for: GPU-native operating systems with font-based execution*
*Researched: 2026-03-02*
