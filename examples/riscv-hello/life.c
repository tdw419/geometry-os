/*
 * life.c -- Conway's Game of Life on the MMIO Framebuffer
 *
 * Proves the read path: reads previous frame, computes next, writes back.
 * This is the canonical "pixels driving pixels" demo.
 *
 * Optimized for RV32IM: hardware multiply/divide available.
 * Power-of-two FB_WIDTH=256 means x%256 -> x&0xFF, y*256 -> y<<8.
 *
 * Build:
 *   riscv64-linux-gnu-gcc -march=rv32imac_zicsr -mabi=ilp32 -nostdlib \
 *       -nostartfiles -T hello.ld -O2 -o life.elf crt0.S life.c -L. -lgeos
 */

#include "libgeos.h"

/* ---- Local framebuffer helpers (program-specific optimizations) ---- */
#define FB_SIZE        (GEOS_FB_WIDTH * GEOS_FB_HEIGHT)
#define FB_MASK_X      (GEOS_FB_WIDTH - 1)   /* 0xFF for x wrap */
#define FB_SHIFT_Y     8                      /* log2(256) for y stride */

static inline volatile uint32_t *fb_pixel(uint32_t x, uint32_t y) {
    return (volatile uint32_t *)(GEOS_FB_BASE + ((y << FB_SHIFT_Y) + x) * 4);
}

/* ---- Shadow bit-grids for double-buffering (8KB each) ---- */
static uint8_t grid_a[FB_SIZE / 8];
static uint8_t grid_b[FB_SIZE / 8];

static inline int cell_get(uint8_t *grid, uint32_t x, uint32_t y) {
    /* Toroidal wrap via bitmask (FB_WIDTH is power of 2) */
    x = x & FB_MASK_X;
    y = y & (GEOS_FB_HEIGHT - 1);
    uint32_t idx = (y << FB_SHIFT_Y) + x;
    return (grid[idx >> 3] >> (7 - (idx & 7))) & 1;
}

static inline void cell_set(uint8_t *grid, uint32_t x, uint32_t y, int val) {
    x = x & FB_MASK_X;
    y = y & (GEOS_FB_HEIGHT - 1);
    uint32_t idx = (y << FB_SHIFT_Y) + x;
    uint32_t byte_idx = idx >> 3;
    uint32_t bit = 7 - (idx & 7);
    if (val)
        grid[byte_idx] |= (1u << bit);
    else
        grid[byte_idx] &= ~(1u << bit);
}

/* ---- Simple PRNG for initial seeding ---- */
static uint32_t rng_state = 0xDEADBEEFu;

static uint32_t xorshift32(void) {
    uint32_t x = rng_state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    rng_state = x;
    return x;
}

/* ---- Initialize with random pattern in center region ---- */
static void seed_grid(uint8_t *grid) {
    uint32_t cx = (GEOS_FB_WIDTH >> 1) - 64;
    uint32_t cy = (GEOS_FB_HEIGHT >> 1) - 64;
    uint32_t y, x;
    for (y = 0; y < 128; y++) {
        for (x = 0; x < 128; x++) {
            int alive = (xorshift32() % 10) < 3;
            cell_set(grid, cx + x, cy + y, alive);
        }
    }
}

/* ---- Render grid to MMIO framebuffer (WRITE path) ---- */
static void render_grid(uint8_t *grid) {
    uint32_t y, x;
    for (y = 0; y < GEOS_FB_HEIGHT; y++) {
        uint32_t y_offset = y << FB_SHIFT_Y;
        for (x = 0; x < GEOS_FB_WIDTH; x++) {
            int alive = cell_get(grid, x, y);
            if (alive) {
                /* Warm color gradient */
                *(volatile uint32_t *)(GEOS_FB_BASE + (y_offset + x) * 4) =
                    geos_rgb((uint8_t)(50 + (x * 205) / GEOS_FB_WIDTH),
                        (uint8_t)(200 - (y * 150) / GEOS_FB_HEIGHT),
                        50);
            } else {
                *(volatile uint32_t *)(GEOS_FB_BASE + (y_offset + x) * 4) =
                    geos_rgb(8, 8, 16);
            }
        }
    }
    geos_fb_present();
}

/* ---- READBACK: read framebuffer pixels back into grid ---- */
static void readback_from_fb(uint8_t *grid) {
    uint32_t y, x;
    for (y = 0; y < GEOS_FB_HEIGHT; y++) {
        uint32_t y_offset = y << FB_SHIFT_Y;
        for (x = 0; x < GEOS_FB_WIDTH; x++) {
            uint32_t pixel = *(volatile uint32_t *)(GEOS_FB_BASE + (y_offset + x) * 4);
            int alive = ((pixel >> 24) & 0xFF) > 32 ||
                        ((pixel >> 16) & 0xFF) > 32 ||
                        ((pixel >> 8) & 0xFF) > 32;
            cell_set(grid, x, y, alive);
        }
    }
}

/* ---- Compute one generation ---- */
static void compute_generation(uint8_t *src, uint8_t *dst) {
    uint32_t y, x;
    for (y = 0; y < GEOS_FB_HEIGHT; y++) {
        for (x = 0; x < GEOS_FB_WIDTH; x++) {
            int n = 0;
            int dy, dx;
            for (dy = -1; dy <= 1; dy++) {
                for (dx = -1; dx <= 1; dx++) {
                    if (dx == 0 && dy == 0) continue;
                    n += cell_get(src, x + dx, y + dy);
                }
            }
            int alive = cell_get(src, x, y);
            if (alive) {
                cell_set(dst, x, y, (n == 2 || n == 3) ? 1 : 0);
            } else {
                cell_set(dst, x, y, (n == 3) ? 1 : 0);
            }
        }
    }
}

/* ---- Entry point ---- */
#define NUM_GENERATIONS 10

void c_start(void) {
    uint8_t *cur = grid_a;
    uint8_t *nxt = grid_b;
    uint32_t gen;

    geos_puts("life: Conway's Game of Life -- MMIO framebuffer\n");
    geos_puts("life: 256x256 toroidal, ");
    geos_put_dec(NUM_GENERATIONS);
    geos_puts(" gens\n");

    geos_puts("life: seeding...\n");
    seed_grid(cur);

    geos_puts("life: render gen 0\n");
    render_grid(cur);

    /* READBACK TEST */
    geos_puts("life: readback from MMIO...\n");
    readback_from_fb(cur);

    for (gen = 1; gen <= NUM_GENERATIONS; gen++) {
        geos_puts("life: gen ");
        geos_put_dec(gen);
        geos_puts("...");

        compute_generation(cur, nxt);
        render_grid(nxt);

        uint8_t *tmp = cur;
        cur = nxt;
        nxt = tmp;

        geos_puts("ok\n");
    }

    /* Final readback verification */
    geos_puts("life: final count...\n");
    uint32_t alive_count = 0;
    uint32_t y, x;
    for (y = 0; y < GEOS_FB_HEIGHT; y++) {
        uint32_t y_offset = y << FB_SHIFT_Y;
        for (x = 0; x < GEOS_FB_WIDTH; x++) {
            uint32_t pixel = *(volatile uint32_t *)(GEOS_FB_BASE + (y_offset + x) * 4);
            if (((pixel >> 24) & 0xFF) > 32)
                alive_count++;
        }
    }
    geos_puts("life: alive=");
    geos_put_dec(alive_count);
    geos_puts("\n");

    if (alive_count > 0) {
        geos_puts("life: READBACK OK\n");
    } else {
        geos_puts("life: READBACK FAIL\n");
    }

    geos_puts("life: shutdown.\n");
    sbi_shutdown();
}
