/**
 * Geometry OS Area Agent SPIR-V Generator
 *
 * Generates specialized SPIR-V programs for the 7 Area Agents.
 * Each agent runs as an isolated kernel process with IPC via shared memory.
 */

// Agent IDs and their shared memory addresses
export const AGENTS = {
    COMPOSITOR: { id: 0, name: 'Compositor', heartbeat: 0, status: 10 },
    SHELL: { id: 1, name: 'Shell', heartbeat: 1, status: 11 },
    COGNITIVE: { id: 2, name: 'Cognitive', heartbeat: 2, status: 12 },
    MEMORY: { id: 3, name: 'Memory', heartbeat: 3, status: 13 },
    IO: { id: 4, name: 'I/O', heartbeat: 4, status: 14 },
    SCHEDULER: { id: 5, name: 'Scheduler', heartbeat: 5, status: 15 },
    NETWORK: { id: 6, name: 'Network', heartbeat: 6, status: 16 }
};

// IPC Memory Map
export const IPC = {
    HEARTBEAT_BASE: 0,      // 0-6: Agent heartbeats
    STATUS_BASE: 10,        // 10-16: Agent status
    MESSAGE_QUEUE: 20,      // 20-50: Message queue
    SHARED_DATA: 50,        // 50-100: Shared data buffer
    MAX_SHARED: 1023        // Max shared memory address
};

// Syscall Opcodes
const OP_SYSCALL = 211;  // Trap to kernel for I/O

// Syscall IDs
const SYS = {
    GET_MOUSE: 1,
    GET_KEY: 2,
    WRITE_LOG: 3,
    GET_TIME: 4
};

// I/O Memory Map (extends IPC)
const IO = {
    MOUSE_X: 50,
    MOUSE_Y: 51,
    MOUSE_BTN: 52,
    KEY_CODE: 53,
    KEY_STATE: 54,
    SYSCALL_ID: 100,
    SYSCALL_ARG1: 101,
    SYSCALL_ARG2: 102,
    SYSCALL_ARG3: 103,
    SYSCALL_RESULT: 104,
    SYSCALL_STATUS: 105
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
export const PROC_STATE = {
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
 * Creates a loop that:
 * 1. Increments heartbeat in shared memory
 * 2. Performs agent-specific work
 * 3. Yields to scheduler
 * 4. Jumps back to start
 */
export class AgentGenerator {
    constructor() {
        this.words = [];
        this.idBound = 10; // Start IDs after reserved
    }

    nextId() {
        return this.idBound++;
    }

    emit(word) {
        this.words.push(word);
    }

    /**
     * Push a constant value onto the stack
     */
    pushConstant(value) {
        const resultId = this.nextId();
        this.emit(instr(OP.CONSTANT, 4)); // count=4
        this.emit(1);  // float type id
        this.emit(resultId);
        this.emit(typeof value === 'number' && !Number.isInteger(value)
            ? floatToWord(value) : value);
        return resultId;
    }

    /**
     * Read from shared memory (address 0-1023)
     */
    sharedLoad(address) {
        this.emit(instr(OP.SHARED_LOAD, 2)); // count=2
        this.emit(address);
    }

    /**
     * Write to shared memory (address 0-1023)
     * Pops value from stack
     */
    sharedStore(address) {
        this.emit(instr(OP.SHARED_STORE, 2)); // count=2
        this.emit(address);
    }

    /**
     * Store to local process memory
     */
    localStore(address) {
        this.emit(instr(OP.STORE, 3)); // count=3
        this.emit(address);
        // result id (unused but required)
        this.emit(this.nextId());
    }

    /**
     * Load from local process memory
     */
    localLoad(address) {
        const resultId = this.nextId();
        this.emit(instr(OP.LOAD, 4)); // count=4
        this.emit(1);  // float type
        this.emit(resultId);
        this.emit(address);
        return resultId;
    }

    /**
     * Add two values on stack
     */
    fadd() {
        this.emit(instr(OP.FADD, 5));
        this.emit(1);  // float type
        this.emit(this.nextId());  // result id
        this.emit(this.nextId());  // operand 1 id
        this.emit(this.nextId());  // operand 2 id
    }

    /**
     * Yield time slice to scheduler
     */
    yield() {
        this.emit(instr(OP.YIELD, 1));
    }

    /**
     * Execute a system call
     * @param {number} syscallId - Syscall ID (1=GET_MOUSE, 2=GET_KEY, etc.)
     * @param {number} arg1 - First argument
     * @param {number} arg2 - Second argument
     * @param {number} arg3 - Third argument
     */
    syscall(syscallId, arg1 = 0, arg2 = 0, arg3 = 0) {
        this.emit(instr(OP_SYSCALL, 5));  // count=5
        this.emit(syscallId);
        this.emit(arg1);
        this.emit(arg2);
        this.emit(arg3);
    }

    /**
     * Read syscall result from shared memory (after syscall completes)
     */
    readSyscallResult() {
        this.sharedLoad(IO.SYSCALL_RESULT);
    }
    /**
     * Unconditional jump to PC offset
     */
    jmp(targetPc) {
        this.emit(instr(OP.JMP, 2));
        this.emit(targetPc);
    }

    /**
     * Exit process
     */
    exit() {
        this.emit(instr(OP.RETURN, 1));
    }

    /**
     * Generate SPIR-V header
     */
    header() {
        return [
            0x07230203,  // Magic
            0x00010000,  // Version 1.0
            0,           // Generator (0 = unknown)
            this.idBound,
            0            // Schema
        ];
    }

    /**
     * Build the final binary
     */
    build() {
        const header = this.header();
        const full = new Uint32Array(header.length + this.words.length);
        full.set(header);
        full.set(this.words, header.length);
        return full.buffer;
    }
}

/**
 * Generate Compositor Agent
 * Manages visual composition and rendering tasks
 */
export function generateCompositorAgent() {
    const gen = new AgentGenerator();
    const agent = AGENTS.COMPOSITOR;

    // PC offset 5 (after header)
    // Loop start:
    const loopStart = 5;

    // Load current heartbeat
    gen.sharedLoad(agent.heartbeat);
    // Push 1
    gen.pushConstant(floatToWord(1.0));
    // Add
    gen.fadd();
    // Store heartbeat
    gen.sharedStore(agent.heartbeat);

    // Set status to RUNNING
    gen.pushConstant(PROC_STATE.RUNNING);
    gen.sharedStore(agent.status);

    // Do compositor work: increment frame counter at shared data 50
    gen.sharedLoad(50);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(50);

    // Yield
    gen.yield();

    // Jump back to loop start
    gen.jmp(loopStart);

    return gen.build();
}

/**
 * Generate Shell Agent
 * Handles command interpretation and UI
 */
export function generateShellAgent() {
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

    // Check for pending command at address 20
    gen.sharedLoad(20);
    gen.pushConstant(floatToWord(0.0));
    // If command pending (non-zero), process it
    // For now, just clear it
    gen.sharedLoad(20);
    gen.pushConstant(floatToWord(0.0));
    gen.sharedStore(20);

    // Yield
    gen.yield();
    gen.jmp(loopStart);

    return gen.build();
}

/**
 * Generate Cognitive Agent
 * AI/LLM integration and inference
 */
export function generateCognitiveAgent() {
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

    // Yield
    gen.yield();
    gen.jmp(loopStart);

    return gen.build();
}

/**
 * Generate Memory Agent
 * Memory management and garbage collection
 */
export function generateMemoryAgent() {
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

    // Yield
    gen.yield();
    gen.jmp(loopStart);

    return gen.build();
}

/**
 * Generate I/O Agent
 * Input/output handling
 */
export function generateIOAgent() {
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

    // === SYSCALL: Get Mouse Position ===
    gen.syscall(SYS.GET_MOUSE, 0, 0, 0);

    // After syscall completes, read result
    // Result is packed: (X << 16) | Y
    gen.readSyscallResult();

    // Store packed result directly at shared memory addr 56
    gen.sharedStore(56);

    // Process I/O queue at address 53
    gen.sharedLoad(53);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(53);

    // Yield
    gen.yield();
    gen.jmp(loopStart);

    return gen.build();
}

/**
 * Generate Scheduler Agent
 * Process coordination and load balancing
 */
export function generateSchedulerAgent() {
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

    // Check all agent heartbeats and update system stats at address 54
    // Sum heartbeats for total activity
    let totalActivity = 0;
    for (let i = 0; i < 7; i++) {
        gen.sharedLoad(i);
        // Accumulate (simplified - just count agent 0 for now)
    }
    gen.sharedStore(54);

    // Yield
    gen.yield();
    gen.jmp(loopStart);

    return gen.build();
}

/**
 * Generate Network Agent
 * Communication and networking
 */
export function generateNetworkAgent() {
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

    // Yield
    gen.yield();
    gen.jmp(loopStart);

    return gen.build();
}

/**
 * Generate all 7 Area Agents
 * @returns {Map<string, ArrayBuffer>} Map of agent name to SPIR-V binary
 */
export function generateAllAgents() {
    return new Map([
        ['compositor', generateCompositorAgent()],
        ['shell', generateShellAgent()],
        ['cognitive', generateCognitiveAgent()],
        ['memory', generateMemoryAgent()],
        ['io', generateIOAgent()],
        ['scheduler', generateSchedulerAgent()],
        ['network', generateNetworkAgent()]
    ]);
}

// Export syscall and I/O constants
export { SYS, IO };
