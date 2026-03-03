/**
 * Geometry OS Kernel Controller
 *
 * Manages the multi-process execution environment on the GPU.
 * Now integrated with GPUMemoryManager for page-based memory.
 */

import { GPUMemoryManager, PAGE_SIZE_FLOATS, PAGE_FLAGS } from './GPUMemoryManager.js';
import { Process } from './Process.js';
import { Scheduler } from './Scheduler.js';

export class GeometryKernel {
    constructor() {
        this.device = null;
        this.pipeline = null;
        this.processes = [];
        this.maxProcesses = 16;
        this.scheduler = new Scheduler();

        // GPU Buffers
        this.programBuffer = null;
        this.stackBuffer = null;
        this.pcbBuffer = null;
        this.labelsBuffer = null;
        this.resultBuffer = null;
        this.pageTableBuffer = null;
        this.freeBitmapBuffer = null;

        // Memory manager
        this.memoryManager = null;
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

        // Initialize memory manager
        this.memoryManager = new GPUMemoryManager(this.device);
        await this.memoryManager.init();

        // Initialize empty buffers
        this._initBuffers();
        console.log('[GOS Kernel] GPU Kernel Initialized with Memory Manager');
    }

    _initBuffers() {
        // Shared Program Memory (64KB)
        this.programBuffer = this.device.createBuffer({
            size: 65536 * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        });

        // Shared Stack (1024 floats per process * 16)
        this.stackBuffer = this.device.createBuffer({
            size: 1024 * 16 * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        });

        // PCB Table (16 processes * 16 words)
        this.pcbBuffer = this.device.createBuffer({
            size: 16 * 16 * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        });

        this.labelsBuffer = this.device.createBuffer({
            size: 1024 * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        });

        this.resultBuffer = this.device.createBuffer({
            size: 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
        });

        // Page table buffer (64K entries * 4 bytes)
        this.pageTableBuffer = this.device.createBuffer({
            size: 65536 * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        });

        // Free page bitmap (2048 words = 65536 bits)
        this.freeBitmapBuffer = this.device.createBuffer({
            size: 2048 * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        });

        // Initialize free bitmap (all pages free except kernel region)
        const freeBitmap = new Uint32Array(2048);
        freeBitmap.fill(0xFFFFFFFF);
        // Mark kernel pages as used (first 256 pages)
        for (let i = 0; i < 8; i++) {
            freeBitmap[i] = 0x00000000;
        }
        this.device.queue.writeBuffer(this.freeBitmapBuffer, 0, freeBitmap);
    }

    spawn(pid, spirvBinary, memBase, memLimit, options = {}) {
        console.log(`[GOS Kernel] Spawning PID ${pid} at RAM offset ${memBase}...`);
        
        // 1. Load binary into program buffer (simple append for MVP)
        const binary = new Uint32Array(spirvBinary);
        this.device.queue.writeBuffer(this.programBuffer, 0, binary);

        // 2. Create PCB entry
        // Layout: pid, pc, sp, mem_base, mem_limit, status, static_priority, dynamic_priority, total_cycles, last_run_timestamp, waiting_on, msg_count, reserved[4]
        const pcb = new Uint32Array(16);
        pcb[0] = pid;
        pcb[1] = 5; // Start after header
        pcb[2] = 0; // SP
        pcb[3] = memBase;
        pcb[4] = memLimit;
        pcb[5] = 1; // Status: Running
        pcb[6] = options.priority || 20; // static_priority
        pcb[7] = pcb[6];                // dynamic_priority
        pcb[8] = 0;                     // total_cycles
        pcb[9] = Date.now() & 0xFFFFFFFF; // last_run_timestamp
        pcb[10] = 0xFF;                 // waiting_on
        pcb[11] = 0;                    // msg_count

        this.device.queue.writeBuffer(this.pcbBuffer, pid * 16 * 4, pcb);
        
        const proc = new Process(pid, options.name || `pid_${pid}`, {
            priority: options.priority,
            memBase,
            memLimit
        });
        proc.status = 'running';
        this.processes[pid] = proc;
    }

    async step() {
        const bindGroup = this.device.createBindGroup({
            layout: this.pipeline.getBindGroupLayout(0),
            entries: [
                { binding: 0, resource: { buffer: this.programBuffer } },
                { binding: 1, resource: { buffer: this.stackBuffer } },
                { binding: 2, resource: { buffer: this.resultBuffer } },
                { binding: 3, resource: { buffer: this.memoryManager.memoryBuffer } },
                { binding: 4, resource: { buffer: this.labelsBuffer } },
                { binding: 5, resource: { buffer: this.pcbBuffer } },
                { binding: 6, resource: { buffer: this.pageTableBuffer } },
                { binding: 7, resource: { buffer: this.freeBitmapBuffer } },
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
     * Spawn a process from SPIR-V binary with auto-assigned PID.
     * Uses GPUMemoryManager for dynamic memory allocation.
     * @param {ArrayBuffer} spirvBinary - The SPIR-V binary
     * @param {string} name - Process name for display
     * @param {number} memorySize - Optional memory size in bytes (default 16KB)
     * @returns {number} The assigned PID
     */
    async spawnProcess(spirvBinary, name = 'unnamed', memorySize = 16384, priority = 20) {
        const pid = this.processes.length;
        if (pid >= this.maxProcesses) {
            throw new Error(`Maximum processes (${this.maxProcesses}) reached`);
        }

        // Allocate memory using GPUMemoryManager
        const memMap = this.memoryManager.malloc(pid, memorySize, PAGE_FLAGS.READ | PAGE_FLAGS.WRITE);

        this.spawn(pid, spirvBinary, memMap.base, memMap.limit, { name, priority });
        this.processes[pid].memMap = memMap;

        // Sync page table to GPU
        this.memoryManager.syncToGPU();

        return pid;
    }

    /**
     * Read all PCB entries from GPU buffer.
     * @returns {Promise<Array>} Array of PCB objects
     */
    async readPCBs() {
        const pcbCount = this.processes.length;
        if (pcbCount === 0) return [];

        // Create staging buffer for reading
        const stagingBuffer = this.device.createBuffer({
            size: pcbCount * 16 * 4,
            usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
        });

        const encoder = this.device.createCommandEncoder();
        encoder.copyBufferToBuffer(
            this.pcbBuffer, 0,
            stagingBuffer, 0,
            pcbCount * 16 * 4
        );
        this.device.queue.submit([encoder.finish()]);

        await stagingBuffer.mapAsync(GPUMapMode.READ);
        const data = new Uint32Array(stagingBuffer.getMappedRange());

        const pcbs = [];
        for (let i = 0; i < pcbCount; i++) {
            const offset = i * 16;
            const pid = data[offset + 0];
            
            // Map GPU status back to string
            const statusMap = ['idle', 'running', 'waiting', 'exit', 'error'];
            const status = statusMap[data[offset + 5]] || 'unknown';

            const gpuState = {
                pid,
                pc: data[offset + 1],
                sp: data[offset + 2],
                status,
                dynamicPriority: data[offset + 7],
                totalCycles: data[offset + 8],
                lastRunTimestamp: data[offset + 9],
                faultCount: data[offset + 11] // fault_count is at offset 11
            };

            // Update local process object if it exists
            if (this.processes[i]) {
                this.processes[i].update(gpuState);
            }

            pcbs.push(gpuState);
        }

        stagingBuffer.unmap();
        stagingBuffer.destroy();

        // Run scheduler tick for aging/decay
        this.scheduler.tick(this.processes);
        this.syncPriorities();

        return pcbs;
    }

    /**
     * Sync CPU-side dynamic priorities back to GPU.
     */
    syncPriorities() {
        for (let i = 0; i < this.processes.length; i++) {
            const proc = this.processes[i];
            if (proc && proc.status !== 'exit') {
                const priorityData = new Uint32Array([proc.dynamicPriority]);
                // dynamic_priority is at offset 7 in the PCB (7 * 4 bytes)
                this.device.queue.writeBuffer(this.pcbBuffer, (i * 16 * 4) + (7 * 4), priorityData);
            }
        }
    }

    /**
     * Read shared memory region (IPC mailboxes).
     * @param {number} offset - Start offset in words
     * @param {number} count - Number of words to read
     * @returns {Promise<Uint32Array>} Shared memory data
     */
    async readSharedMemory(offset = 0, count = 512) {
        const stagingBuffer = this.device.createBuffer({
            size: count * 4,
            usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
        });

        const encoder = this.device.createCommandEncoder();
        encoder.copyBufferToBuffer(
            this.memoryManager.memoryBuffer, offset * 4,
            stagingBuffer, 0,
            count * 4
        );
        this.device.queue.submit([encoder.finish()]);

        await stagingBuffer.mapAsync(GPUMapMode.READ);
        const data = new Uint32Array(stagingBuffer.getMappedRange().slice(0));

        stagingBuffer.unmap();
        stagingBuffer.destroy();

        return data;
    }

    /**
     * Write to shared memory region.
     * @param {number} offset - Start offset in words
     * @param {Uint32Array} data - Data to write
     */
    writeSharedMemory(offset, data) {
        this.device.queue.writeBuffer(this.memoryManager.memoryBuffer, offset * 4, data);
    }

    /**
     * Kill a process and free its memory.
     * @param {number} pid - Process ID to kill
     */
    killProcess(pid) {
        if (pid >= this.processes.length || !this.processes[pid]) {
            return false;
        }

        // Free memory
        this.memoryManager.free(pid);

        // Update PCB to terminated status
        const pcb = new Uint32Array(16);
        pcb[5] = 3; // Status: terminated
        this.device.queue.writeBuffer(this.pcbBuffer, pid * 16 * 4, pcb);

        this.processes[pid].status = 'terminated';
        console.log(`[GOS Kernel] Killed PID ${pid} and freed memory`);

        return true;
    }

    /**
     * Get memory statistics.
     * @returns {Object} Memory stats
     */
    getMemoryStats() {
        return this.memoryManager.getStats();
    }

    /**
     * Get process memory info.
     * @param {number} pid - Process ID
     * @returns {Object|null} Memory map or null
     */
    getProcessMemory(pid) {
        return this.memoryManager.getProcessMemory(pid);
    }

    /**
     * Read page table from GPU.
     * @param {number} startPage - Start page index
     * @param {number} count - Number of pages to read
     * @returns {Promise<Uint32Array>} Page table entries
     */
    async readPageTable(startPage = 0, count = 256) {
        const stagingBuffer = this.device.createBuffer({
            size: count * 4,
            usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
        });

        const encoder = this.device.createCommandEncoder();
        encoder.copyBufferToBuffer(
            this.pageTableBuffer, startPage * 4,
            stagingBuffer, 0,
            count * 4
        );
        this.device.queue.submit([encoder.finish()]);

        await stagingBuffer.mapAsync(GPUMapMode.READ);
        const data = new Uint32Array(stagingBuffer.getMappedRange().slice(0));

        stagingBuffer.unmap();
        stagingBuffer.destroy();

        return data;
    }
}
