/**
 * GPU-Native Health Watchdog Agent
 * 
 * Manages the system health monitoring on the GPU. The Watchdog runs as
 * a system service (PID 1) and monitors all processes for:
 * - Zombie processes (terminated but not cleaned)
 * - Hog processes (exceeding cycle quotas)
 * - Deadlock conditions
 * - Memory leaks
 * 
 * This moves core OS policy from JavaScript to the GPU substrate.
 */

// Watchdog commands
const WDOG_COMMAND = {
    IDLE: 0,
    SCAN: 1,
    REMEDIATE: 2,
    GET_STATS: 3
};

// Issue types (from watchdog.wgsl)
const ISSUE_TYPE = {
    NONE: 0,
    ZOMBIE: 1,
    HOG: 2,
    ERRORS: 3,
    MEMORY_LEAK: 4,
    DEADLOCK: 5
};

// Actions
const WDOG_ACTION = {
    NONE: 0,
    WARN: 1,
    KILL: 2,
    RESTART: 3,
    SIGNAL: 4
};

// Process states
const PROC_STATE = {
    IDLE: 0,
    RUNNING: 1,
    WAITING: 2,
    EXIT: 3,
    ERROR: 4
};

export class WatchdogAgent {
    constructor(kernel, options = {}) {
        this.kernel = kernel;
        this.device = kernel.device;
        
        // Configuration
        this.scanInterval = options.scanInterval || 1000;  // ms
        this.autoRemediate = options.autoRemediate ?? true;
        this.cycleQuota = options.cycleQuota || 1000000;
        
        // GPU resources
        this.healthRecordsBuffer = null;
        this.watchdogStatsBuffer = null;
        this.controlBuffer = null;
        this.actionQueueBuffer = null;
        this.pipeline = null;
        this.bindGroup = null;
        
        // State
        this.isRunning = false;
        this.lastScan = 0;
        this.stats = {
            totalScans: 0,
            zombiesFound: 0,
            hogsFound: 0,
            deadlocksFound: 0,
            memoryLeaksFound: 0,
            processesKilled: 0,
            processesWarned: 0
        };
        
        // Callbacks
        this.onIssue = options.onIssue || null;
        this.onAction = options.onAction || null;
    }
    
    /**
     * Initialize the watchdog agent.
     */
    async init() {
        // Create health records buffer (one record per process)
        this.healthRecordsBuffer = this.device.createBuffer({
            size: 256 * 32,  // 256 processes * 32 bytes each
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        });
        
        // Create stats buffer
        this.watchdogStatsBuffer = this.device.createBuffer({
            size: 32,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        });
        
        // Create control buffer
        this.controlBuffer = this.device.createBuffer({
            size: 32,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        });
        
        // Create action queue buffer (actions to take)
        this.actionQueueBuffer = this.device.createBuffer({
            size: 256 * 4,  // 256 actions * 4 bytes
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        });
        
        // Load and create pipeline
        await this._loadPipeline();
        
        // Initialize control
        this._resetControl();
        
        console.log('[WatchdogAgent] GPU-native watchdog initialized');
    }
    
    /**
     * Load the watchdog pipeline.
     */
    async _loadPipeline() {
        const response = await fetch('watchdog.wgsl');
        const shaderCode = await response.text();
        
        const shaderModule = this.device.createShaderModule({
            code: shaderCode
        });
        
        // Create bind group layout
        const bindGroupLayout = this.device.createBindGroupLayout({
            entries: [
                { binding: 0, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'read-only-storage' } },
                { binding: 1, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'storage' } },
                { binding: 2, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'storage' } },
                { binding: 3, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'storage' } },
                { binding: 4, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'storage' } },
            ]
        });
        
        this.pipeline = this.device.createComputePipeline({
            layout: this.device.createPipelineLayout({
                bindGroupLayouts: [bindGroupLayout]
            }),
            compute: {
                module: shaderModule,
                entryPoint: 'main'
            }
        });
        
        // Create bind group
        this.bindGroup = this.device.createBindGroup({
            layout: bindGroupLayout,
            entries: [
                { binding: 0, resource: { buffer: this.kernel.pcbBuffer } },
                { binding: 1, resource: { buffer: this.healthRecordsBuffer } },
                { binding: 2, resource: { buffer: this.watchdogStatsBuffer } },
                { binding: 3, resource: { buffer: this.controlBuffer } },
                { binding: 4, resource: { buffer: this.actionQueueBuffer } },
            ]
        });
    }
    
    /**
     * Reset control buffer.
     */
    _resetControl() {
        const control = new Uint32Array(8);
        control[0] = WDOG_COMMAND.IDLE;
        control[1] = this.scanInterval;
        control[2] = this.autoRemediate ? 1 : 0;
        control[3] = this.cycleQuota;
        control[4] = 0;  // status
        this.device.queue.writeBuffer(this.controlBuffer, 0, control);
    }
    
    /**
     * Start the watchdog.
     */
    start() {
        if (this.isRunning) return;
        
        this.isRunning = true;
        this._scanLoop();
        console.log('[WatchdogAgent] Watchdog started');
    }
    
    /**
     * Stop the watchdog.
     */
    stop() {
        this.isRunning = false;
        console.log('[WatchdogAgent] Watchdog stopped');
    }
    
    /**
     * Main scan loop.
     */
    async _scanLoop() {
        if (!this.isRunning) return;
        
        // Trigger scan
        await this._runScan();
        
        // Schedule next scan
        setTimeout(() => this._scanLoop(), this.scanInterval);
    }
    
    /**
     * Run a single scan.
     */
    async _runScan() {
        // Set command to scan
        const control = new Uint32Array(8);
        control[0] = WDOG_COMMAND.SCAN;
        control[1] = this.scanInterval;
        control[2] = this.autoRemediate ? 1 : 0;
        control[3] = this.cycleQuota;
        this.device.queue.writeBuffer(this.controlBuffer, 1, control);
        
        // Dispatch watchdog
        const commandEncoder = this.device.createCommandEncoder();
        const passEncoder = commandEncoder.beginComputePass();
        passEncoder.setPipeline(this.pipeline);
        passEncoder.setBindGroup(0, this.bindGroup);
        passEncoder.dispatchWorkgroups(1);
        passEncoder.end();
        this.device.queue.submit([commandEncoder.finish()]);
        
        // Read results
        await this._readResults();
        
        // Process actions
        await this._processActions();
        
        this.lastScan = Date.now();
    }
    
    /**
     * Read scan results.
     */
    async _readResults() {
        // Read stats
        const statsData = await this._readBuffer(this.watchdogStatsBuffer, 32);
        const statsView = new DataView(statsData);
        
        this.stats.totalScans = statsView.getUint32(0, true);
        this.stats.zombiesFound = statsView.getUint32(1 * 4, true);
        this.stats.hogsFound = statsView.getUint32(2 * 4, true);
        this.stats.deadlocksFound = statsView.getUint32(3 * 4, true);
        this.stats.memoryLeaksFound = statsView.getUint32(4 * 4, true);
        this.stats.processesKilled = statsView.getUint32(5 * 4, true);
        this.stats.processesWarned = statsView.getUint32(6 * 4, true);
    }
    
    /**
     * Process pending actions.
     */
    async _processActions() {
        const actionData = await this._readBuffer(this.actionQueueBuffer, 256 * 4);
        const actions = new Uint32Array(actionData);
        
        for (let i = 0; i < 256; i++) {
            const actionType = actions[i * 4 + 0];
            const pid = actions[i * 4 + 1];
            const signal = actions[i * 4 + 2];
            const _reserved = actions[i * 4 + 3];
            
            if (actionType === 0 || pid === 0xFFFFFFFF) break;  // End marker
            
            if (actionType === WDOG_ACTION.KILL) {
                console.log(`[WatchdogAgent] Killing process ${pid}`);
                if (this.kernel.killProcess) {
                    this.kernel.killProcess(pid);
                }
                if (this.onAction) {
                    this.onAction('kill', pid, signal);
                }
            } else if (actionType === WDOG_ACTION.WARN) {
                console.log(`[WatchdogAgent] Warning: Process ${pid} unhealthy`);
                if (this.onIssue) {
                    this.onIssue(pid, 'unhealthy', actionType);
                }
            } else if (actionType === WDOG_ACTION.SIGNAL) {
                console.log(`[WatchdogAgent] Sending signal ${signal} to process ${pid}`);
                if (this.kernel.signalProcess) {
                    this.kernel.signalProcess(pid, signal);
                }
            }
        }
    }
    
    /**
     * Read a GPU buffer.
     */
    async _readBuffer(buffer, size) {
        const staging = this.device.createBuffer({
            size: size,
            usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST
        });
        
        const commandEncoder = this.device.createCommandEncoder();
        commandEncoder.copyBufferToBuffer(buffer, 0, staging, 0, size);
        this.device.queue.submit([commandEncoder.finish()]);
        
        await staging.mapAsync(GPUMapMode.READ);
        const data = staging.getMappedRange().slice(0);
        staging.unmap();
        staging.destroy();
        
        return data;
    }
    
    /**
     * Get current stats.
     */
    getStats() {
        return { ...this.stats };
    }
    
    /**
     * Check if watchdog is running.
     */
    isHealthy() {
        return this.isRunning && (Date.now() - this.lastScan) < this.scanInterval * 2;
    }
    
    /**
     * Force an immediate scan.
     */
    async scanNow() {
        await this._runScan();
    }
}

// Re-export constants for external use
export { WDOG_COMMAND, ISSUE_TYPE, WDOG_ACTION, PROC_STATE };
