/**
 * Geometry OS Kernel (WGSL)
 *
 * Multi-process scheduler and memory-mapped OS core.
 * Manages processes in a cooperative multitasking model on the GPU.
 *
 * IPC Architecture:
 * - Shared Memory Region: RAM[0..1023] (first 1K words)
 * - Message Queues: RAM[0..511] (16 mailboxes × 32 words each)
 * - Process Mailboxes: Each PID has a dedicated mailbox at (PID * 32)
 * - Message Format: [sender, type, size, data0, data1, ...]
 */

struct Process {
    pid: u32,
    pc: u32,
    sp: u32,
    mem_base: u32,
    mem_limit: u32,
    status: u32, // 0=Idle, 1=Running, 2=Waiting(IPC), 3=Exit, 4=Error
    priority: u32,
    waiting_on: u32, // PID we're waiting for message from (0xFF = any)
    msg_count: u32,  // Messages received this session
    reserved: array<u32, 6>,
}

// Message header offsets
const MSG_SENDER: u32 = 0u;
const MSG_TYPE: u32 = 1u;
const MSG_SIZE: u32 = 2u;
const MSG_DATA: u32 = 3u;
const MAILBOX_SIZE: u32 = 32u;
const MAX_MAILBOXES: u32 = 16u;

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
            } else if (opcode == 208u) { // OP_MSG_SEND - Send message to another process
                // Format: [count|208], [target_pid], [msg_type], [data_word]
                // Uses stack for additional data
                let target_pid = program[pc + 1];
                let msg_type = program[pc + 2];
                let data = program[pc + 3];

                if (target_pid < MAX_MAILBOXES) {
                    let mailbox_base = target_pid * MAILBOX_SIZE;

                    // Write message header
                    ram[mailbox_base + MSG_SENDER] = p.pid;
                    ram[mailbox_base + MSG_TYPE] = msg_type;
                    ram[mailbox_base + MSG_SIZE] = 1u;
                    ram[mailbox_base + MSG_DATA] = data;

                    // Check if target is waiting for this message
                    if (target_pid < arrayLength(&pcb_table)) {
                        var target = pcb_table[target_pid];
                        if (target.status == 2u &&  // Waiting
                            (target.waiting_on == 0xFFu || target.waiting_on == p.pid)) {
                            target.status = 1u;  // Wake up
                            target.msg_count = target.msg_count + 1u;
                            pcb_table[target_pid] = target;
                        }
                    }
                }
            } else if (opcode == 209u) { // OP_MSG_RECV - Receive message (blocking)
                // Format: [count|209], [from_pid], [timeout]
                // from_pid = 0xFF means receive from anyone
                let from_pid = program[pc + 1];
                let timeout = program[pc + 2];

                let mailbox_base = p.pid * MAILBOX_SIZE;
                let has_message = ram[mailbox_base + MSG_SIZE] > 0u;

                // Check if message is from expected sender
                let sender = ram[mailbox_base + MSG_SENDER];
                let valid_sender = (from_pid == 0xFFu) || (sender == from_pid);

                if (has_message && valid_sender) {
                    // Push message data to stack
                    stack[stack_base + sp] = ram[mailbox_base + MSG_SENDER];
                    stack[stack_base + sp + 1] = ram[mailbox_base + MSG_TYPE];
                    stack[stack_base + sp + 2] = ram[mailbox_base + MSG_DATA];
                    sp = sp + 3;

                    // Clear mailbox (mark as read)
                    ram[mailbox_base + MSG_SIZE] = 0u;
                } else {
                    // Block and wait for message
                    p.status = 2u;  // Waiting
                    p.waiting_on = from_pid;
                    pc = pc + count;
                    break;  // Yield time slice
                }
            } else if (opcode == 210u) { // OP_MSG_PEEK - Non-blocking message check
                // Format: [count|210], [from_pid]
                let from_pid = program[pc + 1];
                let mailbox_base = p.pid * MAILBOX_SIZE;

                let has_message = ram[mailbox_base + MSG_SIZE] > 0u;
                let sender = ram[mailbox_base + MSG_SENDER];
                let valid_sender = (from_pid == 0xFFu) || (sender == from_pid);

                // Push result: 1 if message available, 0 otherwise
                if (has_message && valid_sender) {
                    stack[stack_base + sp] = 1.0;
                } else {
                    stack[stack_base + sp] = 0.0;
                }
                sp = sp + 1;
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
