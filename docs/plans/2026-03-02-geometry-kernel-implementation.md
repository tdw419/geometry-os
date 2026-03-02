# Geometry Kernel Process Manager Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a process manager that loads, isolates, and schedules multiple visual programs with shared memory IPC.

**Architecture:** Hybrid JavaScript + WebGPU approach. ProcessManager handles lifecycle in JS, SpirvRunner dispatches to GPU with memory bounds. Round-robin scheduler with 100-cycle quantum.

**Tech Stack:** JavaScript (ES6 modules), WebGPU, WGSL, SPIR-V, Python (test program generation)

---

## Task 1: Create Process Model

**Files:**
- Create: `web/Process.js`

**Step 1: Create Process class**

```javascript
/**
 * Geometry OS Process Control Block
 * Represents a single visual program with isolated memory.
 */
export class Process {
    constructor(pid, name, spirvBinary, labelMap = {}) {
        this.pid = pid;
        this.name = name;
        this.state = 'ready'; // ready | running | blocked | terminated
        this.program = spirvBinary;
        this.labelMap = labelMap;
        this.memoryBase = pid * 1000;
        this.memorySize = 1000;
        this.pc = 0;
        this.priority = 0;
        this.ticks = 0;
        this.stack = new Float32Array(1024).fill(0);
        this.ram = new Float32Array(256).fill(0);
    }

    reset() {
        this.state = 'ready';
        this.pc = 0;
        this.ticks = 0;
        this.stack.fill(0);
        this.ram.fill(0);
    }
}
```

**Step 2: Commit**

```bash
git add web/Process.js
git commit -m "feat(kernel): add Process model class"
```

---

## Task 2: Create Scheduler

**Files:**
- Create: `web/Scheduler.js`

**Step 1: Create Scheduler class**

```javascript
/**
 * Round-Robin Scheduler for Geometry OS processes.
 */
export class Scheduler {
    constructor(quantum = 100) {
        this.quantum = quantum;
    }

    /**
     * Get next runnable process.
     * @param {Map<number, Process>} processes
     * @returns {Process | null}
     */
    getNext(processes) {
        const ready = Array.from(processes.values())
            .filter(p => p.state === 'ready' || p.state === 'running')
            .sort((a, b) => a.priority - b.priority);

        return ready.length > 0 ? ready[0] : null;
    }

    /**
     * Check if current process should be preempted.
     * @param {Process} process
     * @param {number} cycles - cycles executed this quantum
     * @returns {boolean}
     */
    shouldPreempt(process, cycles) {
        return cycles >= this.quantum;
    }

    /**
     * Boost priorities to prevent starvation.
     * @param {Map<number, Process>} processes
     */
    boostPriorities(processes) {
        for (const process of processes.values()) {
            if (process.priority > 0 && process.state !== 'terminated') {
                process.priority = Math.max(0, process.priority - 1);
            }
        }
    }
}
```

**Step 2: Commit**

```bash
git add web/Scheduler.js
git commit -m "feat(kernel): add round-robin scheduler"
```

---

## Task 3: Modify SpirvRunner for Process Context

**Files:**
- Modify: `web/SpirvRunner.js`

**Step 1: Add process context support to execute method**

Find the `execute` method and modify the signature and buffer creation:

```javascript
async execute(processId, spirvBinary, labelMap = {}, inputData = {}, processContext = null) {
    if (!this.device) await this.init();

    // Get or create process state
    if (!this.processes.has(processId)) {
        this.processes.set(processId, {
            ram: new Float32Array(256).fill(0),
            visual_ram: new Float32Array(11000).fill(0), // Extended for all processes + shared
            stack: new Float32Array(1024).fill(0)
        });
    }
    const state = this.processes.get(processId);

    // ... existing buffer setup code ...

    // Add process context uniforms if provided
    let processBaseBuffer = null;
    let processLimitBuffer = null;

    if (processContext) {
        processBaseBuffer = this.device.createBuffer({
            size: 4,
            usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
        });
        this.device.queue.writeBuffer(processBaseBuffer, 0, new Uint32Array([processContext.memoryBase]));

        processLimitBuffer = this.device.createBuffer({
            size: 4,
            usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
        });
        this.device.queue.writeBuffer(processLimitBuffer, 0, new Uint32Array([processContext.memoryBase + processContext.memorySize]));
    }

    // ... in bindGroup entries, add bindings 6 and 7 ...
    const bindGroupEntries = [
        { binding: 0, resource: { buffer: programBuffer } },
        { binding: 1, resource: { buffer: stackBuffer } },
        { binding: 2, resource: { buffer: resultBuffer } },
        { binding: 3, resource: { buffer: ramBuffer } },
        { binding: 4, resource: { buffer: labelsBuffer } },
        { binding: 5, resource: { buffer: visualRamBuffer } },
    ];

    if (processBaseBuffer && processLimitBuffer) {
        bindGroupEntries.push({ binding: 6, resource: { buffer: processBaseBuffer } });
        bindGroupEntries.push({ binding: 7, resource: { buffer: processLimitBuffer } });
    }

    // ... rest of method ...
}
```

**Step 2: Commit**

```bash
git add web/SpirvRunner.js
git commit -m "feat(runner): add process context support for memory isolation"
```

---

## Task 4: Modify Executor WGSL for Bounds Checking

**Files:**
- Modify: `web/executor.wgsl`

**Step 1: Add process context uniforms**

Add at the top of the file after existing bindings:

```wgsl
@group(0) @binding(6) var<uniform> process_base: u32;
@group(0) @binding(7) var<uniform> process_limit: u32;
```

**Step 2: Modify VISUAL_LOAD opcode (204)**

Find the `opcode == 204u` block and replace:

```wgsl
} else if (opcode == 204u) { // OP_VISUAL_LOAD - Load from visual_ram with bounds check
    let local_addr = program[pc + 1];
    let global_addr = process_base + local_addr;
    if (global_addr < process_limit) {
        stack[sp] = visual_ram[global_addr];
        sp = sp + 1;
    } else {
        // Access violation - push 0 and continue
        stack[sp] = 0.0;
        sp = sp + 1;
    }
```

**Step 3: Modify VISUAL_STORE opcode (205)**

Find the `opcode == 205u` block and replace:

```wgsl
} else if (opcode == 205u) { // OP_VISUAL_STORE - Store to visual_ram with bounds check
    let local_addr = program[pc + 1];
    let global_addr = process_base + local_addr;
    if (global_addr < process_limit && sp >= 1u) {
        visual_ram[global_addr] = stack[sp - 1];
        sp = sp - 1;
    } else if (sp >= 1u) {
        // Access violation - just pop, don't store
        sp = sp - 1;
    }
```

**Step 4: Add shared memory access opcodes**

Add new opcodes for shared memory (bypass process isolation):

```wgsl
} else if (opcode == 206u) { // OP_SHARED_LOAD - Load from shared memory (10000+)
    let shared_addr = 10000u + program[pc + 1];
    if (shared_addr < 11000u) {
        stack[sp] = visual_ram[shared_addr];
        sp = sp + 1;
    }
} else if (opcode == 207u) { // OP_SHARED_STORE - Store to shared memory (10000+)
    let shared_addr = 10000u + program[pc + 1];
    if (shared_addr < 11000u && sp >= 1u) {
        visual_ram[shared_addr] = stack[sp - 1];
        sp = sp - 1;
    }
```

**Step 5: Commit**

```bash
git add web/executor.wgsl
git commit -m "feat(executor): add bounds-checked memory access and shared memory opcodes"
```

---

## Task 5: Create ProcessManager

**Files:**
- Create: `web/ProcessManager.js`

**Step 1: Create ProcessManager class**

```javascript
import { Process } from './Process.js';
import { Scheduler } from './Scheduler.js';
import { SpirvRunner } from './SpirvRunner.js';

/**
 * Geometry OS Process Manager
 * Handles loading, scheduling, and running multiple visual programs.
 */
export class ProcessManager {
    constructor() {
        this.processes = new Map();
        this.sharedMemory = new Float32Array(1000).fill(0);
        this.scheduler = new Scheduler(100);
        this.runner = new SpirvRunner();
        this.running = false;
        this.currentProcess = null;
        this.tickCount = 0;
    }

    async init() {
        await this.runner.init();
        console.log('[ProcessManager] Initialized');
    }

    /**
     * Load a SPIR-V program as a new process.
     * @param {string} name - Process name
     * @param {Uint32Array} spirvBinary - Compiled SPIR-V
     * @param {Object} labelMap - Label ID to address mapping
     * @returns {number} Process ID
     */
    loadProgram(name, spirvBinary, labelMap = {}) {
        const pid = this.processes.size;
        const process = new Process(pid, name, spirvBinary, labelMap);
        this.processes.set(pid, process);
        console.log(`[ProcessManager] Loaded process ${pid}: ${name}`);
        return pid;
    }

    /**
     * Start a loaded process.
     * @param {number} pid
     */
    startProcess(pid) {
        const process = this.processes.get(pid);
        if (process) {
            process.state = 'ready';
            console.log(`[ProcessManager] Started process ${pid}: ${process.name}`);
        }
    }

    /**
     * Terminate a process.
     * @param {number} pid
     */
    terminateProcess(pid) {
        const process = this.processes.get(pid);
        if (process) {
            process.state = 'terminated';
            console.log(`[ProcessManager] Terminated process ${pid}: ${process.name}`);
        }
    }

    /**
     * Execute one scheduling cycle.
     */
    async tick() {
        const process = this.scheduler.getNext(this.processes);
        if (!process) {
            return null;
        }

        this.currentProcess = process;
        process.state = 'running';

        // Execute with process context for memory isolation
        const result = await this.runner.execute(
            process.pid,
            process.program,
            process.labelMap,
            {},
            {
                memoryBase: process.memoryBase,
                memorySize: process.memorySize
            }
        );

        process.ticks++;
        this.tickCount++;

        // Check for preemption
        if (process.state === 'running') {
            process.state = 'ready';
        }

        // Boost priorities periodically
        if (this.tickCount % 10 === 0) {
            this.scheduler.boostPriorities(this.processes);
        }

        // Sync shared memory from runner's visual_ram
        const visualRam = result.visual_ram;
        for (let i = 0; i < 1000; i++) {
            this.sharedMemory[i] = visualRam[10000 + i];
        }

        return result;
    }

    /**
     * Start the scheduler loop.
     * @param {number} intervalMs - Milliseconds between ticks
     */
    async start(intervalMs = 100) {
        this.running = true;
        console.log('[ProcessManager] Scheduler started');

        const loop = async () => {
            if (!this.running) return;

            const hasRunnable = Array.from(this.processes.values())
                .some(p => p.state === 'ready' || p.state === 'running');

            if (hasRunnable) {
                await this.tick();
            }

            setTimeout(loop, intervalMs);
        };

        loop();
    }

    /**
     * Stop the scheduler.
     */
    stop() {
        this.running = false;
        console.log('[ProcessManager] Scheduler stopped');
    }

    /**
     * Write to shared memory.
     * @param {number} offset - Address offset (0-999)
     * @param {number} value
     */
    writeSharedMemory(offset, value) {
        if (offset >= 0 && offset < 1000) {
            this.sharedMemory[offset] = value;
        }
    }

    /**
     * Read from shared memory.
     * @param {number} offset - Address offset (0-999)
     * @returns {number}
     */
    readSharedMemory(offset) {
        if (offset >= 0 && offset < 1000) {
            return this.sharedMemory[offset];
        }
        return 0;
    }

    /**
     * Get process list for debugging.
     */
    getProcessList() {
        return Array.from(this.processes.values()).map(p => ({
            pid: p.pid,
            name: p.name,
            state: p.state,
            ticks: p.ticks,
            memoryBase: p.memoryBase
        }));
    }

    dispose() {
        this.stop();
        this.runner.dispose();
        this.processes.clear();
    }
}
```

**Step 2: Commit**

```bash
git add web/ProcessManager.js
git commit -m "feat(kernel): add ProcessManager with scheduling and IPC"
```

---

## Task 6: Create Test Program Generator

**Files:**
- Create: `tests/test_process_manager.py`

**Step 1: Create producer program generator**

```python
"""
Generate test SPIR-V programs for process manager testing.
"""
import struct
import os

# SPIR-V opcodes
OP_CONSTANT = 43
OP_FADD = 129
OP_FSUB = 131
OP_STORE = 62
OP_LOAD = 61
OP_VISUAL_STORE = 205
OP_VISUAL_LOAD = 204
OP_SHARED_STORE = 207
OP_SHARED_LOAD = 206
OP_JMP = 202
OP_RETURN = 253

def emit_spirv_header():
    """SPIR-V header: magic, version, generator, bound, schema"""
    return [
        0x07230203,  # Magic
        0x00010000,  # Version 1.0
        0x00000000,  # Generator
        100,         # Bound (max ID)
        0            # Schema
    ]

def emit_constant(value, result_id):
    """Emit a float constant instruction."""
    float_bits = struct.unpack('<I', struct.pack('<f', value))[0]
    return [OP_CONSTANT | (4 << 16), 1, result_id, float_bits]

def emit_shared_store(value_id, offset):
    """Emit shared memory store instruction."""
    return [OP_SHARED_STORE | (2 << 16), offset]

def emit_shared_load(result_id, offset):
    """Emit shared memory load instruction."""
    return [OP_SHARED_LOAD | (2 << 16), offset]

def emit_fadd(a_id, b_id, result_id):
    """Emit float add instruction."""
    return [OP_FADD | (5 << 16), 1, result_id, a_id, b_id]

def emit_jmp(label_id):
    """Emit unconditional jump."""
    return [OP_JMP | (2 << 16), label_id]

def emit_return():
    """Emit return instruction."""
    return [OP_RETURN | (1 << 16)]

def generate_producer_program():
    """
    Generate producer program that writes incrementing counter to shared memory.
    Pseudo-assembly:
        counter = 0
        loop:
            counter = counter + 1
            shared[0] = counter
            jmp loop
    """
    words = emit_spirv_header()

    # Constants
    words.extend(emit_constant(0.0, 1))     # Initial counter
    words.extend(emit_constant(1.0, 2))     # Increment value
    words.extend(emit_constant(0.0, 3))     # Shared memory offset

    # Loop label at word position
    loop_label_pos = len(words)

    # Load counter, add 1, store back, store to shared
    # Note: This is simplified - real impl needs proper SSA
    words.extend(emit_fadd(1, 2, 4))        # counter + 1 -> id 4
    words.extend(emit_shared_store(4, 0))   # shared[0] = counter
    words.extend(emit_jmp(0))               # Jump to loop (label 0)
    words.extend(emit_return())

    return words

def generate_consumer_program():
    """
    Generate consumer program that reads from shared memory.
    Pseudo-assembly:
        loop:
            value = shared[0]
            ram[0] = value  # Store for verification
            jmp loop
    """
    words = emit_spirv_header()

    # Constants
    words.extend(emit_constant(0.0, 1))     # Shared memory offset

    # Loop
    loop_label_pos = len(words)

    words.extend(emit_shared_load(2, 0))    # Load shared[0] -> id 2
    words.extend([OP_STORE | (3 << 16), 0, 2])  # ram[0] = value
    words.extend(emit_jmp(0))               # Jump to loop
    words.extend(emit_return())

    return words

def write_spirv(words, filename):
    """Write SPIR-V words to binary file."""
    binary = b''.join(struct.pack('<I', w) for w in words)
    with open(filename, 'wb') as f:
        f.write(binary)
    print(f"Wrote {len(words)} words to {filename}")

if __name__ == '__main__':
    os.makedirs('web/assets', exist_ok=True)

    producer = generate_producer_program()
    write_spirv(producer, 'web/assets/producer.spv')

    consumer = generate_consumer_program()
    write_spirv(consumer, 'web/assets/consumer.spv')

    print("Generated test programs")
```

**Step 2: Run test program generator**

```bash
python tests/test_process_manager.py
```

Expected output:
```
Wrote 19 words to web/assets/producer.spv
Wrote 17 words to web/assets/consumer.spv
Generated test programs
```

**Step 3: Commit**

```bash
git add tests/test_process_manager.py web/assets/producer.spv web/assets/consumer.spv
git commit -m "feat(test): add process manager test program generator"
```

---

## Task 7: Create Demo Page

**Files:**
- Create: `web/demo_multiprocess.html`

**Step 1: Create demo HTML**

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Geometry OS - Multi-Process Demo</title>
    <style>
        body {
            background: #0a0a0a;
            color: #00ffcc;
            font-family: 'Courier New', monospace;
            padding: 20px;
        }
        h1 { color: #ff6600; }
        .panel {
            background: #1a1a1a;
            border: 1px solid #333;
            padding: 15px;
            margin: 10px 0;
        }
        .process-list {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 10px;
        }
        .process-card {
            background: #2a2a2a;
            padding: 10px;
            border-left: 3px solid #00ffcc;
        }
        .process-card.running { border-left-color: #00ff00; }
        .process-card.blocked { border-left-color: #ff6600; }
        .process-card.terminated { border-left-color: #ff0000; }
        .shared-memory {
            font-family: monospace;
            white-space: pre;
            background: #0f0f0f;
            padding: 10px;
            overflow-x: auto;
        }
        button {
            background: #333;
            color: #00ffcc;
            border: 1px solid #00ffcc;
            padding: 8px 16px;
            cursor: pointer;
            margin: 5px;
        }
        button:hover { background: #00ffcc; color: #000; }
        .log {
            height: 150px;
            overflow-y: auto;
            background: #0f0f0f;
            padding: 10px;
            font-size: 12px;
        }
    </style>
</head>
<body>
    <h1>Geometry OS Process Manager Demo</h1>

    <div class="panel">
        <h2>Controls</h2>
        <button id="startBtn">Start Scheduler</button>
        <button id="stopBtn">Stop Scheduler</button>
        <button id="loadBtn">Load Demo Programs</button>
    </div>

    <div class="panel">
        <h2>Processes</h2>
        <div id="processList" class="process-list">
            <p>No processes loaded</p>
        </div>
    </div>

    <div class="panel">
        <h2>Shared Memory (IPC Region)</h2>
        <div id="sharedMemory" class="shared-memory">
            [0]: 0.00
        </div>
    </div>

    <div class="panel">
        <h2>Log</h2>
        <div id="log" class="log"></div>
    </div>

    <script type="module">
        import { ProcessManager } from './ProcessManager.js';

        const pm = new ProcessManager();
        let updateInterval = null;

        function log(msg) {
            const logDiv = document.getElementById('log');
            const time = new Date().toLocaleTimeString();
            logDiv.innerHTML += `[${time}] ${msg}\n`;
            logDiv.scrollTop = logDiv.scrollHeight;
        }

        function updateUI() {
            // Update process list
            const processes = pm.getProcessList();
            const listDiv = document.getElementById('processList');

            if (processes.length === 0) {
                listDiv.innerHTML = '<p>No processes loaded</p>';
            } else {
                listDiv.innerHTML = processes.map(p => `
                    <div class="process-card ${p.state}">
                        <strong>PID ${p.pid}</strong>: ${p.name}<br>
                        State: ${p.state}<br>
                        Ticks: ${p.ticks}<br>
                        Memory: ${p.memoryBase}-${p.memoryBase + 999}
                    </div>
                `).join('');
            }

            // Update shared memory display
            const sharedDiv = document.getElementById('sharedMemory');
            const values = [];
            for (let i = 0; i < 10; i++) {
                values.push(`[${i}]: ${pm.readSharedMemory(i).toFixed(2)}`);
            }
            sharedDiv.textContent = values.join(' | ');
        }

        document.getElementById('loadBtn').addEventListener('click', async () => {
            await pm.init();

            // Create simple test programs
            // Producer: increment shared[0]
            const producerBinary = createSimpleProducer();
            const consumerBinary = createSimpleConsumer();

            pm.loadProgram('producer', producerBinary, {});
            pm.loadProgram('consumer', consumerBinary, {});

            pm.startProcess(0);
            pm.startProcess(1);

            log('Loaded 2 processes');
            updateUI();
        });

        document.getElementById('startBtn').addEventListener('click', async () => {
            await pm.start(100);
            updateInterval = setInterval(updateUI, 100);
            log('Scheduler started');
        });

        document.getElementById('stopBtn').addEventListener('click', () => {
            pm.stop();
            if (updateInterval) {
                clearInterval(updateInterval);
                updateInterval = null;
            }
            log('Scheduler stopped');
        });

        // Simple SPIR-V binary generators
        function createSimpleProducer() {
            // Minimal SPIR-V that stores incrementing value to shared memory
            // This is a simplified version - real programs need proper structure
            const words = [
                0x07230203, 0x00010000, 0, 100, 0,  // Header
                43 | (4 << 16), 1, 1, 0x3f800000,   // Constant 1.0
                43 | (4 << 16), 1, 2, 0x40000000,   // Constant 2.0
                207 | (2 << 16), 0,                  // Shared store at 0
                253 | (1 << 16)                      // Return
            ];
            return new Uint32Array(words);
        }

        function createSimpleConsumer() {
            // Minimal SPIR-V that reads from shared memory
            const words = [
                0x07230203, 0x00010000, 0, 100, 0,  // Header
                206 | (2 << 16), 0,                  // Shared load from 0
                62 | (3 << 16), 0, 1,               // Store to ram[0]
                253 | (1 << 16)                      // Return
            ];
            return new Uint32Array(words);
        }

        updateUI();
    </script>
</body>
</html>
```

**Step 2: Commit**

```bash
git add web/demo_multiprocess.html
git commit -m "feat(demo): add multi-process demo page"
```

---

## Task 8: Integration Test

**Step 1: Start local server**

```bash
cd web && python3 -m http.server 8770
```

**Step 2: Open demo in WebGPU browser**

Navigate to: http://localhost:8770/demo_multiprocess.html

**Step 3: Verify functionality**

1. Click "Load Demo Programs" - should see 2 processes
2. Click "Start Scheduler" - processes should run
3. Check Shared Memory section - values should update
4. Click "Stop Scheduler" - processes should stop

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat(kernel): complete Geometry Kernel process manager with IPC"
```

---

## Summary

| Task | Description | Files |
|------|-------------|-------|
| 1 | Process model | `web/Process.js` |
| 2 | Scheduler | `web/Scheduler.js` |
| 3 | Runner modifications | `web/SpirvRunner.js` |
| 4 | Executor bounds checking | `web/executor.wgsl` |
| 5 | ProcessManager | `web/ProcessManager.js` |
| 6 | Test programs | `tests/test_process_manager.py` |
| 7 | Demo page | `web/demo_multiprocess.html` |
| 8 | Integration test | Manual verification |

---

**Plan complete and saved to `docs/plans/2026-03-02-geometry-kernel-implementation.md`.**
