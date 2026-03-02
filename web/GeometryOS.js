/**
 * Geometry OS - Unified Desktop Substrate
 *
 * Root controller that initializes a single GeometryKernel and WebGPU device,
 * sharing them across all subsystems (Shell, FileManager, MemoryBrowser).
 *
 * The "Boot Loader" for the entire spatial computing environment.
 */

import { GeometryKernel } from './GeometryKernel.js';
import { GeometryFont } from './GeometryFont.js';
import { AgentManager } from './AgentManager.js';

// Hilbert Sector Map - Different subsystems occupy dedicated regions
const HILBERT_SECTORS = {
    KERNEL: { base: 0, size: 1024 },        // 0-1023: Kernel/PCBs
    MAILBOX: { base: 1024, size: 512 },     // 1024-1535: IPC Mailboxes
    SHELL: { base: 2048, size: 4096 },      // 2048-6143: Shell processes
    FILES: { base: 8192, size: 16384 },     // 8192-24575: File system
    MEMORY: { base: 32768, size: 32768 },   // 32768-65535: Memory substrate
    AGENTS: { base: 65536, size: 16384 },   // 65536-81919: Agent space
    USER: { base: 100000, size: 100000 }    // User space
};

// Window z-order and interaction states
const WINDOW_STATES = {
    NORMAL: 0,
    MINIMIZED: 1,
    MAXIMIZED: 2,
    DRAGGING: 3,
    RESIZING: 4
};

/**
 * Represents a 3D window in the unified desktop
 */
class DesktopWindow {
    constructor(options) {
        this.id = options.id || `window-${Date.now()}`;
        this.title = options.title || 'Untitled';
        this.type = options.type || 'generic';

        // 3D position and size
        this.position = options.position || { x: 0, y: 0, z: 0 };
        this.size = options.size || { width: 400, height: 300 };
        this.rotation = options.rotation || { x: 0, y: 0, z: 0 };

        // State
        this.state = WINDOW_STATES.NORMAL;
        this.zIndex = options.zIndex || 0;
        this.visible = true;
        this.focused = false;

        // Content
        this.content = options.content || null;
        this.manager = options.manager || null;

        // Interaction
        this.draggable = options.draggable !== false;
        this.resizable = options.resizable !== false;
    }

    containsPoint(x, y) {
        const hw = this.size.width / 2;
        const hh = this.size.height / 2;
        return x >= this.position.x - hw && x <= this.position.x + hw &&
               y >= this.position.y - hh && y <= this.position.y + hh;
    }
}

/**
 * GeometryOS - The unified root controller
 */
export class GeometryOS {
    constructor(canvas) {
        this.canvas = canvas;

        // Core systems
        this.device = null;
        this.context = null;
        this.format = null;
        this.kernel = null;
        this.font = null;

        // Desktop state
        this.windows = new Map();
        this.activeWindow = null;
        this.windowZCounter = 0;

        // Hilbert landscape
        this.hilbertSize = 128;  // 128x128 grid
        this.cameraPosition = { x: 64, y: 64, z: 50 };
        this.cameraTarget = { x: 64, y: 64, z: 0 };

        // Interaction state
        this.dragging = null;
        this.dragOffset = { x: 0, y: 0 };

        // Agents
        this.agents = [];
        this.agentPositions = new Map();
        this.agentManager = null;

        // Stats
        this.frameCount = 0;
        this.lastFrameTime = 0;
        this.fps = 0;

        // IPC event log
        this.ipcEvents = [];
    }

    /**
     * Initialize the unified desktop environment
     */
    async init() {
        console.log('[GeometryOS] Igniting Unified Desktop Substrate...');

        // 1. Initialize WebGPU
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

        // 2. Initialize shared kernel
        this.kernel = new GeometryKernel();
        this.kernel.device = this.device;  // Share device

        // Initialize kernel buffers manually (skip fetch)
        await this._initKernelBuffers();

        // 3. Load font
        this.font = new GeometryFont();
        await this.font.load();

        // 4. Initialize rendering pipeline
        await this._initPipelines();

        // 5. Create default windows
        this._createDefaultWindows();

        // 6. Initialize Agent Manager
        this.agentManager = new AgentManager(this);
        await this.agentManager.init();

        // 7. Wire interactions
        this._wireInteractions();

        // 8. Start render loop
        requestAnimationFrame((t) => this._render(t));

        console.log('[GeometryOS] Desktop Environment ready');
        return this;
    }

    async _initKernelBuffers() {
        // Create kernel buffers using shared device
        this.kernel.programBuffer = this.device.createBuffer({
            size: 65536 * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        });

        this.kernel.stackBuffer = this.device.createBuffer({
            size: 1024 * 16 * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        });

        this.kernel.ramBuffer = this.device.createBuffer({
            size: 256 * 1024 * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        });

        this.kernel.pcbBuffer = this.device.createBuffer({
            size: 16 * 16 * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        });

        this.kernel.labelsBuffer = this.device.createBuffer({
            size: 1024 * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        });

        this.kernel.resultBuffer = this.device.createBuffer({
            size: 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
        });

        // Load kernel shader
        const response = await fetch('kernel.wgsl');
        const code = await response.text();

        this.kernel.pipeline = this.device.createComputePipeline({
            layout: 'auto',
            compute: {
                module: this.device.createShaderModule({ code }),
                entryPoint: 'main',
            },
        });

        console.log('[GeometryOS] Kernel buffers initialized');
    }

    async _initPipelines() {
        // Desktop rendering shader
        const shaderCode = `
struct Uniforms {
    cameraPos: vec4f,
    cameraTarget: vec4f,
    time: f32,
    windowCount: f32,
    padding: vec2f,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var<storage, read> windowData: array<vec4f>;  // position, size per window

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
    @location(1) windowId: f32,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VertexOutput {
    var out: VertexOutput;

    // Quad vertices
    let quad = array<vec2f, 6>(
        vec2f(-0.5, -0.5),
        vec2f(0.5, -0.5),
        vec2f(-0.5, 0.5),
        vec2f(0.5, -0.5),
        vec2f(0.5, 0.5),
        vec2f(-0.5, 0.5)
    );

    let vertex = quad[vi];
    let win = windowData[ii];
    let winPos = win.xy;
    let winSize = win.zw;

    // Transform to view space
    let worldPos = vec3f(
        winPos.x + vertex.x * winSize.x,
        winPos.y + vertex.y * winSize.y,
        -5.0 + f32(ii) * 0.1
    );

    // Simple ortho projection
    let scale = 0.01;
    out.pos = vec4f(
        (worldPos.x - uniforms.cameraPos.x) * scale,
        (worldPos.y - uniforms.cameraPos.y) * scale,
        worldPos.z * 0.1,
        1.0
    );

    out.uv = vertex + 0.5;
    out.windowId = f32(ii);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let windowId = u32(in.windowId);

    // Window border glow
    let borderDist = min(
        min(in.uv.x, 1.0 - in.uv.x),
        min(in.uv.y, 1.0 - in.uv.y)
    );

    let borderColor = vec3f(0.0, 1.0, 0.8);  // Cyan
    let bgColor = vec3f(0.02, 0.08, 0.08);

    let borderGlow = smoothstep(0.02, 0.0, borderDist) * borderColor;
    let bg = bgColor * (1.0 - in.uv.y * 0.3);  // Gradient

    let color = bg + borderGlow;

    return vec4f(color, 0.95);
}
`;

        this.desktopShader = this.device.createShaderModule({ code: shaderCode });

        // Uniform buffer
        this.uniformBuffer = this.device.createBuffer({
            size: 64,
            usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
        });

        // Window data buffer
        this.windowDataBuffer = this.device.createBuffer({
            size: 16 * 64,  // Up to 64 windows
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        });

        // Render pipeline
        this.desktopPipeline = this.device.createRenderPipeline({
            layout: 'auto',
            vertex: { module: this.desktopShader, entryPoint: 'vs_main' },
            fragment: {
                module: this.desktopShader,
                entryPoint: 'fs_main',
                targets: [{
                    format: this.format,
                    blend: {
                        color: { srcFactor: 'src-alpha', dstFactor: 'one-minus-src-alpha' },
                        alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha' }
                    }
                }]
            },
            primitive: { topology: 'triangle-list' }
        });

        this.desktopBindGroup = this.device.createBindGroup({
            layout: this.desktopPipeline.getBindGroupLayout(0),
            entries: [
                { binding: 0, resource: { buffer: this.uniformBuffer } },
                { binding: 1, resource: { buffer: this.windowDataBuffer } }
            ]
        });

        // Depth texture
        this.depthTexture = this.device.createTexture({
            size: [this.canvas.width, this.canvas.height],
            format: 'depth24plus',
            usage: GPUBufferUsage.RENDER_ATTACHMENT,
        });
    }

    _createDefaultWindows() {
        // Shell Window (left side)
        this.createWindow({
            id: 'shell',
            title: 'Visual Shell',
            type: 'shell',
            position: { x: 200, y: 300, z: 0 },
            size: { width: 350, height: 450 }
        });

        // File Manager Window (center-top)
        this.createWindow({
            id: 'files',
            title: 'File Manager',
            type: 'files',
            position: { x: 600, y: 300, z: 0 },
            size: { width: 400, height: 400 }
        });

        // Memory Browser Window (right side)
        this.createWindow({
            id: 'memory',
            title: 'Memory Browser',
            type: 'memory',
            position: { x: 1000, y: 300, z: 0 },
            size: { width: 350, height: 450 }
        });

        // IPC Monitor Window (bottom center)
        this.createWindow({
            id: 'ipc',
            title: 'IPC Monitor',
            type: 'ipc',
            position: { x: 600, y: 600, z: 0 },
            size: { width: 400, height: 200 }
        });
    }

    /**
     * Create a new desktop window
     */
    createWindow(options) {
        const win = new DesktopWindow({
            ...options,
            zIndex: this.windowZCounter++
        });

        this.windows.set(win.id, win);
        this.activeWindow = win;

        console.log(`[GeometryOS] Created window: ${win.title}`);
        return win;
    }

    /**
     * Focus a window (bring to front)
     */
    focusWindow(windowId) {
        const win = this.windows.get(windowId);
        if (!win) return;

        // Unfocus all
        for (const w of this.windows.values()) {
            w.focused = false;
        }

        // Focus target
        win.focused = true;
        win.zIndex = this.windowZCounter++;
        this.activeWindow = win;
    }

    /**
     * Remove a window
     */
    closeWindow(windowId) {
        this.windows.delete(windowId);
        if (this.activeWindow?.id === windowId) {
            this.activeWindow = this.windows.values().next().value || null;
        }
    }

    /**
     * Spawn a process from a file (drag-drop from FileManager to Shell)
     */
    async spawnFromFile(filePath, spirvBinary) {
        const pid = await this.kernel.spawnProcess(spirvBinary, filePath);

        // Record IPC event
        this.ipcEvents.push({
            type: 'spawn',
            from: 'files',
            to: 'shell',
            data: { pid, path: filePath },
            timestamp: Date.now()
        });

        console.log(`[GeometryOS] Spawned process ${pid} from ${filePath}`);
        return pid;
    }

    /**
     * Register an agent for visualization
     */
    registerAgent(agent) {
        this.agents.push(agent);
        this.agentPositions.set(agent.id, {
            x: Math.random() * 200 + 100,
            y: Math.random() * 200 + 100,
            z: 0
        });

        console.log(`[GeometryOS] Registered agent: ${agent.name || agent.id}`);
    }

    _wireInteractions() {
        // Mouse interactions
        this.canvas.addEventListener('mousedown', (e) => this._onMouseDown(e));
        this.canvas.addEventListener('mousemove', (e) => this._onMouseMove(e));
        this.canvas.addEventListener('mouseup', (e) => this._onMouseUp(e));
        this.canvas.addEventListener('wheel', (e) => this._onWheel(e));

        // Keyboard
        window.addEventListener('keydown', (e) => this._onKeyDown(e));
    }

    _onMouseDown(e) {
        const rect = this.canvas.getBoundingClientRect();
        const x = ((e.clientX - rect.left) / rect.width - 0.5) * 100 + this.cameraPosition.x;
        const y = ((e.clientY - rect.top) / rect.height - 0.5) * 100 + this.cameraPosition.y;

        // Find clicked window (reverse z-order)
        const sortedWindows = Array.from(this.windows.values())
            .sort((a, b) => b.zIndex - a.zIndex);

        for (const win of sortedWindows) {
            if (win.visible && win.containsPoint(x, y)) {
                this.focusWindow(win.id);

                if (win.draggable) {
                    this.dragging = win;
                    this.dragOffset = {
                        x: x - win.position.x,
                        y: y - win.position.y
                    };
                }
                break;
            }
        }
    }

    _onMouseMove(e) {
        if (!this.dragging) return;

        const rect = this.canvas.getBoundingClientRect();
        const x = ((e.clientX - rect.left) / rect.width - 0.5) * 100 + this.cameraPosition.x;
        const y = ((e.clientY - rect.top) / rect.height - 0.5) * 100 + this.cameraPosition.y;

        this.dragging.position.x = x - this.dragOffset.x;
        this.dragging.position.y = y - this.dragOffset.y;
    }

    _onMouseUp(e) {
        // Check for drop target
        if (this.dragging) {
            const dropTarget = this._findDropTarget(this.dragging);
            if (dropTarget && dropTarget !== this.dragging) {
                this._handleDrop(this.dragging, dropTarget);
            }
        }

        this.dragging = null;
    }

    _findDropTarget(source) {
        for (const win of this.windows.values()) {
            if (win.id === source.id) continue;
            if (win.containsPoint(source.position.x, source.position.y)) {
                return win;
            }
        }
        return null;
    }

    _handleDrop(source, target) {
        console.log(`[GeometryOS] Drop: ${source.title} -> ${target.title}`);

        // Emit drop event for managers to handle
        this.emit('drop', { source, target });

        // Record IPC event
        this.ipcEvents.push({
            type: 'drop',
            from: source.id,
            to: target.id,
            timestamp: Date.now()
        });
    }

    _onWheel(e) {
        e.preventDefault();
        this.cameraPosition.z += e.deltaY * 0.05;
        this.cameraPosition.z = Math.max(10, Math.min(200, this.cameraPosition.z));
    }

    _onKeyDown(e) {
        // Global hotkeys
        if (e.key === 'Escape') {
            this.dragging = null;
        }
    }

    /**
     * Event system
     */
    on(event, callback) {
        if (!this._listeners) this._listeners = new Map();
        if (!this._listeners.has(event)) this._listeners.set(event, []);
        this._listeners.get(event).push(callback);
    }

    emit(event, data) {
        if (this._listeners?.has(event)) {
            this._listeners.get(event).forEach(cb => cb(data));
        }
    }

    _updateUniforms(time) {
        const data = new Float32Array(16);
        data[0] = this.cameraPosition.x;
        data[1] = this.cameraPosition.y;
        data[2] = this.cameraPosition.z;
        data[3] = 0;
        data[4] = this.cameraTarget.x;
        data[5] = this.cameraTarget.y;
        data[6] = this.cameraTarget.z;
        data[7] = 0;
        data[8] = time * 0.001;
        data[9] = this.windows.size;

        this.device.queue.writeBuffer(this.uniformBuffer, 0, data);
    }

    _updateWindowData() {
        const data = new Float32Array(64 * 4);
        let i = 0;

        for (const win of this.windows.values()) {
            if (!win.visible) continue;

            data[i++] = win.position.x;
            data[i++] = win.position.y;
            data[i++] = win.size.width;
            data[i++] = win.size.height;
        }

        this.device.queue.writeBuffer(this.windowDataBuffer, 0, data);
    }

    async _stepKernel() {
        if (this.kernel.processes.length === 0) return;

        await this.kernel.step();
    }

    _render(time) {
        if (!this.device) return;

        // FPS calculation
        const dt = time - this.lastFrameTime;
        this.lastFrameTime = time;
        this.fps = 1000 / dt;

        this.frameCount++;

        // Update state
        this._updateUniforms(time);
        this._updateWindowData();

        // Step kernel occasionally
        if (this.frameCount % 3 === 0) {
            this._stepKernel();
        }

        // Update agents
        if (this.agentManager) {
            this.agentManager.update(dt);

            // Simulate activity for demo
            if (this.frameCount % 60 === 0) {
                this.agentManager.simulateActivity();
            }
        }

        // Render
        const encoder = this.device.createCommandEncoder();

        // Render desktop windows
        const renderPass = encoder.beginRenderPass({
            colorAttachments: [{
                view: this.context.getCurrentTexture().createView(),
                clearValue: { r: 0.02, g: 0.02, b: 0.04, a: 1.0 },
                loadOp: 'clear',
                storeOp: 'store'
            }],
            depthStencilAttachment: {
                view: this.depthTexture.createView(),
                depthClearValue: 1.0,
                depthLoadOp: 'clear',
                depthStoreOp: 'store'
            }
        });

        renderPass.setPipeline(this.desktopPipeline);
        renderPass.setBindGroup(0, this.desktopBindGroup);

        // Draw windows
        const visibleWindows = Array.from(this.windows.values()).filter(w => w.visible);
        if (visibleWindows.length > 0) {
            renderPass.draw(6, visibleWindows.length);
        }

        // Render agents
        if (this.agentManager) {
            this.agentManager.render(encoder, renderPass);
        }

        renderPass.end();

        this.device.queue.submit([encoder.finish()]);

        requestAnimationFrame((t) => this._render(t));
    }

    /**
     * Get system statistics
     */
    getStats() {
        const agentStats = this.agentManager?.getStats() || {};
        return {
            fps: this.fps,
            windowCount: this.windows.size,
            processCount: this.kernel.processes.length,
            agentCount: agentStats.agentCount || 0,
            agentCycles: agentStats.totalCycles || 0,
            agentReads: agentStats.totalReads || 0,
            agentWrites: agentStats.totalWrites || 0,
            ipcEventCount: this.ipcEvents.length,
            cameraPosition: { ...this.cameraPosition }
        };
    }
}

// Export sector constants
export { HILBERT_SECTORS, WINDOW_STATES, DesktopWindow };
