# Geometry OS Phase 3: Process Model (Enhanced Scheduler)

## Goal
Implement a priority-aware, starvation-resistant scheduler for the Geometry Kernel that can manage multiple processes efficiently on the GPU.

## Architecture
- **Multi-Level Feedback Queue (MLFQ) Principles:** Implement multiple priority levels for processes.
- **Dynamic Priority (Aging):** Increase priority of waiting processes to prevent starvation.
- **Cycle-Aware Decay:** Penalize CPU-intensive processes by lowering their priority over time.
- **Parallel Dispatch:** Leverage WebGPU workgroups to run multiple processes in parallel, while still respecting priority-based resource allocation.

## Components

### 1. Enhanced PCB (Process Control Block)
Update the `Process` struct in `kernel.wgsl` and the `Process` class in `web/Process.js` to include:
- `static_priority`: The base priority (nice value).
- `dynamic_priority`: The current priority after aging/decay.
- `total_cycles`: Total instructions executed.
- `quantum_remaining`: Number of instructions left in the current time slice.
- `last_run_timestamp`: Last time the process was scheduled.

### 2. GPU Scheduler (WGSL)
Modify `kernel.wgsl` to:
- Dispatch work per-process (using `global_invocation_id`).
- Implement a "preemption" check based on `quantum_remaining`.
- Handle state transitions (Waking, Blocking, Yielding) more robustly.

### 3. JavaScript Scheduler Manager
Update `web/Scheduler.js` and `web/GeometryKernel.js` to:
- Sync priority values between the CPU and GPU.
- Implement the "Aging" logic periodically on the CPU (or in a separate compute pass).
- Provide a visualization of the scheduler queues in the `VisualShell`.

## Implementation Tasks

### Task 3.1: Refine PCB and Constants
- [ ] Add `PRIORITY` constants to `web/agents/AgentGenerator.js`.
- [ ] Update `struct Process` in `kernel.wgsl` with new fields.
- [ ] Update `web/Process.js` constructor to match.

### Task 3.2: Implement Parallel Process Execution
- [ ] Change `kernel.wgsl` to remove the for-loop and use `global_invocation_id.x`.
- [ ] Update `GeometryKernel.js` to dispatch 16 workgroups or a workgroup of size 16.
- [ ] Add bounds checking to ensure each process only accesses its own stack/RAM region.

### Task 3.3: Implement Priority Aging and Decay
- [ ] Implement `Scheduler.tick()` logic to update `dynamic_priority` in the PCB buffer.
- [ ] Add a "priority boost" mechanism for interactive processes (shell, I/O).

### Task 3.4: Integrate with Visual Shell
- [ ] Add a "Process Inspector" to `web/VisualShell.js` that shows priority, cycles, and state.
- [ ] Implement a live "Run Queue" visualization.

## Verification
- [ ] Test with multiple "CPU-hog" processes and one "interactive" process.
- [ ] Verify that the interactive process remains responsive.
- [ ] Confirm that no process is permanently starved.
