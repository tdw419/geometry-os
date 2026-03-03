/**
 * Geometry OS Agent Manager
 *
 * Manages autonomous agents (.loop, .bridge) as visible 3D avatars
 * in the AGENTS sector (65536-81919). Implements "Glass Box" visibility
 * where agent actions are physically animated in the substrate.
 *
 * Agent Avatars:
 * - Represented by morphological glyphs (geometric signatures)
 * - "Eyes" (raycast beams) show read/write targets
 * - Tethers show message passing between agents
 * - Movement traces path through Hilbert space
 */

// Agent types and their visual properties
const AGENT_TYPES = {
    LOOP_ORCHESTRATOR: {
        id: 0,
        name: 'Loop Orchestrator',
        color: '#ff00ff',
        glyph: '◇',  // Diamond
        sector: 'AGENTS'
    },
    BRIDGE_PLANNER: {
        id: 1,
        name: 'Bridge Planner (Gemini)',
        color: '#4285f4',
        glyph: '△',  // Triangle
        sector: 'AGENTS'
    },
    BRIDGE_WORKER: {
        id: 2,
        name: 'Bridge Worker (Claude)',
        color: '#00ff88',
        glyph: '□',  // Square
        sector: 'AGENTS'
    },
    MEMORY_AGENT: {
        id: 3,
        name: 'Memory Agent',
        color: '#ffaa00',
        glyph: '○',  // Circle
        sector: 'AGENTS'
    },
    SCHEDULER: {
        id: 4,
        name: 'Scheduler',
        color: '#00ffff',
        glyph: '☆',  // Star
        sector: 'KERNEL'
    }
};

// Action types for saccade visualization
const ACTION_TYPES = {
    IDLE: 0,
    READ: 1,
    WRITE: 2,
    COMPUTE: 3,
    WAIT: 4,
    HANDOFF: 5
};

/**
 * Represents a single agent avatar in 3D space
 */
class AgentAvatar {
    constructor(options) {
        this.id = options.id || `agent-${Date.now()}`;
        this.type = options.type || AGENT_TYPES.LOOP_ORCHESTRATOR;
        this.name = options.name || this.type.name;

        // Position in 3D space (mapped to AGENTS sector)
        this.position = options.position || { x: 0, y: 0, z: 0 };
        this.targetPosition = { ...this.position };
        this.velocity = { x: 0, y: 0, z: 0 };

        // Visual state
        this.color = this.type.color;
        this.glyph = this.type.glyph;
        this.scale = 1.0;
        this.pulse = 0;

        // Action state
        this.action = ACTION_TYPES.IDLE;
        this.actionTarget = null;  // What the agent is looking at
        this.actionProgress = 0;   // 0-1 for animation

        // Eye (raycast beam)
        this.eyeTarget = { ...this.position };
        this.eyeActive = false;

        // Handoff tether
        this.tetherTarget = null;
        this.tetherProgress = 0;

        // Stats
        this.cycles = 0;
        this.reads = 0;
        this.writes = 0;
        this.lastAction = Date.now();

        // Trail (path through space)
        this.trail = [];
        this.maxTrailLength = 50;
    }

    /**
     * Set the agent's current action
     */
    setAction(action, target = null) {
        this.action = action;
        this.actionTarget = target;
        this.actionProgress = 0;
        this.lastAction = Date.now();

        if (action === ACTION_TYPES.READ) {
            this.reads++;
            if (target) this.eyeTarget = target;
            this.eyeActive = true;
        } else if (action === ACTION_TYPES.WRITE) {
            this.writes++;
            if (target) this.eyeTarget = target;
            this.eyeActive = true;
        } else {
            this.eyeActive = false;
        }

        this.cycles++;
    }

    /**
     * Start a handoff to another agent
     */
    startHandoff(targetAgent) {
        this.tetherTarget = targetAgent;
        this.tetherProgress = 0;
        this.setAction(ACTION_TYPES.HANDOFF, targetAgent.position);
    }

    /**
     * Complete the handoff
     */
    completeHandoff() {
        this.tetherTarget = null;
        this.tetherProgress = 0;
        this.setAction(ACTION_TYPES.IDLE);
    }

    /**
     * Move toward target position
     */
    moveTo(target, speed = 0.1) {
        this.targetPosition = { ...target };

        // Add to trail
        this.trail.push({ ...this.position });
        if (this.trail.length > this.maxTrailLength) {
            this.trail.shift();
        }
    }

    /**
     * Update physics and animation
     */
    update(dt) {
        // Smooth movement toward target
        const dx = this.targetPosition.x - this.position.x;
        const dy = this.targetPosition.y - this.position.y;
        const dz = this.targetPosition.z - this.position.z;

        const speed = 0.05;
        this.position.x += dx * speed;
        this.position.y += dy * speed;
        this.position.z += dz * speed;

        // Pulse animation
        this.pulse = (Math.sin(Date.now() * 0.005) + 1) * 0.5;

        // Action progress
        if (this.actionProgress < 1) {
            this.actionProgress += 0.02;
        }

        // Tether progress
        if (this.tetherTarget) {
            this.tetherProgress += 0.03;
            if (this.tetherProgress >= 1) {
                this.completeHandoff();
            }
        }

        // Eye smooth follow
        if (this.eyeActive && this.actionTarget) {
            this.eyeTarget.x += (this.actionTarget.x - this.eyeTarget.x) * 0.1;
            this.eyeTarget.y += (this.actionTarget.y - this.eyeTarget.y) * 0.1;
        }
    }
}

/**
 * AgentManager - Orchestrates all agent avatars
 */
export class AgentManager {
    constructor(os) {
        this.os = os;
        this.device = os.device;

        // Agent registry
        this.agents = new Map();
        this.activeAgents = [];

        // Sector bounds
        this.sectorBase = 65536;
        this.sectorSize = 16384;

        // Event log for IPC visualization
        this.events = [];
        this.maxEvents = 100;

        // GPU resources
        this.agentBuffer = null;
        this.eventBuffer = null;
        this.pipeline = null;
    }

    /**
     * Initialize the agent manager
     */
    async init() {
        // Create default agents
        this._createDefaultAgents();

        // Initialize GPU resources
        await this._initGPU();

        // Subscribe to OS events
        this._wireEvents();

        console.log('[AgentManager] Agent avatars initialized');
        return this;
    }

    _createDefaultAgents() {
        // Loop Orchestrator
        const loop = new AgentAvatar({
            id: 'loop-orchestrator',
            type: AGENT_TYPES.LOOP_ORCHESTRATOR,
            position: this._sectorToPosition(0)
        });
        this.agents.set(loop.id, loop);

        // Bridge Planner (Gemini)
        const planner = new AgentAvatar({
            id: 'bridge-planner',
            type: AGENT_TYPES.BRIDGE_PLANNER,
            position: this._sectorToPosition(1)
        });
        this.agents.set(planner.id, planner);

        // Bridge Worker (Claude)
        const worker = new AgentAvatar({
            id: 'bridge-worker',
            type: AGENT_TYPES.BRIDGE_WORKER,
            position: this._sectorToPosition(2)
        });
        this.agents.set(worker.id, worker);

        // Memory Agent
        const memory = new AgentAvatar({
            id: 'memory-agent',
            type: AGENT_TYPES.MEMORY_AGENT,
            position: this._sectorToPosition(3)
        });
        this.agents.set(memory.id, memory);

        this.activeAgents = Array.from(this.agents.values());
    }

    /**
     * Convert sector index to 3D position
     */
    _sectorToPosition(index) {
        // Map agent index to position in AGENTS sector
        // Using a simple grid layout within the sector
        const gridSize = Math.sqrt(this.sectorSize);
        const x = (index % gridSize) * 10 + 700;  // Offset to AGENTS sector
        const y = Math.floor(index / gridSize) * 10 + 300;
        return { x, y, z: 0 };
    }

    async _initGPU() {
        // Agent data buffer
        this.agentBuffer = this.device.createBuffer({
            size: 16 * 64,  // 16 floats per agent, 64 agents max
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        });

        // Event/tether buffer
        this.eventBuffer = this.device.createBuffer({
            size: 16 * 32,  // 16 floats per event, 32 events
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        });

        // Create shader for agent rendering
        const shaderCode = `
struct AgentData {
    pos: vec3f,
    color: vec3f,
    scale: f32,
    action: f32,
    eyeTarget: vec3f,
    eyeActive: f32,
    pulse: f32,
    padding: vec2f,
}

@group(0) @binding(0) var<storage, read> agents: array<AgentData>;
@group(0) @binding(1) var<storage, read> events: array<vec4f>;  // tethers

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
    @location(1) agentId: f32,
    @location(2) isEye: f32,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VertexOutput {
    var out: VertexOutput;

    let agent = agents[ii];

    // Agent body (quad)
    let quad = array<vec2f, 6>(
        vec2f(-0.5, -0.5), vec2f(0.5, -0.5), vec2f(-0.5, 0.5),
        vec2f(0.5, -0.5), vec2f(0.5, 0.5), vec2f(-0.5, 0.5)
    );

    let vertex = quad[vi];
    let scale = agent.scale * (1.0 + agent.pulse * 0.2);
    let worldPos = vec3f(
        agent.pos.x + vertex.x * 20.0 * scale,
        agent.pos.y + vertex.y * 20.0 * scale,
        agent.pos.z
    );

    // Simple projection
    let camX = 600.0;
    let camY = 400.0;
    let zoom = 0.01;

    out.pos = vec4f(
        (worldPos.x - camX) * zoom,
        (worldPos.y - camY) * zoom,
        worldPos.z * 0.01,
        1.0
    );

    out.uv = vertex + 0.5;
    out.agentId = f32(ii);
    out.isEye = 0.0;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let agent = agents[u32(in.agentId)];

    // Border glow
    let borderDist = min(
        min(in.uv.x, 1.0 - in.uv.x),
        min(in.uv.y, 1.0 - in.uv.y)
    );

    let glow = smoothstep(0.05, 0.0, borderDist);
    let bgColor = agent.color * (0.3 + in.uv.y * 0.2);

    // Action indicator
    var actionColor = vec3f(0.5, 0.5, 0.5);
    if (agent.action == 1.0) { actionColor = vec3f(0.0, 1.0, 0.0); }      // READ
    else if (agent.action == 2.0) { actionColor = vec3f(1.0, 0.5, 0.0); } // WRITE
    else if (agent.action == 3.0) { actionColor = vec3f(0.0, 0.5, 1.0); } // COMPUTE
    else if (agent.action == 5.0) { actionColor = vec3f(1.0, 0.0, 1.0); } // HANDOFF

    let actionGlow = smoothstep(0.4, 0.5, max(abs(in.uv.x - 0.5), abs(in.uv.y - 0.5)));

    let color = bgColor + agent.color * glow + actionColor * actionGlow * 0.5;

    return vec4f(color, 0.9);
}
`;

        this.shader = this.device.createShaderModule({ code: shaderCode });

        // Pipeline
        this.pipeline = this.device.createRenderPipeline({
            layout: 'auto',
            vertex: { module: this.shader, entryPoint: 'vs_main' },
            fragment: {
                module: this.shader,
                entryPoint: 'fs_main',
                targets: [{
                    format: this.os.format,
                    blend: {
                        color: { srcFactor: 'src-alpha', dstFactor: 'one-minus-src-alpha' },
                        alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha' }
                    }
                }]
            },
            primitive: { topology: 'triangle-list' }
        });

        this.bindGroup = this.device.createBindGroup({
            layout: this.pipeline.getBindGroupLayout(0),
            entries: [
                { binding: 0, resource: { buffer: this.agentBuffer } },
                { binding: 1, resource: { buffer: this.eventBuffer } }
            ]
        });
    }

    _wireEvents() {
        // Listen for IPC events from OS
        this.os.on('ipc', (event) => this._handleIPCEvent(event));
        this.os.on('drop', (event) => this._handleDropEvent(event));

        // Register agents with OS
        for (const agent of this.agents.values()) {
            this.os.registerAgent({
                id: agent.id,
                name: agent.name,
                type: agent.type.id
            });
        }
    }

    _handleIPCEvent(event) {
        this.events.push({
            ...event,
            timestamp: Date.now()
        });

        if (this.events.length > this.maxEvents) {
            this.events.shift();
        }

        // Trigger agent animation based on event
        if (event.type === 'spawn') {
            // Loop orchestrator initiated spawn
            const loopAgent = this.agents.get('loop-orchestrator');
            if (loopAgent) {
                loopAgent.setAction(ACTION_TYPES.WRITE, { x: 200, y: 300, z: 0 });
            }
        }
    }

    _handleDropEvent(event) {
        // Handoff animation between agents
        const planner = this.agents.get('bridge-planner');
        const worker = this.agents.get('bridge-worker');

        if (planner && worker && event.from === 'files') {
            planner.startHandoff(worker);
        }
    }

    /**
     * Register a new agent
     */
    registerAgent(options) {
        const agent = new AgentAvatar(options);
        this.agents.set(agent.id, agent);
        this.activeAgents = Array.from(this.agents.values());
        return agent;
    }

    /**
     * Remove an agent
     */
    unregisterAgent(agentId) {
        this.agents.delete(agentId);
        this.activeAgents = Array.from(this.agents.values());
    }

    /**
     * Update all agents
     */
    update(dt) {
        for (const agent of this.agents.values()) {
            agent.update(dt);
        }

        this._updateGPUBuffers();
    }

    _updateGPUBuffers() {
        // Update agent data
        const agentData = new Float32Array(16 * 64);
        let i = 0;

        for (const agent of this.activeAgents) {
            // Position (3) + color (3) + scale (1) + action (1)
            agentData[i++] = agent.position.x;
            agentData[i++] = agent.position.y;
            agentData[i++] = agent.position.z;

            // Parse color
            const color = this._parseColor(agent.color);
            agentData[i++] = color.r;
            agentData[i++] = color.g;
            agentData[i++] = color.b;

            agentData[i++] = agent.scale;
            agentData[i++] = agent.action;

            // Eye target
            agentData[i++] = agent.eyeTarget.x;
            agentData[i++] = agent.eyeTarget.y;
            agentData[i++] = agent.eyeTarget.z;
            agentData[i++] = agent.eyeActive ? 1.0 : 0.0;

            // Pulse
            agentData[i++] = agent.pulse;
            agentData[i++] = 0;  // padding
            agentData[i++] = 0;
            agentData[i++] = 0;
        }

        this.device.queue.writeBuffer(this.agentBuffer, 0, agentData);
    }

    _parseColor(hex) {
        const r = parseInt(hex.slice(1, 3), 16) / 255;
        const g = parseInt(hex.slice(3, 5), 16) / 255;
        const b = parseInt(hex.slice(5, 7), 16) / 255;
        return { r, g, b };
    }

    /**
     * Render agents to command encoder
     */
    render(encoder, renderPass) {
        if (this.activeAgents.length === 0) return;

        renderPass.setPipeline(this.pipeline);
        renderPass.setBindGroup(0, this.bindGroup);
        renderPass.draw(6, this.activeAgents.length);
    }

    /**
     * Simulate agent activity (for demo/testing)
     */
    simulateActivity() {
        const loop = this.agents.get('loop-orchestrator');
        const planner = this.agents.get('bridge-planner');
        const worker = this.agents.get('bridge-worker');
        const memory = this.agents.get('memory-agent');

        // Random actions for demo
        const actions = [ACTION_TYPES.READ, ACTION_TYPES.WRITE, ACTION_TYPES.COMPUTE, ACTION_TYPES.IDLE];
        const targets = [
            { x: 200, y: 300, z: 0 },   // Shell
            { x: 600, y: 300, z: 0 },   // Files
            { x: 1000, y: 300, z: 0 },  // Memory
            { x: 600, y: 600, z: 0 }    // IPC
        ];

        // Simulate loop orchestrator
        if (Math.random() < 0.02) {
            loop.setAction(
                actions[Math.floor(Math.random() * actions.length)],
                targets[Math.floor(Math.random() * targets.length)]
            );
        }

        // Simulate planner -> worker handoff
        if (Math.random() < 0.005) {
            planner.startHandoff(worker);
        }

        // Simulate memory agent reading
        if (Math.random() < 0.03) {
            memory.setAction(ACTION_TYPES.READ, targets[2]);
        }
    }

    /**
     * Get agent statistics
     */
    getStats() {
        const stats = {
            agentCount: this.agents.size,
            totalCycles: 0,
            totalReads: 0,
            totalWrites: 0,
            activeActions: 0,
            handoffs: 0
        };

        for (const agent of this.agents.values()) {
            stats.totalCycles += agent.cycles;
            stats.totalReads += agent.reads;
            stats.totalWrites += agent.writes;
            if (agent.action !== ACTION_TYPES.IDLE) stats.activeActions++;
            if (agent.action === ACTION_TYPES.HANDOFF) stats.handoffs++;
        }

        return stats;
    }
}

// Export types
export { AGENT_TYPES, ACTION_TYPES, AgentAvatar };
