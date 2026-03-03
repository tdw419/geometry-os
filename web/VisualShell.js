/**
 * Geometry OS Visual Shell
 *
 * Interactive process manager for the Geometry Kernel.
 * Provides process spawning, PCB inspection, and kernel saccades.
 */

import { MemoryBrowser } from './MemoryBrowser.js';
import { SyscallManager } from "./SyscallManager.js";
import { GeometryKernel } from './GeometryKernel.js';
import { PROC_STATE } from './agents/index.js';

// State colors
const STATE_COLORS = {
    [PROC_STATE.IDLE]: '#888888',
    [PROC_STATE.RUNNING]: '#00ff88',
    [PROC_STATE.WAITING]: '#ffaa00',
    [PROC_STATE.DONE]: '#00aaff',
    [PROC_STATE.ERROR]: '#ff4444'
};

export class VisualShell extends MemoryBrowser {
    constructor(canvas, options = {}) {
        super(canvas);

        // Shell-specific state
        this.kernel = options.kernel || new GeometryKernel();
        this.processes = new Map();  // PID -> ProcessInfo
        this.syscalls = new SyscallManager(this.kernel);
        this.overlayCanvas = options.overlayCanvas || null;
        this.overlayCtx = this.overlayCanvas ? this.overlayCanvas.getContext("2d") : null;
        this.activePID = 0;
        this.saccadeEnabled = false;
        this.cameraTarget = { x: 0, y: 0, z: 0 };

        // UI elements (created in _initUI)
        this.spawnerPanel = null;
        this.dashboardPanel = null;
        this.pcbOverlays = new Map();

        // Neural pulse tracking
        this.lastIterationCount = 0;
        this.pulseRate = 0;

        // IPC tracking
        this.ipcCount = 0;
        this.ipcChannels = new Map();  // channelKey -> { from, to, count, lastMsg }
    }

    async init() {
        // Initialize kernel first (it creates its own WebGPU device)
        await this.kernel.init();

        // Initialize parent MemoryBrowser (shares device if compatible)
        await super.init();
        // Attach syscall manager to capture events
        this.syscalls.attach(this.canvas, this.overlayCtx);

        // Build UI
        this._initUI();

        // Start PCB reader loop
        this._startPCBReader();

        console.log('[VisualShell] OS Interface ready');
    }

    _initUI() {
        // Create spawner panel (left sidebar)
        this.spawnerPanel = document.createElement('div');
        this.spawnerPanel.id = 'spawner-panel';
        this.spawnerPanel.innerHTML = `
            <div class="panel-header">Process Spawner</div>
            <div class="drop-zone" id="spv-drop-zone">
                <span>Drop .spv files here</span>
                <input type="file" id="spv-file-input" accept=".spv" multiple hidden>
            </div>
            <button class="btn" id="spawn-from-brain">Spawn from Brain</button>
            <div class="process-list" id="process-list">
                <!-- Populated dynamically -->
            </div>
        `;
        this.spawnerPanel.style.cssText = `
            position: absolute; top: 80px; left: 20px; width: 250px;
            background: rgba(0, 20, 20, 0.9); border: 1px solid #00ffcc;
            padding: 15px; color: #00ffcc; font-family: monospace;
        `;

        // Create dashboard panel (right sidebar)
        this.dashboardPanel = document.createElement('div');
        this.dashboardPanel.id = 'dashboard-panel';
        this.dashboardPanel.innerHTML = `
            <div class="panel-header">System Dashboard</div>
            <div class="stat-row">
                <span>Visual RAM:</span>
                <div class="bar-container">
                    <div class="bar-fill" id="ram-bar"></div>
                </div>
                <span id="ram-text">0 / 64 KB</span>
            </div>
            <div class="stat-row">
                <span>Processes:</span>
                <span id="process-count">0</span>
            </div>
            <div class="stat-row">
                <span>Neural Pulse:</span>
                <span id="neural-pulse">0</span>
                <span>iter/s</span>
            </div>
            <div class="stat-row">
                <span>IPC Messages:</span>
                <span id="ipc-count">0</span>
            </div>
            <div class="stat-row">
                <span>Uptime:</span>
                <span id="uptime">00:00:00</span>
            </div>
            <div class="divider"></div>
            <div class="stat-row">
                <label><input type="checkbox" id="saccade-toggle"> Kernel Saccades</label>
            </div>
            <div class="divider"></div>
            <div class="panel-subheader">IPC Channels</div>
            <div id="ipc-monitor" class="ipc-monitor">
                <div class="ipc-empty">No active channels</div>
            </div>
        `;
        this.dashboardPanel.style.cssText = `
            position: absolute; top: 80px; right: 20px; width: 250px;
            background: rgba(0, 20, 20, 0.9); border: 1px solid #00ffcc;
            padding: 15px; color: #00ffcc; font-family: monospace;
        `;

        // Append to canvas parent
        this.canvas.parentElement.appendChild(this.spawnerPanel);
        this.canvas.parentElement.appendChild(this.dashboardPanel);

        // Wire events
        this._wireEvents();

        // Start uptime counter
        this._startTime = Date.now();
        setInterval(() => this._updateUptime(), 1000);
    }

    _wireEvents() {
        // Drag-drop handling
        const dropZone = document.getElementById('spv-drop-zone');
        const fileInput = document.getElementById('spv-file-input');

        dropZone.addEventListener('click', () => fileInput.click());

        dropZone.addEventListener('dragover', (e) => {
            e.preventDefault();
            dropZone.classList.add('drag-over');
        });

        dropZone.addEventListener('dragleave', () => {
            dropZone.classList.remove('drag-over');
        });

        dropZone.addEventListener('drop', async (e) => {
            e.preventDefault();
            dropZone.classList.remove('drag-over');
            const files = Array.from(e.dataTransfer.files).filter(f => f.name.endsWith('.spv'));
            for (const file of files) {
                await this.spawnFromFile(file);
            }
        });

        fileInput.addEventListener('change', async (e) => {
            const files = Array.from(e.target.files);
            for (const file of files) {
                await this.spawnFromFile(file);
            }
        });

        // Spawn from brain button
        document.getElementById('spawn-from-brain').addEventListener('click', () => {
            this._showBrainPicker();
        });

        // Saccade toggle
        document.getElementById('saccade-toggle').addEventListener('change', (e) => {
            this.saccadeEnabled = e.target.checked;
            console.log(`[VisualShell] Saccades ${this.saccadeEnabled ? 'enabled' : 'disabled'}`);
        });
    }

    async spawnFromFile(file) {
        console.log(`[VisualShell] Spawning process from ${file.name}`);

        try {
            const buffer = await file.arrayBuffer();
            const pid = await this.kernel.spawnProcess(buffer, file.name);

            this._addProcessToList(pid, file.name);
            this._updateDashboard();

            console.log(`[VisualShell] Process ${pid} spawned`);
        } catch (e) {
            console.error(`[VisualShell] Failed to spawn: ${e.message}`);
        }
    }

    _addProcessToList(pid, name) {
        const list = document.getElementById('process-list');
        const item = document.createElement('div');
        item.className = 'process-item';
        item.id = `process-${pid}`;
        item.innerHTML = `
            <span class="pid">#${pid}</span>
            <span class="pname">${name}</span>
            <span class="pstate" style="color: ${STATE_COLORS[PROC_STATE.IDLE]}">IDLE</span>
        `;
        item.addEventListener('click', () => this._focusProcess(pid));
        list.appendChild(item);

        this.processes.set(pid, {
            name,
            state: PROC_STATE.IDLE,
            pc: 0,
            sp: 0,
            cycles: 0
        });
    }

    _focusProcess(pid) {
        // Calculate Hilbert position for PID
        const pos = this._hilbertDecode(pid);

        // Set camera target for saccade
        this.cameraTarget = {
            x: pos.x,
            y: pos.y,
            z: 5  // Elevated view
        };

        this.activePID = pid;
        console.log(`[VisualShell] Focus on process ${pid} at (${pos.x}, ${pos.y})`);
    }

    _hilbertDecode(index) {
        // Simplified Hilbert curve decode for 32x32 grid
        let x = 0, y = 0;
        let s = 1;
        let d = index;

        while (s < 32) {
            const rx = 1 & (d / 2);
            const ry = 1 & (d ^ rx);

            if (ry === 0) {
                if (rx === 1) {
                    x = s - 1 - x;
                    y = s - 1 - y;
                }
                [x, y] = [y, x];
            }

            x += s * rx;
            y += s * ry;
            d = Math.floor(d / 4);
            s *= 2;
        }

        return { x, y };
    }

    _startPCBReader() {
        // Read PCB data from GPU every frame
        const readPCBs = async () => {
            if (!this.kernel || !this.device) return;

            try {
                // Sync I/O to GPU before step
                await this.syscalls.syncToGPU();

                // Dispatch kernel step (if not already handled by another loop)
                // Note: The main kernel execution usually happens in its own ticker,
                // but we ensure synchronization here.
                await this.kernel.step();

                // Process pending syscalls after step
                await this.syscalls.processSyscalls();

                const pcbs = await this.kernel.readPCBs();
                this._updateProcessStates(pcbs);

                // Update neural pulse rate
                const totalCycles = Array.from(this.processes.values())
                    .reduce((sum, p) => sum + p.cycles, 0);
                this.pulseRate = totalCycles - this.lastIterationCount;
                this.lastIterationCount = totalCycles;

                this._updateDashboard();

                // Apply saccade if enabled
                if (this.saccadeEnabled) {
                    this._applySaccade();
                }
            } catch (e) {
                // Kernel not ready yet
            }

            requestAnimationFrame(readPCBs);
        };

        requestAnimationFrame(readPCBs);
    }

    _updateProcessStates(pcbs) {
        for (const pcb of pcbs) {
            const { pid, pc, sp, state, cycles } = pcb;
            const proc = this.processes.get(pid);

            if (proc) {
                proc.pc = pc;
                proc.sp = sp;
                proc.state = state;
                proc.cycles = cycles;

                // Update process list item
                const item = document.getElementById(`process-${pid}`);
                if (item) {
                    const stateEl = item.querySelector('.pstate');
                    stateEl.textContent = Object.keys(PROC_STATE).find(k => PROC_STATE[k] === state);
                    stateEl.style.color = STATE_COLORS[state];
                }
            }
        }
    }

    _applySaccade() {
        // Find most active process
        let maxCycles = 0;
        let activePID = 0;

        for (const [pid, proc] of this.processes) {
            if (proc.state === PROC_STATE.RUNNING && proc.cycles > maxCycles) {
                maxCycles = proc.cycles;
                activePID = pid;
            }
        }

        if (activePID !== this.activePID && activePID > 0) {
            this._focusProcess(activePID);
        }
    }

    _updateDashboard() {
        // RAM usage
        const usedRAM = this.processes.size * 512 * 4;  // 512 words * 4 bytes
        const totalRAM = 64 * 1024;
        const ramPercent = (usedRAM / totalRAM) * 100;

        const ramBar = document.getElementById('ram-bar');
        if (ramBar) {
            ramBar.style.width = `${ramPercent}%`;
            ramBar.style.background = ramPercent > 80 ? '#ff4444' : '#00ff88';
        }

        const ramText = document.getElementById('ram-text');
        if (ramText) {
            ramText.textContent = `${(usedRAM / 1024).toFixed(1)} / ${totalRAM / 1024} KB`;
        }

        // Process count
        const procCount = document.getElementById('process-count');
        if (procCount) {
            const running = Array.from(this.processes.values()).filter(p => p.state === PROC_STATE.RUNNING).length;
            procCount.textContent = `${this.processes.size} (${running} running)`;
        }

        // Neural pulse
        const pulseEl = document.getElementById('neural-pulse');
        if (pulseEl) {
            pulseEl.textContent = this.pulseRate;
        }

        // IPC count
        const ipcEl = document.getElementById('ipc-count');
        if (ipcEl) {
            ipcEl.textContent = this.ipcCount;
        }

        // Update IPC monitor
        this._updateIPCMonitor();
    }

    /**
     * Record an IPC message sent between processes.
     * @param {number} fromPid - Sender PID
     * @param {number} toPid - Receiver PID
     * @param {number} msgType - Message type
     */
    recordIPC(fromPid, toPid, msgType = 0) {
        this.ipcCount++;

        const key = `${fromPid}->${toPid}`;
        const channel = this.ipcChannels.get(key) || {
            from: fromPid,
            to: toPid,
            count: 0,
            lastMsg: 0
        };
        channel.count++;
        channel.lastMsg = msgType;
        this.ipcChannels.set(key, channel);
    }

    _updateIPCMonitor() {
        const monitor = document.getElementById('ipc-monitor');
        if (!monitor) return;

        if (this.ipcChannels.size === 0) {
            monitor.innerHTML = '<div class="ipc-empty">No active channels</div>';
            return;
        }

        const rows = [];
        for (const [key, ch] of this.ipcChannels) {
            const fromName = this.processes.get(ch.from)?.name || `#${ch.from}`;
            const toName = this.processes.get(ch.to)?.name || `#${ch.to}`;
            rows.push(`
                <div class="ipc-channel">
                    <span class="ipc-from">${fromName.slice(0, 10)}</span>
                    <span class="ipc-arrow">→</span>
                    <span class="ipc-to">${toName.slice(0, 10)}</span>
                    <span class="ipc-count">${ch.count}</span>
                </div>
            `);
        }
        monitor.innerHTML = rows.join('');
    }

    _updateUptime() {
        const elapsed = Math.floor((Date.now() - this._startTime) / 1000);
        const hours = Math.floor(elapsed / 3600).toString().padStart(2, '0');
        const minutes = Math.floor((elapsed % 3600) / 60).toString().padStart(2, '0');
        const seconds = (elapsed % 60).toString().padStart(2, '0');

        const uptimeEl = document.getElementById('uptime');
        if (uptimeEl) {
            uptimeEl.textContent = `${hours}:${minutes}:${seconds}`;
        }
    }

    _showBrainPicker() {
        // Fetch available memories and show picker
        console.log('[VisualShell] Opening Brain Picker...');

        // For now, just fetch the substrate and list
        fetch('assets/memory_substrate.spv')
            .then(r => r.arrayBuffer())
            .then(buffer => {
                const view = new DataView(buffer, 56);  // Skip header
                const count = buffer.byteLength / (512 * 4);

                const picker = document.createElement('div');
                picker.className = 'brain-picker';
                picker.innerHTML = `
                    <div class="picker-header">Select Memory to Spawn</div>
                    <div class="picker-list">
                        ${Array.from({length: Math.min(count, 20)}, (_, i) => `
                            <div class="picker-item" data-pid="${i}">
                                Memory #${i}
                            </div>
                        `).join('')}
                    </div>
                    <button class="btn close-picker">Cancel</button>
                `;

                document.body.appendChild(picker);

                picker.querySelector('.close-picker').onclick = () => picker.remove();
                picker.querySelectorAll('.picker-item').forEach(item => {
                    item.onclick = () => {
                        const pid = parseInt(item.dataset.pid);
                        this._spawnFromMemory(pid);
                        picker.remove();
                    };
                });
            });
    }

    async _spawnFromMemory(pid) {
        console.log(`[VisualShell] Spawning from memory ${pid}`);
        // This would create a new process from the memory's embedding
        // For now, just focus on it
        this._focusProcess(pid);
    }
}
