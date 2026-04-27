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
 *   P        - Save canvas to VFS surface (persist)
 *   O        - Load canvas from VFS surface (open)
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

/* Canvas background color */
#define BG_COLOR 0x0A0A14FF  /* geos_rgb(10,10,20) */

/* ---- Drawing helpers ---- */
static void draw_palette_bar(int selected) {
    /* Draw a 10-pixel-tall color palette bar across the bottom of the screen */
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
                geos_fb_pixel(x, bar_y - 1, 0xFFFFFFFF);  /* top */
                geos_fb_pixel(x, bar_y + 10, 0xFFFFFFFF); /* bottom */
            }
            for (uint32_t dy = 0; dy < 10; dy++) {
                geos_fb_pixel(x0, bar_y + dy, 0xFFFFFFFF);  /* left */
                geos_fb_pixel(x1, bar_y + dy, 0xFFFFFFFF);  /* right */
            }
        }
    }
}

static void clear_canvas(void) {
    volatile uint32_t *fb = (volatile uint32_t *)GEOS_FB_BASE;
    for (int i = 0; i < GEOS_FB_WIDTH * GEOS_FB_HEIGHT; i++) {
        fb[i] = BG_COLOR;
    }
}

/* ---- Entry point ---- */
void c_start(void) {
    geos_puts("paint: pixel-native interactive paint\n");
    geos_puts("WASD=move Space=paint 1-9=colors 0=eraser C=clear F=fill P=save O=load ESC=quit\n");

    /* Initialize canvas */
    clear_canvas();
    geos_fb_present();

    /* Cursor state */
    uint32_t cx = GEOS_FB_WIDTH / 2;   /* cursor X */
    uint32_t cy = GEOS_FB_HEIGHT / 2;  /* cursor Y */
    int color_idx = 1;                  /* start with red */
    int fill_mode = 0;                  /* paint-on-move */
    /* Track what's under the cursor so we can restore it */
    uint32_t saved_pixel = BG_COLOR;

    /* Draw initial state */
    draw_palette_bar(color_idx);
    /* Draw cursor: XOR with bright color to make it visible regardless of bg */
    saved_pixel = geos_fb_read(cx, cy);
    geos_fb_pixel(cx, cy, 0xFFFF00FF);  /* yellow cursor pixel */
    geos_fb_present();

    geos_puts("paint: ready!\n");

    while (1) {
        char ch = geos_getchar();

        int moved = 0;
        int painted = 0;

        /* Movement: restore old cursor, move, save new pixel */
        if (ch == 'w' || ch == 'W') {
            if (cy > 0) { moved = 1; }
        } else if (ch == 's' || ch == 'S') {
            if (cy < GEOS_FB_HEIGHT - 13) { moved = 1; }
        } else if (ch == 'a' || ch == 'A') {
            if (cx > 0) { moved = 1; }
        } else if (ch == 'd' || ch == 'D') {
            if (cx < GEOS_FB_WIDTH - 1) { moved = 1; }
        }

        if (moved) {
            /* Restore the pixel under the old cursor */
            geos_fb_pixel(cx, cy, saved_pixel);

            /* In fill mode: paint at old position before moving */
            if (fill_mode) {
                geos_fb_pixel(cx, cy, palette[color_idx]);
            }

            /* Update position */
            if (ch == 'w' || ch == 'W') cy--;
            else if (ch == 's' || ch == 'S') cy++;
            else if (ch == 'a' || ch == 'A') cx--;
            else if (ch == 'd' || ch == 'D') cx++;

            /* Save the pixel under the new cursor position */
            saved_pixel = geos_fb_read(cx, cy);

            /* Draw cursor at new position */
            geos_fb_pixel(cx, cy, 0xFFFF00FF);  /* yellow */
            draw_palette_bar(color_idx);
            geos_fb_present();
        }
        /* Paint: space bar paints at current position */
        else if (ch == ' ') {
            geos_fb_pixel(cx, cy, palette[color_idx]);
            saved_pixel = palette[color_idx];  /* cursor stays, update saved */
            /* Redraw cursor on top of paint so user sees it */
            geos_fb_pixel(cx, cy, 0xFFFF00FF);
            draw_palette_bar(color_idx);
            geos_fb_present();
            painted = 1;
        }
        /* Color select: 0-9 */
        else if (ch >= '0' && ch <= '9') {
            color_idx = ch - '0';
            draw_palette_bar(color_idx);
            geos_fb_present();
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
            saved_pixel = BG_COLOR;
            geos_fb_pixel(cx, cy, 0xFFFF00FF);  /* redraw cursor */
            draw_palette_bar(color_idx);
            geos_fb_present();
            geos_puts("paint: canvas cleared\n");
        }
        /* Save canvas to VFS surface */
        else if (ch == 'p' || ch == 'P') {
            geos_puts("paint: saving canvas...\n");
            if (geos_save_canvas() == 0) {
                geos_puts("paint: canvas saved!\n");
            } else {
                geos_puts("paint: save FAILED\n");
            }
        }
        /* Load canvas from VFS surface */
        else if (ch == 'o' || ch == 'O') {
            geos_puts("paint: loading canvas...\n");
            if (geos_load_canvas() == 0) {
                /* Redraw palette bar and cursor on top of loaded canvas */
                saved_pixel = geos_fb_read(cx, cy);
                draw_palette_bar(color_idx);
                geos_fb_pixel(cx, cy, 0xFFFF00FF);
                geos_fb_present();
                geos_puts("paint: canvas loaded!\n");
            } else {
                geos_puts("paint: no saved canvas found\n");
            }
        }
        /* Quit */
        else if (ch == 0x1B) {
            /* Restore pixel under cursor before leaving */
            geos_fb_pixel(cx, cy, saved_pixel);
            geos_fb_present();
            geos_puts("paint: goodbye!\n");
            break;
        }
    }

    sbi_shutdown();
}
