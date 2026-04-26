/*
 * vfs_pixel.h -- Header-only C library for Pixel VFS Surface file access
 *
 * "Pixels move pixels" -- no ecall, no syscall, just memory reads/writes.
 * File data lives as RGBA pixels at 0x7000_0000 (MMIO region).
 *
 * Surface layout:
 *   Row 0: Directory index
 *     Col 0: magic = 0x50584653 ("PXFS")
 *     Col 1: file_count
 *     Col 2..255: one pixel per file [start_row(16) | name_hash(16)]
 *   Row 1+: File data regions
 *     Each file starts with a header pixel at col 0:
 *       [byte_count(16) | name_hash_low(8) | flags(8)]
 *     Followed by data pixels at col 1+ (4 bytes per pixel, little-endian)
 *
 * Control register at 0x70100000 (VFS_SURFACE_BASE + 256KB):
 *   Bit 0 = flush dirty rows to host filesystem
 *
 * Usage:
 *   #include "vfs_pixel.h"
 *   struct vfs_entry entries[16];
 *   int n = vfs_list(entries, 16);
 *   char buf[256];
 *   int bytes = vfs_read(entries[0].start_row, buf, sizeof(buf));
 *   vfs_write(entries[0].start_row, buf, bytes);
 *   vfs_flush();
 */

#ifndef VFS_PIXEL_H
#define VFS_PIXEL_H

#include <stdint.h>

/* ---- Constants ---- */
#define VFS_SURFACE_BASE   0x70000000u
#define VFS_COLS           256
#define VFS_ROWS           256
#define VFS_CONTROL_ADDR   0x70100000u  /* base + 256*256*4 */

#define VFS_MAGIC          0x50584653u  /* "PXFS" */
#define VFS_FLAG_VALID     0x01u
#define VFS_FLAG_DIRTY     0x02u
#define VFS_MAX_FILES      254
#define VFS_MAX_FILENAME   64

/* ---- Types ---- */

/** Directory entry for a file in the VFS surface */
struct vfs_entry {
    uint32_t start_row;   /* Row where the file header pixel lives */
    uint32_t name_hash;   /* FNV-1a hash of the filename (16-bit) */
};

/* ---- SBI helpers (for UART output only, not for file access) ---- */

static inline long sbi_console_putchar(int ch) {
    register long a0 __asm__("a0") = ch;
    register long a7 __asm__("a7") = 1;
    __asm__ volatile("ecall" : "+r"(a0) : "r"(a7) : "memory", "a1");
    return a0;
}

static __attribute__((noreturn)) void sbi_shutdown(void) {
    register long a7 __asm__("a7") = 8;
    __asm__ volatile("ecall" : : "r"(a7) : "memory", "a0", "a1");
    __builtin_unreachable();
}

/* ---- Internal helpers ---- */

static inline volatile uint32_t *vfs_surface_ptr(void) {
    return (volatile uint32_t *)VFS_SURFACE_BASE;
}

static inline volatile uint32_t *vfs_control_ptr(void) {
    return (volatile uint32_t *)VFS_CONTROL_ADDR;
}

/* ---- FNV-1a hash (matches Rust implementation) ---- */

static inline uint32_t vfs_fnv1a(const char *s) {
    uint32_t hash = 0x811c9dc5u;
    while (*s) {
        hash ^= (uint8_t)*s;
        hash *= 0x01000193u;
        s++;
    }
    return hash;
}

/* ---- Public API ---- */

/**
 * vfs_list - List all files in the VFS surface.
 * @entries: output array of directory entries
 * @max: maximum number of entries to fill
 * Returns: number of files found, or -1 if surface is invalid
 */
static inline int vfs_list(struct vfs_entry *entries, int max) {
    volatile uint32_t *surface = vfs_surface_ptr();
    uint32_t magic = surface[0];
    if (magic != VFS_MAGIC) return -1;
    uint32_t count = surface[1];
    if ((int)count > max) count = (uint32_t)max;
    for (uint32_t i = 0; i < count; i++) {
        uint32_t entry = surface[2 + i];
        entries[i].start_row = entry >> 16;
        entries[i].name_hash = entry & 0xFFFF;
    }
    return (int)count;
}

/**
 * vfs_read - Read file data from the pixel surface into a buffer.
 * @start_row: the row containing the file header pixel
 * @buf: output buffer
 * @maxlen: maximum bytes to read
 * Returns: number of bytes read, or -1 on error
 */
static inline int vfs_read(int start_row, void *buf, int maxlen) {
    volatile uint32_t *surface = vfs_surface_ptr();
    uint32_t header = surface[start_row * VFS_COLS];
    int byte_count = (int)(header >> 16);
    int flags = (int)(header & 0xFF);
    if (!(flags & VFS_FLAG_VALID)) return -1;
    if (byte_count > maxlen) byte_count = maxlen;

    /* Data starts at start_row, col 1 (same row as header) */
    volatile uint32_t *data = &surface[start_row * VFS_COLS + 1];
    uint8_t *out = (uint8_t *)buf;
    for (int i = 0; i < byte_count; i++) {
        int pixel = i / 4;
        int byte_in_pixel = i % 4;
        out[i] = (data[pixel] >> (byte_in_pixel * 8)) & 0xFF;
    }
    return byte_count;
}

/**
 * vfs_write - Write data to an existing file's region in the pixel surface.
 * @start_row: the row containing the file header pixel
 * @buf: source data
 * @len: number of bytes to write
 *
 * Sets the DIRTY flag so the host knows to flush this file.
 */
static inline void vfs_write(int start_row, const void *buf, int len) {
    volatile uint32_t *surface = vfs_surface_ptr();
    uint32_t header = surface[start_row * VFS_COLS];
    uint32_t name_hash_low = (header >> 8) & 0xFF;

    /* Update header: new byte_count | same hash | valid | dirty */
    surface[start_row * VFS_COLS] =
        ((uint32_t)len << 16) | (name_hash_low << 8) | VFS_FLAG_VALID | VFS_FLAG_DIRTY;

    const uint8_t *src = (const uint8_t *)buf;
    volatile uint32_t *data = &surface[start_row * VFS_COLS + 1];
    int full_pixels = len / 4;

    for (int i = 0; i < full_pixels; i++) {
        data[i] = (uint32_t)src[i*4]
                 | ((uint32_t)src[i*4+1] << 8)
                 | ((uint32_t)src[i*4+2] << 16)
                 | ((uint32_t)src[i*4+3] << 24);
    }

    /* Handle remaining bytes */
    int rem = len % 4;
    if (rem > 0) {
        uint32_t px = 0;
        for (int j = 0; j < rem; j++) {
            px |= (uint32_t)src[full_pixels * 4 + j] << (j * 8);
        }
        data[full_pixels] = px;
    }
}

/**
 * vfs_create - Create a new file entry in the VFS surface.
 * @name: filename (used to compute hash)
 * @start_row: which row to use for the new file
 * @buf: initial file data (can be NULL for empty file)
 * @len: number of bytes of initial data
 *
 * Writes a directory index entry in row 0 and a header pixel at start_row.
 * The caller must pick a start_row that doesn't overlap existing files.
 * Use vfs_find_free_row() to find the next available row.
 *
 * Note: This only writes to the pixel surface. The file appears on the
 * host filesystem when vfs_flush() is called AND the host has the
 * file_map entry for this row. For new files created by the guest,
 * use vfs_create_named() which also registers in a way the host can flush.
 */
static inline void vfs_create(const char *name, int start_row,
                               const void *buf, int len) {
    volatile uint32_t *surface = vfs_surface_ptr();
    uint32_t name_hash = vfs_fnv1a(name) & 0xFFFF;

    /* Find current file count and add directory entry */
    uint32_t file_count = surface[1];
    if (file_count >= VFS_MAX_FILES) return;

    /* Add to directory index */
    surface[2 + file_count] = ((uint32_t)start_row << 16) | name_hash;

    /* Write file header at start_row, col 0 */
    surface[start_row * VFS_COLS] =
        ((uint32_t)len << 16) | ((name_hash & 0xFF) << 8)
        | VFS_FLAG_VALID | VFS_FLAG_DIRTY;

    /* Write data if provided */
    if (buf && len > 0) {
        vfs_write(start_row, buf, len);
    }

    /* Increment file count */
    surface[1] = file_count + 1;
}

/**
 * vfs_find_free_row - Find the next free row in the surface.
 * Returns: the first row after all existing files, or -1 if full.
 */
static inline int vfs_find_free_row(void) {
    volatile uint32_t *surface = vfs_surface_ptr();
    uint32_t file_count = surface[1];
    int max_row = 1;

    for (uint32_t i = 0; i < file_count; i++) {
        uint32_t entry = surface[2 + i];
        int start_row = (int)(entry >> 16);
        uint32_t header = surface[start_row * VFS_COLS];
        int byte_count = (int)(header >> 16);
        int pixel_count = (byte_count + 3) / 4;
        int rows_needed = 1 + (pixel_count + 255) / 256;
        int end_row = start_row + rows_needed;
        if (end_row > max_row) max_row = end_row;
    }

    if (max_row >= VFS_ROWS - 1) return -1;
    return max_row;
}

/**
 * vfs_delete - Mark a file as deleted by clearing its valid flag.
 * @start_row: the row containing the file header pixel
 *
 * Clears the valid flag in the header pixel. The directory index
 * entry remains but is marked invalid. The space is not reclaimed.
 */
static inline void vfs_delete(int start_row) {
    volatile uint32_t *surface = vfs_surface_ptr();
    uint32_t header = surface[start_row * VFS_COLS];

    /* Clear valid flag, keep byte_count and hash */
    surface[start_row * VFS_COLS] = (header & ~0xFFu) | VFS_FLAG_DIRTY;

    /* Also remove from directory index by shifting entries */
    uint32_t file_count = surface[1];
    for (uint32_t i = 0; i < file_count; i++) {
        uint32_t entry = surface[2 + i];
        if ((entry >> 16) == (uint32_t)start_row) {
            /* Shift remaining entries down */
            for (uint32_t j = i; j < file_count - 1; j++) {
                surface[2 + j] = surface[2 + j + 1];
            }
            surface[2 + file_count - 1] = 0;
            surface[1] = file_count - 1;
            break;
        }
    }
}

/**
 * vfs_flush - Trigger write-back of dirty rows to host filesystem.
 * Writes to the VFS control register to signal the host.
 */
static inline void vfs_flush(void) {
    volatile uint32_t *ctrl = vfs_control_ptr();
    *ctrl = 1;  /* bit 0 = flush */
}

/**
 * vfs_get_file_count - Get the number of files in the VFS surface.
 * Returns: file count, or -1 if surface is invalid.
 */
static inline int vfs_get_file_count(void) {
    volatile uint32_t *surface = vfs_surface_ptr();
    if (surface[0] != VFS_MAGIC) return -1;
    return (int)surface[1];
}

/**
 * vfs_find_by_name - Find a file by its filename hash.
 * @name: filename to search for
 * @entries: pre-allocated array for directory listing
 * @max: size of entries array
 * Returns: index in entries array, or -1 if not found
 */
static inline int vfs_find_by_name(const char *name,
                                    struct vfs_entry *entries, int max) {
    int n = vfs_list(entries, max);
    if (n < 0) return -1;
    uint32_t target = vfs_fnv1a(name) & 0xFFFF;
    for (int i = 0; i < n; i++) {
        if (entries[i].name_hash == target) return i;
    }
    return -1;
}

/* ---- Utility: string output via SBI ---- */

static inline void vfs_puts(const char *s) {
    while (*s) sbi_console_putchar(*s++);
}

static inline void vfs_put_hex(uint32_t val) {
    const char *hex = "0123456789ABCDEF";
    vfs_puts("0x");
    for (int i = 28; i >= 0; i -= 4) {
        sbi_console_putchar(hex[(val >> i) & 0xF]);
    }
}

static inline void vfs_put_dec(uint32_t val) {
    char buf[12];
    int i = 0;
    if (val == 0) { sbi_console_putchar('0'); return; }
    while (val > 0) {
        buf[i++] = '0' + (val % 10);
        val /= 10;
    }
    while (i > 0) sbi_console_putchar(buf[--i]);
}

#endif /* VFS_PIXEL_H */
