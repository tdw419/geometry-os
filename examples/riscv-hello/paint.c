/*
 * paint.c -- Interactive pixel paint program for Geometry OS
 *
 * The pixel-native demo: keyboard-driven painting on the 256x256 MMIO
 * framebuffer. Arrow keys (WASD) move the cursor, Space paints, number
 * keys select colors, C clears the canvas, ESC quits.
 *
 * This is the first guest program that combines ALL the RISC-V bridge
 * pieces: host keyboard input -> UART rx -> SBI getchar -> guest logic
 * -> MMIO framebuffer writes -> fb_present -> live display.
 *
 * Controls:
 *   W/A/S/D  - Move cursor (up/left/down/right)
 *   Space    - Paint current pixel
 *   1-9      - Select color palette
 *   0        - Select eraser (black)
 *   F        - Fill mode (hold to paint while moving)
 *   C        - Clear canvas
 *   P        - Toggle cursor visibility
 *   ESC      - Quit
 *
 * Build: ./build.sh paint.c paint.elf
 * Run:   riscv_run paint.elf
 */

#include "libgeos.h"

/* ---- Palette ----
 * Format: 0xRRGGBBAA (alpha=0xFF). geos_rgb() is a function so we
 * use pre-computed constants for static initialization. */
static const uint32_t palette[] = {
    0x000000FF,  /* 0: black (eraser) */
    0xFF3C3CFF,  /* 1: red */
    0x3CC83CFF,  /* 2: green */
    0x3C64FFFF,  /* 3: blue */
    0xFFFF3CFF,  /* 4: yellow */
    0xFF3CFFFF,  /* 5: magenta */
    0x3CFFFFFF,  /* 6: cyan */
    0xFFFFFFFF,  /* 7: white */
    0xFF8C00FF,  /* 8: orange */
    0xB450FFFF,  /* 9: purple */
};
#define PALETTE_SIZE 10

/* ---- Drawing helpers ---- */
static inline void draw_crosshair(uint32_t cx, uint32_t cy, uint32_t color) {
    /* 3x3 crosshair with center pixel highlighted */
    for (int dx = -1; dx <= 1; dx++) {
        for (int dy = -1; dy <= 1; dy++) {
            int32_t px = (int32_t)cx + dx;
            int32_t py = (int32_t)cy + dy;
            if (px >= 0 && px < GEOS_FB_WIDTH && py >= 0 && py < GEOS_FB_HEIGHT) {
                /* Center pixel is brighter, arms are dimmer */
                if (dx == 0 && dy == 0) {
                    geos_fb_pixel(px, py, color);
                } else {
                    /* Half-brightness version */
                    uint8_t r = (uint8_t)((color >> 24) >> 1);
                    uint8_t g = (uint8_t)((color >> 16) >> 1);
                    uint8_t b = (uint8_t)((color >> 8) >> 1);
                    geos_fb_pixel(px, py, geos_rgb(r, g, b));
                }
            }
        }
    }
}

static void draw_palette_bar(int selected) {
    /* Draw a 10-pixel-tall color palette bar across the top of the screen */
    uint32_t bar_y = GEOS_FB_HEIGHT - 12;
    for (int i = 0; i < PALETTE_SIZE; i++) {
        uint32_t x0 = i * 25;
        uint32_t x1 = x0 + 24;
        for (uint32_t x = x0; x <= x1 && x < GEOS_FB_WIDTH; x++) {
            for (uint32_t y = bar_y; y < bar_y + 10; y++) {
                geos_fb_pixel(x, y, palette[i]);
            }
        }
        /* Highlight selected color with white border */
        if (i == selected) {
            for (uint32_t x = x0; x <= x1 && x < GEOS_FB_WIDTH; x++) {
                geos_fb_pixel(x, bar_y - 1, geos_rgb(255, 255, 255));
                geos_fb_pixel(x, bar_y + 10, geos_rgb(255, 255, 255));
            }
            for (uint32_t dy = 0; dy < 10; dy++) {
                geos_fb_pixel(x0, bar_y + dy, geos_rgb(255, 255, 255));
                geos_fb_pixel(x1, bar_y + dy, geos_rgb(255, 255, 255));
            }
        }
    }
}

static void clear_canvas(void) {
    volatile uint32_t *fb = (volatile uint32_t *)GEOS_FB_BASE;
    for (int i = 0; i < GEOS_FB_WIDTH * GEOS_FB_HEIGHT; i++) {
        fb[i] = geos_rgb(10, 10, 20);  /* dark blue-black */
    }
}

/* ---- Entry point ---- */
void c_start(void) {
    geos_puts("paint: pixel-native interactive paint\n");
    geos_puts("WASD=move Space=paint 1-9=colors 0=eraser C=clear F=fill ESC=quit\n");

    /* Initialize canvas */
    clear_canvas();
    geos_fb_present();

    /* Cursor state */
    uint32_t cx = GEOS_FB_WIDTH / 2;   /* cursor X */
    uint32_t cy = GEOS_FB_HEIGHT / 2;  /* cursor Y */
    int color_idx = 1;                  /* start with red */
    int fill_mode = 0;                  /* paint-on-move */
    int show_cursor = 1;
    uint32_t prev_px = 0;               /* pixel under cursor (saved for restore) */

    /* Draw initial state */
    draw_palette_bar(color_idx);
    draw_crosshair(cx, cy, geos_rgb(255, 255, 0));  /* yellow cursor */
    geos_fb_present();

    geos_puts("paint: ready!\n");

    while (1) {
        char ch = geos_getchar();

        int need_redraw = 0;
        int painted = 0;

        /* Movement */
        if (ch == 'w' || ch == 'W') {
            if (cy > 0) { cy--; need_redraw = 1; }
        } else if (ch == 's' || ch == 'S') {
            if (cy < GEOS_FB_HEIGHT - 13) { cy++; need_redraw = 1; }  /* stop above palette */
        } else if (ch == 'a' || ch == 'A') {
            if (cx > 0) { cx--; need_redraw = 1; }
        } else if (ch == 'd' || ch == 'D') {
            if (cx < GEOS_FB_WIDTH - 1) { cx++; need_redraw = 1; }
        }
        /* Paint */
        else if (ch == ' ') {
            geos_fb_pixel(cx, cy, palette[color_idx]);
            painted = 1;
        }
        /* Color select: 0-9 */
        else if (ch >= '0' && ch <= '9') {
            color_idx = ch - '0';
            need_redraw = 1;
            geos_puts("paint: color ");
            geos_put_dec((uint32_t)color_idx);
            geos_puts("\n");
        }
        /* Toggle fill mode */
        else if (ch == 'f' || ch == 'F') {
            fill_mode = !fill_mode;
            geos_puts(fill_mode ? "paint: fill ON\n" : "paint: fill OFF\n");
        }
        /* Clear */
        else if (ch == 'c' || ch == 'C') {
            clear_canvas();
            need_redraw = 1;
            geos_puts("paint: canvas cleared\n");
        }
        /* Toggle cursor */
        else if (ch == 'p' || ch == 'P') {
            show_cursor = !show_cursor;
            need_redraw = 1;
        }
        /* Quit */
        else if (ch == 0x1B) {
            geos_puts("paint: goodbye!\n");
            break;
        }

        /* In fill mode, paint on every move */
        if (fill_mode && need_redraw) {
            geos_fb_pixel(cx, cy, palette[color_idx]);
            painted = 1;
        }

        /* Redraw cursor crosshair */
        if (need_redraw || painted) {
            /* Redraw crosshair at new position */
            if (show_cursor) {
                draw_crosshair(cx, cy, geos_rgb(255, 255, 0));
            }
            /* Redraw palette bar */
            draw_palette_bar(color_idx);
            geos_fb_present();
        }
    }

    sbi_shutdown();
}
