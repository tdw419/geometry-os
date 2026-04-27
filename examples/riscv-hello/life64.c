/*
 * life64.c -- Conway's Game of Life (64x64) for live display demo
 *
 * 64x64 grid in a 256x256 framebuffer: each cell = 4x4 pixel block.
 * Toroidal wrap. 40% initial density. Runs infinitely with fb_present.
 * This is the default demo for the Phase B GUI bridge.
 *
 * Build:
 *   riscv64-linux-gnu-gcc -march=rv32imac_zicsr -mabi=ilp32 -nostdlib \
 *       -nostartfiles -T hello.ld -O2 -o life64.elf crt0.S life64.c
 */

#include <stdint.h>

/* ---- MMIO Framebuffer ---- */
#define FB_BASE        0x60000000u
#define FB_WIDTH       256
#define FB_HEIGHT      256
#define FB_CONTROL     (FB_BASE + (FB_WIDTH * FB_HEIGHT) * 4)

/* ---- Life grid ---- */
#define GRID_W         64
#define GRID_H         64
#define CELL_SIZE      4   /* 4x4 pixel block per cell */
#define DENSITY        40  /* 40% initial fill */

/* ---- SBI helpers ---- */
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

/* ---- Utility ---- */
static void puts(const char *s) {
    while (*s) sbi_console_putchar(*s++);
}

/* ---- Color helpers ---- */
static inline uint32_t rgb(uint8_t r, uint8_t g, uint8_t b) {
    return ((uint32_t)r << 24) | ((uint32_t)g << 16) | ((uint32_t)b << 8) | 0xFF;
}

/* ---- Shadow grids (512 bytes each for 64x64 = 4096 bits) ---- */
static uint8_t grid_a[GRID_W * GRID_H / 8];
static uint8_t grid_b[GRID_W * GRID_H / 8];

static inline int cell_get(uint8_t *grid, int x, int y) {
    x = x & (GRID_W - 1);
    y = y & (GRID_H - 1);
    uint32_t idx = y * GRID_W + x;
    return (grid[idx >> 3] >> (7 - (idx & 7))) & 1;
}

static inline void cell_set(uint8_t *grid, int x, int y, int val) {
    x = x & (GRID_W - 1);
    y = y & (GRID_H - 1);
    uint32_t idx = y * GRID_W + x;
    uint32_t byte_idx = idx >> 3;
    uint32_t bit = 7 - (idx & 7);
    if (val)
        grid[byte_idx] |= (1u << bit);
    else
        grid[byte_idx] &= ~(1u << bit);
}

static void fb_present(void) {
    *(volatile uint32_t *)FB_CONTROL = 1;
}

/* ---- Simple PRNG ---- */
static uint32_t rng_state = 0xCAFEF00Du;

static uint32_t xorshift32(void) {
    uint32_t x = rng_state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    rng_state = x;
    return x;
}

/* ---- Seed grid with 40% density ---- */
static void seed_grid(uint8_t *grid) {
    int y, x;
    for (y = 0; y < GRID_H; y++) {
        for (x = 0; x < GRID_W; x++) {
            int alive = (int)(xorshift32() % 100) < DENSITY;
            cell_set(grid, x, y, alive);
        }
    }
}

/* ---- Render grid to MMIO framebuffer ---- */
static void render_grid(uint8_t *grid) {
    int gy, gx, dy, dx;
    for (gy = 0; gy < GRID_H; gy++) {
        for (gx = 0; gx < GRID_W; gx++) {
            int alive = cell_get(grid, gx, gy);
            uint32_t color;
            if (alive) {
                /* Warm gradient based on position */
                uint8_t r = (uint8_t)(80 + (gx * 175) / GRID_W);
                uint8_t g = (uint8_t)(220 - (gy * 160) / GRID_H);
                uint8_t b = 60;
                color = rgb(r, g, b);
            } else {
                color = rgb(6, 6, 14);
            }
            /* Fill 4x4 pixel block */
            int base_y = gy * CELL_SIZE;
            int base_x = gx * CELL_SIZE;
            for (dy = 0; dy < CELL_SIZE; dy++) {
                for (dx = 0; dx < CELL_SIZE; dx++) {
                    *(volatile uint32_t *)(FB_BASE + ((base_y + dy) * FB_WIDTH + base_x + dx) * 4) = color;
                }
            }
        }
    }
    fb_present();
}

/* ---- Compute one generation ---- */
static void compute_generation(uint8_t *src, uint8_t *dst) {
    int y, x;
    for (y = 0; y < GRID_H; y++) {
        for (x = 0; x < GRID_W; x++) {
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

/* ---- Entry point: run forever ---- */
void c_start(void) {
    uint8_t *cur = grid_a;
    uint8_t *nxt = grid_b;
    uint32_t gen = 0;

    puts("life64: Conway's Game of Life 64x64\n");
    puts("life64: 40% density, infinite generations\n");

    seed_grid(cur);
    render_grid(cur);

    while (1) {
        compute_generation(cur, nxt);
        render_grid(nxt);

        uint8_t *tmp = cur;
        cur = nxt;
        nxt = tmp;

        gen++;
        if ((gen & 0xFF) == 0) {
            puts("life64: gen ");
            /* cheap put_dec for gen counter */
            if (gen == 0) { puts("0"); }
            else {
                char buf[12];
                int i = 0;
                uint32_t v = gen;
                while (v > 0) { buf[i++] = '0' + (v % 10); v /= 10; }
                while (i > 0) sbi_console_putchar(buf[--i]);
            }
            puts("\n");
        }
    }
}
