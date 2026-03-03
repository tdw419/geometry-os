# Kernel Syscalls Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement a system call mechanism that allows GPU processes to request external I/O data (mouse, keyboard, logging) from the browser environment.

**Architecture:** Add a SYSCALL opcode (211) to kernel.wgsl that traps execution and writes requests to shared memory. A SyscallManager in JavaScript syncs browser events to GPU memory before each kernel step, enabling reactive GPU programs.

**Tech Stack:** WebGPU (WGSL), JavaScript, SPIR-V bytecode

---

## Memory Map (Syscall Region)

```
Shared Memory Layout (addresses 0-1023):
┌─────────────────────────────────────────┐
│ 0-6:     Agent Heartbeats               │
│ 10-16:   Agent Status                   │
│ 20-49:   Message Queue                  │
│ 50-99:   I/O Region (NEW)               │
│   50-51: Mouse X, Y (float)             │
│   52:    Mouse buttons (bitmask)        │
│   53-54: Key code, Key state            │
│   55-59: Reserved                       │
│ 100-109: Syscall Request (NEW)          │
│   100:   Syscall ID                     │
│   101-103: Arguments                    │
│   104:   Result                         │
│   105:   Status (0=pending, 1=done)     │
│ 110-1023: User Data                     │
└─────────────────────────────────────────┘
```

## Syscall IDs

| ID | Name      | Args              | Result          |
|----|-----------|-------------------|-----------------|
| 1  | GET_MOUSE | none              | x, y on stack   |
| 2  | GET_KEY   | keycode           | pressed (0/1)   |
| 3  | WRITE_LOG | value             | none            |
| 4  | GET_TIME  | none              | timestamp       |

---

### Task 1: Add Syscall Constants to AgentGenerator

**Files:**
- Modify: `web/agents/AgentGenerator.js:27-45`

**Step 1: Add syscall constants and I/O memory map**

Add after line 26 (after the IPC constant block):

```javascript
// Syscall Opcodes
const OP_SYSCALL = 211;  // Trap to kernel for I/O

// Syscall IDs
const SYS = {
    GET_MOUSE: 1,
    GET_KEY: 2,
    WRITE_LOG: 3,
    GET_TIME: 4
};

// I/O Memory Map (extends IPC)
const IO = {
    MOUSE_X: 50,
    MOUSE_Y: 51,
    MOUSE_BTN: 52,
    KEY_CODE: 53,
    KEY_STATE: 54,
    SYSCALL_ID: 100,
    SYSCALL_ARG1: 101,
    SYSCALL_ARG2: 102,
    SYSCALL_ARG3: 103,
    SYSCALL_RESULT: 104,
    SYSCALL_STATUS: 105
};
```

**Step 2: Verify the file syntax**

Run: `node -c web/agents/AgentGenerator.js`
Expected: No output (syntax OK)

**Step 3: Commit**

```bash
git add web/agents/AgentGenerator.js
git commit -m "feat(syscalls): add syscall constants and I/O memory map"
```

---

### Task 2: Add Syscall Opcode to kernel.wgsl

**Files:**
- Modify: `web/kernel.wgsl:191-197`

**Step 1: Add syscall handler before OP_RETURN**

Insert before line 191 (`} else if (opcode == 253u) {`):

```wgsl
            } else if (opcode == 211u) { // OP_SYSCALL - Request external I/O
                // Format: [count|211], [syscall_id], [arg1], [arg2], [arg3]
                let syscall_id = program[pc + 1];
                let arg1 = program[pc + 2];
                let arg2 = program[pc + 3];
                let arg3 = program[pc + 4];

                // Write syscall request to shared memory
                ram[100u] = syscall_id;
                ram[101u] = bitcast<f32>(arg1);
                ram[102u] = bitcast<f32>(arg2);
                ram[103u] = bitcast<f32>(arg3);
                ram[105u] = 0.0;  // Status: pending

                // Set process to WAITING state
                p.status = 2u;  // Waiting for syscall
                p.waiting_on = 0xFEu;  // Special: waiting for syscall
                pc = pc + count;
                break;  // Yield until syscall completes
```

**Step 2: Verify WGSL syntax**

Run: `grep -A 20 "opcode == 211u" web/kernel.wgsl`
Expected: The new handler code block

**Step 3: Commit**

```bash
git add web/kernel.wgsl
git commit -m "feat(kernel): add OP_SYSCALL (211) handler with wait state"
```

---

### Task 3: Create SyscallManager Class

**Files:**
- Create: `web/SyscallManager.js`

**Step 1: Create the SyscallManager class**

```javascript
/**
 * Geometry OS Syscall Manager
 *
 * Handles system calls from GPU processes to browser I/O.
 * Syncs mouse/keyboard state to shared memory before each kernel step.
 */

import { IO, SYS } from './agents/AgentGenerator.js';

export class SyscallManager {
    constructor(kernel) {
        this.kernel = kernel;

        // Current I/O state
        this.mouseX = 0;
        this.mouseY = 0;
        this.mouseButtons = 0;
        this.keys = new Map();  // keycode -> pressed (0/1)
        this.pendingSyscalls = [];

        // Bound handlers for cleanup
        this._onMouseMove = this._handleMouseMove.bind(this);
        this._onMouseDown = this._handleMouseDown.bind(this);
        this._onMouseUp = this._handleMouseUp.bind(this);
        this._onKeyDown = this._handleKeyDown.bind(this);
        this._onKeyUp = this._handleKeyUp.bind(this);
    }

    /**
     * Attach browser event listeners
     */
    attach(canvas) {
        this.canvas = canvas;
        canvas.addEventListener('mousemove', this._onMouseMove);
        canvas.addEventListener('mousedown', this._onMouseDown);
        canvas.addEventListener('mouseup', this._onMouseUp);
        window.addEventListener('keydown', this._onKeyDown);
        window.addEventListener('keyup', this._onKeyUp);
        console.log('[SyscallManager] I/O listeners attached');
    }

    /**
     * Detach all event listeners
     */
    detach() {
        if (this.canvas) {
            this.canvas.removeEventListener('mousemove', this._onMouseMove);
            this.canvas.removeEventListener('mousedown', this._onMouseDown);
            this.canvas.removeEventListener('mouseup', this._onMouseUp);
        }
        window.removeEventListener('keydown', this._onKeyDown);
        window.removeEventListener('keyup', this._onKeyUp);
    }

    _handleMouseMove(e) {
        const rect = this.canvas.getBoundingClientRect();
        this.mouseX = e.clientX - rect.left;
        this.mouseY = e.clientY - rect.top;
    }

    _handleMouseDown(e) {
        this.mouseButtons |= (1 << e.button);
    }

    _handleMouseUp(e) {
        this.mouseButtons &= ~(1 << e.button);
    }

    _handleKeyDown(e) {
        this.keys.set(e.keyCode, 1);
    }

    _handleKeyUp(e) {
        this.keys.set(e.keyCode, 0);
    }

    /**
     * Sync I/O state to GPU shared memory before kernel step.
     * Call this BEFORE kernel.step().
     */
    async syncToGPU() {
        // Write mouse state
        const ioData = new Float32Array([
            this.mouseX,
            this.mouseY,
            this.mouseButtons,
            0, // key code (updated on demand)
            0  // key state
        ]);
        this.kernel.writeSharedMemory(IO.MOUSE_X, ioData);
    }

    /**
     * Process pending syscalls after kernel step.
     * Call this AFTER kernel.step().
     */
    async processSyscalls() {
        // Read syscall request region
        const syscallData = await this.kernel.readSharedMemory(IO.SYSCALL_ID, 6);

        const status = syscallData[5];
        if (status === 0) {  // Pending syscall
            const syscallId = syscallData[0];
            const result = await this._executeSyscall(syscallId, syscallData);

            // Write result back
            const resultBuf = new Float32Array([result, 1]);  // result, status=done
            this.kernel.writeSharedMemory(IO.SYSCALL_RESULT, resultBuf);
        }
    }

    async _executeSyscall(id, args) {
        switch (id) {
            case SYS.GET_MOUSE:
                // Result is packed: high 16 bits = X, low 16 bits = Y
                return (Math.floor(this.mouseX) << 16) | Math.floor(this.mouseY);

            case SYS.GET_KEY:
                const keyCode = args[1];
                return this.keys.get(keyCode) || 0;

            case SYS.WRITE_LOG:
                const value = args[1];
                console.log(`[GPU LOG] ${value}`);
                return 0;

            case SYS.GET_TIME:
                return Date.now() / 1000;

            default:
                console.warn(`[SyscallManager] Unknown syscall: ${id}`);
                return -1;
        }
    }
}
```

**Step 2: Verify syntax**

Run: `node -c web/SyscallManager.js`
Expected: No output (syntax OK)

**Step 3: Commit**

```bash
git add web/SyscallManager.js
git commit -m "feat(syscalls): create SyscallManager for browser I/O sync"
```

---

### Task 4: Add Syscall Method to AgentGenerator

**Files:**
- Modify: `web/agents/AgentGenerator.js:161-178`

**Step 1: Add syscall method after yield()**

Insert after line 163 (after the yield method):

```javascript
    /**
     * Execute a system call
     * @param {number} syscallId - Syscall ID (1=GET_MOUSE, 2=GET_KEY, etc.)
     * @param {number} arg1 - First argument
     * @param {number} arg2 - Second argument
     * @param {number} arg3 - Third argument
     */
    syscall(syscallId, arg1 = 0, arg2 = 0, arg3 = 0) {
        this.emit(instr(OP_SYSCALL, 5));  // count=5
        this.emit(syscallId);
        this.emit(arg1);
        this.emit(arg2);
        this.emit(arg3);
    }

    /**
     * Read syscall result from shared memory (after syscall completes)
     */
    readSyscallResult() {
        this.sharedLoad(IO.SYSCALL_RESULT);
    }
```

**Step 2: Verify syntax**

Run: `node -c web/agents/AgentGenerator.js`
Expected: No output (syntax OK)

**Step 3: Commit**

```bash
git add web/agents/AgentGenerator.js
git commit -m "feat(agent): add syscall() method to AgentGenerator"
```

---

### Task 5: Update I/O Agent to Use Syscalls

**Files:**
- Modify: `web/agents/AgentGenerator.js:351-378`

**Step 1: Replace the I/O agent implementation**

Replace the `generateIOAgent()` function (lines 351-378) with:

```javascript
/**
 * Generate I/O Agent
 * Input/output handling with syscall support
 */
export function generateIOAgent() {
    const gen = new AgentGenerator();
    const agent = AGENTS.IO;

    const loopStart = 5;

    // Increment heartbeat
    gen.sharedLoad(agent.heartbeat);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(agent.heartbeat);

    // Set status
    gen.pushConstant(PROC_STATE.RUNNING);
    gen.sharedStore(agent.status);

    // === SYSCALL: Get Mouse Position ===
    gen.syscall(SYS.GET_MOUSE, 0, 0, 0);

    // After syscall completes, read result
    // Result is packed: (X << 16) | Y
    gen.readSyscallResult();

    // Store X at address 56 (unpack high 16 bits)
    // For now, store the packed result directly
    gen.sharedStore(56);  // Mouse packed coords

    // Process I/O queue at address 53
    gen.sharedLoad(53);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(53);

    // Yield
    gen.yield();
    gen.jmp(loopStart);

    return gen.build();
}
```

**Step 2: Regenerate the io.spv binary**

Run: `cd web && node -e "import('./agents/generate-agents.js').then(m => m.generateAll())"`
Expected: "Generated 7 agents" or similar output

**Step 3: Verify the new binary**

Run: `ls -l web/agents/io.spv`
Expected: File exists with updated timestamp

**Step 4: Commit**

```bash
git add web/agents/AgentGenerator.js web/agents/io.spv
git commit -m "feat(io-agent): implement syscall-based mouse tracking"
```

---

### Task 6: Create Syscall Test Page

**Files:**
- Create: `web/test-syscalls.html`

**Step 1: Create the test page**

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Geometry OS - Syscall Test</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: 'JetBrains Mono', monospace;
            background: #0a0a0a;
            color: #00ff88;
            min-height: 100vh;
            display: flex;
            flex-direction: column;
            align-items: center;
            padding: 20px;
        }
        h1 {
            font-size: 24px;
            margin-bottom: 20px;
            text-shadow: 0 0 10px #00ff88;
        }
        #canvas-container {
            position: relative;
            border: 2px solid #00ff88;
            border-radius: 8px;
            overflow: hidden;
        }
        #gpu-canvas {
            width: 800px;
            height: 500px;
            background: #111;
            cursor: crosshair;
        }
        .dashboard {
            display: grid;
            grid-template-columns: repeat(2, 1fr);
            gap: 10px;
            margin-top: 20px;
            width: 800px;
        }
        .panel {
            background: #1a1a1a;
            border: 1px solid #333;
            border-radius: 4px;
            padding: 15px;
        }
        .panel h3 {
            color: #888;
            font-size: 12px;
            margin-bottom: 10px;
            text-transform: uppercase;
        }
        .value {
            font-size: 18px;
            color: #00ff88;
        }
        .mouse-pos { color: #ff8800; }
        .key-state { color: #00aaff; }
        .syscall-log {
            grid-column: span 2;
            max-height: 150px;
            overflow-y: auto;
            font-size: 12px;
        }
        .log-entry {
            padding: 2px 0;
            border-bottom: 1px solid #222;
        }
        #status {
            margin-top: 10px;
            padding: 10px;
            background: #1a1a1a;
            border-radius: 4px;
        }
        .error { color: #ff4444; }
        .success { color: #00ff88; }
    </style>
</head>
<body>
    <h1>Geometry OS - Syscall Test</h1>

    <div id="canvas-container">
        <canvas id="gpu-canvas" width="800" height="500"></canvas>
    </div>

    <div class="dashboard">
        <div class="panel">
            <h3>Mouse Position</h3>
            <div class="value mouse-pos" id="mouse-pos">X: 0, Y: 0</div>
        </div>
        <div class="panel">
            <h3>Mouse Buttons</h3>
            <div class="value" id="mouse-btns">None</div>
        </div>
        <div class="panel">
            <h3>Last Key</h3>
            <div class="value key-state" id="last-key">None</div>
        </div>
        <div class="panel">
            <h3>I/O Agent Heartbeat</h3>
            <div class="value" id="io-heartbeat">0</div>
        </div>
        <div class="panel syscall-log">
            <h3>Syscall Log</h3>
            <div id="syscall-log"></div>
        </div>
    </div>

    <div id="status">Initializing...</div>

    <script type="module">
        import { GeometryKernel } from './GeometryKernel.js';
        import { SyscallManager } from './SyscallManager.js';
        import { generateIOAgent, AGENTS, IO } from './agents/AgentGenerator.js';

        const canvas = document.getElementById('gpu-canvas');
        const statusEl = document.getElementById('status');
        const mousePosEl = document.getElementById('mouse-pos');
        const mouseBtnsEl = document.getElementById('mouse-btns');
        const lastKeyEl = document.getElementById('last-key');
        const ioHeartbeatEl = document.getElementById('io-heartbeat');
        const syscallLogEl = document.getElementById('syscall-log');

        function log(msg) {
            const entry = document.createElement('div');
            entry.className = 'log-entry';
            entry.textContent = `[${new Date().toLocaleTimeString()}] ${msg}`;
            syscallLogEl.prepend(entry);
        }

        async function init() {
            try {
                // Initialize kernel
                const kernel = new GeometryKernel();
                await kernel.init();
                statusEl.innerHTML = '<span class="success">Kernel initialized</span>';
                log('Kernel ready');

                // Initialize syscall manager
                const syscalls = new SyscallManager(kernel);
                syscalls.attach(canvas);
                log('Syscall manager attached');

                // Spawn I/O agent
                const ioBinary = generateIOAgent();
                const pid = await kernel.spawnProcess(ioBinary, 'I/O Agent');
                log(`Spawned I/O Agent (PID ${pid})`);

                // Main loop
                let frameCount = 0;
                async function step() {
                    // Sync I/O to GPU
                    await syscalls.syncToGPU();

                    // Run kernel step
                    await kernel.step();

                    // Process any pending syscalls
                    await syscalls.processSyscalls();

                    // Read shared memory for display
                    const shared = await kernel.readSharedMemory(0, 60);

                    // Update dashboard
                    const heartbeat = shared[AGENTS.IO.heartbeat];
                    ioHeartbeatEl.textContent = heartbeat;

                    const mousePacked = shared[56];
                    const mouseX = (mousePacked >> 16) & 0xFFFF;
                    const mouseY = mousePacked & 0xFFFF;
                    mousePosEl.textContent = `X: ${mouseX}, Y: ${mouseY}`;

                    const btns = shared[IO.MOUSE_BTN];
                    const btnNames = [];
                    if (btns & 1) btnNames.push('Left');
                    if (btns & 2) btnNames.push('Right');
                    if (btns & 4) btnNames.push('Middle');
                    mouseBtnsEl.textContent = btnNames.length ? btnNames.join(', ') : 'None';

                    frameCount++;
                    if (frameCount % 60 === 0) {
                        log(`Frame ${frameCount}: Mouse(${mouseX}, ${mouseY})`);
                    }

                    requestAnimationFrame(step);
                }

                step();
                log('Main loop started');

            } catch (err) {
                statusEl.innerHTML = `<span class="error">Error: ${err.message}</span>`;
                console.error(err);
            }
        }

        // Track last key
        window.addEventListener('keydown', (e) => {
            lastKeyEl.textContent = `${e.key} (code: ${e.keyCode})`;
        });

        init();
    </script>
</body>
</html>
```

**Step 2: Verify HTML syntax**

Run: `grep -c "</html>" web/test-syscalls.html`
Expected: 1

**Step 3: Commit**

```bash
git add web/test-syscalls.html
git commit -m "feat(test): add syscall test page with mouse/keyboard tracking"
```

---

### Task 7: Export New Constants from AgentGenerator

**Files:**
- Modify: `web/agents/AgentGenerator.js:462-463`

**Step 1: Add exports for SYS and IO constants**

Add after line 463 (after the generateAllAgents export):

```javascript
// Export syscall and I/O constants
export { OP, SYS, IO };
```

**Step 2: Verify exports**

Run: `grep "export.*SYS" web/agents/AgentGenerator.js`
Expected: `export { OP, SYS, IO };`

**Step 3: Commit**

```bash
git add web/agents/AgentGenerator.js
git commit -m "feat(exports): export SYS and IO constants from AgentGenerator"
```

---

### Task 8: Integration Test

**Files:**
- Test: `web/test-syscalls.html`

**Step 1: Start local server**

Run: `cd web && python3 -m http.server 8080 &`

**Step 2: Open test page in browser**

Manual: Navigate to `http://localhost:8080/test-syscalls.html`

**Step 3: Verify functionality**

Check:
- [ ] Page loads without errors
- [ ] Mouse position updates when moving over canvas
- [ ] Mouse button state changes when clicking
- [ ] I/O Agent heartbeat increments
- [ ] Syscall log shows periodic updates

**Step 4: Document results**

If all checks pass, commit:

```bash
git add -A
git commit -m "test(syscalls): verify syscall integration with I/O agent"
```

---

## Summary

| Task | Description | Status |
|------|-------------|--------|
| 1 | Add syscall constants to AgentGenerator | ⬜ |
| 2 | Add OP_SYSCALL to kernel.wgsl | ⬜ |
| 3 | Create SyscallManager class | ⬜ |
| 4 | Add syscall method to AgentGenerator | ⬜ |
| 5 | Update I/O Agent to use syscalls | ⬜ |
| 6 | Create syscall test page | ⬜ |
| 7 | Export SYS/IO constants | ⬜ |
| 8 | Integration test | ⬜ |

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                         BROWSER                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │ mousemove   │  │ keydown     │  │ keyup       │             │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘             │
│         │                │                │                     │
│         ▼                ▼                ▼                     │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              SyscallManager.js                           │   │
│  │  • syncToGPU() → writes mouse/key to shared memory       │   │
│  │  • processSyscalls() → handles syscall requests          │   │
│  └──────────────────────────┬──────────────────────────────┘   │
│                             │                                   │
│                             ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              GeometryKernel.js                           │   │
│  │  step() → dispatches compute shader                      │   │
│  └──────────────────────────┬──────────────────────────────┘   │
└─────────────────────────────┼───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                          GPU                                     │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    kernel.wgsl                            │   │
│  │  OP_SYSCALL (211):                                       │   │
│  │    • Write request to ram[100-105]                       │   │
│  │    • Set process status = WAITING                        │   │
│  │    • Yield until syscall completes                       │   │
│  └──────────────────────────┬──────────────────────────────┘   │
│                             │                                   │
│                             ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                 Shared Memory                            │   │
│  │  [50-54]: Mouse X/Y/Btn, Key/State                       │   │
│  │  [100-105]: Syscall ID, Args, Result, Status             │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```
