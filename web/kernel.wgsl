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
    static_priority: u32,
    dynamic_priority: u32,
    total_cycles: u32,
    last_run_timestamp: u32,
    waiting_on: u32, // PID we're waiting for message from (0xFF = any)
    msg_count: u32,  // Messages received this session
    fault_count: u32, // Number of page faults / memory violations
    // Signal system fields
    saved_pc: u32,       // PC saved during signal handler execution
    saved_sp: u32,       // SP saved during signal handler execution
    pending_signals: u32, // Bitmask of pending signals
    signal_mask: u32,    // Bitmask of blocked signals
    signal_handlers_base: u32, // RAM address of signal handler table (32 entries)
    // Remaining reserved field
    reserved: array<u32, 0>,
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
@group(0) @binding(6) var<storage, read_write> page_table: array<u32>;
@group(0) @binding(7) var<storage, read_write> free_bitmap: array<u32>;

// Memory management constants
const PAGE_SIZE_FLOATS: u32 = 1024u;
const PAGE_SHIFT: u32 = 10u;
const FLAG_READ: u32 = 1u;
const FLAG_WRITE: u32 = 2u;
const FLAG_EXECUTE: u32 = 4u;
const RING_USER: u32 = 3u;
const USER_REGION_START: u32 = 256u;

const MAX_INST_PER_SLICE: u32 = 100u;
const KERNEL_MEM_BASE: u32 = 1024u; // User RAM starts at 1024

// Memory Region boundaries (in pages)
const KERNEL_START: u32 = 0u;
const KERNEL_END: u32 = 256u;
const USER_START: u32 = 256u;
const USER_END: u32 = 8192u;
const SHARED_START: u32 = 8192u;
const SHARED_END: u32 = 12288u;

/**
 * Translate virtual address to physical address.
 * Returns physical address (in floats) or 0xFFFFFFFF on fault.
 */
fn translate(virt_addr: u32, pid: u32, required_flags: u32) -> u32 {
    let virt_page = virt_addr >> PAGE_SHIFT;
    let offset = virt_addr & (PAGE_SIZE_FLOATS - 1u);

    // Enforce isolation: Check if page belongs to PID or is SHARED
    // User range: 1024 pages per PID
    let pid_user_start = USER_START + (pid * 1024u);
    let pid_user_end = pid_user_start + 1024u;
    
    let is_user_page = (virt_page >= pid_user_start) && (virt_page < pid_user_end);
    let is_shared_page = (virt_page >= SHARED_START) && (virt_page < SHARED_END);
    let is_kernel_page = (virt_page < KERNEL_END); // Only kernel ring can access this

    if (!is_user_page && !is_shared_page && !is_kernel_page) {
        return 0xFFFFFFFFu; // Segfault
    }

    if (virt_page >= arrayLength(&page_table)) {
        return 0xFFFFFFFFu; // Invalid virtual address
    }

    let entry = page_table[virt_page];
    if (entry == 0u) {
        return 0xFFFFFFFFu; // Page not mapped
    }

    let phys_page = entry >> 12u;
    let flags = (entry >> 4u) & 0xFFu;
    let ring = entry & 0xFu;

    // Check permissions
    if ((flags & required_flags) != required_flags) {
        return 0xFFFFFFFFu; // Permission denied
    }
    
    // Check ring level (user can't access ring 0 pages unless shared?)
    // In our model, Shared pages might have ring 0 but FLAG_SHARED set.
    if (ring == 0u && (flags & FLAG_SHARED) == 0u && virt_page >= KERNEL_END) {
         // This logic might need refinement based on how ring levels are used
    }

    return (phys_page << PAGE_SHIFT) + offset;
}

/**
 * Read from virtual memory address.
 */
fn read_virt(virt_addr: u32, pid: u32) -> f32 {
    let phys_addr = translate(virt_addr, pid, FLAG_READ);
    if (phys_addr == 0xFFFFFFFFu) {
        return 0.0 / 0.0; // NaN on fault
    }
    return ram[phys_addr];
}

/**
 * Write to virtual memory address.
 */
fn write_virt(virt_addr: u32, pid: u32, value: f32) -> bool {
    let phys_addr = translate(virt_addr, pid, FLAG_WRITE);
    if (phys_addr == 0xFFFFFFFFu) {
        return false; // Fault
    }
    ram[phys_addr] = value;
    return true;
}

/**
 * Check if a physical page is free.
 */
fn is_page_free(page_num: u32) -> bool {
    let word_idx = page_num / 32u;
    let bit_idx = page_num % 32u;
    if (word_idx >= arrayLength(&free_bitmap)) {
        return false;
    }
    return (free_bitmap[word_idx] & (1u << bit_idx)) != 0u;
}

/**
 * Mark a physical page as used.
 */
fn set_page_used(page_num: u32) {
    let word_idx = page_num / 32u;
    let bit_idx = page_num % 32u;
    if (word_idx < arrayLength(&free_bitmap)) {
        free_bitmap[word_idx] &= ~(1u << bit_idx);
    }
}

/**
 * Mark a physical page as free.
 */
fn set_page_free(page_num: u32) {
    let word_idx = page_num / 32u;
    let bit_idx = page_num % 32u;
    if (word_idx < arrayLength(&free_bitmap)) {
        free_bitmap[word_idx] |= (1u << bit_idx);
    }
}

/**
 * Allocate contiguous pages for a process.
 * Returns base virtual address or 0 on failure.
 */
fn alloc_pages(pid: u32, count: u32, flags: u32) -> u32 {
    // Find contiguous free pages in user region
    var start_page: u32 = USER_START;
    var found: bool = false;

    for (var i: u32 = USER_START; i < USER_END - count; i = i + 1u) {
        var contiguous: bool = true;
        for (var j: u32 = 0u; j < count; j = j + 1u) {
            if (!is_page_free(i + j)) {
                contiguous = false;
                break;
            }
        }
        if (contiguous) {
            start_page = i;
            found = true;
            break;
        }
    }

    if (!found) {
        return 0u; // Out of memory
    }

    // Mark pages as used and create page table entries
    let virtual_base = USER_START + (pid * 1024u); // 1024 pages per process

    for (var i: u32 = 0u; i < count; i = i + 1u) {
        let phys_page = start_page + i;
        let virt_page = virtual_base / PAGE_SIZE_FLOATS + i;

        set_page_used(phys_page);

        // Create entry: physical_page | flags | ring
        let entry = (phys_page << 12u) | (flags << 4u) | RING_USER;
        page_table[virt_page] = entry;
    }

    return virtual_base;
}

/**
 * Free pages allocated to a process.
 */
fn free_pages(pid: u32, base: u32, count: u32) -> bool {
    let virtual_base = base / PAGE_SIZE_FLOATS;

    for (var i: u32 = 0u; i < count; i = i + 1u) {
        let virt_page = virtual_base + i;
        if (virt_page >= arrayLength(&page_table)) {
            continue;
        }

        let entry = page_table[virt_page];
        if (entry == 0u) {
            continue;
        }

        let phys_page = entry >> 12u;
        set_page_free(phys_page);
        page_table[virt_page] = 0u;
    }

    return true;
}

@compute @workgroup_size(16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let p_idx = global_id.x;
    if (p_idx >= arrayLength(&pcb_table)) { return; }
    
    var p = pcb_table[p_idx];
    if (p.status != 1u) { return; }
    
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
                let virt_addr = p.mem_base + addr;
                let phys_addr = translate(virt_addr, p.pid, FLAG_WRITE);
                if (phys_addr != 0xFFFFFFFFu) {
                    ram[phys_addr] = stack[stack_base + sp - 1];
                    sp = sp - 1;
                } else {
                    p.status = 4u; // SIGSEGV
                    p.fault_count = p.fault_count + 1u;
                    break;
                }
            }
        } else if (opcode == 61u) { // OP_LOAD (Relative to mem_base)
            let addr = program[pc + 3];
            let virt_addr = p.mem_base + addr;
            let phys_addr = translate(virt_addr, p.pid, FLAG_READ);
            if (phys_addr != 0xFFFFFFFFu) {
                stack[stack_base + sp] = ram[phys_addr];
                sp = sp + 1;
            } else {
                p.status = 4u; // SIGSEGV
                p.fault_count = p.fault_count + 1u;
                break;
            }        } else if (opcode == 206u) { // OP_SHARED_STORE - Write to shared memory
            let shared_addr = program[pc + 1];
            if (sp >= 1u) {
                let virt_addr = SHARED_START * PAGE_SIZE_FLOATS + shared_addr;
                let phys_addr = translate(virt_addr, p.pid, FLAG_WRITE);
                if (phys_addr != 0xFFFFFFFFu) {
                    ram[phys_addr] = stack[stack_base + sp - 1];
                    sp = sp - 1;
                } else {
                    p.status = 4u; // SIGSEGV
                    p.fault_count = p.fault_count + 1u;
                    break;
                }
            }
        } else if (opcode == 207u) { // OP_SHARED_LOAD - Read from shared memory
            let shared_addr = program[pc + 1];
            let virt_addr = SHARED_START * PAGE_SIZE_FLOATS + shared_addr;
            let phys_addr = translate(virt_addr, p.pid, FLAG_READ);
            if (phys_addr != 0xFFFFFFFFu) {
                stack[stack_base + sp] = ram[phys_addr];
                sp = sp + 1;
            } else {
                p.status = 4u; // SIGSEGV
                p.fault_count = p.fault_count + 1u;
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
        } else if (opcode == 130u) { // OP_ALLOC (0x82) - Allocate memory pages
            // Format: [count|130], [size_bytes], [flags]
            // Returns: base address on stack, or 0 on failure
            let size_bytes = program[pc + 1];
            let flags = program[pc + 2];

            // Calculate pages needed (4KB pages = 1024 floats)
            let pages_needed = (size_bytes + 4095u) / 4096u;

            // Find contiguous free pages in user region
            var alloc_start: u32 = 0u;
            var found: bool = false;

            for (var i: u32 = USER_REGION_START; i < 8192u - pages_needed; i = i + 1u) {
                var contiguous: bool = true;
                for (var j: u32 = 0u; j < pages_needed; j = j + 1u) {
                    let word_idx = (i + j) / 32u;
                    let bit_idx = (i + j) % 32u;
                    if (word_idx >= arrayLength(&free_bitmap)) {
                        contiguous = false;
                        break;
                    }
                    if ((free_bitmap[word_idx] & (1u << bit_idx)) == 0u) {
                        contiguous = false;
                        break;
                    }
                }
                if (contiguous) {
                    alloc_start = i;
                    found = true;
                    break;
                }
            }

            if (found) {
                // Mark pages as used and create page table entries
                let virtual_base = USER_REGION_START + (p.pid * 1024u);

                for (var i: u32 = 0u; i < pages_needed; i = i + 1u) {
                    let phys_page = alloc_start + i;
                    let virt_page = (virtual_base / PAGE_SIZE_FLOATS) + i;

                    // Mark as used in bitmap
                    let word_idx = phys_page / 32u;
                    let bit_idx = phys_page % 32u;
                    free_bitmap[word_idx] &= ~(1u << bit_idx);

                    // Create page table entry
                    let entry = (phys_page << 12u) | (flags << 4u) | RING_USER;
                    page_table[virt_page] = entry;
                }

                // Return base address
                stack[stack_base + sp] = f32(virtual_base * 4u); // Convert to bytes
                sp = sp + 1;
            } else {
                stack[stack_base + sp] = 0.0; // Allocation failed
                sp = sp + 1;
            }
        } else if (opcode == 131u) { // OP_FREE (0x83) - Free memory pages
            // Format: [count|131], [base_addr], [page_count]
            let base_addr = program[pc + 1];
            let page_count = program[pc + 2];

            let virtual_base = base_addr / (PAGE_SIZE_FLOATS * 4u);

            for (var i: u32 = 0u; i < page_count; i = i + 1u) {
                let virt_page = virtual_base + i;
                if (virt_page >= arrayLength(&page_table)) {
                    continue;
                }

                let entry = page_table[virt_page];
                if (entry == 0u) {
                    continue;
                }

                let phys_page = entry >> 12u;

                // Mark as free in bitmap
                let word_idx = phys_page / 32u;
                let bit_idx = phys_page % 32u;
                free_bitmap[word_idx] |= (1u << bit_idx);

                // Clear page table entry
                page_table[virt_page] = 0u;
            }

            // Push success
            stack[stack_base + sp] = 1.0;
            sp = sp + 1;
        } else if (opcode == 140u) { // OP_OPEN - Open file
            // Format: [count|140], [path_addr], [mode]
            // Returns: file descriptor on stack, or -1 on error
            let path_addr = program[pc + 1];
            let mode = program[pc + 2];

            // Request file open via syscall
            ram[100u] = bitcast<f32>(0x10u); // FS_OPEN
            ram[101u] = bitcast<f32>(path_addr);
            ram[102u] = bitcast<f32>(mode);
            ram[105u] = 0.0;  // Status: pending

            p.status = 2u;  // Waiting for syscall
            p.waiting_on = 0xFEu;
            pc = pc + count;
            break;
        } else if (opcode == 141u) { // OP_CLOSE - Close file
            // Format: [count|141], [fd]
            let fd = program[pc + 1];

            ram[100u] = bitcast<f32>(0x11u); // FS_CLOSE
            ram[101u] = bitcast<f32>(fd);
            ram[105u] = 0.0;

            p.status = 2u;
            p.waiting_on = 0xFEu;
            pc = pc + count;
            break;
        } else if (opcode == 142u) { // OP_READ - Read from file
            // Format: [count|142], [fd], [buffer_addr], [length]
            // Returns: bytes read on stack
            let fd = program[pc + 1];
            let buffer_addr = program[pc + 2];
            let length = program[pc + 3];

            ram[100u] = bitcast<f32>(0x12u); // FS_READ
            ram[101u] = bitcast<f32>(fd);
            ram[102u] = bitcast<f32>(buffer_addr);
            ram[103u] = bitcast<f32>(length);
            ram[105u] = 0.0;

            p.status = 2u;
            p.waiting_on = 0xFEu;
            pc = pc + count;
            break;
        } else if (opcode == 143u) { // OP_WRITE - Write to file
            // Format: [count|143], [fd], [buffer_addr], [length]
            // Returns: bytes written on stack
            let fd = program[pc + 1];
            let buffer_addr = program[pc + 2];
            let length = program[pc + 3];

            ram[100u] = bitcast<f32>(0x13u); // FS_WRITE
            ram[101u] = bitcast<f32>(fd);
            ram[102u] = bitcast<f32>(buffer_addr);
            ram[103u] = bitcast<f32>(length);
            ram[105u] = 0.0;

            p.status = 2u;
            p.waiting_on = 0xFEu;
            pc = pc + count;
            break;
        } else if (opcode == 144u) { // OP_SEEK - Seek in file
            // Format: [count|144], [fd], [offset], [whence]
            let fd = program[pc + 1];
            let offset = program[pc + 2];
            let whence = program[pc + 3];

            ram[100u] = bitcast<f32>(0x14u); // FS_SEEK
            ram[101u] = bitcast<f32>(fd);
            ram[102u] = bitcast<f32>(offset);
            ram[103u] = bitcast<f32>(whence);
            ram[105u] = 0.0;

            p.status = 2u;
            p.waiting_on = 0xFEu;
            pc = pc + count;
            break;
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
    p.total_cycles = p.total_cycles + inst_executed;
    pcb_table[p_idx] = p;
}
