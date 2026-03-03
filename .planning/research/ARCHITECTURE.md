# Architecture Research

**Domain:** GPU-native operating systems with font-based execution
**Researched:** 2026-03-02
**Confidence:** MEDIUM (novel domain, limited prior art)

## Standard Architecture

### System Overview

Geometry OS follows a layered architecture where the GPU serves as the primary compute substrate. Unlike traditional OS architectures that treat GPU as an accelerator, here the GPU IS the execution environment.

```
+------------------------------------------------------------------+
|                      PRESENTATION LAYER                           |
|  +----------------+  +----------------+  +----------------+       |
|  |  Visual Shell  |  |  File Manager  |  |  Memory Browser|       |
|  +-------+--------+  +-------+--------+  +-------+--------+       |
|          |                  |                   |                 |
+----------|------------------|-------------------|-----------------+
|          v                  v                   v                 |
|  +------------------------------------------------------------+   |
|  |                    DESKTOP ENVIRONMENT                      |   |
|  |  (GeometryOS.js - Window management, IPC routing, agents)   |   |
|  +--------------------------+---------------------------------+   |
|                             |                                     |
+-----------------------------|-------------------------------------+
|                             v                                     |
|  +------------------------------------------------------------+   |
|  |                    APPLICATION LAYER                        |   |
|  |  +-------------+  +-------------+  +-------------+          |   |
|  |  | VisualIDE   |  | SpirvRunner |  | SoundSystem |          |   |
|  |  +------+------+  +------+------+  +------+------+          |   |
|  +---------|--------------|--------------|--------------------+   |
|            |              |              |                       |
+------------|--------------|--------------|-----------------------+
|            v              v              v                       |
|  +------------------------------------------------------------+   |
|  |                  KERNEL SERVICES LAYER                      |   |
|  |  +---------------+  +---------------+  +-----------------+  |   |
|  |  | Process Mgmt  |  | Memory Mgmt   |  | IPC/Msg Passing |  |   |
|  |  +-------+-------+  +-------+-------+  +--------+--------+  |   |
|  +----------|----------------|--------------------|------------+   |
|             |                |                    |               |
+-------------|----------------|--------------------|---------------+
|             v                v                    v               |
|  +------------------------------------------------------------+   |
|  |                    GPU KERNEL LAYER                         |   |
|  |  (kernel.wgsl - SPIR-V interpreter, scheduler, PCB table)   |   |
|  +--------------------------+---------------------------------+   |
|                             |                                     |
+-----------------------------|-------------------------------------+
|                             v                                     |
|  +------------------------------------------------------------+   |
|  |                    HARDWARE LAYER                           |   |
|  |  +----------------+  +----------------+  +----------------+ |   |
|  |  | WebGPU Device  |  | Storage Buffers|  | Compute Shader | |   |
|  |  | (Vulkan/Metal) |  | (RAM/Stack/PCB)|  |   Pipeline     | |   |
|  |  +----------------+  +----------------+  +----------------+ |   |
|  +------------------------------------------------------------+   |
+------------------------------------------------------------------+
```

### Component Responsibilities

| Component | Responsibility | Implementation |
|-----------|----------------|----------------|
| **kernel.wgsl** | SPIR-V bytecode interpreter, multi-process scheduling, IPC mailbox handling | WGSL compute shader |
| **executor.wgsl** | Single-process SPIR-V execution (simpler case) | WGSL compute shader |
| **GeometryKernel.js** | Process lifecycle management, PCB buffer management, shared memory allocation | JavaScript WebGPU wrapper |
| **SpirvRunner.js** | Per-process state persistence, memory-mapped I/O handling | JavaScript WebGPU wrapper |
| **VisualCompiler.js** | Grid-to-SPIR-V compilation, Hilbert curve traversal, opcode emission | JavaScript compiler |
| **VisualIDE.js** | Interactive glyph grid editor, PNG import/export | Canvas 2D renderer |
| **VisualShell.js** | Process spawning UI, PCB inspection, dashboard metrics | UI controller |
| **GeometryOS.js** | Unified desktop substrate, window management, agent coordination | Root controller |
| **GeometryFont.js** | Font atlas loading, glyph rendering, semantic color decoding | Font renderer |

## Recommended Project Structure

```
web/
+-- kernel.wgsl              # Multi-process GPU scheduler (THE HEART)
+-- executor.wgsl            # Single-process SPIR-V interpreter
+-- GeometryKernel.js        # Kernel controller (spawn, step, readPCBs)
+-- SpirvRunner.js           # Process runner with persistent state
+-- VisualCompiler.js        # Grid -> SPIR-V compiler
+-- VisualIDE.js             # Glyph grid editor
+-- VisualShell.js           # Process management UI
+-- GeometryOS.js            # Unified desktop root
+-- GeometryFont.js          # Font atlas renderer
+-- SoundSystem.js           # Memory-mapped audio output
+-- MemoryBrowser.js         # GPU RAM visualization
+-- ProcessManager.js        # Process lifecycle API
+-- AgentManager.js          # 7 Area Agents coordination
+-- agents/
|   +-- AgentGenerator.js    # SPIR-V agent generation
|   +-- index.js             # Agent exports
|   +-- *.spv                # Pre-compiled agent binaries
+-- assets/
|   +-- universal_font.spv   # Font as executable
|   +-- universal_font.rts.png # Font atlas texture
|   +-- glyph_info.json      # Glyph metadata
```

### Structure Rationale

- **kernel.wgsl at root**: The GPU kernel is the foundation; everything else is a service
- **agents/ as SPIR-V**: Agents ARE programs, compiled to SPIR-V like user programs
- **assets/ as executable**: Font files are not just data - they are programs (universal_font.spv)

## Architectural Patterns

### Pattern 1: Storage Buffer State Machine

**What:** The entire OS state lives in WebGPU storage buffers, read/written by compute shaders.

**When to use:** Always - this is the core pattern for GPU-native execution.

**Example:**
```wgsl
// All state in storage buffers
@group(0) @binding(0) var<storage, read_write> program: array<u32>;  // Code
@group(0) @binding(1) var<storage, read_write> stack: array<f32>;    // Execution stack
@group(0) @binding(2) var<storage, read_write> ram: array<f32>;      // Process memory
@group(0) @binding(3) var<storage, read_write> pcb_table: array<Process>; // PCBs
```

**Trade-offs:**
- (+) All state accessible from GPU parallel execution
- (+) No CPU-GPU sync during compute passes
- (-) CPU must explicitly map/read buffers to observe state
- (-) Debugging requires staging buffer copies

### Pattern 2: Cooperative Multitasking on GPU

**What:** The kernel iterates through PCB entries in a single compute shader dispatch, giving each process a time slice.

**When to use:** When you need multi-process execution without CPU intervention.

**Example:**
```wgsl
@compute @workgroup_size(1)
fn main() {
    let process_count = arrayLength(&pcb_table);

    for (var p_idx: u32 = 0u; p_idx < process_count; p_idx++) {
        var p = pcb_table[p_idx];
        if (p.status != RUNNING) { continue; }

        // Execute up to MAX_INST_PER_SLICE instructions
        for (var i = 0u; i < 100u; i++) {
            // ... interpret instruction at program[p.pc]
            p.pc += instruction_length;
        }

        pcb_table[p_idx] = p;  // Save state back
    }
}
```

**Trade-offs:**
- (+) No CPU scheduling overhead
- (+) Deterministic time slicing
- (-) Single workgroup limits parallelism
- (-) Long-running shaders may timeout

### Pattern 3: Hilbert Curve Memory Layout

**What:** Memory addresses are laid out using Hilbert curve ordering to preserve spatial locality.

**When to use:** For visual RAM, memory atlases, and spatial data structures.

**Example:**
```javascript
class HilbertCurve {
    d2xy(d) {
        // Convert 1D Hilbert distance to 2D coordinates
        let x = 0, y = 0, s = 1, t = d;
        while (s < this.size) {
            const rx = 1 & (t >> 1);
            const ry = 1 & (t ^ rx);
            // ... rotation logic
            x += s * rx;
            y += s * ry;
            t >>= 2;
            s <<= 1;
        }
        return { x, y };
    }
}
```

**Trade-offs:**
- (+) Adjacent memory locations are spatially nearby
- (+) Better cache coherence for 2D operations
- (-) More complex address calculation than row-major
- (-) Not all access patterns benefit

### Pattern 4: Memory-Mapped I/O via RAM Regions

**What:** Specific RAM addresses trigger external behavior when written by GPU programs.

**When to use:** For audio output, display updates, syscall interfaces.

**Example:**
```javascript
// In SpirvRunner.js - CPU side
const trigger = state.ram[126];  // Sound trigger
if (trigger !== 0) {
    const frequency = state.ram[125];
    const duration = state.ram[124];
    const volume = state.ram[123];
    this.soundSystem.playTone(frequency, duration, volume);
    state.ram[126] = 0;  // Clear trigger
}
```

```wgsl
// In executor.wgsl - GPU side
else if (opcode == 201u) { // OP_TONE
    ram[125] = stack[sp - 3]; // frequency
    ram[124] = stack[sp - 2]; // duration
    ram[123] = stack[sp - 1]; // volume
    ram[126] = 1.0;           // trigger CPU
    sp = sp - 3;
}
```

**Trade-offs:**
- (+) Clean separation of GPU compute and CPU I/O
- (+) Programs use same load/store instructions
- (-) Polling required (no GPU->CPU interrupts)
- (-) Latency depends on read frequency

### Pattern 5: Font-as-Program

**What:** Glyphs are not just visual - they encode semantic operations via color channels.

**When to use:** For visual programming where appearance = behavior.

**Example:**
```javascript
// Glyph color channels encode semantics:
// R channel: category (operator, data, control, syscall)
// G channel: operation within category
// B channel: immediate value or address

if (g === 0) {
    // Data category - B is the literal value
    emit(OP_CONSTANT, [floatId, rid, b]);
} else if (g === 0x6A) {
    // Operator category - 0x6A = FADD
    emit(OP_FADD, [floatId, rid, v1, v2]);
} else if (g === 0x72) {
    // Memory category - B is address
    emit(OP_STORE, [b, v1]);
}
```

**Trade-offs:**
- (+) Programs are human-readable as images
- (+) Can edit programs in image editors
- (+) Self-documenting via visual appearance
- (-) Limited to 256 operations per category
- (-) Complex operations need multi-glyph sequences

## Data Flow

### Program Execution Flow

```
[User draws glyphs in VisualIDE]
           |
           v
[VisualCompiler converts grid to SPIR-V]
           |
           v
[SpirvRunner loads binary to GPU storage buffer]
           |
           v
[GPU compute shader interprets SPIR-V opcodes]
           |
           v
[Results written to RAM storage buffer]
           |
           v
[CPU reads RAM via staging buffer]
           |
           v
[Memory-mapped I/O triggers side effects]
```

### IPC Message Flow

```
[Process A calls MSG_SEND]
           |
           v
[Kernel writes to Process B's mailbox (shared RAM)]
           |
           v
[Kernel wakes Process B if waiting]
           |
           v
[Process B calls MSG_RECV]
           |
           v
[Kernel reads mailbox, pushes to Process B's stack]
```

### State Management Flow

```
[CPU initializes PCB table]
           |
           v
[GPU kernel dispatch reads/writes PCB entries]
           |
           v
[CPU periodically reads PCBs via staging buffer]
           |
           v
[VisualShell updates UI with process states]
           |
           v
[User actions modify PCB entries (kill, pause, etc.)]
           |
           v
[CPU writes changes back to GPU buffer]
```

### Key Data Flows

1. **Boot Flow:** CPU loads font atlas -> generates kernel buffers -> dispatches kernel -> reads PCBs
2. **Spawn Flow:** User drops .spv file -> CPU writes binary to program buffer -> creates PCB entry -> GPU executes
3. **IPC Flow:** GPU writes mailbox -> CPU observes via shared RAM read -> GPU receiver reads mailbox
4. **I/O Flow:** GPU writes trigger address -> CPU polls and detects -> CPU executes side effect -> CPU clears trigger

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| 1-16 processes | Current architecture (single workgroup, linear PCB iteration) |
| 16-64 processes | Increase MAX_MAILBOXES, partition RAM by sector, use Hilbert sector map |
| 64+ processes | Multiple workgroups with process groups, hierarchical scheduler |

### Scaling Priorities

1. **First bottleneck:** Single workgroup limits parallelism. Solution: Dispatch multiple workgroups with process groups.
2. **Second bottleneck:** RAM size (256KB current). Solution: Virtual memory with swap to CPU-side storage.
3. **Third bottleneck:** Message queue depth (single message per mailbox). Solution: Ring buffer mailboxes.

## Anti-Patterns

### Anti-Pattern 1: CPU-Centric Scheduling

**What people do:** Run scheduling logic on CPU, dispatch GPU for each process step.

**Why it's wrong:** CPU-GPU sync overhead dominates. Each dispatch has ~0.1ms latency.

**Do this instead:** Batch all process execution in a single GPU dispatch. Let the kernel iterate.

### Anti-Pattern 2: Separate Buffers Per Process

**What people do:** Create new GPU buffers for each process spawn.

**Why it's wrong:** Buffer allocation is expensive. Fragmentation. No shared memory.

**Do this instead:** Pre-allocate large buffers (program, stack, RAM, PCB). Use offsets for process isolation.

### Anti-Pattern 3: Immediate Mode GPU->CPU Communication

**What people do:** Try to get GPU results immediately after dispatch.

**Why it's wrong:** WebGPU is async-only. mapAsync() returns a Promise. No synchronous reads.

**Do this instead:** Poll on next frame. Use staging buffers. Accept 1-frame latency.

### Anti-Pattern 4: Treating Font as Pure Data

**What people do:** Load font atlas as texture only, hardcode glyph meanings.

**Why it's wrong:** Loses the self-describing nature. Programs become opaque.

**Do this instead:** Encode semantics in color channels. Compile font metadata. Make glyphs executable.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| **Audio API** | Memory-mapped I/O (RAM[123-126]) | Polling-based, no interrupts |
| **File System** | Drag-drop .spv files | Browser File API, no native FS |
| **Network** | Agent-based (network.spv) | IPC to network agent process |
| **Display** | Canvas 2D + WebGPU | Separate visual RAM buffer |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| **kernel.wgsl <-> GeometryKernel.js** | Storage buffers, staging copies | Async readback via mapAsync |
| **GeometryKernel.js <-> VisualShell.js** | JavaScript method calls | PCB data passed as arrays |
| **VisualIDE.js <-> VisualCompiler.js** | Grid array -> SPIR-V binary | Synchronous compilation |
| **VisualCompiler.js <-> SpirvRunner.js** | ArrayBuffer (SPIR-V) | Single-pass execution |
| **Processes (IPC)** | Shared RAM mailboxes | Cooperative, no preemption |

## Build Order Implications

Based on the architecture, the following build order is recommended:

### Phase 1: Core Kernel (Foundation)
1. **kernel.wgsl** - The SPIR-V interpreter is the heart of the system
2. **executor.wgsl** - Simpler single-process version for testing
3. **GeometryKernel.js** - CPU-side kernel controller

**Dependency rationale:** Nothing can run without the interpreter. Must be first.

### Phase 2: Execution Environment
4. **SpirvRunner.js** - Persistent process execution
5. **VisualCompiler.js** - Grid-to-SPIR-V compilation
6. **GeometryFont.js** - Glyph loading and semantic decoding

**Dependency rationale:** Need kernel first. Compilation produces SPIR-V that kernel runs.

### Phase 3: Visual Interface
7. **VisualIDE.js** - Glyph grid editor
8. **VisualShell.js** - Process management UI
9. **MemoryBrowser.js** - RAM visualization

**Dependency rationale:** Needs compiler and runner to test programs.

### Phase 4: Desktop Integration
10. **GeometryOS.js** - Unified desktop
11. **AgentManager.js** - System agents
12. **SoundSystem.js** - Audio I/O

**Dependency rationale:** Desktop composes existing components. Agents are special processes.

## Sources

- [Khronos SPIR-V Registry](https://registry.khronos.org/SPIR-V/) - Official SPIR-V specification
- [WebGPU Storage Buffer Tutorial](http://juejin.cn/entry/7497075904108118051) - Buffer management patterns
- [CUDA IPC Architecture](https://blog.csdn.net/HaoZiHuang/article/details/151760864) - GPU inter-process communication
- [Hilbert Curve GPU Computing](http://ch.whu.edu.cn/article/doi/10.13203/j.whugis20220142) - Spatial memory layout
- [Visual Programming Architecture](https://www.sciencedirect.com/topics/computer-science/visual-programming) - Glyph-based systems
- [SPIRV-VM Project](https://gitcode.com/gh_mirrors/sp/SPIRV-VM) - CPU-side SPIR-V execution reference
- [Font Atlas Generation](https://m.blog.csdn.net/gitblog_00189/article/details/151881708) - Texture atlas patterns

---
*Architecture research for: GPU-native operating systems with font-based execution*
*Researched: 2026-03-02*
