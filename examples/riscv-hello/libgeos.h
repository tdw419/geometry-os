/*
 * libgeos.h -- shared bare-metal primitives for Geometry OS guest programs
 *
 * SBI console I/O, framebuffer helpers, and utility functions shared across
 * life, life64, painter2, hello, sh, cat, etc.
 *
 * Usage:
 *   #include "libgeos.h"
 *   // then link with -lgeos (libgeos.a)
 *
 * Build: libgeos.a is built by build.sh or Makefile alongside your program.
 */

#ifndef LIBGEOS_H
#define LIBGEOS_H

#include <stdint.h>

/* ---- SBI helpers ---- */

/* Write a character to the SBI console. Returns the SBI result (0 on success). */
long sbi_console_putchar(int ch);

/* Shut down the machine via SBI. Does not return. */
__attribute__((noreturn)) void sbi_shutdown(void);

/* Blocking read of one character from SBI console. Returns -1 on failure. */
long sbi_console_getchar(void);

/* Blocking read of one character (spins until available). */
char geos_getchar(void);

/* ---- Console output ---- */

/* Print a null-terminated string to the SBI console. */
void geos_puts(const char *s);

/* Print an unsigned decimal number to the SBI console. */
void geos_put_dec(uint32_t val);

/* Print an unsigned hex number (0x-prefixed) to the SBI console. */
void geos_put_hex(uint32_t val);

/* ---- Framebuffer helpers ---- */

/* MMIO framebuffer base address (256x256 RGBA). */
#define GEOS_FB_BASE        0x60000000u
#define GEOS_FB_WIDTH       256
#define GEOS_FB_HEIGHT      256
#define GEOS_FB_CONTROL     (GEOS_FB_BASE + (GEOS_FB_WIDTH * GEOS_FB_HEIGHT) * 4)

/* Pack RGB channels into framebuffer pixel format (0xRRGGBBAA, alpha=0xFF). */
static inline uint32_t geos_rgb(uint8_t r, uint8_t g, uint8_t b) {
    return ((uint32_t)r << 24) | ((uint32_t)g << 16) | ((uint32_t)b << 8) | 0xFF;
}

/* Signal the host to display the current framebuffer contents. */
static inline void geos_fb_present(void) {
    *(volatile uint32_t *)GEOS_FB_CONTROL = 1;
}

/* Write a pixel to the framebuffer at (x, y). */
static inline void geos_fb_pixel(uint32_t x, uint32_t y, uint32_t color) {
    if (x < GEOS_FB_WIDTH && y < GEOS_FB_HEIGHT) {
        *(volatile uint32_t *)(GEOS_FB_BASE + (y * GEOS_FB_WIDTH + x) * 4) = color;
    }
}

/* Read a pixel from the framebuffer at (x, y). */
static inline uint32_t geos_fb_read(uint32_t x, uint32_t y) {
    if (x < GEOS_FB_WIDTH && y < GEOS_FB_HEIGHT) {
        return *(volatile uint32_t *)(GEOS_FB_BASE + (y * GEOS_FB_WIDTH + x) * 4);
    }
    return 0;
}

/* ---- Timing helpers ---- */

/* CLINT mtime register (64-bit free-running counter).
 *
 * IMPORTANT: mtime increments once per interpreted instruction, NOT per
 * wall-clock millisecond. At ~52 MIPS, 1ms ≈ 52000 ticks, but this is an
 * approximation. On a faster host the interpreter runs faster, so
 * geos_wait_ms(16) completes in less than 16ms wall-clock time. On a slower
 * host it takes longer.
 *
 * This means:
 *   - geos_wait_ms() is fine for "give programs a sense of relative time
 *     so they don't spin-paint maximally"
 *   - geos_wait_ms() is NOT suitable for "60fps wall-clock-locked animation"
 *   - Real wall-clock sync requires a future phase (e.g. host-side VSync
 *     signal or real-time clock MMIO)
 *
 * Read as two 32-bit halves (RISC-V is little-endian). */
#define GEOS_CLINT_MTIME  0x0200BFF8u

/* Read the full 64-bit mtime counter. */
static inline uint64_t geos_mtime(void) {
    uint32_t lo, hi;
    /* Read high word twice to handle wrap-around */
    do {
        hi = *(volatile uint32_t *)(GEOS_CLINT_MTIME + 4);
        lo = *(volatile uint32_t *)(GEOS_CLINT_MTIME);
    } while (hi != *(volatile uint32_t *)(GEOS_CLINT_MTIME + 4));
    return ((uint64_t)hi << 32) | lo;
}

/* Spin-wait until mtime >= target. */
static inline void geos_wait_until(uint64_t target) {
    while (geos_mtime() < target) {
        /* busy loop */
    }
}

/* Spin-wait for N ticks from now. */
static inline void geos_wait_ticks(uint64_t ticks) {
    uint64_t target = geos_mtime() + ticks;
    geos_wait_until(target);
}

/* Approximate ticks per millisecond at 52 MIPS. */
#define GEOS_TICKS_PER_MS 52000

/* Spin-wait for N milliseconds (approximate). */
static inline void geos_wait_ms(uint32_t ms) {
    geos_wait_ticks((uint64_t)ms * GEOS_TICKS_PER_MS);
}

/* ---- VFS Pixel Surface ---- */

/* MMIO base for the VFS Pixel Surface (256x256 RGBA, same format as framebuffer). */
#define GEOS_VFS_BASE       0x70000000u
#define GEOS_VFS_SIZE       (256 * 256 * 4)
#define GEOS_VFS_CONTROL    (GEOS_VFS_BASE + GEOS_VFS_SIZE)

/* Canvas save marker: written to VFS row 0, column 255 when canvas is saved.
   Row 0 is the VFS directory index. Column 255 is unused by the directory
   (directory uses pixels 0..1+file_count, max ~256 for 254 files).
   Value 0x43414E56 = "CANV" in ASCII. */
#define GEOS_CANVAS_MARKER  0x43414E56u
#define GEOS_CANVAS_MARKER_COL 255

/* Maximum number of framebuffer rows that can be saved (rows 1-255 of VFS). */
#define GEOS_CANVAS_MAX_ROWS 255

/* Save the framebuffer canvas to the VFS Pixel Surface.
   Saves rows 0..254 of the framebuffer to VFS rows 1..255.
   Row 255 of the framebuffer (typically UI chrome) is not saved.
   Sets a marker pixel at VFS (0, 255) to signal valid data.
   Returns 0 on success, -1 on error. */
int geos_save_canvas(void);

/* Load a previously saved canvas from the VFS Pixel Surface.
   Checks for the canvas marker at VFS (0, 255). If present,
   copies VFS rows 1..255 into framebuffer rows 0..254 and presents.
   Returns 0 on success, -1 if no saved canvas found. */
int geos_load_canvas(void);

#endif /* LIBGEOS_H */
