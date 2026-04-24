/*
 * vfs_pixel.h -- Guest-side pixel surface file access library
 *
 * "Pixels move pixels" file I/O for RISC-V guests on Geometry OS.
 * Files are encoded as RGBA pixels in a 256x256 MMIO surface at 0x70000000.
 *
 * Surface layout:
 *   Row 0: Directory index
 *     [0] = PXFS magic (0x50584653)
 *     [1] = file_count
 *     [2..255] = directory entries: (start_row << 16) | name_hash
 *   Row N: File data
 *     [0] = header: (byte_count << 16) | (name_hash << 8) | flags
 *           flags: bit 0 = valid, bit 1 = guest_created
 *     Guest files: [1..63] = filename bytes, [64+] = file data
 *     Host files:  [1+] = file data
 */

#ifndef VFS_PIXEL_H
#define VFS_PIXEL_H

#include <stdint.h>
#include <string.h>

#define VFS_BASE       0x70000000UL
#define VFS_MAGIC      0x50584653UL
#define VFS_MAX_FILES  254
#define VFS_COLS       256

#define VFS_PIXEL(row, col) (*((volatile uint32_t *)(VFS_BASE + ((row) * VFS_COLS + (col)) * 4)))
#define VFS_CONTROL     (*((volatile uint32_t *)(VFS_BASE + VFS_COLS * VFS_COLS * 4)))
#define VFS_CTRL_FLUSH  0x01

static inline uint32_t vfs_fnv1a(const char *s) {
    uint32_t h = 0x811c9dc5;
    while (*s) { h ^= (uint8_t)*s++; h *= 0x01000193; }
    return h;
}

static inline uint32_t vfs_file_count(void) { return VFS_PIXEL(0, 1); }

static inline uint16_t vfs_find(const char *name) {
    uint32_t count = vfs_file_count();
    uint32_t hash = vfs_fnv1a(name);
    for (uint32_t i = 0; i < count && i < VFS_MAX_FILES; i++) {
        uint32_t entry = VFS_PIXEL(0, 2 + i);
        if ((entry & 0xFFFF) == (hash & 0xFFFF)) {
            uint16_t sr = (uint16_t)(entry >> 16);
            if (sr > 0 && sr < 256 && (VFS_PIXEL(sr, 0) & 0x01)) return sr;
        }
    }
    return 0;
}

static inline uint16_t vfs_create(const char *name, uint32_t len, const void *data) {
    if (!name || !name[0] || strlen(name) > 63) return 0;
    uint32_t count = vfs_file_count();
    if (count >= VFS_MAX_FILES) return 0;
    uint16_t free_row = 1;
    for (uint32_t i = 0; i < count; i++) {
        uint32_t entry = VFS_PIXEL(0, 2 + i);
        uint16_t sr = (uint16_t)(entry >> 16);
        if (sr == 0) continue;
        uint32_t header = VFS_PIXEL(sr, 0);
        if (!(header & 0x01)) continue;
        uint32_t bc = header >> 16;
        uint32_t pc = (bc + 3) / 4;
        uint16_t end = sr + (uint16_t)((64 + pc + 255) / 256);
        if (end > free_row) free_row = end;
    }
    if (free_row == 0 || free_row >= 255) return 0;
    uint32_t hash = vfs_fnv1a(name);
    for (int i = 0; i < 63 && name[i]; i++)
        VFS_PIXEL(free_row, 1 + i) = (uint32_t)(uint8_t)name[i];
    VFS_PIXEL(free_row, 0) = ((len & 0xFFFF) << 16) | ((hash & 0xFF) << 8) | 0x03;
    const uint8_t *bytes = (const uint8_t *)data;
    uint32_t pc = (len + 3) / 4;
    for (uint32_t i = 0; i < pc; i++) {
        uint32_t pixel = 0;
        for (int j = 0; j < 4; j++) {
            uint32_t idx = i * 4 + j;
            if (idx < len) pixel |= (uint32_t)bytes[idx] << (j * 8);
        }
        uint16_t col = 64 + i;
        uint16_t row = free_row + col / 256;
        VFS_PIXEL(row, col % 256) = pixel;
    }
    VFS_PIXEL(0, 2 + count) = ((uint32_t)free_row << 16) | (hash & 0xFFFF);
    VFS_PIXEL(0, 1) = count + 1;
    VFS_CONTROL = VFS_CTRL_FLUSH;
    return free_row;
}

static inline int vfs_read(const char *name, void *buf, uint32_t buflen) {
    uint16_t sr = vfs_find(name);
    if (sr == 0) return -1;
    uint32_t header = VFS_PIXEL(sr, 0);
    uint32_t bc = header >> 16;
    uint32_t guest = (header & 0x02) != 0;
    uint32_t dc = guest ? 64 : 1;
    uint32_t to_read = bc < buflen ? bc : buflen;
    uint8_t *dst = (uint8_t *)buf;
    uint32_t pc = (to_read + 3) / 4;
    for (uint32_t i = 0; i < pc; i++) {
        uint16_t col = dc + i;
        uint16_t row = sr + col / 256;
        uint32_t pixel = VFS_PIXEL(row, col % 256);
        for (int j = 0; j < 4; j++) {
            uint32_t idx = i * 4 + j;
            if (idx < to_read) dst[idx] = (uint8_t)(pixel >> (j * 8));
        }
    }
    return (int)to_read;
}

static inline int vfs_unlink(const char *name) {
    uint32_t count = vfs_file_count();
    uint32_t hash = vfs_fnv1a(name);
    for (uint32_t i = 0; i < count; i++) {
        uint32_t entry = VFS_PIXEL(0, 2 + i);
        uint16_t sr = (uint16_t)(entry >> 16);
        if (sr == 0 || (entry & 0xFFFF) != (hash & 0xFFFF)) continue;
        uint32_t header = VFS_PIXEL(sr, 0);
        VFS_PIXEL(sr, 0) = header & ~0x01u;
        for (uint32_t j = i; j < count - 1; j++)
            VFS_PIXEL(0, 2 + j) = VFS_PIXEL(0, 2 + j + 1);
        VFS_PIXEL(0, 2 + count - 1) = 0;
        VFS_PIXEL(0, 1) = count - 1;
        VFS_CONTROL = VFS_CTRL_FLUSH;
        return 0;
    }
    return -1;
}

static inline void vfs_flush(void) { VFS_CONTROL = VFS_CTRL_FLUSH; }

#endif /* VFS_PIXEL_H */
