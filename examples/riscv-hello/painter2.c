/*
 * painter2.c -- MMIO Framebuffer demo for Geometry OS
 *
 * Draws directly to the 256x256 MMIO framebuffer at 0x6000_0000.
 * Zero ecall overhead -- just load/store instructions.
 * "Pixels move pixels" -- this is what pixel-native means.
 *
 * Build:
 *   riscv64-linux-gnu-gcc -march=rv32imac_zicsr -mabi=ilp32 -nostdlib \
 *       -nostartfiles -T hello.ld -O2 -o painter2.elf crt0.S painter2.c -L. -lgeos
 */

#include "libgeos.h"

/* ---- Drawing via direct MMIO writes ---- */
static inline volatile uint32_t *fb_pixel(uint32_t x, uint32_t y) {
    return (volatile uint32_t *)(GEOS_FB_BASE + (y * GEOS_FB_WIDTH + x) * 4);
}

static void draw_pixel(uint32_t x, uint32_t y, uint32_t color) {
    if (x < GEOS_FB_WIDTH && y < GEOS_FB_HEIGHT) {
        *fb_pixel(x, y) = color;
    }
}

static void draw_rect(uint32_t x0, uint32_t y0, uint32_t w, uint32_t h, uint32_t color) {
    for (uint32_t y = y0; y < y0 + h && y < GEOS_FB_HEIGHT; y++)
        for (uint32_t x = x0; x < x0 + w && x < GEOS_FB_WIDTH; x++)
            *fb_pixel(x, y) = color;
}

/* ---- Entry point ---- */
void c_start(void) {
    volatile uint32_t *fb = (volatile uint32_t *)GEOS_FB_BASE;

    geos_puts("painter2: MMIO framebuffer at 0x60000000\n");
    geos_puts("painter2: 256x256 direct-write mode\n");

    /* Phase 1: Sweep gradient -- blue horizon */
    geos_puts("painter2: drawing sky gradient...\n");
    for (uint32_t y = 0; y < GEOS_FB_HEIGHT; y++) {
        for (uint32_t x = 0; x < GEOS_FB_WIDTH; x++) {
            uint8_t r = (uint8_t)((x * 128) / GEOS_FB_WIDTH);
            uint8_t g = (uint8_t)((y * 200) / GEOS_FB_HEIGHT);
            uint8_t b = (uint8_t)(128 + (x * 127) / GEOS_FB_WIDTH);
            *fb_pixel(x, y) = geos_rgb(r, g, b);
        }
    }
    geos_fb_present();
    geos_puts("painter2: gradient presented\n");

    /* Phase 2: Yellow sun (filled circle) at (200, 60), radius 40 */
    geos_puts("painter2: drawing sun...\n");
    for (int32_t y = -40; y <= 40; y++) {
        for (int32_t x = -40; x <= 40; x++) {
            if (x * x + y * y <= 40 * 40) {
                uint32_t px = (uint32_t)(200 + x);
                uint32_t py = (uint32_t)(60 + y);
                if (px < GEOS_FB_WIDTH && py < GEOS_FB_HEIGHT) {
                    uint8_t bright = (uint8_t)(255 - (x * x + y * y) * 255 / (40 * 40));
                    *fb_pixel(px, py) = geos_rgb(255, bright, 0);
                }
            }
        }
    }
    geos_fb_present();
    geos_puts("painter2: sun presented\n");

    /* Phase 3: Green hills at the bottom */
    geos_puts("painter2: drawing hills...\n");
    for (uint32_t x = 0; x < GEOS_FB_WIDTH; x++) {
        /* Sinusoidal hill line */
        uint32_t hill_y = (uint32_t)(180 + 30 * ((int32_t)((x * 7 / 32) % 13) - 6) / 6);
        for (uint32_t y = hill_y; y < GEOS_FB_HEIGHT; y++) {
            uint8_t g_val = (uint8_t)(80 + (y - hill_y) * 100 / (GEOS_FB_HEIGHT - hill_y));
            *fb_pixel(x, y) = geos_rgb(0, g_val, 0);
        }
    }
    geos_fb_present();
    geos_puts("painter2: hills presented\n");

    /* Phase 4: White border */
    geos_puts("painter2: drawing border...\n");
    for (uint32_t i = 0; i < GEOS_FB_WIDTH; i++) {
        draw_pixel(i, 0, geos_rgb(255, 255, 255));
        draw_pixel(i, 1, geos_rgb(255, 255, 255));
        draw_pixel(i, GEOS_FB_HEIGHT - 1, geos_rgb(255, 255, 255));
        draw_pixel(i, GEOS_FB_HEIGHT - 2, geos_rgb(255, 255, 255));
    }
    for (uint32_t i = 0; i < GEOS_FB_HEIGHT; i++) {
        draw_pixel(0, i, geos_rgb(255, 255, 255));
        draw_pixel(1, i, geos_rgb(255, 255, 255));
        draw_pixel(GEOS_FB_WIDTH - 1, i, geos_rgb(255, 255, 255));
        draw_pixel(GEOS_FB_WIDTH - 2, i, geos_rgb(255, 255, 255));
    }
    geos_fb_present();
    geos_puts("painter2: border presented\n");

    /* Verify readback: check pixel (0,0) is white */
    uint32_t test = *fb_pixel(0, 0);
    geos_puts("painter2: readback (0,0)=");
    if ((test >> 24) == 0xFF && ((test >> 16) & 0xFF) == 0xFF && ((test >> 8) & 0xFF) == 0xFF) {
        geos_puts("OK (white)\n");
    } else {
        geos_puts("FAIL (expected white)\n");
    }

    geos_puts("painter2: done! shutting down.\n");
    sbi_shutdown();
}
