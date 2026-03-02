/**
 * Geometry OS Kernel (WGSL)
 * 
 * Multi-process scheduler and memory-mapped OS core.
 * Manages processes in a cooperative multitasking model on the GPU.
 */

struct Process {
    pid: u32,
    pc: u32,
    sp: u32,
    mem_base: u32,
    mem_limit: u32,
    status: u32, // 0=Idle, 1=Running, 2=Waiting, 3=Exit
    priority: u32,
    reserved: array<u32, 9>,
}

@group(0) @binding(0) var<storage, read_write> program: array<u32>;
@group(0) @binding(1) var<storage, read_write> stack: array<f32>;
@group(0) @binding(2) var<storage, read_write> result: array<f32>;
@group(0) @binding(3) var<storage, read_write> ram: array<f32>;
@group(0) @binding(4) var<storage, read> labels: array<u32>;
@group(0) @binding(5) var<storage, read_write> pcb_table: array<Process>;

const MAX_INST_PER_SLICE: u32 = 100u;
const KERNEL_MEM_BASE: u32 = 1024u; // User RAM starts at 1024

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let process_count = arrayLength(&pcb_table);
    
    // Simple Round-Robin Scheduler
    for (var p_idx: u32 = 0u; p_idx < process_count; p_idx = p_idx + 1u) {
        var p = pcb_table[p_idx];
        
        if (p.status != 1u) { continue; } // Only run active processes
        
        var pc = p.pc;
        var sp = p.sp;
        let stack_base = p_idx * 1024u; // Each process gets 1024 floats of stack
        let ram_base = p.mem_base;
        
        for (var inst_count: u32 = 0u; inst_count < MAX_INST_PER_SLICE; inst_count = inst_count + 1u) {
            if (pc >= arrayLength(&program)) {
                p.status = 3u; // Terminate if OOB
                break;
            }
            
            let word = program[pc];
            let opcode = word & 0xFFFFu;
            let count = (word >> 16u) & 0xFFFFu;
            
            if (count == 0u) { p.status = 3u; break; }
            
            // --- Opcode Interpretation ---
            
            if (opcode == 43u) { // OP_CONSTANT
                stack[stack_base + sp] = bitcast<f32>(program[pc + 3]);
                sp = sp + 1;
            } else if (opcode == 129u) { // OP_FADD
                if (sp >= 2u) {
                    let v2 = stack[stack_base + sp - 1];
                    let v1 = stack[stack_base + sp - 2];
                    stack[stack_base + sp - 2] = v1 + v2;
                    sp = sp - 1;
                }
            } else if (opcode == 133u) { // OP_FMUL
                if (sp >= 2u) {
                    let v2 = stack[stack_base + sp - 1];
                    let v1 = stack[stack_base + sp - 2];
                    stack[stack_base + sp - 2] = v1 * v2;
                    sp = sp - 1;
                }
            } else if (opcode == 62u) { // OP_STORE (Relative to mem_base)
                let addr = program[pc + 1];
                if (sp >= 1u && addr < p.mem_limit) {
                    ram[ram_base + addr] = stack[stack_base + sp - 1];
                    sp = sp - 1;
                }
            } else if (opcode == 61u) { // OP_LOAD (Relative to mem_base)
                let addr = program[pc + 3];
                if (addr < p.mem_limit) {
                    stack[stack_base + sp] = ram[ram_base + addr];
                    sp = sp + 1;
                }
            } else if (opcode == 206u) { // OP_SHARED_STORE - Write to shared memory
                // Format: [count|206], [shared_addr]
                // Pops value from stack and writes to shared memory region
                let shared_addr = program[pc + 1];
                if (sp >= 1u && shared_addr < 1024u) {
                    ram[shared_addr] = stack[stack_base + sp - 1];
                    sp = sp - 1;
                }
            } else if (opcode == 207u) { // OP_SHARED_LOAD - Read from shared memory
                // Format: [count|207], [shared_addr]
                // Reads from shared memory and pushes to stack
                let shared_addr = program[pc + 1];
                if (shared_addr < 1024u) {
                    stack[stack_base + sp] = ram[shared_addr];
                    sp = sp + 1;
                }
            } else if (opcode == 253u) { // OP_RETURN (Exit Process)
                p.status = 3u;
                break;
            } else if (opcode == 228u) { // GEO_YIELD (Custom Opcode)
                pc = pc + count;
                break; // End current time slice
            }
            
            pc = pc + count;
        }
        
        // Save process state back to PCB
        p.pc = pc;
        p.sp = sp;
        pcb_table[p_idx] = p;
    }
}
