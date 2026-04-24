/*
 * guest_file_demo.c -- RISC-V Guest File Creation Demo
 *
 * Demonstrates guest file creation via VFS pixel surface.
 * Mock test: gcc -DGUEST_MOCK_TEST -o /tmp/test_demo guest_file_demo.c && /tmp/test_demo
 */
#include <stdint.h>
#include <string.h>
#include <stdio.h>

static uint32_t mock_surface[256 * 256];
#define VFS_PIXEL(row, col) mock_surface[(row) * 256 + (col)]
#define VFS_CONTROL mock_surface[256 * 256]
#define VFS_CTRL_FLUSH 0x01
#define VFS_MAGIC 0x50584653

static uint32_t vfs_fnv1a(const char *s) {
    uint32_t h = 0x811c9dc5;
    while (*s) { h ^= (uint8_t)*s++; h *= 0x01000193; }
    return h;
}

static void init_surface(void) {
    memset(mock_surface, 0, sizeof(mock_surface));
    VFS_PIXEL(0, 0) = VFS_MAGIC;
}

static uint16_t vfs_create(const char *name, uint32_t len, const void *data) {
    uint32_t count = VFS_PIXEL(0, 1);
    if (count >= 254 || !name || !name[0]) return 0;
    uint16_t free_row = 1;
    for (uint32_t i = 0; i < count; i++) {
        uint32_t entry = VFS_PIXEL(0, 2 + i);
        uint16_t sr = (uint16_t)(entry >> 16);
        if (sr > 0 && (VFS_PIXEL(sr, 0) & 0x01)) {
            if (sr + 1 > free_row) free_row = sr + 1;
        }
    }
    uint32_t hash = vfs_fnv1a(name);
    for (int i = 0; name[i] && i < 63; i++)
        VFS_PIXEL(free_row, 1 + i) = (uint8_t)name[i];
    VFS_PIXEL(free_row, 0) = ((len & 0xFFFF) << 16) | ((hash & 0xFF) << 8) | 0x03;
    const uint8_t *b = (const uint8_t *)data;
    for (uint32_t i = 0; i < (len + 3) / 4; i++) {
        uint32_t px = 0;
        for (int j = 0; j < 4; j++) {
            uint32_t idx = i * 4 + j;
            if (idx < len) px |= (uint32_t)b[idx] << (j * 8);
        }
        VFS_PIXEL(free_row, 64 + i) = px;
    }
    VFS_PIXEL(0, 2 + count) = ((uint32_t)free_row << 16) | (hash & 0xFFFF);
    VFS_PIXEL(0, 1) = count + 1;
    return free_row;
}

static int vfs_find(const char *name) {
    uint32_t count = VFS_PIXEL(0, 1), hash = vfs_fnv1a(name);
    for (uint32_t i = 0; i < count; i++) {
        uint32_t entry = VFS_PIXEL(0, 2 + i);
        if ((entry & 0xFFFF) == (hash & 0xFFFF)) {
            uint16_t sr = entry >> 16;
            if (sr > 0 && (VFS_PIXEL(sr, 0) & 1)) return sr;
        }
    }
    return 0;
}

int main(void) {
    int pass = 0, fail = 0;
    #define T(cond, msg) do { if (cond) pass++; else { printf("FAIL: %s\n", msg); fail++; } } while(0)

    /* Test 1: Init */
    init_surface();
    T(VFS_PIXEL(0, 0) == VFS_MAGIC, "magic");
    T(VFS_PIXEL(0, 1) == 0, "count");

    /* Test 2: Create */
    uint16_t row = vfs_create("hello.txt", 6, "Hello!");
    T(row == 1, "row");
    T(VFS_PIXEL(0, 1) == 1, "count2");
    T((VFS_PIXEL(0, 2) >> 16) == 1, "dir_idx");

    /* Test 3: Header & data */
    uint32_t h = VFS_PIXEL(1, 0);
    T((h >> 16) == 6, "byte_count");
    T((h & 0x03) == 0x03, "flags");
    T(VFS_PIXEL(1, 64) == 0x6C6C6548, "data1");
    T(VFS_PIXEL(1, 65) == 0x0000216F, "data2");

    /* Test 4: Find */
    T(vfs_find("hello.txt") == 1, "find");
    T(vfs_find("nope.txt") == 0, "find2");

    /* Test 5: Multiple */
    T(vfs_create("log.bin", 4, "\x01\x02\x03\x04") == 2, "row2");
    T(VFS_PIXEL(0, 1) == 2, "count3");

    /* Test 6: Filename */
    T(VFS_PIXEL(1, 1) == 'h', "name0");
    T(VFS_PIXEL(1, 2) == 'e', "name1");
    T(VFS_PIXEL(1, 9) == 't', "name8");

    printf("Results: %d/%d passed\n", pass, pass + fail);
    return fail ? 1 : 0;
}
