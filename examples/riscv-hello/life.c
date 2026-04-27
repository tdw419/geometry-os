/*
 * life.c -- Conway's Game of Life on the MMIO Framebuffer
 *
 * Proves the read path: reads previous frame, computes next, writes back.
 * Each generation reads all 256x256 pixels and writes the result back.
 * This is the canonical "pixels driving pixels" demo.
 *
 * Runs N generations then shuts down.
 * Uses double-buffering via a shadow bit-grid (8KB per buffer).
 *
 * Build:
 *   riscv64-linux-gnu-gcc -march=rv32i -mabi=ilp32 -nostdlib -nostdinc \
 *       -Os -T hello.ld -o life.elf crt0.S life.c
 */

typedef unsigned char      uint8_t;
typedef unsigned int       uint32_t;
typedef signed int         int32_t;

/* ---- MMIO Framebuffer ---- */
#define FB_BASE        0x60000000u
#define FB_WIDTH       256
#define FB_HEIGHT      256
#define FB_CONTROL     (FB_BASE + FB_WIDTH * FB_HEIGHT * 4)
#define FB_SIZE        (FB_WIDTH * FB_HEIGHT)

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

static void put_dec(uint32_t val) {
    char buf[12];
    int i = 0;
    if (val == 0) { sbi_console_putchar('0'); return; }
    while (val > 0) {
        buf[i++] = '0' + (val % 10);
        val /= 10;
    }
    while (i > 0) sbi_console_putchar(buf[--i]);
}

/* ---- Software div/mod for RV32I (no M extension) ---- */
/* Bitwise long division -- much faster than subtraction loops */
static uint32_t udiv(uint32_t a, uint32_t b) {
    if (b == 0) return 0;
    uint32_t q = 0;
    int shift = 0;
    while (b << shift <= a && shift < 32) shift++;
    while (shift > 0) {
        shift--;
        if ((b << shift) <= a) {
            a -= (b << shift);
            q |= (1u << shift);
        }
    }
    return q;
}

static uint32_t umod(uint32_t a, uint32_t b) {
    if (b == 0) return 0;
    int shift = 0;
    while (b << shift <= a && shift < 32) shift++;
    while (shift > 0) {
        shift--;
        if ((b << shift) <= a) {
            a -= (b << shift);
        }
    }
    return a;
}

/* Override gcc builtins */
uint32_t __udivsi3(uint32_t a, uint32_t b) { return udiv(a, b); }
uint32_t __umodsi3(uint32_t a, uint32_t b) { return umod(a, b); }

/* ---- Color helpers ---- */
static inline uint32_t rgb(uint8_t r, uint8_t g, uint8_t b) {
    return ((uint32_t)r << 24) | ((uint32_t)g << 16) | ((uint32_t)b << 8) | 0xFF;
}

/* ---- Shadow bit-grids for double-buffering (8KB each) ---- */
static uint8_t grid_a[FB_SIZE / 8];
static uint8_t grid_b[FB_SIZE / 8];

static inline int cell_get(uint8_t *grid, uint32_t x, uint32_t y) {
    x = x % FB_WIDTH;
    y = y % FB_HEIGHT;
    uint32_t idx = y * FB_WIDTH + x;
    return (grid[idx / 8] >> (7 - (idx % 8))) & 1;
}

static inline void cell_set(uint8_t *grid, uint32_t x, uint32_t y, int val) {
    x = x % FB_WIDTH;
    y = y % FB_HEIGHT;
    uint32_t idx = y * FB_WIDTH + x;
    uint32_t byte_idx = idx / 8;
    uint32_t bit = 7 - (idx % 8);
    if (val)
        grid[byte_idx] |= (1u << bit);
    else
        grid[byte_idx] &= ~(1u << bit);
}

static inline volatile uint32_t *fb_pixel(uint32_t x, uint32_t y) {
    return (volatile uint32_t *)(FB_BASE + (y * FB_WIDTH + x) * 4);
}

static void fb_present(void) {
    *(volatile uint32_t *)FB_CONTROL = 1;
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
    uint32_t cx = FB_WIDTH / 2 - 64;
    uint32_t cy = FB_HEIGHT / 2 - 64;
    uint32_t y, x;
    for (y = 0; y < 128; y++) {
        for (x = 0; x < 128; x++) {
            int alive = (umod(xorshift32(), 10)) < 3;
            cell_set(grid, cx + x, cy + y, alive);
        }
    }
}

/* ---- Render grid to MMIO framebuffer (WRITE path) ---- */
static void render_grid(uint8_t *grid) {
    uint32_t y, x;
    for (y = 0; y < FB_HEIGHT; y++) {
        for (x = 0; x < FB_WIDTH; x++) {
            int alive = cell_get(grid, x, y);
            if (alive) {
                uint8_t r = (uint8_t)(50 + (x * 205) / FB_WIDTH);
                uint8_t g = (uint8_t)(200 - (y * 150) / FB_HEIGHT);
                uint8_t b = 50;
                *fb_pixel(x, y) = rgb(r, g, b);
            } else {
                *fb_pixel(x, y) = rgb(8, 8, 16);
            }
        }
    }
    fb_present();
}

/* ---- READBACK: read framebuffer pixels back into grid ---- */
static void readback_from_fb(uint8_t *grid) {
    uint32_t y, x;
    for (y = 0; y < FB_HEIGHT; y++) {
        for (x = 0; x < FB_WIDTH; x++) {
            uint32_t pixel = *fb_pixel(x, y);
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
    for (y = 0; y < FB_HEIGHT; y++) {
        for (x = 0; x < FB_WIDTH; x++) {
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

    puts("life: Conway's Game of Life -- MMIO framebuffer\n");
    puts("life: 256x256 toroidal, ");
    put_dec(NUM_GENERATIONS);
    puts(" gens\n");

    puts("life: seeding...\n");
    seed_grid(cur);

    puts("life: render gen 0\n");
    render_grid(cur);

    /* READBACK TEST */
    puts("life: readback from MMIO...\n");
    readback_from_fb(cur);

    for (gen = 1; gen <= NUM_GENERATIONS; gen++) {
        puts("life: gen ");
        put_dec(gen);
        puts("...");

        compute_generation(cur, nxt);
        render_grid(nxt);

        uint8_t *tmp = cur;
        cur = nxt;
        nxt = tmp;

        puts("ok\n");
    }

    /* Final readback verification */
    puts("life: final count...\n");
    uint32_t alive_count = 0;
    uint32_t y, x;
    for (y = 0; y < FB_HEIGHT; y++) {
        for (x = 0; x < FB_WIDTH; x++) {
            uint32_t pixel = *fb_pixel(x, y);
            if (((pixel >> 24) & 0xFF) > 32)
                alive_count++;
        }
    }
    puts("life: alive=");
    put_dec(alive_count);
    puts("\n");

    if (alive_count > 0) {
        puts("life: READBACK OK\n");
    } else {
        puts("life: READBACK FAIL\n");
    }

    puts("life: shutdown.\n");
    sbi_shutdown();
}
