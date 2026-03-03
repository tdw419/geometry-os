/**
 * Geometry OS GPU Memory Manager
 *
 * Implements a page-based memory management system on the GPU.
 * Uses Hilbert curve addressing for spatial locality optimization.
 *
 * Memory Layout (256MB total):
 * - 0x00000000 - 0x000FFFFF: Kernel Space (1MB)
 * - 0x00100000 - 0x01FFFFFF: User Space (31MB)
 * - 0x02000000 - 0x02FFFFFF: Shared Memory (16MB)
 * - 0x03000000 - 0x03FFFFFF: I/O Buffers (16MB)
 * - 0x04000000 - 0x0FFFFFFF: File Storage (192MB)
 */

// Page size: 4KB = 1024 float4 values = 4096 floats
const PAGE_SIZE = 4096;
const PAGE_SIZE_FLOATS = 1024; // float4 = 4 floats, 1024 * 4 = 4096

// Memory regions (in pages)
const REGIONS = {
    KERNEL: { start: 0, count: 256 },           // 1MB / 4KB = 256 pages
    USER: { start: 256, count: 7936 },          // 31MB / 4KB = 7936 pages
    SHARED: { start: 8192, count: 4096 },       // 16MB / 4KB = 4096 pages
    IO: { start: 12288, count: 4096 },          // 16MB / 4KB = 4096 pages
    FILES: { start: 16384, count: 49152 }       // 192MB / 4KB = 49152 pages
};

// Page flags
const PAGE_FLAGS = {
    NONE: 0,
    READ: 1,
    WRITE: 2,
    EXECUTE: 4,
    SHARED: 8,
    KERNEL: 16
};

// Hilbert curve utilities for spatial locality
function hilbertIndex(x, y, order = 6) {
    let d = 0;
    let s = 1 << order;
    for (let s = 1 << (order - 1); s > 0; s >>= 1) {
        const rx = (x & s) > 0 ? 1 : 0;
        const ry = (y & s) > 0 ? 1 : 0;
        d += s * s * ((3 * rx) ^ ry);
        if (ry === 0) {
            if (rx === 1) {
                x = s - 1 - x;
                y = s - 1 - y;
            }
            [x, y] = [y, x];
        }
    }
    return d;
}

export class GPUMemoryManager {
    constructor(device) {
        this.device = device;

        // Page table: maps virtual page -> physical page + flags
        // Entry format: [physical_page (20 bits) | flags (8 bits) | ring (4 bits)]
        this.pageTable = new Uint32Array(65536); // 64K virtual pages

        // Free page bitmap
        this.freePages = new Uint32Array(2048); // 65536 bits = 65536 pages
        this.freePages.fill(0xFFFFFFFF); // All free initially

        // Physical memory buffer (256MB)
        this.memoryBuffer = null;

        // Page table buffer (GPU-side)
        this.pageTableBuffer = null;

        // Stats
        this.stats = {
            totalPages: 65536,
            allocatedPages: 0,
            freePages: 65536,
            allocations: 0,
            deallocations: 0
        };

        // Process memory maps (pid -> { base, limit, pages[] })
        this.processMaps = new Map();
    }

    /**
     * Initialize GPU buffers for memory management.
     */
    async init() {
        // Main memory buffer (256MB)
        this.memoryBuffer = this.device.createBuffer({
            size: 256 * 1024 * 1024,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        });

        // Page table buffer (64K entries * 4 bytes)
        this.pageTableBuffer = this.device.createBuffer({
            size: 65536 * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        });

        // Reserve kernel pages (not allocatable)
        this._reserveRegion(REGIONS.KERNEL);

        console.log('[GPUMemoryManager] Initialized 256MB memory space with 64K pages');
    }

    /**
     * Allocate pages for a process.
     * @param {number} pid - Process ID
     * @param {number} sizeInBytes - Requested memory size
     * @param {number} flags - Page flags (READ|WRITE|EXECUTE)
     * @returns {{ base: number, limit: number, pages: number[] }}
     */
    malloc(pid, sizeInBytes, flags = PAGE_FLAGS.READ | PAGE_FLAGS.WRITE) {
        const pagesNeeded = Math.ceil(sizeInBytes / (PAGE_SIZE * 4)); // float = 4 bytes
        const allocatedPages = [];

        // Find contiguous free pages in user region
        let currentPage = REGIONS.USER.start;
        let contiguousCount = 0;

        while (currentPage < REGIONS.USER.start + REGIONS.USER.count && contiguousCount < pagesNeeded) {
            if (this._isPageFree(currentPage)) {
                contiguousCount++;
            } else {
                contiguousCount = 0;
                allocatedPages.length = 0;
                currentPage++;
                continue;
            }
            allocatedPages.push(currentPage);
            currentPage++;
        }

        if (contiguousCount < pagesNeeded) {
            throw new Error(`Out of memory: cannot allocate ${pagesNeeded} pages for PID ${pid}`);
        }

        // Mark pages as allocated and set up page table entries
        const virtualBase = this._findVirtualBase(pid, pagesNeeded);
        const ring = 3; // User mode

        for (let i = 0; i < allocatedPages.length; i++) {
            const physPage = allocatedPages[i];
            const virtPage = virtualBase + i;

            // Mark physical page as used
            this._setPageUsed(physPage);

            // Create page table entry: physical_page | flags | ring
            const entry = (physPage << 12) | (flags << 4) | ring;
            this.pageTable[virtPage] = entry;
        }

        // Update stats
        this.stats.allocatedPages += pagesNeeded;
        this.stats.freePages -= pagesNeeded;
        this.stats.allocations++;

        // Store process memory map
        const memMap = {
            base: virtualBase * PAGE_SIZE_FLOATS,
            limit: pagesNeeded * PAGE_SIZE_FLOATS,
            pages: allocatedPages,
            physicalBase: allocatedPages[0] * PAGE_SIZE_FLOATS
        };
        this.processMaps.set(pid, memMap);

        console.log(`[GPUMemoryManager] Allocated ${pagesNeeded} pages for PID ${pid} at vbase 0x${(virtualBase * PAGE_SIZE_FLOATS).toString(16)}`);

        return memMap;
    }

    /**
     * Free memory allocated to a process.
     * @param {number} pid - Process ID
     */
    free(pid) {
        const memMap = this.processMaps.get(pid);
        if (!memMap) {
            console.warn(`[GPUMemoryManager] No memory map for PID ${pid}`);
            return;
        }

        // Free all physical pages
        for (const physPage of memMap.pages) {
            this._setPageFree(physPage);
        }

        // Clear page table entries
        const virtualBase = memMap.base / PAGE_SIZE_FLOATS;
        for (let i = 0; i < memMap.pages.length; i++) {
            this.pageTable[virtualBase + i] = 0;
        }

        // Update stats
        this.stats.allocatedPages -= memMap.pages.length;
        this.stats.freePages += memMap.pages.length;
        this.stats.deallocations++;

        this.processMaps.delete(pid);
        console.log(`[GPUMemoryManager] Freed ${memMap.pages.length} pages for PID ${pid}`);
    }

    /**
     * Translate virtual address to physical address.
     * @param {number} pid - Process ID
     * @param {number} virtAddr - Virtual address (in floats)
     * @returns {number} Physical address (in floats) or -1 on fault
     */
    translate(pid, virtAddr) {
        const memMap = this.processMaps.get(pid);
        if (!memMap) return -1;

        // Check bounds
        if (virtAddr < memMap.base || virtAddr >= memMap.base + memMap.limit) {
            return -1; // Segfault
        }

        const virtPage = Math.floor(virtAddr / PAGE_SIZE_FLOATS);
        const offset = virtAddr % PAGE_SIZE_FLOATS;

        const entry = this.pageTable[virtPage];
        if (entry === 0) return -1;

        const physPage = (entry >> 12) & 0xFFFFF;
        return physPage * PAGE_SIZE_FLOATS + offset;
    }

    /**
     * Sync page table to GPU buffer.
     */
    syncToGPU() {
        this.device.queue.writeBuffer(this.pageTableBuffer, 0, this.pageTable);
    }

    /**
     * Get memory stats.
     */
    getStats() {
        return { ...this.stats };
    }

    /**
     * Get process memory info.
     */
    getProcessMemory(pid) {
        return this.processMaps.get(pid) || null;
    }

    /**
     * Allocate shared memory region.
     */
    allocateShared(sizeInBytes, flags = PAGE_FLAGS.READ | PAGE_FLAGS.WRITE | PAGE_FLAGS.SHARED) {
        const pagesNeeded = Math.ceil(sizeInBytes / (PAGE_SIZE * 4));
        const basePage = REGIONS.SHARED.start;

        // Find contiguous pages in shared region
        const allocatedPages = [];
        let currentPage = basePage;
        let contiguousCount = 0;

        while (currentPage < REGIONS.SHARED.start + REGIONS.SHARED.count && contiguousCount < pagesNeeded) {
            if (this._isPageFree(currentPage)) {
                contiguousCount++;
                allocatedPages.push(currentPage);
            } else {
                contiguousCount = 0;
                allocatedPages.length = 0;
            }
            currentPage++;
        }

        if (contiguousCount < pagesNeeded) {
            throw new Error(`Out of shared memory: cannot allocate ${pagesNeeded} pages`);
        }

        // Mark as used
        for (const page of allocatedPages) {
            this._setPageUsed(page);
            const entry = (page << 12) | (flags << 4) | 0; // Ring 0 for shared
            this.pageTable[page] = entry;
        }

        return {
            base: allocatedPages[0] * PAGE_SIZE_FLOATS,
            limit: pagesNeeded * PAGE_SIZE_FLOATS,
            pages: allocatedPages
        };
    }

    // --- Private helpers ---

    _isPageFree(pageNum) {
        const word = Math.floor(pageNum / 32);
        const bit = pageNum % 32;
        return (this.freePages[word] & (1 << bit)) !== 0;
    }

    _setPageUsed(pageNum) {
        const word = Math.floor(pageNum / 32);
        const bit = pageNum % 32;
        this.freePages[word] &= ~(1 << bit);
    }

    _setPageFree(pageNum) {
        const word = Math.floor(pageNum / 32);
        const bit = pageNum % 32;
        this.freePages[word] |= (1 << bit);
    }

    _reserveRegion(region) {
        for (let i = region.start; i < region.start + region.count; i++) {
            this._setPageUsed(i);
        }
        this.stats.allocatedPages += region.count;
        this.stats.freePages -= region.count;
    }

    _findVirtualBase(pid, pageCount) {
        // Simple allocation: use PID-based offset in user virtual space
        const virtualBase = REGIONS.USER.start + (pid * 1024); // 1024 pages per process
        return virtualBase;
    }
}

export { PAGE_SIZE, PAGE_SIZE_FLOATS, PAGE_FLAGS, REGIONS };
