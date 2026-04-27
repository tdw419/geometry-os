/*
 * painter2.c -- MMIO Framebuffer demo for Geometry OS
 *
 * Draws directly to the 256x256 MMIO framebuffer at 0x6000_0000.
 * Zero ecall overhead -- just load/store instructions.
 * "Pixels move pixels" -- this is what pixel-native means.
 *
 * Build:
 *   riscv32-unknown-elf-gcc -march=rv32i -mabi=ilp32 -nostdlib \
 *       -T hello.ld -o painter2.elf crt0.S painter2.c
 */

#include <stdint.h>

/* ---- MMIO Framebuffer ---- */
#define FB_BASE        0x60000000u
#define FB_WIDTH       256
#define FB_HEIGHT      256
#define FB_CONTROL     (FB_BASE + FB_WIDTH * FB_HEIGHT * 4)

/* ---- SBI helpers (for UART output only) ---- */
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

/* ---- Color helpers ---- */
static inline uint32_t rgb(uint8_t r, uint8_t g, uint8_t b) {
    return ((uint32_t)r << 24) | ((uint32_t)g << 16) | ((uint32_t)b << 8) | 0xFF;
}

/* ---- Drawing via direct MMIO writes ---- */
static inline volatile uint32_t *fb_pixel(uint32_t x, uint32_t y) {
    return (volatile uint32_t *)(FB_BASE + (y * FB_WIDTH + x) * 4);
}

static void draw_pixel(uint32_t x, uint32_t y, uint32_t color) {
    if (x < FB_WIDTH && y < FB_HEIGHT) {
        *fb_pixel(x, y) = color;
    }
}

static void draw_rect(uint32_t x0, uint32_t y0, uint32_t w, uint32_t h, uint32_t color) {
    for (uint32_t y = y0; y < y0 + h && y < FB_HEIGHT; y++)
        for (uint32_t x = x0; x < x0 + w && x < FB_WIDTH; x++)
            *fb_pixel(x, y) = color;
}

static void fb_present(void) {
    *(volatile uint32_t *)FB_CONTROL = 1;
}

/* ---- Entry point ---- */
void c_start(void) {
    volatile uint32_t *fb = (volatile uint32_t *)FB_BASE;

    puts("painter2: MMIO framebuffer at 0x");
    /* Quick hex of FB_BASE */
    puts("60000000\n");
    puts("painter2: 256x256 direct-write mode\n");

    /* Phase 1: Sweep gradient -- blue horizon */
    puts("painter2: drawing sky gradient...\n");
    for (uint32_t y = 0; y < FB_HEIGHT; y++) {
        for (uint32_t x = 0; x < FB_WIDTH; x++) {
            uint8_t r = (uint8_t)((x * 128) / FB_WIDTH);
            uint8_t g = (uint8_t)((y * 200) / FB_HEIGHT);
            uint8_t b = (uint8_t)(128 + (x * 127) / FB_WIDTH);
            *fb_pixel(x, y) = rgb(r, g, b);
        }
    }
    fb_present();
    puts("painter2: gradient presented\n");

    /* Phase 2: Yellow sun (filled circle) at (200, 60), radius 40 */
    puts("painter2: drawing sun...\n");
    for (int32_t y = -40; y <= 40; y++) {
        for (int32_t x = -40; x <= 40; x++) {
            if (x * x + y * y <= 40 * 40) {
                uint32_t px = (uint32_t)(200 + x);
                uint32_t py = (uint32_t)(60 + y);
                if (px < FB_WIDTH && py < FB_HEIGHT) {
                    uint8_t bright = (uint8_t)(255 - (x * x + y * y) * 255 / (40 * 40));
                    *fb_pixel(px, py) = rgb(255, bright, 0);
                }
            }
        }
    }
    fb_present();
    puts("painter2: sun presented\n");

    /* Phase 3: Green hills at the bottom */
    puts("painter2: drawing hills...\n");
    for (uint32_t x = 0; x < FB_WIDTH; x++) {
        /* Sinusoidal hill line */
        uint32_t hill_y = (uint32_t)(180 + 30 * ((int32_t)((x * 7 / 32) % 13) - 6) / 6);
        for (uint32_t y = hill_y; y < FB_HEIGHT; y++) {
            uint8_t g_val = (uint8_t)(80 + (y - hill_y) * 100 / (FB_HEIGHT - hill_y));
            *fb_pixel(x, y) = rgb(0, g_val, 0);
        }
    }
    fb_present();
    puts("painter2: hills presented\n");

    /* Phase 4: White border */
    puts("painter2: drawing border...\n");
    for (uint32_t i = 0; i < FB_WIDTH; i++) {
        draw_pixel(i, 0, rgb(255, 255, 255));
        draw_pixel(i, 1, rgb(255, 255, 255));
        draw_pixel(i, FB_HEIGHT - 1, rgb(255, 255, 255));
        draw_pixel(i, FB_HEIGHT - 2, rgb(255, 255, 255));
    }
    for (uint32_t i = 0; i < FB_HEIGHT; i++) {
        draw_pixel(0, i, rgb(255, 255, 255));
        draw_pixel(1, i, rgb(255, 255, 255));
        draw_pixel(FB_WIDTH - 1, i, rgb(255, 255, 255));
        draw_pixel(FB_WIDTH - 2, i, rgb(255, 255, 255));
    }
    fb_present();
    puts("painter2: border presented\n");

    /* Verify readback: check pixel (0,0) is white */
    uint32_t test = *fb_pixel(0, 0);
    puts("painter2: readback (0,0)=");
    if ((test >> 24) == 0xFF && ((test >> 16) & 0xFF) == 0xFF && ((test >> 8) & 0xFF) == 0xFF) {
        puts("OK (white)\n");
    } else {
        /* Print hex */
        puts("FAIL (expected white)\n");
    }

    puts("painter2: done! shutting down.\n");
    sbi_shutdown();
}
