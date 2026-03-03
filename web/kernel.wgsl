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
    // Signal system fields (formerly part of reserved array)
    saved_pc: u32,       // PC saved during signal handler execution
    saved_sp: u32,       // SP saved during signal handler execution
    pending_signals: u32, // Bitmask of pending signals
    signal_mask: u32,    // Bitmask of blocked signals
    signal_handlers_base: u32, // RAM address of signal handler table (32 entries)
    // Remaining reserved field
    // reserved[0]: program base offset (used by OP_EXEC)
    reserved: array<u32, 1>,
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
        
        var inst_executed: u32 = 0u;
        for (var inst_count: u32 = 0u; inst_count < MAX_INST_PER_SLICE; inst_count = inst_count + 1u) {
            if (pc >= arrayLength(&program)) {
                p.status = 3u; // Terminate if OOB
                break;
            }
            
            let word = program[pc];
            let opcode = word & 0xFFFFu;
            let count = (word >> 16u) & 0xFFFFu;
            
            if (count == 0u) { p.status = 3u; break; }
            
            inst_executed = inst_executed + 1u;
            
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
                if (sp >= 1u) {
                    if (addr < p.mem_limit) {
                        ram[ram_base + addr] = stack[stack_base + sp - 1];
                        sp = sp - 1;
                    } else {
                        p.status = 4u; // SIGSEGV
                        break;
                    }
                }
            } else if (opcode == 61u) { // OP_LOAD (Relative to mem_base)
                let addr = program[pc + 3];
                if (addr < p.mem_limit) {
                    stack[stack_base + sp] = ram[ram_base + addr];
                    sp = sp + 1;
                } else {
                    p.status = 4u; // SIGSEGV
                    break;
                }
            } else if (opcode == 206u) { // OP_SHARED_STORE - Write to shared memory
                let shared_addr = program[pc + 1];
                if (sp >= 1u) {
                    if (shared_addr < 1024u) {
                        ram[shared_addr] = stack[stack_base + sp - 1];
                        sp = sp - 1;
                    } else {
                        p.status = 4u; // SIGSEGV
                        break;
                    }
                }
            } else if (opcode == 207u) { // OP_SHARED_LOAD - Read from shared memory
                let shared_addr = program[pc + 1];
                if (shared_addr < 1024u) {
                    stack[stack_base + sp] = ram[shared_addr];
                    sp = sp + 1;
                } else {
                    p.status = 4u; // SIGSEGV
                    break;
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
            } else if (opcode == 211u) { // OP_SYSCALL - Request external I/O
                // Format: [count|211], [syscall_id], [arg1], [arg2], [arg3]
                let syscall_id = program[pc + 1];
                let arg1 = program[pc + 2];
                let arg2 = program[pc + 3];
                let arg3 = program[pc + 4];

                // Write syscall request to shared memory
                ram[100u] = bitcast<f32>(syscall_id);
                ram[101u] = bitcast<f32>(arg1);
                ram[102u] = bitcast<f32>(arg2);
                ram[103u] = bitcast<f32>(arg3);
                ram[105u] = 0.0;  // Status: pending

                // Set process to WAITING state
                p.status = 2u;  // Waiting for syscall
                p.waiting_on = 0xFEu;  // Special: waiting for syscall
                pc = pc + count;
                break;  // Yield until syscall completes
            } else if (opcode == 220u) { // OP_FORK - Clone current process
                var child_pid: u32 = 0u;
                var found_slot: bool = false;
                
                // Find first idle PCB (skip parent pid)
                for (var i: u32 = 0u; i < process_count; i = i + 1u) {
                    if (i != p_idx && pcb_table[i].status == 0u) {
                        child_pid = i;
                        found_slot = true;
                        break;
                    }
                }
                
                if (found_slot) {
                    // Clone parent to child PCB
                    var child = p;
                    child.pid = child_pid;
                    child.status = 1u; // Running
                    child.pc = pc + count; // Start after FORK instruction
                    child.sp = sp + 1u; // Add result to child stack
                    child.mem_base = KERNEL_MEM_BASE + (child_pid * 1024u); // Offset by pid
                    
                    // Clone parent stack to child stack
                    let child_stack_base = child_pid * 1024u;
                    for (var s: u32 = 0u; s < 1024u; s = s + 1u) {
                        stack[child_stack_base + s] = stack[stack_base + s];
                    }
                    
                    // Child gets 0 on stack
                    stack[child_stack_base + sp] = 0.0;
                    
                    // Parent gets child_pid on stack
                    stack[stack_base + sp] = f32(child_pid);
                    sp = sp + 1u;
                    
                    pcb_table[child_pid] = child;
                } else {
                    // No slots: push -1
                    stack[stack_base + sp] = -1.0;
                    sp = sp + 1u;
                }
            } else if (opcode == 221u) { // OP_EXEC - Load new program from RAM
                // Format: [count|221], [ram_addr], [size]
                let ram_addr = program[pc + 1];
                let prog_size = program[pc + 2];
                let prog_offset = pcb_table[p_idx].reserved[0]; // Program base offset

                // Copy new program from RAM to dedicated program space
                for (var i: u32 = 0u; i < prog_size; i = i + 1u) {
                    program[prog_offset + i] = bitcast<u32>(ram[ram_base + ram_addr + i]);
                }

                // Reset Process State
                pc = prog_offset + 5u; // Start at new program entry (after header)
                sp = 0u; // Reset stack
                continue; // Restart execution with new program
            } else if (opcode == 223u) { // OP_SIGRET - Return from signal handler
                // Format: [count|223]
                // Restores PC and SP from saved state to resume after signal handler
                if (p.saved_pc != 0u) {
                    pc = p.saved_pc;
                    sp = p.saved_sp;
                    p.saved_pc = 0u;  // Clear saved state
                    p.saved_sp = 0u;
                    continue;  // Resume at saved location immediately
                }
                // If no saved state, just advance PC (no-op)
            } else if (opcode == 253u) { // OP_RETURN (Exit Process)
                p.status = 3u;
                break;
            } else if (opcode == 228u) { // GEO_YIELD (Custom Opcode)
                pc = pc + count;
                break; // End current time slice
            } else if (opcode == 176u) { // GEO_ROUTE (0xB0)
                let target_addr = program[pc + 1];
                ram[2047u] = bitcast<f32>(1u); // Request: Register Route
                ram[2048u + 256u] = bitcast<f32>(target_addr);
            } else if (opcode == 177u) { // GEO_FWD (0xB1)
                let target_addr = program[pc + 1];
                ram[2047u] = bitcast<f32>(2u); // Request: Forward
                ram[2048u + 256u] = bitcast<f32>(target_addr);
            }
            
            pc = pc + count;
        }
        
        // Save process state back to PCB
        p.pc = pc;
        p.sp = sp;
        p.priority = p.priority + inst_executed;
        pcb_table[p_idx] = p;
    }
}
