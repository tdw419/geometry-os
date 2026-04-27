/*
 * keytest.c -- Verify GUI-to-RISC-V input bridge
 *
 * Loops on geos_getchar(), prints each received byte as hex + ASCII.
 * Displays a green header bar on the framebuffer so you know it's running.
 * Each keypress draws a colored stripe so the feedback is visual too.
 *
 * Build: ./build.sh keytest.c keytest.elf
 * Run:   In GeOS, type: riscv_run keytest.elf
 * Test:  Type keys in the GeOS window, confirm they show on screen.
 */

#include "libgeos.h"

void c_start(void) {
    geos_puts("keytest: input bridge verifier\n");
    geos_puts("type keys -- each byte should echo as hex+ascii\n");

    /* Draw green header bar to show we're alive */
    for (int x = 0; x < GEOS_FB_WIDTH; x++) {
        for (int y = 0; y < 8; y++) {
            geos_fb_pixel(x, y, geos_rgb(0, 180, 0));
        }
    }
    geos_fb_present();

    int row = 10;
    int count = 0;

    while (1) {
        char ch = geos_getchar();

        /* Print hex */
        geos_puts("[");
        geos_put_hex((uint32_t)(unsigned char)ch);
        geos_puts("] ");

        /* Print ASCII if printable, otherwise show dot */
        if (ch >= 0x20 && ch <= 0x7E) {
            sbi_console_putchar(ch);
        } else if (ch == 0x0D || ch == 0x0A) {
            geos_puts("\\n");
        } else if (ch == 0x1B) {
            geos_puts("ESC");
        } else {
            geos_puts(".");
        }
        geos_puts("\n");

        /* Visual feedback: draw a stripe per key, cycling colors */
        uint32_t colors[] = {
            geos_rgb(255, 60, 60),    /* red */
            geos_rgb(60, 255, 60),    /* green */
            geos_rgb(60, 60, 255),    /* blue */
            geos_rgb(255, 255, 60),   /* yellow */
            geos_rgb(255, 60, 255),   /* magenta */
            geos_rgb(60, 255, 255),   /* cyan */
            geos_rgb(255, 255, 255),  /* white */
        };
        uint32_t color = colors[count % 7];
        int stripe_y = 10 + (count % 30) * 8;
        if (stripe_y + 8 <= GEOS_FB_HEIGHT) {
            for (int dx = 0; dx < 8; dx++) {
                for (int dy = 0; dy < 8; dy++) {
                    geos_fb_pixel(dx, stripe_y + dy, color);
                }
            }
        }
        geos_fb_present();
        count++;

        /* ESC (0x1B) shuts down */
        if (ch == 0x1B) {
            geos_puts("ESC received, shutting down.\n");
            break;
        }
    }

    sbi_shutdown();
}
