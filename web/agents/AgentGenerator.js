/**
 * Geometry OS Area Agent SPIR-V Generator
 *
 * Generates specialized SPIR-V programs for the 7 Area Agents.
 */

export const AGENTS = {
    COMPOSITOR: { id: 0, name: 'Compositor', heartbeat: 0, status: 10 },
    SHELL: { id: 1, name: 'Shell', heartbeat: 1, status: 11 },
    COGNITIVE: { id: 2, name: 'Cognitive', heartbeat: 2, status: 12 },
    MEMORY: { id: 3, name: 'Memory', heartbeat: 3, status: 13 },
    IO: { id: 4, name: 'I/O', heartbeat: 4, status: 14 },
    SCHEDULER: { id: 5, name: 'Scheduler', heartbeat: 5, status: 15 },
    NETWORK: { id: 6, name: 'Network', heartbeat: 6, status: 16 }
};

export const IPC = {
    HEARTBEAT_BASE: 0,
    STATUS_BASE: 10,
    MESSAGE_QUEUE: 20,
    SHARED_DATA: 50,
    MAX_SHARED: 1023
};

const OP_SYSCALL = 211;

export const SYS = {
    GET_MOUSE: 1,
    GET_KEY: 2,
    WRITE_LOG: 3,
    GET_TIME: 4,
    DRAW_PIXEL: 5
};

export const IO = {
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

export const PROC_STATE = {
    IDLE: 0,
    RUNNING: 1,
    WAITING: 2,
    DONE: 3,
    ERROR: 4
};

function instr(opcode, count) {
    return (count << 16) | opcode;
}

function floatToWord(f) {
    const buf = new ArrayBuffer(4);
    const view = new DataView(buf);
    view.setFloat32(0, f, true);
    return view.getUint32(0, true);
}

export class AgentGenerator {
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

    yield() {
        this.emit(instr(OP.YIELD, 1));
    }

    syscall(syscallId, arg1 = 0, arg2 = 0, arg3 = 0) {
        this.emit(instr(OP_SYSCALL, 5));
        this.emit(syscallId);
        this.emit(arg1);
        this.emit(arg2);
        this.emit(arg3);
    }

    readSyscallResult() {
        this.sharedLoad(IO.SYSCALL_RESULT);
    }

    drawPixel(x, y, r, g, b) {
        const color = (r << 16) | (g << 8) | b;
        this.syscall(SYS.DRAW_PIXEL, x, y, color);
    }

    jmp(targetPc) {
        this.emit(instr(OP.JMP, 2));
        this.emit(targetPc);
    }

    exit() {
        this.emit(instr(OP.RETURN, 1));
    }

    header() {
        return [0x07230203, 0x00010000, 0, this.idBound, 0];
    }

    build() {
        const header = this.header();
        const full = new Uint32Array(header.length + this.words.length);
        full.set(header);
        full.set(this.words, header.length);
        return full.buffer;
    }
}

export function generateCompositorAgent() {
    const gen = new AgentGenerator();
    const agent = AGENTS.COMPOSITOR;
    const loopStart = 5;
    gen.sharedLoad(agent.heartbeat);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(agent.heartbeat);
    gen.pushConstant(PROC_STATE.RUNNING);
    gen.sharedStore(agent.status);
    gen.drawPixel(100, 100, 0, 255, 204);
    gen.yield();
    gen.jmp(loopStart);
    return gen.build();
}

export function generateShellAgent() {
    const gen = new AgentGenerator();
    const agent = AGENTS.SHELL;
    const loopStart = 5;
    gen.sharedLoad(agent.heartbeat);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(agent.heartbeat);
    gen.pushConstant(PROC_STATE.RUNNING);
    gen.sharedStore(agent.status);
    gen.yield();
    gen.jmp(loopStart);
    return gen.build();
}

export function generateCognitiveAgent() {
    const gen = new AgentGenerator();
    const agent = AGENTS.COGNITIVE;
    const loopStart = 5;
    gen.sharedLoad(agent.heartbeat);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(agent.heartbeat);
    gen.pushConstant(PROC_STATE.RUNNING);
    gen.sharedStore(agent.status);
    gen.yield();
    gen.jmp(loopStart);
    return gen.build();
}

export function generateMemoryAgent() {
    const gen = new AgentGenerator();
    const agent = AGENTS.MEMORY;
    const loopStart = 5;
    gen.sharedLoad(agent.heartbeat);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(agent.heartbeat);
    gen.pushConstant(PROC_STATE.RUNNING);
    gen.sharedStore(agent.status);
    gen.yield();
    gen.jmp(loopStart);
    return gen.build();
}

export function generateIOAgent() {
    const gen = new AgentGenerator();
    const agent = AGENTS.IO;
    const loopStart = 5;
    gen.sharedLoad(agent.heartbeat);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(agent.heartbeat);
    gen.pushConstant(PROC_STATE.RUNNING);
    gen.sharedStore(agent.status);
    gen.syscall(SYS.GET_MOUSE, 0, 0, 0);
    gen.readSyscallResult();
    gen.sharedStore(56);
    gen.yield();
    gen.jmp(loopStart);
    return gen.build();
}

export function generateSchedulerAgent() {
    const gen = new AgentGenerator();
    const agent = AGENTS.SCHEDULER;
    const loopStart = 5;
    gen.sharedLoad(agent.heartbeat);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(agent.heartbeat);
    gen.pushConstant(PROC_STATE.RUNNING);
    gen.sharedStore(agent.status);
    gen.yield();
    gen.jmp(loopStart);
    return gen.build();
}

export function generateNetworkAgent() {
    const gen = new AgentGenerator();
    const agent = AGENTS.NETWORK;
    const loopStart = 5;
    gen.sharedLoad(agent.heartbeat);
    gen.pushConstant(floatToWord(1.0));
    gen.fadd();
    gen.sharedStore(agent.heartbeat);
    gen.pushConstant(PROC_STATE.RUNNING);
    gen.sharedStore(agent.status);
    gen.yield();
    gen.jmp(loopStart);
    return gen.build();
}

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
