import { Process } from './Process.js';
import { Scheduler } from './Scheduler.js';

/**
 * Geometry OS Process Manager
 *
 * Manages multi-process execution with memory isolation and IPC.
 */

export class ProcessManager {
    constructor() {
        this.device = null;
        this.pipeline = null;
        this.processes = new Map();
        this.scheduler = new Scheduler();
        this.maxProcesses = 16;
        this.tickCount = 0;

        // Memory configuration
        this.SHARED_MEM_BASE = 0;
        this.SHARED_MEM_SIZE = 1024;
        this.PROCESS_MEM_BASE = 1024;
        this.PROCESS_MEM_SIZE = 1024;

        // Program storage offsets (each program gets 16KB)
        this.PROGRAM_SIZE = 16384;
        this.nextProgramOffset = 0;

        // GPU Buffers
        this.programBuffer = null;
        this.stackBuffer = null;
        this.ramBuffer = null;
        this.pcbBuffer = null;
        this.labelsBuffer = null;
        this.resultBuffer = null;

        // Staging buffers for readback
        this.readRamBuffer = null;
        this.readPcbBuffer = null;
    }

    async init() {
        if (!navigator.gpu) throw new Error('WebGPU not supported');
        const adapter = await navigator.gpu.requestAdapter();
        this.device = await adapter.requestDevice();

        const response = await fetch('kernel.wgsl');
        const code = await response.text();

        this.pipeline = this.device.createComputePipeline({
            layout: 'auto',
            compute: {
                module: this.device.createShaderModule({ code }),
                entryPoint: 'main',
            },
        });

        this._initBuffers();
        console.log('[ProcessManager] GPU Kernel Initialized');
        console.log(`[ProcessManager] Shared Memory: [${this.SHARED_MEM_BASE}-${this.SHARED_MEM_BASE + this.SHARED_MEM_SIZE - 1}]`);
        console.log(`[ProcessManager] Process Memory: starts at ${this.PROCESS_MEM_BASE}, ${this.PROCESS_MEM_SIZE} per process`);
    }

    _initBuffers() {
        // Program Memory (256KB total, enough for 16 programs)
        this.programBuffer = this.device.createBuffer({
            size: this.PROGRAM_SIZE * this.maxProcesses,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        });

        // Stack per process (1024 floats each)
        this.stackBuffer = this.device.createBuffer({
            size: 1024 * this.maxProcesses * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        });

        // Total RAM: Shared + Process memories
        const totalRam = this.PROCESS_MEM_BASE + (this.PROCESS_MEM_SIZE * this.maxProcesses);
        this.ramBuffer = this.device.createBuffer({
            size: totalRam * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        });

        // PCB Table (16 processes * 16 words)
        this.pcbBuffer = this.device.createBuffer({
            size: 16 * 16 * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        });

        // Labels buffer
        this.labelsBuffer = this.device.createBuffer({
            size: 1024 * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        });

        // Result buffer
        this.resultBuffer = this.device.createBuffer({
            size: 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
        });

        // Readback buffers
        this.readRamBuffer = this.device.createBuffer({
            size: totalRam * 4,
            usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
        });

        this.readPcbBuffer = this.device.createBuffer({
            size: 16 * 16 * 4,
            usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
        });

        // Initialize PCB table to zeros
        const emptyPcb = new Uint32Array(16 * 16);
        this.device.queue.writeBuffer(this.pcbBuffer, 0, emptyPcb);
    }

    /**
     * Spawn a new process with isolated memory
     * @param {number} pid - Process ID (0-15)
     * @param {ArrayBuffer} spirvBinary - SPIR-V binary
     * @param {Object} options - Optional configuration
     */
    spawn(pid, spirvBinary, options = {}) {
        if (this.processes.has(pid)) {
            console.warn(`[ProcessManager] PID ${pid} already exists, replacing`);
        }

        const programOffset = pid * this.PROGRAM_SIZE;
        const memBase = this.PROCESS_MEM_BASE + (pid * this.PROCESS_MEM_SIZE);

        console.log(`[ProcessManager] Spawning PID ${pid}`);

        // Load binary into program buffer at dedicated offset
        const binary = new Uint32Array(spirvBinary);
        this.device.queue.writeBuffer(this.programBuffer, programOffset, binary);

        const proc = new Process(pid, options.name || `pid_${pid}`, {
            priority: options.priority || 5,
            memBase,
            memLimit: this.PROCESS_MEM_SIZE
        });

        // Create PCB entry
        // Layout: pid, pc, sp, mem_base, mem_limit, status, priority, waiting_on, msg_count, reserved[7]
        // Note: programOffset moved to pcb[15] (reserved)
        const pcb = new Uint32Array(16);
        pcb[0] = pid;
        pcb[1] = 5 + programOffset;  // PC starts after header, offset by program location
        pcb[2] = 0;                   // SP
        pcb[3] = memBase;             // Memory base
        pcb[4] = this.PROCESS_MEM_SIZE; // Memory limit
        pcb[5] = 1;                   // Status: Running
        pcb[6] = proc.priority;       // Priority/Cycles
        pcb[7] = 0xFF;                // waiting_on (0xFF = none/any)
        pcb[8] = 0;                   // msg_count
        pcb[15] = programOffset;      // Store program offset in last reserved word

        this.device.queue.writeBuffer(this.pcbBuffer, pid * 16 * 4, pcb);
        this.processes.set(pid, proc);
    }

    /**
     * Write to shared memory region for IPC
     * @param {number} offset - Offset within shared memory (0-1023)
     * @param {Float32Array|number[]} values - Values to write
     */
    writeSharedMemory(offset, values) {
        if (offset + values.length > this.SHARED_MEM_SIZE) {
            throw new Error(`Shared memory write out of bounds: ${offset} + ${values.length} > ${this.SHARED_MEM_SIZE}`);
        }
        const data = values instanceof Float32Array ? values : new Float32Array(values);
        this.device.queue.writeBuffer(this.ramBuffer, (this.SHARED_MEM_BASE + offset) * 4, data);
        console.log(`[ProcessManager] Wrote ${values.length} values to shared memory [${offset}-${offset + values.length - 1}]`);
    }

    /**
     * Write to a specific process's memory region
     * @param {number} pid - Process ID
     * @param {number} offset - Offset within process memory
     * @param {Float32Array|number[]} values - Values to write
     */
    writeProcessMemory(pid, offset, values) {
        const proc = this.processes.get(pid);
        if (!proc) throw new Error(`Process ${pid} not found`);

        if (offset + values.length > proc.memLimit) {
            throw new Error(`Process memory write out of bounds`);
        }

        const data = values instanceof Float32Array ? values : new Float32Array(values);
        this.device.queue.writeBuffer(this.ramBuffer, (proc.memBase + offset) * 4, data);
    }

    /**
     * Spawn a process from SPIR-V binary with auto-assigned PID.
     * @param {ArrayBuffer} spirvBinary - The SPIR-V binary
     * @param {string} name - Process name for display
     * @returns {Promise<number>} The assigned PID
     */
    async spawnProcess(spirvBinary, name = 'unnamed') {
        const pid = this.processes.size;
        if (pid >= this.maxProcesses) {
            throw new Error(`Maximum processes (${this.maxProcesses}) reached`);
        }

        this.spawn(pid, spirvBinary, { name });
        return pid;
    }

    /**
     * Execute one scheduler step (alias for step)
     */
    async tick() {
        return this.step();
    }

    /**
     * Execute one scheduler step (runs all active processes)
     * @returns {Promise<void>}
     */
    async step() {
        const bindGroup = this.device.createBindGroup({
            layout: this.pipeline.getBindGroupLayout(0),
            entries: [
                { binding: 0, resource: { buffer: this.programBuffer } },
                { binding: 1, resource: { buffer: this.stackBuffer } },
                { binding: 2, resource: { buffer: this.resultBuffer } },
                { binding: 3, resource: { buffer: this.ramBuffer } },
                { binding: 4, resource: { buffer: this.labelsBuffer } },
                { binding: 5, resource: { buffer: this.pcbBuffer } },
            ],
        });

        const encoder = this.device.createCommandEncoder();
        const pass = encoder.beginComputePass();
        pass.setPipeline(this.pipeline);
        pass.setBindGroup(0, bindGroup);
        pass.dispatchWorkgroups(1);
        pass.end();

        this.device.queue.submit([encoder.finish()]);
    }

    /**
     * Read shared memory region
     * @returns {Promise<Float32Array>}
     */
    async readSharedMemory() {
        return this._readRAM(this.SHARED_MEM_BASE, this.SHARED_MEM_SIZE);
    }

    /**
     * Read a process's memory region
     * @param {number} pid - Process ID
     * @param {number} offset - Offset within process memory
     * @param {number} length - Number of floats to read
     * @returns {Promise<Float32Array>}
     */
    async readProcessMemory(pid, offset = 0, length = null) {
        const proc = this.processes.get(pid);
        if (!proc) throw new Error(`Process ${pid} not found`);

        const len = length || proc.memLimit;
        return this._readRAM(proc.memBase + offset, len);
    }

    /**
     * Read PCB table to get process states and update local Process objects
     * @returns {Promise<Process[]>}
     */
    async readProcessStates() {
        const encoder = this.device.createCommandEncoder();
        encoder.copyBufferToBuffer(this.pcbBuffer, 0, this.readPcbBuffer, 0, 16 * 16 * 4);
        this.device.queue.submit([encoder.finish()]);

        await this.readPcbBuffer.mapAsync(GPUMapMode.READ);
        const pcbData = new Uint32Array(this.readPcbBuffer.getMappedRange().slice(0));
        this.readPcbBuffer.unmap();

        const states = [];
        for (let i = 0; i < 16; i++) {
            const base = i * 16;
            const pid = pcbData[base + 0];
            const status = pcbData[base + 5];

            if (pid !== 0 || status !== 0) {
                let proc = this.processes.get(pid);
                if (!proc) {
                    proc = new Process(pid, `pid_${pid}`);
                    this.processes.set(pid, proc);
                }

                proc.update({
                    pc: pcbData[base + 1],
                    sp: pcbData[base + 2],
                    status: ['idle', 'running', 'waiting', 'exit'][status] || 'unknown',
                    cycles: pcbData[base + 6]
                });
                states.push(proc);
            }
        }

        this.tickCount++;
        if (this.tickCount % 10 === 0) {
            this.scheduler.boostPriorities(this.processes);
            this._syncPriorities();
        }

        return states;
    }

    /**
     * Sync CPU-side priorities back to GPU
     */
    _syncPriorities() {
        for (const [pid, proc] of this.processes) {
            if (proc.status !== 'exit') {
                const data = new Uint32Array([proc.priority]);
                // Priority is at offset 6 in the PCB (6 * 4 bytes)
                this.device.queue.writeBuffer(this.pcbBuffer, (pid * 16 * 4) + (6 * 4), data);
            }
        }
    }

    /**
     * Internal: Read RAM region
     */
    async _readRAM(offset, length) {
        const readBuffer = this.device.createBuffer({
            size: length * 4,
            usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
        });

        const encoder = this.device.createCommandEncoder();
        encoder.copyBufferToBuffer(this.ramBuffer, offset * 4, readBuffer, 0, length * 4);
        this.device.queue.submit([encoder.finish()]);

        await readBuffer.mapAsync(GPUMapMode.READ);
        const result = new Float32Array(readBuffer.getMappedRange().slice(0));
        readBuffer.unmap();
        readBuffer.destroy();

        return result;
    }

    /**
     * Kill a process
     * @param {number} pid - Process ID
     */
    kill(pid) {
        const pcb = new Uint32Array(16);
        pcb[5] = 3; // Status: Exit
        this.device.queue.writeBuffer(this.pcbBuffer, pid * 16 * 4, pcb);
        this.processes.delete(pid);
        console.log(`[ProcessManager] Killed PID ${pid}`);
    }

    /**
     * Get memory layout info
     */
    getMemoryLayout() {
        return {
            sharedMemory: { base: this.SHARED_MEM_BASE, size: this.SHARED_MEM_SIZE },
            processMemory: { base: this.PROCESS_MEM_BASE, size: this.PROCESS_MEM_SIZE },
            processes: Array.from(this.processes.values()).map(p => ({
                pid: p.pid,
                memBase: p.memBase,
                memEnd: p.memBase + p.memLimit - 1
            }))
        };
    }
}
