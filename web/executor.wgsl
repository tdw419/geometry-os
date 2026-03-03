@group(0) @binding(0) var<storage, read> program: array<u32>;
@group(0) @binding(1) var<storage, read_write> stack: array<f32>;
@group(0) @binding(2) var<storage, read_write> result: array<f32>;
@group(0) @binding(3) var<storage, read_write> ram: array<f32>;
@group(0) @binding(4) var<storage, read> labels: array<u32>;
@group(0) @binding(5) var<storage, read_write> visual_ram: array<f32>;

@compute @workgroup_size(1)
fn main() {
    var sp: u32 = 0; // Stack pointer
    var pc: u32 = 5; // Start after SPIR-V header (5 words)
    
    loop {
        if (pc >= arrayLength(&program)) { break; }
        
        let word = program[pc];
        let opcode = word & 0xFFFFu;
        let count = (word >> 16u) & 0xFFFFu;
        
        if (count == 0u) { break; } // Safety break
        
        if (opcode == 43u) { // OP_CONSTANT
            // SPIR-V Constant: Word 0: [Count|43], Word 1: Type, Word 2: ResultID, Word 3: Value
            stack[sp] = bitcast<f32>(program[pc + 3]);
            sp = sp + 1;
        } else if (opcode == 129u) { // OP_FADD (GeoASM +)
            if (sp >= 2u) {
                let v2 = stack[sp - 1];
                let v1 = stack[sp - 2];
                stack[sp - 2] = v1 + v2;
                sp = sp - 1;
            }
        } else if (opcode == 131u) { // OP_FSUB (GeoASM -)
            if (sp >= 2u) {
                let v2 = stack[sp - 1];
                let v1 = stack[sp - 2];
                stack[sp - 2] = v1 - v2;
                sp = sp - 1;
            }
        } else if (opcode == 133u) { // OP_FMUL (GeoASM *)
            if (sp >= 2u) {
                let v2 = stack[sp - 1];
                let v1 = stack[sp - 2];
                stack[sp - 2] = v1 * v2;
                sp = sp - 1;
            }
        } else if (opcode == 135u) { // OP_FDIV (GeoASM /)
            if (sp >= 2u) {
                let v2 = stack[sp - 1];
                let v1 = stack[sp - 2];
                stack[sp - 2] = v1 / v2;
                sp = sp - 1;
            }
        } else if (opcode == 182u) { // OP_FGreaterThan (GeoASM >)
            if (sp >= 2u) {
                let v2 = stack[sp - 1];
                let v1 = stack[sp - 2];
                if (v1 > v2) { stack[sp - 2] = 1.0; } else { stack[sp - 2] = 0.0; }
                sp = sp - 1;
            }
        } else if (opcode == 184u) { // OP_FLessThan (GeoASM <)
            if (sp >= 2u) {
                let v2 = stack[sp - 1];
                let v1 = stack[sp - 2];
                if (v1 < v2) { stack[sp - 2] = 1.0; } else { stack[sp - 2] = 0.0; }
                sp = sp - 1;
            }
        } else if (opcode == 12u) { // OP_EXT_INST (GLSL.std.450)
            let inst = program[pc + 3];
            if (inst == 13u) { // Sin
                stack[sp - 1] = sin(stack[sp - 1]);
            } else if (inst == 14u) { // Cos
                stack[sp - 1] = cos(stack[sp - 1]);
            }
        } else if (opcode == 62u) { // OP_STORE (GeoASM store)
            let addr = program[pc + 1];
            if (sp >= 1u) {
                ram[addr] = stack[sp - 1];
                sp = sp - 1;
            }
        } else if (opcode == 61u) { // OP_LOAD (GeoASM load)
            let addr = program[pc + 3];
            stack[sp] = ram[addr];
            sp = sp + 1;
        } else if (opcode == 200u) { // OP_JNZ (Custom GeoASM Jump if Not Zero)
            let label_id = program[pc + 1];
            if (sp >= 1u) {
                let val = stack[sp - 1];
                sp = sp - 1;
                if (val != 0.0) {
                    pc = labels[label_id];
                    continue;
                }
            }
        } else if (opcode == 202u) { // OP_JMP (Custom GeoASM Unconditional Jump)
            let label_id = program[pc + 1];
            pc = labels[label_id];
            continue;
        } else if (opcode == 203u) { // OP_JZ (Custom GeoASM Jump if Zero)
            let label_id = program[pc + 1];
            if (sp >= 1u) {
                let val = stack[sp - 1];
                sp = sp - 1;
                if (val == 0.0) {
                    pc = labels[label_id];
                    continue;
                }
            }
        } else if (opcode == 128u) { // OP_RECT (Custom Drawing Syscall)
            if (sp >= 5u) {
                let color = stack[sp - 1];
                let h = u32(stack[sp - 2]);
                let w = u32(stack[sp - 3]);
                let y_start = u32(stack[sp - 4]);
                let x_start = u32(stack[sp - 5]);
                sp = sp - 5;
                
                for (var dy: u32 = 0; dy < h; dy = dy + 1) {
                    for (var dx: u32 = 0; dx < w; dx = dx + 1) {
                        let tx = x_start + dx;
                        let ty = y_start + dy;
                        if (tx < 16u && ty < 8u) {
                            ram[128 + ty * 16 + tx] = color;
                        }
                    }
                }
            }
        } else if (opcode == 129u) { // OP_CLR (Custom Drawing Syscall)
            if (sp >= 1u) {
                let color = stack[sp - 1];
                sp = sp - 1;
                for (var i: u32 = 0; i < 128u; i = i + 1) {
                    ram[128 + i] = color;
                }
            }
        } else if (opcode == 201u) { // OP_TONE (GeoASM tone)
            // Pops 3: frequency, duration, volume (stored to RAM[123-125])
            // Sets RAM[126] as trigger flag for CPU to process
            if (sp >= 3u) {
                ram[125] = stack[sp - 3]; // frequency
                ram[124] = stack[sp - 2]; // duration
                ram[123] = stack[sp - 1]; // volume
                ram[126] = 1.0;            // trigger flag
                sp = sp - 3;
            }
        } else if (opcode == 204u) { // OP_VISUAL_LOAD - Load from visual_ram
            // Word format: [count|204], [address]
            let vaddr = program[pc + 1];
            stack[sp] = visual_ram[vaddr];
            sp = sp + 1;
        } else if (opcode == 205u) { // OP_VISUAL_STORE - Store to visual_ram
            // B channel (program[pc + 1]) contains the address
            let vaddr = program[pc + 1];
            if (sp >= 1u) {
                visual_ram[vaddr] = stack[sp - 1];
                sp = sp - 1;
            }
        } else if (opcode == 253u) { // OP_RETURN
            break;
        }
        
        pc = pc + count;
    }
    
    result[0] = stack[0];
}
