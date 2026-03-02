/**
 * Geometry OS 3D Memory Browser
 * 
 * Visualizes the SPIR-V memory substrate as a 3D semantic landscape.
 * Uses a Hybrid (Hilbert + Semantic Gravity) topography.
 */

import { GeometryFont } from './GeometryFont.js';

export class MemoryBrowser {
    constructor(canvas) {
        this.canvas = canvas;
        this.device = null;
        this.context = null;
        this.format = null;
        
        this.font = new GeometryFont();
        this.memories = [];
        this.tension = 0.15;
        this.listeners = new Map();
        
        // GPU Buffers
        this.memoryBuffer = null;
        this.embeddingBuffer = null;
        this.positionBuffer = null;
        this.pickingBuffer = null;
        
        this.frameCount = 0;
    }

    on(event, callback) {
        if (!this.listeners.has(event)) this.listeners.set(event, []);
        this.listeners.get(event).push(callback);
    }

    emit(event, data) {
        if (this.listeners.has(event)) {
            this.listeners.get(event).forEach(cb => cb(data));
        }
    }

    async init() {
        console.log('[GOS Browser] Igniting WebGPU Substrate...');
        
        if (!navigator.gpu) throw new Error('WebGPU not supported');
        
        const adapter = await navigator.gpu.requestAdapter();
        this.device = await adapter.requestDevice();
        this.context = this.canvas.getContext('webgpu');
        this.format = navigator.gpu.getPreferredCanvasFormat();

        this.context.configure({
            device: this.device,
            format: this.format,
            alphaMode: 'premultiplied',
        });

        // 1. Load Font
        await this.font.load();

        // 2. Load Memory Substrate
        await this._loadSubstrate();

        // 3. Initialize Shaders and Pipelines
        await this._initPipelines();

        // 4. Start Render Loop
        requestAnimationFrame((t) => this._render(t));
        
        this.emit('stats', { 
            count: this.memories.length, 
            format: 'SPIR-V' 
        });
    }

    async _loadSubstrate() {
        const response = await fetch('assets/memory_substrate.spv');
        const buffer = await response.arrayBuffer();
        
        // Skip SPIR-V Header (14 words = 56 bytes based on open_brain/spirv_encoder.py)
        const headerOffset = 14 * 4;
        const words = new Uint32Array(buffer, headerOffset);
        const floats = new Float32Array(buffer, headerOffset);
        
        // Stride is 512 words
        const stride = 512;
        const memoryCount = words.length / stride;
        
        console.log(`[GOS Browser] Loading ${memoryCount} memories from substrate...`);
        
        const embeddingData = new Float32Array(memoryCount * 384);
        const metaData = new Float32Array(memoryCount * 4); // [ID, Opcode, Priority, 0]
        
        for (let i = 0; i < memoryCount; i++) {
            const base = i * stride;
            
            // Layout from spirv_encoder.py:
            // Offset 0: ID
            // Offset 1: Type Opcode
            // Offset 2: Priority
            // Offset 3-386: Embedding
            
            const id = floats[base];
            const opcode = floats[base + 1];
            const priority = floats[base + 2];
            
            metaData[i * 4 + 0] = id;
            metaData[i * 4 + 1] = opcode;
            metaData[i * 4 + 2] = priority;
            
            // Extract embedding
            embeddingData.set(floats.subarray(base + 3, base + 3 + 384), i * 384);
            
            // Cache locally for UI interaction
            this.memories.push({
                id,
                type: this._getMemoryType(opcode),
                priority,
                content: `Memory Entry #${id}`, // Placeholder until we fetch text
                tags: ['geometry_os', 'substrate'],
                embedding: embeddingData.subarray(i * 384, (i + 1) * 384)
            });
        }

        // Create GPU Buffers
        this.embeddingBuffer = this.device.createBuffer({
            size: embeddingData.byteLength,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
            mappedAtCreation: true
        });
        new Float32Array(this.embeddingBuffer.getMappedRange()).set(embeddingData);
        this.embeddingBuffer.unmap();

        this.memoryBuffer = this.device.createBuffer({
            size: metaData.byteLength,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
            mappedAtCreation: true
        });
        new Float32Array(this.memoryBuffer.getMappedRange()).set(metaData);
        this.memoryBuffer.unmap();

        this.positionBuffer = this.device.createBuffer({
            size: memoryCount * 4 * 4, // vec4 positions
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_SRC,
        });

        // 4. Setup Picking
        this.pickingTexture = this.device.createTexture({
            size: [this.canvas.width, this.canvas.height],
            format: 'r32uint',
            usage: GPUBufferUsage.RENDER_ATTACHMENT | GPUBufferUsage.COPY_SRC,
        });

        this.pickingReadBuffer = this.device.createBuffer({
            size: 4,
            usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
        });

        this.canvas.addEventListener('mousedown', (e) => this._onMouseDown(e));
    }

    async _onMouseDown(e) {
        const rect = this.canvas.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;

        const encoder = this.device.createCommandEncoder();
        encoder.copyTextureToBuffer(
            { texture: this.pickingTexture, origin: { x: Math.floor(x), y: Math.floor(y) } },
            { buffer: this.pickingReadBuffer },
            [1, 1]
        );
        this.device.queue.submit([encoder.finish()]);

        await this.pickingReadBuffer.mapAsync(GPUMapMode.READ);
        const pixel = new Uint32Array(this.pickingReadBuffer.getMappedRange());
        const id = pixel[0];
        this.pickingReadBuffer.unmap();

        if (id > 0) {
            this.emit('select', this.memories[id - 1]);
        }
    }

    async _initPipelines() {
        const shaderResponse = await fetch('memory_browser.wgsl');
        const shaderCode = await shaderResponse.text();
        this.shaderModule = this.device.createShaderModule({ code: shaderCode });

        // 1. Uniforms
        this.uniformBuffer = this.device.createBuffer({
            size: 16 * 4 * 3 + 16, // View, Proj, and floats
            usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
        });

        // 2. Compute Pipeline (Gravity)
        this.computePipeline = this.device.createComputePipeline({
            layout: 'auto',
            compute: { module: this.shaderModule, entryPoint: 'compute_gravity' }
        });

        // 3. Render Pipeline (Particles)
        this.renderPipeline = this.device.createRenderPipeline({
            layout: 'auto',
            vertex: { module: this.shaderModule, entryPoint: 'vs_main' },
            fragment: {
                module: this.shaderModule,
                entryPoint: 'fs_main',
                targets: [{
                    format: this.format,
                    blend: {
                        color: { srcFactor: 'src-alpha', dstFactor: 'one-minus-src-alpha' },
                        alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha' }
                    }
                }]
            },
            primitive: { topology: 'triangle-strip' },
            depthStencil: { depthWriteEnabled: true, depthCompare: 'less', format: 'depth24plus' }
        });

        // 4. Picking Pipeline
        this.pickingPipeline = this.device.createRenderPipeline({
            layout: 'auto',
            vertex: { module: this.shaderModule, entryPoint: 'vs_main' },
            fragment: {
                module: this.shaderModule,
                entryPoint: 'fs_picking',
                targets: [{ format: 'r32uint' }]
            },
            primitive: { topology: 'triangle-strip' },
            depthStencil: { depthWriteEnabled: true, depthCompare: 'less', format: 'depth24plus' }
        });

        // 5. Bind Groups
        this.computeBindGroup = this.device.createBindGroup({
            layout: this.computePipeline.getBindGroupLayout(0),
            entries: [
                { binding: 0, resource: { buffer: this.embeddingBuffer } },
                { binding: 1, resource: { buffer: this.memoryBuffer } },
                { binding: 2, resource: { buffer: this.positionBuffer } },
                { binding: 3, resource: { buffer: this.uniformBuffer } },
            ]
        });

        this.renderBindGroup = this.device.createBindGroup({
            layout: this.renderPipeline.getBindGroupLayout(0),
            entries: [
                { binding: 0, resource: { buffer: this.embeddingBuffer } },
                { binding: 1, resource: { buffer: this.memoryBuffer } },
                { binding: 2, resource: { buffer: this.positionBuffer } },
                { binding: 3, resource: { buffer: this.uniformBuffer } },
            ]
        });

        this.depthTexture = this.device.createTexture({
            size: [this.canvas.width, this.canvas.height],
            format: 'depth24plus',
            usage: GPUBufferUsage.RENDER_ATTACHMENT,
        });
    }

    _updateUniforms() {
        const viewMatrix = new Float32Array(16);
        const projMatrix = new Float32Array(16);
        
        // Simple perspective projection (ortho-like for MVP)
        const aspect = this.canvas.width / this.canvas.height;
        projMatrix.set([
            0.1, 0, 0, 0,
            0, 0.1 * aspect, 0, 0,
            0, 0, -0.01, -1,
            0, 0, 0, 1
        ]);

        // Simple look-at (centered on Hilbert grid 32x32)
        viewMatrix.set([
            1, 0, 0, 0,
            0, 1, 0, 0,
            0, 0, 1, 0,
            -16, -16, -20, 1
        ]);

        const params = new Float32Array(4);
        params[0] = this.tension;
        params[1] = this.frameCount * 0.01;
        params[2] = this.memories.length;

        this.device.queue.writeBuffer(this.uniformBuffer, 0, params);
        this.device.queue.writeBuffer(this.uniformBuffer, 16, viewMatrix);
        this.device.queue.writeBuffer(this.uniformBuffer, 80, projMatrix);
    }

    _render(time) {
        if (!this.device || !this.computePipeline) return;

        this.frameCount++;
        this._updateUniforms();

        const encoder = this.device.createCommandEncoder();

        // Pass 1: Gravity Compute
        const computePass = encoder.beginComputePass();
        computePass.setPipeline(this.computePipeline);
        computePass.setBindGroup(0, this.computeBindGroup);
        computePass.dispatchWorkgroups(Math.ceil(this.memories.length / 64));
        computePass.end();

        // Pass 2: Render Particles
        const renderPass = encoder.beginRenderPass({
            colorAttachments: [{
                view: this.context.getCurrentTexture().createView(),
                clearValue: { r: 0.02, g: 0.02, b: 0.02, a: 1.0 },
                loadOp: 'clear',
                storeOp: 'store'
            }],
            depthStencilAttachment: {
                view: this.depthTexture.createView(),
                depthClearValue: 1.0,
                depthLoadOp: 'clear',
                depthStoreOp: 'store',
            }
        });
        renderPass.setPipeline(this.renderPipeline);
        renderPass.setBindGroup(0, this.renderBindGroup);
        // Bind group 1 (atlas) skipped for MVP
        renderPass.draw(4, this.memories.length);
        renderPass.end();

        // Pass 3: Picking
        const pickingPass = encoder.beginRenderPass({
            colorAttachments: [{
                view: this.pickingTexture.createView(),
                clearValue: { r: 0, g: 0, b: 0, a: 0 },
                loadOp: 'clear',
                storeOp: 'store'
            }],
            depthStencilAttachment: {
                view: this.depthTexture.createView(),
                loadOp: 'load',
                storeOp: 'store',
            }
        });
        pickingPass.setPipeline(this.pickingPipeline);
        pickingPass.setBindGroup(0, this.renderBindGroup);
        pickingPass.draw(4, this.memories.length);
        pickingPass.end();

        this.device.queue.submit([encoder.finish()]);
        
        requestAnimationFrame((t) => this._render(t));
    }
}
