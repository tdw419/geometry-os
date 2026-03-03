/**
 * Geometry OS Memory Manager (WGSL)
 *
 * GPU-side page table management and virtual-to-physical translation.
 * Implements OP_ALLOC (0x82) and OP_FREE (0x83) as kernel operations.
 *
 * Page Table Entry Format (32-bit):
 * - Bits 0-3:   Ring level (0=kernel, 3=user)
 * - Bits 4-11:  Flags (R/W/X/S)
 * - Bits 12-31: Physical page number
 */

// Page constants
const PAGE_SIZE_FLOATS: u32 = 1024u;
const PAGE_SHIFT: u32 = 10u;  // log2(1024)

// Flag bits
const FLAG_READ: u32 = 1u;
const FLAG_WRITE: u32 = 2u;
const FLAG_EXECUTE: u32 = 4u;
const FLAG_SHARED: u32 = 8u;
const FLAG_KERNEL: u32 = 16u;

// Ring levels
const RING_KERNEL: u32 = 0u;
const RING_USER: u32 = 3u;

// Syscall region offsets
const SYSCALL_ID: u32 = 100u;
const SYSCALL_ARG1: u32 = 101u;
const SYSCALL_ARG2: u32 = 102u;
const SYSCALL_ARG3: u32 = 103u;
const SYSCALL_RESULT: u32 = 104u;
const SYSCALL_STATUS: u32 = 105u;

// Memory management syscall IDs
const SYS_ALLOC: u32 = 0x82u;
const SYS_FREE: u32 = 0x83u;
const SYS_REALLOC: u32 = 0x84u;

struct PageTableEntry {
    entry: u32,
}

struct AllocRequest {
    pid: u32,
    size_pages: u32,
    flags: u32,
    result_base: u32,
}

@group(0) @binding(0) var<storage, read_write> ram: array<f32>;
@group(0) @binding(1) var<storage, read_write> page_table: array<u32>;
@group(0) @binding(2) var<storage, read_write> free_bitmap: array<u32>;
@group(0) @binding(3) var<storage, read_write> alloc_requests: array<AllocRequest>;

// Region boundaries (in pages)
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

/**
 * Memory management compute shader.
 * Processes allocation requests from the request queue.
 */
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let request_idx = global_id.x;

    if (request_idx >= arrayLength(&alloc_requests)) {
        return;
    }

    let req = alloc_requests[request_idx];

    // Process allocation request
    if (req.size_pages > 0u) {
        let result = alloc_pages(req.pid, req.size_pages, req.flags);
        alloc_requests[request_idx].result_base = result;
    }
}

/**
 * Handle memory syscall from kernel.
 * Called by kernel.wgsl when OP_SYSCALL (211) is executed.
 */
fn handle_memory_syscall(pid: u32, syscall_id: u32, arg1: u32, arg2: u32, arg3: u32) -> u32 {
    switch (syscall_id) {
        case SYS_ALLOC: {
            // arg1 = size in bytes, arg2 = flags
            let pages_needed = (arg1 + 4095u) / 4096u;
            return alloc_pages(pid, pages_needed, arg2);
        }
        case SYS_FREE: {
            // arg1 = base address, arg2 = size in pages
            let result = free_pages(pid, arg1, arg2);
            return select(0xFFFFFFFFu, 0u, result);
        }
        case SYS_REALLOC: {
            // TODO: Implement reallocation
            return 0xFFFFFFFFu;
        }
        default: {
            return 0xFFFFFFFFu; // Unknown syscall
        }
    }
}
