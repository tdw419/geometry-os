# Design: Geometry OS Kernel (Phase 30)

## Overview
The **Geometry Kernel** is the high-level management layer for Geometry OS. It transitions the system from a single-threaded "Visual Computer" to a multi-tasking **Visual Operating System**.

The kernel runs entirely on the GPU via WebGPU (WGSL), managing multiple independent visual programs (processes) within a shared **Visual RAM** substrate.

## Core Responsibilities

### 1. Process Management
- **Process Control Block (PCB)**: A 16-word structure in GPU RAM for each process.
  - `word 0`: Process ID (PID)
  - `word 1`: Program Counter (PC)
  - `word 2`: Stack Pointer (SP)
  - `word 3`: Memory Base (RAM Offset)
  - `word 4`: Memory Limit (RAM Size)
  - `word 5`: Status (0=Idle, 1=Running, 2=Waiting, 3=Terminated)
  - `word 6`: Priority
  - `words 7-15`: Reserved / CPU context

### 2. Memory Isolation (Segmentation)
- Each process is assigned a **Memory Base** and **Memory Limit**.
- The `GEO_VISUAL_LOAD` and `GEO_VISUAL_STORE` opcodes are modified to be relative to the process's `Memory Base`.
- Out-of-bounds access triggers a kernel panic (process termination).

### 3. Cooperative Scheduling
- The `executor.wgsl` main loop is transformed into a **Scheduler**.
- It iterates through the process list and executes N instructions per "time slice" (frame).
- A new `GEO_YIELD` opcode allows processes to voluntarily give up control.

### 4. System Calls (Syscalls)
- Processes interact with the kernel via the `GEO_SYSCALL` opcode.
- **Syscall 0x01: SPAWN**: Launch a new visual program from a SPIR-V module.
- **Syscall 0x02: EXIT**: Terminate current process.
- **Syscall 0x03: MALLOC**: Request additional memory segment.
- **Syscall 0x04: SEND**: Send a message to another process (IPC).

## Data Layout (Visual RAM)

| Address Range | Purpose |
| :--- | :--- |
| `0x0000 - 0x03FF` | **Kernel Space**: PCBs, Process Table, Global Settings |
| `0x0400 - 0xFFFF` | **User Space**: Process-specific RAM segments |

## Implementation Plan

1. **`web/kernel.wgsl`**: Re-implement `executor.wgsl` as a multi-process scheduler.
2. **`web/GeometryKernel.js`**: JavaScript controller to load .spv files into the process table and manage the GPU execution loop.
3. **`open_brain/kernel_bridge.py`**: Integration with the Visual Open Brain so agents can "see" active processes in the 3D browser.
