# Geometry Kernel: Process Manager Design

**Date:** 2026-03-02
**Status:** Approved
**Approach:** Hybrid (JavaScript + WebGPU)

## Overview

Build a process manager for Geometry OS that can load, isolate, and schedule multiple visual programs (.spv). Implement memory isolation so each process has its own region in Visual RAM, with a shared memory segment for inter-process communication (IPC).

## Goals

1. Load multiple .spv programs as separate processes
2. Isolate memory: Process 0 gets 0-999, Process 1 gets 1000-1999, etc.
3. Schedule execution with round-robin algorithm
4. Demonstrate IPC via shared memory region

## Architecture

### Memory Layout

Visual RAM is partitioned into private regions plus shared memory:

```
┌─────────────────────────────────────────────┐
│ 0-999:     Process 0 Private Memory        │
├─────────────────────────────────────────────┤
│ 1000-1999: Process 1 Private Memory        │
├─────────────────────────────────────────────┤
│ 2000-2999: Process 2 Private Memory        │
├─────────────────────────────────────────────┤
│ ...                                         │
├─────────────────────────────────────────────┤
│ 9000-9999: Process 9 Private Memory        │
├─────────────────────────────────────────────┤
│ 10000-10999: Shared Memory Region (IPC)    │
└─────────────────────────────────────────────┘
```

**Total:** 11,000 floats = 44KB

### Process Control Block (PCB)

Each process has:

```javascript
{
    pid: number,           // Process ID (0-9)
    name: string,          // Process name
    state: string,         // 'ready' | 'running' | 'blocked' | 'terminated'
    program: Uint32Array,  // SPIR-V binary
    memoryBase: number,    // pid * 1000
    memorySize: number,    // 1000
    pc: number,            // Program counter
    priority: number,      // 0=highest
    ticks: number,         // CPU ticks consumed
    stack: Float32Array,   // Private stack (1024 floats)
}
```

### Components

#### 1. ProcessManager (`web/ProcessManager.js`)

```javascript
class ProcessManager {
    processes: Map<number, Process>
    sharedMemory: Float32Array  // 1000 floats
    scheduler: Scheduler
    runner: SpirvRunner

    async loadProgram(pid, spirvPath): Promise<void>
    async startProcess(pid): Promise<void>
    async terminateProcess(pid): Promise<void>
    async tick(): Promise<void>  // Execute one scheduling cycle
    writeSharedMemory(offset, value): void
    readSharedMemory(offset): number
}
```

#### 2. Scheduler (`web/Scheduler.js`)

```javascript
class Scheduler {
    quantum: number  // Max cycles per process (default: 100)

    getNext(processes: Map<number, Process>): Process | null
    shouldPreempt(process: Process, cycles: number): boolean
}
```

**Algorithm:** Round-robin with priority boost
- Each process runs up to `quantum` cycles
- Blocked processes are skipped
- Priority boost every 10 ticks to prevent starvation

#### 3. SpirvRunner Modifications

Add process context to execution:

```javascript
async execute(processId, spirvBinary, labelMap, {
    memoryBase: number,
    memoryLimit: number,
    sharedMemory: Float32Array
})
```

#### 4. Executor WGSL Modifications

Add bounds-checked memory access:

```wgsl
@group(0) @binding(6) var<uniform> process_base: u32;
@group(0) @binding(7) var<uniform> process_limit: u32;

// Modified VISUAL_STORE (opcode 205)
if (opcode == 205u) {
    let local_addr = program[pc + 1];
    let global_addr = process_base + local_addr;
    if (global_addr < process_limit && sp >= 1u) {
        visual_ram[global_addr] = stack[sp - 1];
        sp = sp - 1;
    }
}
```

## Demo Programs

### Producer (Process 0)

Writes incrementing counter to shared memory:

```python
# Generates producer.spv
# Pseudo-assembly:
#   PUSH 0         # counter = 0
# LABEL loop:
#   DUP
#   PUSH 1
#   ADD            # counter++
#   PUSH 10000     # shared memory base
#   OVER
#   ADD            # addr = 10000 + counter
#   SWAP
#   VISUAL_STORE   # visual_ram[addr] = counter
#   JMP loop
```

### Consumer (Process 1)

Reads from shared memory, plays tone:

```python
# Generates consumer.spv
# Pseudo-assembly:
# LABEL check:
#   PUSH 10000
#   VISUAL_LOAD    # load shared[0]
#   DUP
#   PUSH 440
#   ADD            # freq = 440 + value
#   PUSH 0.1       # duration
#   PUSH 0.5       # volume
#   TONE
#   JMP check
```

## File Changes

### New Files

| File | Purpose |
|------|---------|
| `web/ProcessManager.js` | Process lifecycle & scheduling |
| `web/Scheduler.js` | Round-robin scheduler |
| `web/demo_multiprocess.html` | Demo page |
| `tests/test_process_manager.py` | Generate test programs |

### Modified Files

| File | Changes |
|------|---------|
| `web/SpirvRunner.js` | Add process context params |
| `web/executor.wgsl` | Bounds-checked memory, shared region |

## Implementation Steps

1. **Create ProcessManager.js** - Process class, load/start/terminate
2. **Create Scheduler.js** - Round-robin algorithm
3. **Modify SpirvRunner.js** - Pass memory bounds to executor
4. **Modify executor.wgsl** - Add process_base/limit uniforms, bounds checking
5. **Create test programs** - Producer/Consumer .spv files
6. **Create demo_multiprocess.html** - Visual demo of IPC

## Success Criteria

1. Can load two .spv programs as separate processes
2. Process 0 cannot access Process 1's memory region
3. Both processes can read/write shared memory at addresses 10000+
4. Producer increments counter, Consumer plays tones based on counter
5. System runs continuously with fair scheduling

## Future Enhancements

- GPU-only scheduler (compute shader)
- More IPC mechanisms (message queues, semaphores)
- Process priorities and real-time scheduling
- Memory protection faults
- Dynamic memory allocation within process space
