/**
 * Geometry OS Visual Compiler (JavaScript)
 * 
 * Port of visual_to_spirv.py to the browser.
 * Compiles a visual grid of glyphs into a SPIR-V binary.
 */

export class HilbertCurve {
    constructor(order) {
        this.order = order;
        this.size = Math.pow(2, order);
    }

    // Fixed d2xy implementation for JS (bitwise friendly)
    d2xy(d) {
        let x = 0;
        let y = 0;
        let s = 1;
        let t = d;
        while (s < this.size) {
            let rx = 1 & (t >> 1);
            let ry = 1 & (t ^ rx);
            const rot = this._rot(s, x, y, rx, ry);
            x = rot.x + s * rx;
            y = rot.y + s * ry;
            t >>= 2;
            s <<= 1;
        }
        return { x, y };
    }

    _rot(s, x, y, rx, ry) {
        if (ry === 0) {
            if (rx === 1) {
                x = s - 1 - x;
                y = s - 1 - y;
            }
            return { x: y, y: x };
        }
        return { x, y };
    }
}

export class VisualCompiler {
    constructor() {
        this.OP_CONSTANT = 43;
        this.OP_FADD = 129;
        this.OP_FSUB = 131;
        this.OP_FMUL = 133;
        this.OP_FDIV = 135;
        this.OP_FGreaterThan = 182;
        this.OP_FLessThan = 184;
        this.OP_EXT_INST = 12;
        this.OP_STORE = 62;
        this.OP_LOAD = 61;
        this.OP_JNZ = 200; // Custom: Jump if Not Zero
        this.OP_JZ = 203; // Custom: Jump if Zero
        this.OP_JMP = 202; // Custom: Unconditional Jump
        this.OP_LABEL = 248; // OpLabel
        this.OP_RETURN = 253;
        this.OP_FUNCTION_END = 56;
        this.OP_LABEL_BLOCK = 248;
        this.OP_FUNCTION = 54;
        this.OP_TYPE_FUNCTION = 33;
        this.OP_TYPE_VOID = 19;
        this.OP_TYPE_FLOAT = 22;
        this.OP_MEMORY_MODEL = 14;
        this.OP_CAPABILITY = 17;
        this.OP_TONE = 201; // Custom Sound Syscall
        this.OP_ROUTE = 176; // 0xB0
        this.OP_FWD = 177;   // 0xB1
    }

    compile(grid, gridSize) {
        const words = [];
        let idBound = 1;
        let currentOffset = 5; // Start after header
        const labelMap = {};

        const nextId = () => idBound++;
        const emit = (opcode, operands = []) => {
            const count = operands.length + 1;
            const word0 = (count << 16) | opcode;
            words.push(word0);
            for (const op of operands) {
                if (typeof op === 'number' && !Number.isInteger(op)) {
                    const buf = new ArrayBuffer(4);
                    const view = new DataView(buf);
                    view.setFloat32(0, op, true);
                    words.push(view.getUint32(0, true));
                } else {
                    words.push(op);
                }
            }
            const emitted = words.length + 5; // Simplified tracking
            return count;
        };

        // Pass 1: Pre-calculate offsets for labels
        // We'll simulate emission to find offsets
        let simulatedOffset = 5;
        const addSim = (c) => simulatedOffset += c;
        
        addSim(2); // Capability
        addSim(3); // MemoryModel
        addSim(3); // FloatType
        addSim(2); // VoidType
        addSim(3); // FuncType
        addSim(5); // Function
        addSim(2); // Label

        const order = Math.log2(gridSize);
        const curve = new HilbertCurve(order);

        for (let d = 0; d < gridSize * gridSize; d++) {
            const { x, y } = curve.d2xy(d);
            const glyph = grid[y * gridSize + x];
            if (!glyph || !glyph.info) continue;
            const g = glyph.info.g;
            if (g === 0) addSim(4); // Constant
            else if (g === 0x6A || g === 0x6B || g === 0x6C || g === 0x6D || g === 0x10 || g === 0x11) addSim(5);
            else if (g === 0x70 || g === 0x71) addSim(6); // ExtInst
            else if (g === 0x72) addSim(3); // Store
            else if (g === 0x73) addSim(4); // Load
            else if (g === 0x80) addSim(1); // Rect
            else if (g === 0x81) addSim(1); // Clr
            else if (g === 0x82) addSim(1); // Tone (Pops 3: freq, duration, volume)
            else if (g === 0xB0) addSim(2); // Route
            else if (g === 0xB1) addSim(2); // Fwd
            else if (g === 0x45) addSim(3); // JNZ [Count|200], LabelID, ResultID(unused)
            else if (g === 0x47) addSim(2); // JMP [Count|202], LabelID
            else if (g === 0x48) addSim(3); // JZ [Count|203], LabelID, ResultID(unused)
            else if (g === 0x46) {
                labelMap[glyph.info.b] = simulatedOffset;
                addSim(2); // Label
            }
        }

        // Pass 2: Actual Emission
        const floatId = nextId();
        const voidId = nextId();
        const funcTypeId = nextId();
        const mainFuncId = nextId();
        const labelId = nextId();

        emit(this.OP_CAPABILITY, [1]);
        emit(this.OP_MEMORY_MODEL, [0, 1]);
        emit(this.OP_TYPE_FLOAT, [floatId, 32]);
        emit(this.OP_TYPE_VOID, [voidId]);
        emit(this.OP_TYPE_FUNCTION, [funcTypeId, voidId]);
        emit(this.OP_FUNCTION, [voidId, mainFuncId, 0, funcTypeId]);
        emit(this.OP_LABEL_BLOCK, [labelId]);

        const stack = [];
        for (let d = 0; d < gridSize * gridSize; d++) {
            const { x, y } = curve.d2xy(d);
            const glyph = grid[y * gridSize + x];
            if (!glyph || !glyph.info) continue;

            const g = glyph.info.g;
            const b = glyph.info.b;

            if (g === 0) {
                const rid = nextId();
                emit(this.OP_CONSTANT, [floatId, rid, b]);
                stack.push(rid);
            } else if (g === 0x6A) {
                if (stack.length >= 2) {
                    const v2 = stack.pop(); const v1 = stack.pop();
                    const rid = nextId(); emit(this.OP_FADD, [floatId, rid, v1, v2]);
                    stack.push(rid);
                }
            } else if (g === 0x6B) {
                if (stack.length >= 2) {
                    const v2 = stack.pop(); const v1 = stack.pop();
                    const rid = nextId(); emit(this.OP_FSUB, [floatId, rid, v1, v2]);
                    stack.push(rid);
                }
            } else if (g === 0x6C) {
                if (stack.length >= 2) {
                    const v2 = stack.pop(); const v1 = stack.pop();
                    const rid = nextId(); emit(this.OP_FMUL, [floatId, rid, v1, v2]);
                    stack.push(rid);
                }
            } else if (g === 0x6D) {
                if (stack.length >= 2) {
                    const v2 = stack.pop(); const v1 = stack.pop();
                    const rid = nextId(); emit(this.OP_FDIV, [floatId, rid, v1, v2]);
                    stack.push(rid);
                }
            } else if (g === 0x10) {
                if (stack.length >= 2) {
                    const v2 = stack.pop(); const v1 = stack.pop();
                    const rid = nextId(); emit(this.OP_FGreaterThan, [floatId, rid, v1, v2]);
                    stack.push(rid);
                }
            } else if (g === 0x11) {
                if (stack.length >= 2) {
                    const v2 = stack.pop(); const v1 = stack.pop();
                    const rid = nextId(); emit(this.OP_FLessThan, [floatId, rid, v1, v2]);
                    stack.push(rid);
                }
            } else if (g === 0x70) {
                if (stack.length >= 1) {
                    const v1 = stack.pop(); const rid = nextId();
                    emit(this.OP_EXT_INST, [floatId, rid, 1, 13, v1]);
                    stack.push(rid);
                }
            } else if (g === 0x71) {
                if (stack.length >= 1) {
                    const v1 = stack.pop(); const rid = nextId();
                    emit(this.OP_EXT_INST, [floatId, rid, 1, 14, v1]);
                    stack.push(rid);
                }
            } else if (g === 0x72) {
                if (stack.length >= 1) {
                    const v1 = stack.pop(); emit(this.OP_STORE, [b, v1]);
                }
            } else if (g === 0x73) {
                const rid = nextId(); emit(this.OP_LOAD, [floatId, rid, b]);
                stack.push(rid);
            } else if (g === 0x80) { // rect
                emit(128, []); // OP_RECT (Pops 5)
            } else if (g === 0x81) { // clr
                emit(129, []); // OP_CLR (Pops 1)
            } else if (g === 0x82) { // tone
                emit(this.OP_TONE, []); // OP_TONE (Pops 3: freq, duration, volume)
            } else if (g === 0xB0) { // route
                emit(this.OP_ROUTE, [b]);
            } else if (g === 0xB1) { // fwd
                emit(this.OP_FWD, [b]);
            } else if (g === 0x45) { // JNZ
                const cond = stack.length > 0 ? stack.pop() : 0;
                emit(this.OP_JNZ, [b, cond]); // Operand 1 is LabelID, Operand 2 is value ID
            } else if (g === 0x47) { // JMP (unconditional)
                emit(this.OP_JMP, [b]); // Operand 1 is LabelID
            } else if (g === 0x48) { // JZ (jump if zero)
                const cond = stack.length > 0 ? stack.pop() : 0;
                emit(this.OP_JZ, [b, cond]); // Operand 1 is LabelID, Operand 2 is value ID
            } else if (g === 0x46) { // Label
                emit(this.OP_LABEL_BLOCK, [nextId()]);
            }
        }

        emit(this.OP_RETURN);
        emit(this.OP_FUNCTION_END);

        const header = [0x07230203, 0x00010000, 0, idBound, 0];
        const fullBinary = new Uint32Array(header.length + words.length);
        fullBinary.set(header);
        fullBinary.set(words, header.length);

        return { binary: fullBinary.buffer, labelMap };
    }
}
