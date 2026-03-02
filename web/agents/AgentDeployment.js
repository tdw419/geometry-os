/**
 * Geometry OS Agent Deployment System
 *
 * Deploys the 7 Area Agents as isolated kernel processes with IPC via shared memory.
 *
 * Agent PIDs and their shared memory addresses:
 * - PID 0: Compositor (heartbeat: 0, status: 10)
 * - PID 1: Shell (heartbeat: 1, status: 11)
 * - PID 2: Cognitive (heartbeat: 2, status: 12)
 * - PID 3: Memory (heartbeat: 3, status: 13)
 * - PID 4: I/O (heartbeat: 4, status: 14)
 * - PID 5: Scheduler (heartbeat: 5, status: 15)
 * - PID 6: Network (heartbeat: 6, status: 16)
 *
 * IPC Memory Map:
 * - 0-6: Agent heartbeats (increment each cycle)
 * - 10-16: Agent status (0=idle, 1=running, 2=waiting, 3=error)
 * - 20-49: Message queue
 * - 50-99: Shared data buffer
 * - 100-1023: Reserved
 */

import { GeometryKernel } from '../GeometryKernel.js';

// Agent definitions
export const AGENTS = {
    COMPOSITOR: { id: 0, name: 'Compositor', file: 'compositor.spv', heartbeat: 0, status: 10 },
    SHELL: { id: 1, name: 'Shell', file: 'shell.spv', heartbeat: 1, status: 11 },
    COGNITIVE: { id: 2, name: 'Cognitive', file: 'cognitive.spv', heartbeat: 2, status: 12 },
    MEMORY: { id: 3, name: 'Memory', file: 'memory.spv', heartbeat: 3, status: 13 },
    IO: { id: 4, name: 'I/O', file: 'io.spv', heartbeat: 4, status: 14 },
    SCHEDULER: { id: 5, name: 'Scheduler', file: 'scheduler.spv', heartbeat: 5, status: 15 },
    NETWORK: { id: 6, name: 'Network', file: 'network.spv', heartbeat: 6, status: 16 }
};

// Process states
export const PROC_STATE = {
    IDLE: 0,
    RUNNING: 1,
    WAITING: 2,
    ERROR: 3
};

// IPC memory addresses
export const IPC = {
    HEARTBEAT_BASE: 0,
    STATUS_BASE: 10,
    MESSAGE_QUEUE: 20,
    SHARED_DATA: 50,
    MAX_SHARED: 1023
};

/**
 * Agent Deployment Manager
 * Manages the lifecycle of all 7 Area Agents in the Geometry Kernel
 */
export class AgentDeployment {
    constructor() {
        this.kernel = new GeometryKernel();
        this.agents = new Map();
        this.running = false;
        this.stepCount = 0;
        this.basePath = './agents/';  // Relative to web/ directory
    }

    /**
     * Initialize the kernel and load all agent binaries
     */
    async init() {
        console.log('[AgentDeployment] Initializing Geometry Kernel...');
        await this.kernel.init();

        console.log('[AgentDeployment] Loading agent binaries...');
        await this._loadAgents();

        console.log('[AgentDeployment] Ready. Call start() to begin execution.');
        return this;
    }

    /**
     * Load all 7 agent SPIR-V binaries
     */
    async _loadAgents() {
        const agentList = Object.values(AGENTS);

        for (const agent of agentList) {
            try {
                const response = await fetch(this.basePath + agent.file);
                if (!response.ok) {
                    throw new Error(`Failed to load ${agent.file}: ${response.status}`);
                }
                const binary = await response.arrayBuffer();
                this.agents.set(agent.id, {
                    ...agent,
                    binary,
                    loaded: true
                });
                console.log(`  ✓ Loaded ${agent.name} (${binary.byteLength} bytes)`);
            } catch (error) {
                console.error(`  ✗ Failed to load ${agent.name}:`, error.message);
                this.agents.set(agent.id, {
                    ...agent,
                    loaded: false,
                    error: error.message
                });
            }
        }
    }

    /**
     * Spawn all agents as kernel processes
     */
    async spawnAll() {
        console.log('[AgentDeployment] Spawning agents...');

        for (const [id, agent] of this.agents) {
            if (!agent.loaded) {
                console.log(`  ⊘ Skipping ${agent.name} (not loaded)`);
                continue;
            }

            try {
                // Use specific PID matching agent ID
                const pid = id;
                const memBase = 1024 + (pid * 512);  // KERNEL_MEM_BASE + offset
                const memLimit = 512;

                this.kernel.spawn(pid, agent.binary, memBase, memLimit);
                console.log(`  ✓ Spawned ${agent.name} as PID ${pid}`);
            } catch (error) {
                console.error(`  ✗ Failed to spawn ${agent.name}:`, error.message);
            }
        }
    }

    /**
     * Start execution loop
     * @param {number} stepsPerSecond - Target steps per second (default: 60)
     */
    start(stepsPerSecond = 60) {
        if (this.running) {
            console.log('[AgentDeployment] Already running');
            return;
        }

        this.running = true;
        const intervalMs = 1000 / stepsPerSecond;

        console.log(`[AgentDeployment] Starting execution at ${stepsPerSecond} steps/sec`);

        this._executionLoop();
    }

    /**
     * Internal execution loop
     */
    async _executionLoop() {
        if (!this.running) return;

        try {
            await this.kernel.step();
            this.stepCount++;

            // Log status every 60 steps
            if (this.stepCount % 60 === 0) {
                this._logStatus();
            }
        } catch (error) {
            console.error('[AgentDeployment] Execution error:', error);
        }

        // Schedule next step
        if (this.running) {
            requestAnimationFrame(() => this._executionLoop());
        }
    }

    /**
     * Stop execution
     */
    stop() {
        console.log('[AgentDeployment] Stopping execution...');
        this.running = false;
    }

    /**
     * Log current agent status
     */
    async _logStatus() {
        const pcbs = await this.kernel.readPCBs();
        if (pcbs.length === 0) return;

        console.log(`[Step ${this.stepCount}] Agent Status:`);
        for (const pcb of pcbs) {
            const agent = Object.values(AGENTS).find(a => a.id === pcb.pid);
            if (agent) {
                console.log(`  ${agent.name}: PC=${pcb.pc}, State=${pcb.state}`);
            }
        }
    }

    /**
     * Get current status of all agents
     * @returns {Promise<Object>} Status object with all agent states
     */
    async getStatus() {
        const pcbs = await this.kernel.readPCBs();
        const status = {
            stepCount: this.stepCount,
            running: this.running,
            agents: {}
        };

        for (const agent of Object.values(AGENTS)) {
            const pcb = pcbs.find(p => p.pid === agent.id);
            status.agents[agent.name.toLowerCase()] = {
                pid: agent.id,
                heartbeat: agent.heartbeat,
                status: pcb ? pcb.state : PROC_STATE.IDLE,
                pc: pcb ? pcb.pc : 0
            };
        }

        return status;
    }

    /**
     * Send a message to an agent via the message queue
     * @param {number} targetAgentId - Target agent ID (0-6)
     * @param {number} message - Message value to send
     */
    sendMessage(targetAgentId, message) {
        // Message queue starts at address 20
        // Each agent has a slot: agent 0 = 20, agent 1 = 21, etc.
        const address = IPC.MESSAGE_QUEUE + targetAgentId;
        console.log(`[AgentDeployment] Sending message to agent ${targetAgentId} at address ${address}`);
        // Note: Actual message writing would require RAM buffer access
        // This is a placeholder for the IPC interface
    }

    /**
     * Read shared memory region (for debugging/monitoring)
     * @returns {Promise<Float32Array>} Shared memory contents
     */
    async readSharedMemory() {
        // This would require implementing a read method in GeometryKernel
        // For now, return placeholder
        console.log('[AgentDeployment] Shared memory read not yet implemented');
        return new Float32Array(1024);
    }
}

/**
 * Quick deployment function for one-line setup
 */
export async function deployAgents() {
    const deployment = new AgentDeployment();
    await deployment.init();
    await deployment.spawnAll();
    return deployment;
}

// Auto-initialize if loaded as main module
if (typeof window !== 'undefined') {
    window.AgentDeployment = AgentDeployment;
    window.deployAgents = deployAgents;
    window.AGENTS = AGENTS;
    console.log('[AgentDeployment] Module loaded. Use deployAgents() to start.');
}
