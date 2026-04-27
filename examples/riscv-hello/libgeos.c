/*
 * libgeos.c -- shared bare-metal primitives for Geometry OS guest programs
 *
 * SBI console I/O and utility functions. Linked as libgeos.a.
 * Framebuffer inline helpers are in libgeos.h (zero function-call overhead).
 *
 * Build: riscv64-linux-gnu-gcc -c -march=rv32imac_zicsr -mabi=ilp32 -O2 libgeos.c
 *        riscv64-linux-gnu-ar rcs libgeos.a libgeos.o
 */

#include "libgeos.h"

/* ---- SBI helpers ---- */

long sbi_console_putchar(int ch) {
    register long a0 __asm__("a0") = ch;
    register long a7 __asm__("a7") = 1;
    __asm__ volatile("ecall" : "+r"(a0) : "r"(a7) : "memory", "a1");
    return a0;
}

__attribute__((noreturn)) void sbi_shutdown(void) {
    register long a7 __asm__("a7") = 8;
    __asm__ volatile("ecall" : : "r"(a7) : "memory", "a0", "a1");
    __builtin_unreachable();
}

long sbi_console_getchar(void) {
    register long a0 __asm__("a0") = 0;
    register long a7 __asm__("a7") = 2;
    __asm__ volatile("ecall" : "+r"(a0) : "r"(a7) : "memory", "a1");
    return a0;
}

/* Read one character from SBI console, blocking until available. */
char geos_getchar(void) {
    long ch;
    while ((ch = sbi_console_getchar()) < 0) {
        /* spin until a character is available */
    }
    return (char)ch;
}

/* ---- Console output ---- */

void geos_puts(const char *s) {
    while (*s) sbi_console_putchar(*s++);
}

void geos_put_dec(uint32_t val) {
    if (val == 0) {
        sbi_console_putchar('0');
        return;
    }
    char buf[12];
    int i = 0;
    while (val > 0) {
        buf[i++] = '0' + (val % 10);
        val /= 10;
    }
    while (i > 0) sbi_console_putchar(buf[--i]);
}

void geos_put_hex(uint32_t val) {
    static const char hex[] = "0123456789ABCDEF";
    sbi_console_putchar('0');
    sbi_console_putchar('x');
    for (int i = 28; i >= 0; i -= 4) {
        sbi_console_putchar(hex[(val >> i) & 0xF]);
    }
}

/* ---- VFS Pixel Surface: Canvas Save/Load ---- */

int geos_save_canvas(void) {
    volatile uint32_t *fb = (volatile uint32_t *)GEOS_FB_BASE;
    volatile uint32_t *vfs = (volatile uint32_t *)GEOS_VFS_BASE;

    /* Copy framebuffer rows 0..254 to VFS rows 1..255.
       Each row is 256 pixels (1024 bytes). We do word-by-word copies. */
    for (int row = 0; row < GEOS_CANVAS_MAX_ROWS; row++) {
        for (int col = 0; col < GEOS_FB_WIDTH; col++) {
            vfs[(row + 1) * GEOS_FB_WIDTH + col] = fb[row * GEOS_FB_WIDTH + col];
        }
    }

    /* Set canvas marker at VFS (row 0, col 255) */
    vfs[GEOS_CANVAS_MARKER_COL] = GEOS_CANVAS_MARKER;

    return 0;
}

int geos_load_canvas(void) {
    volatile uint32_t *fb = (volatile uint32_t *)GEOS_FB_BASE;
    volatile uint32_t *vfs = (volatile uint32_t *)GEOS_VFS_BASE;

    /* Check for canvas marker */
    if (vfs[GEOS_CANVAS_MARKER_COL] != GEOS_CANVAS_MARKER) {
        return -1;
    }

    /* Copy VFS rows 1..255 into framebuffer rows 0..254 */
    for (int row = 0; row < GEOS_CANVAS_MAX_ROWS; row++) {
        for (int col = 0; col < GEOS_FB_WIDTH; col++) {
            fb[row * GEOS_FB_WIDTH + col] = vfs[(row + 1) * GEOS_FB_WIDTH + col];
        }
    }

    geos_fb_present();
    return 0;
}
