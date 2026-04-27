/*
 * painter.c -- SBI Pixel Extension demo for Geometry OS
 *
 * Draws a colorful pattern to the 64x64 SBI framebuffer.
 * Uses SBI extension "GEO\0" (0x47454F00):
 *   fn 1: sbi_pixel_set(x, y, color)
 *   fn 2: sbi_pixel_present()
 *   fn 3: sbi_pixel_get_info() -> (width, height)
 *
 * Build:
 *   riscv32-unknown-elf-gcc -march=rv32i -mabi=ilp32 -nostdlib \
 *       -T hello.ld -o painter.elf crt0.S painter.c
 */

#include <stdint.h>

/* ---- SBI ecall helpers ---- */

static inline long sbi_ecall(long a7, long a6, long a0, long a1, long a2) {
    register long _a0 __asm__("a0") = a0;
    register long _a1 __asm__("a1") = a1;
    register long _a2 __asm__("a2") = a2;
    register long _a7 __asm__("a7") = a7;
    register long _a6 __asm__("a6") = a6;
    __asm__ volatile("ecall"
        : "+r"(_a0), "+r"(_a1)
        : "r"(_a2), "r"(_a6), "r"(_a7)
        : "memory");
    return _a0;
}

static inline long sbi_console_putchar(int ch) {
    register long a0 __asm__("a0") = ch;
    register long a7 __asm__("a7") = 1;
    __asm__ volatile("ecall" : "+r"(a0) : "r"(a7) : "memory", "a1");
    return a0;
}

static inline long sbi_pixel_set(uint32_t x, uint32_t y, uint32_t color) {
    return sbi_ecall(0x47454F00L, 1L, (long)x, (long)y, (long)color);
}

static inline long sbi_pixel_present(void) {
    return sbi_ecall(0x47454F00L, 2L, 0, 0, 0);
}

static inline long sbi_pixel_get_info(uint32_t *w, uint32_t *h) {
    register long a0 __asm__("a0") = 0;
    register long a1 __asm__("a1") = 0;
    register long a7 __asm__("a7") = 0x47454F00L;
    register long a6 __asm__("a6") = 3L;
    __asm__ volatile("ecall"
        : "+r"(a0), "+r"(a1)
        : "r"(a6), "r"(a7)
        : "memory");
    if (w) *w = (uint32_t)a0;
    if (h) *h = (uint32_t)a1;
    return 0;
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

static void put_hex(uint32_t val) {
    const char *hex = "0123456789ABCDEF";
    puts("0x");
    for (int i = 28; i >= 0; i -= 4)
        sbi_console_putchar(hex[(val >> i) & 0xF]);
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

static uint32_t rgb(uint8_t r, uint8_t g, uint8_t b) {
    return ((uint32_t)r << 24) | ((uint32_t)g << 16) | ((uint32_t)b << 8) | 0xFF;
}

/* ---- Drawing primitives ---- */

static void draw_rect(uint32_t x0, uint32_t y0, uint32_t w, uint32_t h, uint32_t color) {
    for (uint32_t y = y0; y < y0 + h && y < 64; y++)
        for (uint32_t x = x0; x < x0 + w && x < 64; x++)
            sbi_pixel_set(x, y, color);
}

static void draw_pixel(uint32_t x, uint32_t y, uint32_t color) {
    sbi_pixel_set(x, y, color);
}

/* ---- Entry point ---- */

void c_start(void) {
    uint32_t fb_w = 0, fb_h = 0;
    sbi_pixel_get_info(&fb_w, &fb_h);

    puts("painter: framebuffer ");
    put_dec(fb_w);
    puts("x");
    put_dec(fb_h);
    puts("\n");

    /* Phase 1: Rainbow gradient background */
    puts("painter: drawing rainbow gradient...\n");
    for (uint32_t y = 0; y < fb_h; y++) {
        for (uint32_t x = 0; x < fb_w; x++) {
            uint8_t r = (uint8_t)((x * 255) / fb_w);
            uint8_t g = (uint8_t)((y * 255) / fb_h);
            uint8_t b = (uint8_t)(((x + y) * 255) / (fb_w + fb_h));
            sbi_pixel_set(x, y, rgb(r, g, b));
        }
    }
    sbi_pixel_present();
    puts("painter: gradient presented\n");

    /* Phase 2: Checkerboard overlay in center */
    puts("painter: drawing checkerboard...\n");
    for (uint32_t y = 16; y < 48; y++) {
        for (uint32_t x = 16; x < 48; x++) {
            if (((x - 16) / 4 + (y - 16) / 4) % 2 == 0) {
                sbi_pixel_set(x, y, rgb(255, 255, 255));
            } else {
                sbi_pixel_set(x, y, rgb(0, 0, 0));
            }
        }
    }
    sbi_pixel_present();
    puts("painter: checkerboard presented\n");

    /* Phase 3: Diagonal line */
    puts("painter: drawing diagonal...\n");
    for (uint32_t i = 0; i < fb_w && i < fb_h; i++) {
        sbi_pixel_set(i, i, rgb(255, 0, 0));
        sbi_pixel_set(fb_w - 1 - i, i, rgb(0, 255, 0));
    }
    sbi_pixel_present();
    puts("painter: diagonal presented\n");

    /* Phase 4: Border */
    puts("painter: drawing border...\n");
    for (uint32_t i = 0; i < fb_w; i++) {
        sbi_pixel_set(i, 0, rgb(255, 255, 0));
        sbi_pixel_set(i, fb_h - 1, rgb(255, 255, 0));
    }
    for (uint32_t i = 0; i < fb_h; i++) {
        sbi_pixel_set(0, i, rgb(0, 255, 255));
        sbi_pixel_set(fb_w - 1, i, rgb(0, 255, 255));
    }
    sbi_pixel_present();
    puts("painter: border presented\n");

    puts("painter: done! shutting down.\n");
    sbi_shutdown();
}
