/**
 * Agent SPIR-V Binary Generator
 *
 * Run this script to generate .spv files for all 7 Area Agents.
 * Usage: node generate-agents.js
 *
 * Output: compositor.spv, shell.spv, cognitive.spv, memory.spv, io.spv, scheduler.spv, network.spv
 */

import { writeFileSync, mkdirSync, existsSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

// Agent IDs and their shared memory addresses
const AGENTS = {
    COMPOSITOR: { id: 0, name: 'Compositor', heartbeat: 0, status: 10 },
    SHELL: { id: 1, name: 'Shell', heartbeat: 1, status: 11 },
    COGNITIVE: { id: 2, name: 'Cognitive', heartbeat: 2, status: 12 },
    MEMORY: { id: 3, name: 'Memory', heartbeat: 3, status: 13 },
    IO: { id: 4, name: 'I/O', heartbeat: 4, status: 14 },
    SCHEDULER: { id: 5, name: 'Scheduler', heartbeat: 5, status: 15 },
    NETWORK: { id: 6, name: 'Network', heartbeat: 6, status: 16 }
};

// Opcodes (matching kernel.wgsl)
const OP = {
    CONSTANT: 43,
    FADD: 129,
    FSUB: 131,
    FMUL: 133,
    FDIV: 135,
    STORE: 62,
    LOAD: 61,
    SHARED_STORE: 206,
    SHARED_LOAD: 207,
    RETURN: 253,
    YIELD: 228,
    JMP: 202,
    JZ: 203,
    JNZ: 200,
    LABEL: 248
};

// Process states
const PROC_STATE = {
    IDLE: 0,
    RUNNING: 1,
    WAITING: 2,
    DONE: 3,
    ERROR: 4
};

/**
 * Helper to create a SPIR-V instruction word
 */
function instr(opcode, count) {
    return (count << 16) | opcode;
}

/**
 * Convert float to uint32 representation
 */
function floatToWord(f) {
    const buf = new ArrayBuffer(4);
    const view = new DataView(buf);
    view.setFloat32(0, f, true);
    return view.getUint32(0, true);
}

/**
 * Base agent program generator
 */
class AgentGenerator {
    constructor() {
        this.words = [];
        this.idBound = 10;
    }

    nextId() {
        return this.idBound++;
    }

    emit(word) {
        this.words.push(word);
    }

    pushConstant(value) {
        const resultId = this.nextId();
        this.emit(instr(OP.CONSTANT, 4));
        this.emit(1);
        this.emit(resultId);
        this.emit(typeof value === 'number' && !Number.isInteger(value)
            ? floatToWord(value) : value);
        return resultId;
    }

    sharedLoad(address) {
        this.emit(instr(OP.SHARED_LOAD, 2));
        this.emit(address);
    }

    sharedStore(address) {
        this.emit(instr(OP.SHARED_STORE, 2));
        this.emit(address);
    }

    localStore(address) {
        this.emit(instr(OP.STORE, 3));
        this.emit(address);
        this.emit(this.nextId());
    }

    localLoad(address) {
        const resultId = this.nextId();
        this.emit(instr(OP.LOAD, 4));
        this.emit(1);
        this.emit(resultId);
        this.emit(address);
        return resultId;
    }

    fadd() {
        this.emit(instr(OP.FADD, 5));
        this.emit(1);
        this.emit(this.nextId());
        this.emit(this.nextId());
        this.emit(this.nextId());
    }

    fsub() {
        this.emit(instr(OP.FSUB, 5));
        this.emit(1);
        this.emit(this.nextId());
        this.emit(this.nextId());
        this.emit(this.nextId());
    }

    yield() {
        this.emit(instr(OP.YIELD, 1));
    }

    jmp(targetPc) {
        this.emit(instr(OP.JMP, 2));
        this.emit(targetPc);
    }

    exit() {
        this.emit(instr(OP.RETURN, 1));
    }

    header() {
        return [
            0x07230203,  // Magic
            0x00010000,  // Version 1.0
            0,           // Generator (0 = unknown)
            this.idBound,
            0            // Schema
        ];
    }

    build() {
        const header = this.header();
        const full = new Uint32Array(header.length + this.words.length);
        full.set(header);
        full.set(this.words, header.length);
        return Buffer.from(full.buffer);
    }
}

/**
 * Generate Compositor Agent
 * Manages visual composition and rendering tasks
 */
function generateCompositorAgent() {
    const gen = new AgentGenerator();
    const agent = AGENTS.COMPOSITOR;
    const loopStart = 5;

    // Increment heartbeat
    gen.sharedLoad(agent.heartbeat);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(agent.heartbeat);

    // Set status to RUNNING
    gen.pushConstant(PROC_STATE.RUNNING);
    gen.sharedStore(agent.status);

    // Do compositor work: increment frame counter at shared data 50
    gen.sharedLoad(50);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(50);

    // Yield and loop
    gen.yield();
    gen.jmp(loopStart);

    return gen.build();
}

/**
 * Generate Shell Agent
 * Handles command interpretation and UI
 */
function generateShellAgent() {
    const gen = new AgentGenerator();
    const agent = AGENTS.SHELL;
    const loopStart = 5;

    // Increment heartbeat
    gen.sharedLoad(agent.heartbeat);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(agent.heartbeat);

    // Set status
    gen.pushConstant(PROC_STATE.RUNNING);
    gen.sharedStore(agent.status);

    // Check for pending command at address 20, clear it
    gen.sharedLoad(20);
    gen.pushConstant(floatToWord(0.0));
    gen.sharedStore(20);

    // Yield and loop
    gen.yield();
    gen.jmp(loopStart);

    return gen.build();
}

/**
 * Generate Cognitive Agent
 * AI/LLM integration and inference
 */
function generateCognitiveAgent() {
    const gen = new AgentGenerator();
    const agent = AGENTS.COGNITIVE;
    const loopStart = 5;

    // Increment heartbeat
    gen.sharedLoad(agent.heartbeat);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(agent.heartbeat);

    // Set status
    gen.pushConstant(PROC_STATE.RUNNING);
    gen.sharedStore(agent.status);

    // Process cognitive task at address 51
    gen.sharedLoad(51);
    gen.pushConstant(floatToWord(0.1));  // Small increment for "thinking"
    gen.fadd();
    gen.sharedStore(51);

    // Yield and loop
    gen.yield();
    gen.jmp(loopStart);

    return gen.build();
}

/**
 * Generate Memory Agent
 * Memory management and garbage collection
 */
function generateMemoryAgent() {
    const gen = new AgentGenerator();
    const agent = AGENTS.MEMORY;
    const loopStart = 5;

    // Increment heartbeat
    gen.sharedLoad(agent.heartbeat);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(agent.heartbeat);

    // Set status
    gen.pushConstant(PROC_STATE.RUNNING);
    gen.sharedStore(agent.status);

    // Update memory stats at address 52
    gen.sharedLoad(52);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(52);

    // Yield and loop
    gen.yield();
    gen.jmp(loopStart);

    return gen.build();
}

/**
 * Generate I/O Agent
 * Input/output handling
 */
function generateIOAgent() {
    const gen = new AgentGenerator();
    const agent = AGENTS.IO;
    const loopStart = 5;

    // Increment heartbeat
    gen.sharedLoad(agent.heartbeat);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(agent.heartbeat);

    // Set status
    gen.pushConstant(PROC_STATE.RUNNING);
    gen.sharedStore(agent.status);

    // Process I/O queue at address 53
    gen.sharedLoad(53);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(53);

    // Yield and loop
    gen.yield();
    gen.jmp(loopStart);

    return gen.build();
}

/**
 * Generate Scheduler Agent
 * Process coordination and load balancing
 */
function generateSchedulerAgent() {
    const gen = new AgentGenerator();
    const agent = AGENTS.SCHEDULER;
    const loopStart = 5;

    // Increment heartbeat
    gen.sharedLoad(agent.heartbeat);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(agent.heartbeat);

    // Set status
    gen.pushConstant(PROC_STATE.RUNNING);
    gen.sharedStore(agent.status);

    // Read agent 0 heartbeat and store at address 54
    gen.sharedLoad(0);
    gen.sharedStore(54);

    // Yield and loop
    gen.yield();
    gen.jmp(loopStart);

    return gen.build();
}

/**
 * Generate Network Agent
 * Communication and networking
 */
function generateNetworkAgent() {
    const gen = new AgentGenerator();
    const agent = AGENTS.NETWORK;
    const loopStart = 5;

    // Increment heartbeat
    gen.sharedLoad(agent.heartbeat);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(agent.heartbeat);

    // Set status
    gen.pushConstant(PROC_STATE.RUNNING);
    gen.sharedStore(agent.status);

    // Process network queue at address 55
    gen.sharedLoad(55);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(55);

    // Yield and loop
    gen.yield();
    gen.jmp(loopStart);

    return gen.build();
}

/**
 * Main: Generate all agents
 */
function main() {
    const __dirname = dirname(fileURLToPath(import.meta.url));

    console.log('[Agent Generator] Generating 7 Area Agents...');

    const agents = [
        { name: 'compositor', generate: generateCompositorAgent },
        { name: 'shell', generate: generateShellAgent },
        { name: 'cognitive', generate: generateCognitiveAgent },
        { name: 'memory', generate: generateMemoryAgent },
        { name: 'io', generate: generateIOAgent },
        { name: 'scheduler', generate: generateSchedulerAgent },
        { name: 'network', generate: generateNetworkAgent }
    ];

    for (const { name, generate } of agents) {
        const binary = generate();
        const filename = join(__dirname, `${name}.spv`);
        writeFileSync(filename, binary);
        console.log(`  ✓ ${name}.spv (${binary.length} bytes)`);
    }

    console.log('[Agent Generator] All agents generated successfully!');
}

main();
