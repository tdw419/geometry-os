# Pitfalls Research

**Domain:** GPU-native operating systems with font-based execution
**Researched:** 2026-03-02
**Confidence:** MEDIUM (novel domain with limited production precedents; based on GPU systems research, WebGPU/Vulkan documentation, and analysis of existing Geometry OS codebase)

---

## Critical Pitfalls

### Pitfall 1: Cooperative Multitasking Starvation

**What goes wrong:**
A single long-running process blocks all other processes because the GPU kernel uses cooperative (not preemptive) multitasking. One infinite loop or compute-heavy glyph sequence freezes the entire OS.

**Why it happens:**
- GPU kernels were designed for batch processing, not interactive multitasking
- Unlike CPUs, GPUs lack hardware context switching at fine granularity
- The `MAX_INST_PER_SLICE` limit (currently 100) is per-dispatch, but a single dispatch runs to completion
- Developers assume GPU scheduling works like CPU preemptive scheduling

**How to avoid:**
1. Set strict `MAX_INST_PER_SLICE` limits (100 is reasonable)
2. Implement yield instructions (`GEO_YIELD` opcode 228) in hot loops
3. Add watchdog timer in CPU-side kernel controller to detect hung dispatches
4. Consider instruction counting per process and force status change to WAITING if quota exceeded

**Warning signs:**
- UI becomes unresponsive when running certain SPIR-V programs
- Process status stays at RUNNING indefinitely
- Frame rate drops to zero during kernel execution
- Other processes never get CPU time despite being READY

**Phase to address:** Phase 1 (Core Kernel) - scheduling mechanics are foundational

---

### Pitfall 2: GPU Memory Race Conditions in Shared RAM

**What goes wrong:**
Multiple processes writing to shared memory regions (mailboxes, syscall buffers) produce corrupted data, lost messages, or inconsistent state. The classic "two threads read-modify-write the same location" problem.

**Why it happens:**
- WebGPU storage buffers have no built-in synchronization between workgroups
- The kernel runs as a single workgroup, but IPC mailboxes are in shared RAM
- No atomic operations for message queue management in current implementation
- Race between `MSG_SEND` writing mailbox and `MSG_RECV` clearing it

**How to avoid:**
1. Use single-producer single-consumer pattern for mailboxes (each PID has ONE designated sender)
2. Implement atomic flags for mailbox ownership using `atomicStore`/`atomicLoad` in WGSL
3. Add message sequence numbers to detect corruption
4. Consider double-buffering mailboxes (one read buffer, one write buffer)

```wgsl
// Current (racy):
ram[mailbox_base + MSG_SIZE] = 1u;  // Non-atomic write

// Better:
atomicStore(&mailbox_flags[target_pid], 1u);  // Atomic flag
```

**Warning signs:**
- IPC messages occasionally have garbage data
- Process A sends to B, but B receives wrong message
- Message count increments but data is wrong
- Syscall arguments corrupted between GPU write and CPU read

**Phase to address:** Phase 1 (Core Kernel) - IPC is fundamental to OS operation

---

### Pitfall 3: CPU-GPU Synchronization Latency Assumption

**What goes wrong:**
Code assumes GPU results are available immediately after `dispatchWorkgroups()`, but WebGPU is inherently async. Results arrive 1+ frames later, causing stale data reads or missed I/O triggers.

**Why it happens:**
- WebGPU has no synchronous readback - `mapAsync()` always returns a Promise
- GPU command buffer is not executed until queue submission
- Developers from CPU backgrounds expect immediate consistency
- Memory-mapped I/O polling assumes instant visibility

**How to avoid:**
1. Always use `await` with `mapAsync()` and staging buffers
2. Design for 1-2 frame latency in all CPU-GPU communication
3. Use double-buffering for frequently read data (current/previous frame)
4. Never assume `dispatchWorkgroups()` has completed when next line runs

```javascript
// WRONG:
device.queue.submit([encoder.finish()]);
const result = readBuffer();  // Stale!

// CORRECT:
device.queue.submit([encoder.finish()]);
await stagingBuffer.mapAsync(GPUMapMode.READ);  // Wait for GPU
const result = readBuffer();
```

**Warning signs:**
- I/O triggers not firing (GPU wrote, CPU hasn't seen it yet)
- PCB state shows old values after kernel step
- SoundSystem misses tone requests
- Visual updates lag by variable frames

**Phase to address:** Phase 2 (Execution Environment) - SpirvRunner must handle async correctly

---

### Pitfall 4: SPIR-V Bytecode Interpretation Errors

**What goes wrong:**
The kernel's SPIR-V interpreter mis-handles edge cases: variable-width instructions, operand ordering, type mismatches. Programs produce wrong results or crash the shader.

**Why it happens:**
- SPIR-V has complex variable-length instruction encoding (word count in high 16 bits)
- Operand types vary by opcode (some are IDs, some are literals, some are result IDs)
- ExtInst instructions have nested operand structures
- The current kernel handles ~15 opcodes; SPIR-V has hundreds

**How to avoid:**
1. Start with minimal opcode subset (the current 15 is reasonable)
2. Add opcodes incrementally with unit tests for each
3. Validate SPIR-V binaries with `spirv-val` before loading
4. Use the existing VisualCompiler rather than arbitrary SPIR-V

**Warning signs:**
- Programs compiled by external tools produce wrong results
- `pc` jumps to unexpected locations
- Stack corruption after certain opcodes
- Validation errors in browser console

**Phase to address:** Phase 1 (Core Kernel) - interpreter correctness is non-negotiable

---

### Pitfall 5: Memory Region Overflow (SIGSEGV Simulation)

**What goes wrong:**
Processes write beyond their allocated `mem_limit`, corrupting other processes' memory or kernel data structures. The current SIGSEGV detection (status = 4) only triggers after corruption occurs.

**Why it happens:**
- Memory bounds checking happens in the interpreter loop, not at hardware level
- No MMU on GPU to enforce protection
- Address arithmetic can overflow
- `mem_base` + address calculation may wrap around

**How to avoid:**
1. Validate ALL memory accesses before execution (current code does this - keep it)
2. Add guard pages between process memory regions
3. Use signed arithmetic with explicit overflow checks
4. Log all SIGSEGV events with faulting address for debugging

```wgsl
// Current (correct):
if (addr < p.mem_limit) {
    ram[ram_base + addr] = value;
} else {
    p.status = 4u;  // SIGSEGV
}
```

**Warning signs:**
- Process A's data changes when Process B runs
- PCB table entries corrupted
- Random process status changes
- Memory browser shows overlapping data

**Phase to address:** Phase 1 (Core Kernel) - memory protection is security-critical

---

### Pitfall 6: Shader Timeout on Long-Running Programs

**What goes wrong:**
A compute-intensive program (infinite loop, deep recursion) causes the GPU shader to exceed browser/driver timeout, triggering a device loss and crashing the entire WebGPU context.

**Why it happens:**
- Browsers impose ~2-10 second timeouts on compute shaders to prevent GPU hangs
- TDR (Timeout Detection and Recovery) on Windows at ~2 seconds
- The kernel runs ALL processes in one dispatch; one bad actor kills everyone
- Cooperative multitasking doesn't help if dispatch doesn't return

**How to avoid:**
1. Keep `MAX_INST_PER_SLICE` conservative (100 is safe, 10000 is risky)
2. Split kernel dispatches across multiple frames if many processes
3. Implement "emergency stop" by having CPU write a kill flag to PCB before dispatch
4. Monitor `device.lost` promise for TDR events

**Warning signs:**
- WebGPU context lost errors in console
- Entire page becomes blank after running certain programs
- "GPU process crashed" browser notification
- Device loss recovery triggered frequently

**Phase to address:** Phase 1 (Core Kernel) - kernel stability is foundational

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| **Single workgroup kernel** | Simpler implementation | Limits parallelism, doesn't scale past 64 processes | MVP only (Phase 1-2) |
| **Polling for I/O** | Easy to implement | CPU overhead, latency, missed events | Never - use interrupt-like pattern with flags |
| **Fixed mailbox size (1 message)** | Simple queue logic | Lost messages if receiver slow | MVP only, add ring buffers in Phase 3 |
| **No opcode validation** | Faster development | Cryptic crashes, hard to debug | Never - validate in VisualCompiler |
| **CPU-side scheduling** | Familiar patterns | Defeats GPU-native purpose | Never - this is the wrong architecture |
| **Separate buffers per process** | Easy isolation | Allocation overhead, no shared memory | Never - use offsets into shared buffers |
| **Skip spirv-val** | Faster iteration | Invalid binaries crash shader | Development only, never in production |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| **WebGPU device creation** | Not handling adapter request failure | Always check `adapter !== null` before requesting device |
| **Buffer mapping** | Forgetting to unmap after read | Always call `unmap()` after reading staged data |
| **SoundSystem I/O** | Clearing trigger before playing sound | Play sound, THEN clear trigger flag |
| **Font atlas loading** | Assuming synchronous image load | Use `await` for all image fetches |
| **SPIR-V binary loading** | Using wrong ArrayBuffer endianness | SPIR-V is little-endian; use `Uint32Array` with correct byte offset |
| **PCB buffer sizing** | Hardcoded size that doesn't match struct | Calculate size from `Process` struct: 16 fields * 4 bytes = 64 bytes per PCB |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| **Per-frame staging buffer creation** | GC pressure, stuttering | Reuse staging buffers, pool them | 60 FPS target |
| **mapAsync on every frame** | GPU-CPU sync overhead | Batch reads, use dirty flags | 10+ processes |
| **Non-coalesced memory access** | Low GPU utilization | Sequential RAM access, avoid strided patterns | Any scale |
| **Too many small dispatches** | Command buffer overhead | Batch operations into single dispatch | Always |
| **Large Hilbert grid traversal** | Slow compilation | Cache compiled SPIR-V, limit grid to 64x64 | 128x128+ grids |
| **Atomic operation spam** | Serialization, lost parallelism | Batch atomics, use workgroup shared memory | High contention |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| **No process isolation** | Malicious font-program reads all RAM | Enforce `mem_base`/`mem_limit` on every memory access |
| **Untrusted SPIR-V loading** | Shader exploitation, GPU hang | Validate with `spirv-val`, limit opcodes to safe subset |
| **Shared memory snooping** | Process A reads Process B's mail | Each process can only read its own mailbox base |
| **Syscall parameter injection** | Kernel corruption via malformed args | Validate syscall IDs and parameter ranges |
| **Infinite loop fonts** | DoS by consuming all GPU time | Watchdog timer, instruction quota per process |
| **PCB table corruption** | Privilege escalation, process takeover | Mark PCB region as kernel-only, add checksums |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| **No visual feedback during kernel execution** | Users think it's frozen | Show "computing..." indicator during dispatch |
| **Silent process death** | Programs disappear without explanation | Display exit code, show death reason in shell |
| **Memory browser shows raw floats** | Unintelligible data | Add hex view, ASCII decode, structured views |
| **No process pause/resume** | Can't debug running programs | Add pause button that sets status to WAITING |
| **IPC events invisible** | Hard to understand inter-process flow | Real-time IPC event log with timestamps |
| **Font grid zoom locked** | Can't edit precisely | Add zoom controls for glyph grid |

---

## "Looks Done But Isn't" Checklist

- [ ] **Multi-process scheduling:** Often missing actual time-slicing - verify multiple processes run in same dispatch
- [ ] **IPC message delivery:** Often missing receiver wakeup - verify waiting process status changes on send
- [ ] **Memory protection:** Often missing actual enforcement - verify SIGSEGV triggers on out-of-bounds
- [ ] **Sound output:** Often missing trigger clear - verify sound plays exactly once per request
- [ ] **PCB persistence:** Often missing state save - verify process resumes correctly after dispatch
- [ ] **Syscall handling:** Often missing CPU-side handler - verify syscall actually executes (check RAM[100..105])
- [ ] **Error propagation:** Often missing GPU->CPU error signal - verify process errors appear in UI

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| **GPU device lost** | HIGH | Reload page, reinitialize all buffers, restore state from CPU-side backup |
| **Process memory corruption** | MEDIUM | Kill process, reallocate its memory region, respawn from source SPIR-V |
| **Deadlock (both waiting)** | LOW | Detect via timeout, force one process to READY, log warning |
| **Infinite loop** | LOW | CPU watchdog detects no PC change, sets status to ERROR |
| **Stack overflow** | LOW | Check SP > stack limit in interpreter, set status to ERROR |
| **Message queue full** | LOW | Return error code to sender, log dropped message |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Cooperative multitasking starvation | Phase 1 (Core Kernel) | Run 16 processes, verify each gets time slice |
| GPU memory race conditions | Phase 1 (Core Kernel) | Send 1000 messages between processes, check for corruption |
| CPU-GPU sync latency | Phase 2 (Execution Environment) | Profile frame latency, verify no stale reads |
| SPIR-V interpretation errors | Phase 1 (Core Kernel) | Run spirv-val on all compiled programs |
| Memory region overflow | Phase 1 (Core Kernel) | Attempt out-of-bounds write, verify SIGSEGV |
| Shader timeout | Phase 1 (Core Kernel) | Run infinite loop, verify watchdog triggers |
| No process isolation | Phase 1 (Core Kernel) | Process A attempts to read Process B's memory |
| Untrusted SPIR-V loading | Phase 2 (Execution Environment) | Load malformed SPIR-V, verify graceful rejection |

---

## Sources

**HIGH Confidence (Official/Verified):**
- [Khronos SPIR-V Specification](https://registry.khronos.org/SPIR-V/) - Instruction encoding, opcode reference
- [WebGPU Specification](https://www.w3.org/TR/webgpu/) - Buffer mapping, async patterns
- [NVIDIA GPU Preemption Research (OSDI'22)](https://www.usenix.org/conference/osdi22) - REEF: microsecond-scale preemption challenges
- [Microsoft IOMMU GPU Isolation](https://learn.microsoft.com/en-us/windows-hardware/drivers/display/iommu-based-gpu-isolation) - Hardware isolation limitations

**MEDIUM Confidence (WebSearch verified):**
- GPU virtual machine interpreter challenges - arXiv 2025, SJTU research
- Vulkan memory management pitfalls - NVIDIA Developer, CSDN guides
- GPU scheduling limitations - Multiple academic papers (SOSP 2024, OSDI 2025)
- WGSL compute shader patterns - Chrome Dev Blog, WebGPU tutorials

**LOW Confidence (Based on codebase analysis):**
- Specific timing thresholds (100 instructions/slice, 2-second timeout) - May vary by hardware
- Process count limits (16 current, 64 planned) - Architectural assumption

---

*Pitfalls research for: GPU-native operating systems with font-based execution*
*Researched: 2026-03-02*
