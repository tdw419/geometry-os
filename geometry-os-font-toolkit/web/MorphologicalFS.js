/**
 * Geometry OS Morphological Filesystem
 *
 * A GPU-native filesystem where "The Screen IS the Hard Drive".
 * Files are stored as spatial regions mapped via Hilbert curves.
 *
 * Storage Layout (192MB = 49,152 blocks @ 4KB each):
 * - Blocks 0-255: Superblock + Inode Table
 * - Blocks 256-49151: Data Blocks
 *
 * Inode Format (64 bytes each):
 * - flags (4B), size (4B), blocks[8] (32B), name[24] (24B)
 */

// Constants
const BLOCK_SIZE = 4096;          // 4KB blocks
const BLOCK_SIZE_FLOATS = 1024;   // float4 per block
const INODE_SIZE = 64;            // bytes per inode
const INODES_PER_BLOCK = 64;      // 4096 / 64
const MAX_INODES = 16384;         // 256 blocks * 64 inodes
const MAX_BLOCKS_PER_FILE = 8;    // Direct blocks only (32KB max file)
const DATA_BLOCK_START = 256;     // First data block

// Inode flags
const INODE_FLAGS = {
    FREE: 0,
    USED: 1,
    DIRECTORY: 2,
    SYMLINK: 4,
    EXECUTABLE: 8
};

// File permissions
const PERM_READ = 1;
const PERM_WRITE = 2;
const PERM_EXEC = 4;

// Syscall numbers for filesystem operations
const FS_SYSCALLS = {
    OPEN: 0x10,
    CLOSE: 0x11,
    READ: 0x12,
    WRITE: 0x13,
    SEEK: 0x14,
    STAT: 0x15,
    UNLINK: 0x16,
    MKDIR: 0x17
};

// File open modes
const OPEN_MODES = {
    READ: 0,
    WRITE: 1,
    READ_WRITE: 2,
    APPEND: 3,
    CREATE: 4
};

/**
 * Hilbert curve utilities for spatial mapping.
 */
class HilbertMapper {
    static encode(x, y, order = 8) {
        // Map 2D coordinates to 1D Hilbert index
        let d = 0;
        for (let s = 1 << (order - 1); s > 0; s >>= 1) {
            const rx = (x & s) ? 1 : 0;
            const ry = (y & s) ? 1 : 0;
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

    static decode(d, order = 8) {
        // Map 1D Hilbert index to 2D coordinates
        let x = 0, y = 0;
        for (let s = 1; s < (1 << order); s <<= 1) {
            const rx = 1 & (d >> 1);
            const ry = 1 & (d ^ rx);
            this._rot(s, x, y, rx, ry);
            x += s * rx;
            y += s * ry;
            d >>= 2;
        }
        return [x, y];
    }

    static _rot(n, x, y, rx, ry) {
        if (ry === 0) {
            if (rx === 1) {
                x = n - 1 - x;
                y = n - 1 - y;
            }
            return [y, x];
        }
        return [x, y];
    }

    /**
     * Get the spatial extent (bounding box) for a range of blocks.
     */
    static getBlockExtent(startBlock, count, gridSize = 256) {
        const startX = startBlock % gridSize;
        const startY = Math.floor(startBlock / gridSize);
        const endBlock = startBlock + count - 1;
        const endX = endBlock % gridSize;
        const endY = Math.floor(endBlock / gridSize);

        return {
            x: startX,
            y: startY,
            width: endX - startX + 1,
            height: endY - startY + 1
        };
    }
}

/**
 * File Inode structure.
 */
class FileInode {
    constructor(buffer = null, offset = 0) {
        if (buffer) {
            this.flags = buffer[offset] || 0;
            this.size = buffer[offset + 1] || 0;
            this.blocks = new Uint32Array(buffer.buffer, (offset + 2) * 4, 8);
            this.name = this._decodeName(new Uint8Array(buffer.buffer, (offset + 10) * 4, 24));
        } else {
            this.flags = INODE_FLAGS.FREE;
            this.size = 0;
            this.blocks = new Uint32Array(8);
            this.name = '';
        }
        this.position = 0; // File seek position
        this.mode = 0;     // Open mode
    }

    _decodeName(bytes) {
        let name = '';
        for (let i = 0; i < 24 && bytes[i] !== 0; i++) {
            name += String.fromCharCode(bytes[i]);
        }
        return name;
    }

    _encodeName(name) {
        const bytes = new Uint8Array(24);
        for (let i = 0; i < Math.min(name.length, 23); i++) {
            bytes[i] = name.charCodeAt(i);
        }
        return bytes;
    }

    isFree() {
        return (this.flags & INODE_FLAGS.USED) === 0;
    }

    isDirectory() {
        return (this.flags & INODE_FLAGS.DIRECTORY) !== 0;
    }

    toBuffer() {
        const buffer = new Uint32Array(16); // 64 bytes = 16 u32
        buffer[0] = this.flags;
        buffer[1] = this.size;
        buffer.set(this.blocks, 2);

        const nameBytes = this._encodeName(this.name);
        const nameView = new Uint8Array(buffer.buffer, 40, 24);
        nameView.set(nameBytes);

        return buffer;
    }
}

/**
 * Morphological Filesystem Manager.
 */
export class MorphologicalFS {
    constructor(memoryManager) {
        this.memoryManager = memoryManager;
        this.device = memoryManager.device;

        // Filesystem state
        this.inodes = [];
        this.blockBitmap = new Uint32Array(1536); // 49152 bits = 1536 words
        this.openFiles = new Map(); // fd -> { inode, position, mode }
        this.nextFd = 3; // 0, 1, 2 reserved for stdin/stdout/stderr

        // GPU buffers
        this.blockBuffer = null;    // Data blocks
        this.inodeBuffer = null;    // Inode table
        this.bitmapBuffer = null;   // Block bitmap

        // Root directory
        this.rootInode = null;

        // Stats
        this.stats = {
            totalBlocks: 49152,
            freeBlocks: 49152 - DATA_BLOCK_START,
            usedBlocks: DATA_BLOCK_START,
            totalInodes: MAX_INODES,
            freeInodes: MAX_INODES,
            openFiles: 0
        };
    }

    /**
     * Initialize the filesystem.
     */
    async init() {
        // Allocate GPU buffers
        // Inode table: 16384 inodes * 64 bytes = 1MB
        this.inodeBuffer = this.device.createBuffer({
            size: MAX_INODES * INODE_SIZE,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        });

        // Block bitmap: 49152 bits = 6144 bytes
        this.bitmapBuffer = this.device.createBuffer({
            size: 1536 * 4,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        });

        // Initialize inode table (all free)
        for (let i = 0; i < MAX_INODES; i++) {
            this.inodes.push(new FileInode());
        }

        // Mark system blocks as used (0-255)
        for (let i = 0; i < DATA_BLOCK_START; i++) {
            this._setBlockUsed(i);
        }

        // Create root directory
        this.rootInode = this._allocateInode('/', INODE_FLAGS.DIRECTORY | INODE_FLAGS.USED);

        // Sync to GPU
        this._syncInodesToGPU();
        this._syncBitmapToGPU();

        console.log('[MorphologicalFS] Initialized filesystem');
        console.log(`  - ${this.stats.freeBlocks} free blocks (${this.stats.freeBlocks * 4}KB)`);
        console.log(`  - ${this.stats.freeInodes} free inodes`);
    }

    /**
     * Open a file.
     */
    open(path, mode = OPEN_MODES.READ) {
        // Find or create inode
        let inode = this._lookupPath(path);

        if (!inode) {
            if (mode === OPEN_MODES.CREATE || mode === OPEN_MODES.WRITE) {
                inode = this._createFile(path);
            } else {
                return -1; // File not found
            }
        }

        // Allocate file descriptor
        const fd = this.nextFd++;
        this.openFiles.set(fd, {
            inode: inode,
            position: 0,
            mode: mode
        });

        this.stats.openFiles++;
        console.log(`[MorphologicalFS] Opened "${path}" as fd ${fd}`);

        return fd;
    }

    /**
     * Close a file.
     */
    close(fd) {
        if (!this.openFiles.has(fd)) {
            return false;
        }

        this.openFiles.delete(fd);
        this.stats.openFiles--;
        return true;
    }

    /**
     * Read from a file.
     */
    read(fd, buffer, offset, length) {
        const file = this.openFiles.get(fd);
        if (!file) return -1;

        const inode = file.inode;
        if ((file.mode !== OPEN_MODES.READ && file.mode !== OPEN_MODES.READ_WRITE)) {
            return -1; // Not open for reading
        }

        // Calculate which blocks we need
        const startByte = file.position;
        const endByte = Math.min(startByte + length, inode.size);
        const bytesToRead = endByte - startByte;

        if (bytesToRead <= 0) return 0;

        // Read from blocks
        let bytesRead = 0;
        while (bytesRead < bytesToRead) {
            const blockIndex = Math.floor((startByte + bytesRead) / BLOCK_SIZE);
            const blockOffset = (startByte + bytesRead) % BLOCK_SIZE;
            const physicalBlock = inode.blocks[blockIndex];

            if (physicalBlock === 0) break;

            // Copy from block to buffer (would need GPU read in real implementation)
            const chunkSize = Math.min(BLOCK_SIZE - blockOffset, bytesToRead - bytesRead);
            bytesRead += chunkSize;
        }

        file.position += bytesRead;
        return bytesRead;
    }

    /**
     * Write to a file.
     */
    write(fd, buffer, offset, length) {
        const file = this.openFiles.get(fd);
        if (!file) return -1;

        const inode = file.inode;
        if (file.mode !== OPEN_MODES.WRITE &&
            file.mode !== OPEN_MODES.READ_WRITE &&
            file.mode !== OPEN_MODES.APPEND &&
            file.mode !== OPEN_MODES.CREATE) {
            return -1; // Not open for writing
        }

        // Allocate blocks as needed
        const endByte = file.position + length;
        const blocksNeeded = Math.ceil(endByte / BLOCK_SIZE);

        for (let i = 0; i < blocksNeeded; i++) {
            if (inode.blocks[i] === 0) {
                inode.blocks[i] = this._allocateBlock();
                if (inode.blocks[i] === 0) {
                    return -1; // Out of space
                }
            }
        }

        // Write data (would need GPU write in real implementation)
        inode.size = Math.max(inode.size, endByte);
        file.position += length;

        this._syncInodesToGPU();
        return length;
    }

    /**
     * Seek in a file.
     */
    seek(fd, position, whence = 0) {
        const file = this.openFiles.get(fd);
        if (!file) return -1;

        switch (whence) {
            case 0: // SEEK_SET
                file.position = position;
                break;
            case 1: // SEEK_CUR
                file.position += position;
                break;
            case 2: // SEEK_END
                file.position = file.inode.size + position;
                break;
        }

        return file.position;
    }

    /**
     * Get file statistics.
     */
    stat(path) {
        const inode = this._lookupPath(path);
        if (!inode) return null;

        return {
            size: inode.size,
            blocks: inode.blocks.filter(b => b !== 0).length,
            isDirectory: inode.isDirectory(),
            name: inode.name
        };
    }

    /**
     * List directory contents.
     */
    listdir(path = '/') {
        // For now, return all files (simple flat filesystem)
        const files = [];
        for (const inode of this.inodes) {
            if (!inode.isFree() && inode.name !== '/') {
                files.push({
                    name: inode.name,
                    size: inode.size,
                    isDirectory: inode.isDirectory()
                });
            }
        }
        return files;
    }

    /**
     * Delete a file.
     */
    unlink(path) {
        const inode = this._lookupPath(path);
        if (!inode) return false;

        // Free blocks
        for (const block of inode.blocks) {
            if (block !== 0) {
                this._setBlockFree(block);
            }
        }

        // Free inode
        inode.flags = INODE_FLAGS.FREE;
        inode.size = 0;
        inode.blocks.fill(0);
        inode.name = '';

        this.stats.freeInodes++;
        this._syncInodesToGPU();

        return true;
    }

    /**
     * Get the spatial extent for a file (for visualization).
     */
    getFileExtent(path) {
        const inode = this._lookupPath(path);
        if (!inode) return null;

        const blocks = inode.blocks.filter(b => b !== 0);
        if (blocks.length === 0) return null;

        return {
            startBlock: blocks[0],
            blockCount: blocks.length,
            extent: HilbertMapper.getBlockExtent(blocks[0], blocks.length)
        };
    }

    /**
     * Get filesystem statistics.
     */
    getStats() {
        return { ...this.stats };
    }

    // --- Private methods ---

    _lookupPath(path) {
        const name = path.startsWith('/') ? path.slice(1) : path;
        for (const inode of this.inodes) {
            if (!inode.isFree() && inode.name === name) {
                return inode;
            }
        }
        return null;
    }

    _createFile(path) {
        const name = path.startsWith('/') ? path.slice(1) : path;
        const inode = this._allocateInode(name, INODE_FLAGS.USED);
        return inode;
    }

    _allocateInode(name, flags) {
        for (let i = 0; i < this.inodes.length; i++) {
            if (this.inodes[i].isFree()) {
                const inode = this.inodes[i];
                inode.flags = flags;
                inode.name = name;
                inode.size = 0;
                inode.blocks.fill(0);
                this.stats.freeInodes--;
                return inode;
            }
        }
        return null; // No free inodes
    }

    _allocateBlock() {
        for (let i = DATA_BLOCK_START; i < this.stats.totalBlocks; i++) {
            if (this._isBlockFree(i)) {
                this._setBlockUsed(i);
                return i;
            }
        }
        return 0; // No free blocks
    }

    _isBlockFree(blockNum) {
        const word = Math.floor(blockNum / 32);
        const bit = blockNum % 32;
        return (this.blockBitmap[word] & (1 << bit)) !== 0;
    }

    _setBlockUsed(blockNum) {
        const word = Math.floor(blockNum / 32);
        const bit = blockNum % 32;
        const wasFree = (this.blockBitmap[word] & (1 << bit)) !== 0;
        this.blockBitmap[word] &= ~(1 << bit);
        if (wasFree) {
            this.stats.freeBlocks--;
            this.stats.usedBlocks++;
        }
    }

    _setBlockFree(blockNum) {
        const word = Math.floor(blockNum / 32);
        const bit = blockNum % 32;
        const wasUsed = (this.blockBitmap[word] & (1 << bit)) === 0;
        this.blockBitmap[word] |= (1 << bit);
        if (wasUsed) {
            this.stats.freeBlocks++;
            this.stats.usedBlocks--;
        }
    }

    _syncInodesToGPU() {
        // Flatten inodes to buffer
        const buffer = new Uint32Array(MAX_INODES * 16);
        for (let i = 0; i < this.inodes.length; i++) {
            const inodeBuffer = this.inodes[i].toBuffer();
            buffer.set(inodeBuffer, i * 16);
        }
        this.device.queue.writeBuffer(this.inodeBuffer, 0, buffer);
    }

    _syncBitmapToGPU() {
        this.device.queue.writeBuffer(this.bitmapBuffer, 0, this.blockBitmap);
    }
}

export {
    BLOCK_SIZE,
    BLOCK_SIZE_FLOATS,
    INODE_SIZE,
    MAX_INODES,
    MAX_BLOCKS_PER_FILE,
    INODE_FLAGS,
    PERM_READ,
    PERM_WRITE,
    PERM_EXEC,
    FS_SYSCALLS,
    OPEN_MODES,
    HilbertMapper,
    FileInode
};
