/**
 * Geometry OS Kernel Controller
 * 
 * Manages the multi-process execution environment on the GPU.
 */

export class GeometryKernel {
    constructor() {
        this.device = null;
        this.pipeline = null;
        this.processes = [];
        this.maxProcesses = 16;
        
        // GPU Buffers
        this.programBuffer = null;
        this.stackBuffer = null;
        this.ramBuffer = null;
        this.pcbBuffer = null;
        this.labelsBuffer = null;
        this.resultBuffer = null;
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

        // Initialize empty buffers
        this._initBuffers();
        console.log('[GOS Kernel] GPU Kernel Initialized');
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

        // Shared RAM (256KB)
        this.ramBuffer = this.device.createBuffer({
            size: 256 * 1024 * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
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
    }

    spawn(pid, spirvBinary, memBase, memLimit) {
        console.log(`[GOS Kernel] Spawning PID ${pid} at RAM offset ${memBase}...`);
        
        // 1. Load binary into program buffer (simple append for MVP)
        const binary = new Uint32Array(spirvBinary);
        this.device.queue.writeBuffer(this.programBuffer, 0, binary);

        // 2. Create PCB entry
        // Layout: pid, pc, sp, mem_base, mem_limit, status, priority, waiting_on, msg_count, reserved[7]
        const pcb = new Uint32Array(16);
        pcb[0] = pid;
        pcb[1] = 5; // Start after header
        pcb[2] = 0; // SP
        pcb[3] = memBase;
        pcb[4] = memLimit;
        pcb[5] = 1; // Running
        pcb[6] = 5; // Default priority
        pcb[7] = 0xFF; // waiting_on
        pcb[8] = 0; // msg_count

        this.device.queue.writeBuffer(this.pcbBuffer, pid * 16 * 4, pcb);
        
        this.processes.push({ pid, status: 'running' });
    }

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
     * Spawn a process from SPIR-V binary with auto-assigned PID.
     * @param {ArrayBuffer} spirvBinary - The SPIR-V binary
     * @param {string} name - Process name for display
     * @returns {number} The assigned PID
     */
    async spawnProcess(spirvBinary, name = 'unnamed') {
        const pid = this.processes.length;
        if (pid >= this.maxProcesses) {
            throw new Error(`Maximum processes (${this.maxProcesses}) reached`);
        }

        // Calculate memory region (512 words per process)
        const memBase = pid * 512;
        const memLimit = 512;

        this.spawn(pid, spirvBinary, memBase, memLimit);
        this.processes[pid].name = name;

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
            pcbs.push({
                pid: data[offset + 0],
                pc: data[offset + 1],
                sp: data[offset + 2],
                state: data[offset + 5],  // status field
                cycles: data[offset + 6], // Using priority field for cycle count
            });
        }

        stagingBuffer.unmap();
        stagingBuffer.destroy();

        return pcbs;
    }
}
