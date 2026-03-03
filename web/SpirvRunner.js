/**
 * Geometry OS SPIR-V Executor
 *
 * Runs visual programs (SPIR-V binaries) on the GPU via WebGPU.
 * Supports persistent process states for multitasking.
 */

import { SoundSystem } from './SoundSystem.js';

export class SpirvRunner {
    constructor() {
        this.device = null;
        this.pipeline = null;
        this.processes = new Map(); // Store state for each process
        this.soundSystem = new SoundSystem();
    }

    async init() {
        if (!navigator.gpu) {
            throw new Error('WebGPU not supported in this browser');
        }

        const adapter = await navigator.gpu.requestAdapter();
        this.device = await adapter.requestDevice();

        // Load executor shader
        const response = await fetch('executor.wgsl');
        const code = await response.text();

        this.pipeline = this.device.createComputePipeline({
            layout: 'auto',
            compute: {
                module: this.device.createShaderModule({ code }),
                entryPoint: 'main',
            },
        });
        
        console.log('[GOS Runner] WebGPU Pipeline Ready');
    }

    async execute(processId, spirvBinary, labelMap = {}, inputData = {}) {
        if (!this.device) await this.init();

        // 1. Get or Create Process State
        if (!this.processes.has(processId)) {
            this.processes.set(processId, {
                ram: new Float32Array(256).fill(0),
                visual_ram: new Float32Array(256).fill(0),
                stack: new Float32Array(1024).fill(0)
            });
        }
        const state = this.processes.get(processId);

        // 2. Prepare Data
        const programData = new Uint32Array(spirvBinary);
        const labelsData = new Uint32Array(256).fill(0);
        for (const [id, offset] of Object.entries(labelMap)) {
            labelsData[parseInt(id)] = offset;
        }

        // 3. Create Buffers
        const programBuffer = this.device.createBuffer({
            size: programData.byteLength,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        });
        this.device.queue.writeBuffer(programBuffer, 0, programData);

        const stackBuffer = this.device.createBuffer({
            size: 1024 * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST,
        });
        this.device.queue.writeBuffer(stackBuffer, 0, state.stack);

        const resultBuffer = this.device.createBuffer({
            size: 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
        });

        const ramBuffer = this.device.createBuffer({
            size: 256 * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST,
        });
        
        // Merge inputData into persistent RAM
        for (const [addr, val] of Object.entries(inputData)) {
            state.ram[parseInt(addr)] = val;
        }
        this.device.queue.writeBuffer(ramBuffer, 0, state.ram);

        const labelsBuffer = this.device.createBuffer({
            size: labelsData.byteLength,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        });
        this.device.queue.writeBuffer(labelsBuffer, 0, labelsData);

        const visualRamBuffer = this.device.createBuffer({
            size: 256 * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST,
        });
        this.device.queue.writeBuffer(visualRamBuffer, 0, state.visual_ram);

        const readRamBuffer = this.device.createBuffer({
            size: 256 * 4,
            usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
        });

        const readVisualRamBuffer = this.device.createBuffer({
            size: 256 * 4,
            usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
        });

        // 4. Create BindGroup
        const bindGroup = this.device.createBindGroup({
            layout: this.pipeline.getBindGroupLayout(0),
            entries: [
                { binding: 0, resource: { buffer: programBuffer } },
                { binding: 1, resource: { buffer: stackBuffer } },
                { binding: 2, resource: { buffer: resultBuffer } },
                { binding: 3, resource: { buffer: ramBuffer } },
                { binding: 4, resource: { buffer: labelsBuffer } },
                { binding: 5, resource: { buffer: visualRamBuffer } },
            ],
        });

        const encoder = this.device.createCommandEncoder();
        const pass = encoder.beginComputePass();
        pass.setPipeline(this.pipeline);
        pass.setBindGroup(0, bindGroup);
        pass.dispatchWorkgroups(1);
        pass.end();

        encoder.copyBufferToBuffer(ramBuffer, 0, readRamBuffer, 0, 256 * 4);
        encoder.copyBufferToBuffer(visualRamBuffer, 0, readVisualRamBuffer, 0, 256 * 4);
        this.device.queue.submit([encoder.finish()]);

        // 5. Read back and Update State
        await readRamBuffer.mapAsync(GPUMapMode.READ);
        const ramResult = new Float32Array(readRamBuffer.getMappedRange());
        state.ram.set(ramResult); // Persist RAM
        readRamBuffer.unmap();

        await readVisualRamBuffer.mapAsync(GPUMapMode.READ);
        const visualRamResult = new Float32Array(readVisualRamBuffer.getMappedRange());
        state.visual_ram.set(visualRamResult); // Persist Visual RAM
        readVisualRamBuffer.unmap();

        // 6. Process Sound Commands (Memory-mapped I/O: RAM[123-126])
        // RAM[123]=volume, RAM[124]=duration, RAM[125]=frequency, RAM[126]=trigger
        const trigger = state.ram[126];
        if (trigger !== 0) {
            const volume = state.ram[123];
            const duration = state.ram[124];
            const frequency = state.ram[125];
            if (frequency > 0 && volume > 0) {
                this.soundSystem.playTone(frequency, duration, volume);
            }
            // Clear sound command to prevent replay
            state.ram[123] = 0;
            state.ram[124] = 0;
            state.ram[125] = 0;
            state.ram[126] = 0;
        }

        return { ram: state.ram, visual_ram: state.visual_ram };
    }

    dispose() {
        if (this.soundSystem) {
            this.soundSystem.dispose();
        }
        this.processes.clear();
    }
}
