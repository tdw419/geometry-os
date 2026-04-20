; color_picker.asm -- Mouse-Driven Color Picker for Geometry OS
;
; Proves: RECT (0x88) outline opcode, HITSET/HITQ mouse interaction,
;         RECTF fills, FRAME animation loop, TEXT rendering.
;
; Controls:
;   Click palette colors at bottom to select
;   Preview square shows selected color with outline border
;   RGB sliders show component levels
;
; RAM Layout (placed after bytecode using .org):
;   0x800  selected_color (u32)
;   0x804  red component (0-255)
;   0x808  green component (0-255)
;   0x80C  blue component (0-255)
;   0x900  string buffer for text

#define PREVIEW_X 80
#define PREVIEW_Y 30
#define PREVIEW_SZ 96
#define SLIDER_X 30
#define SLIDER_W 196
#define SLIDER_H 12
#define R_SLIDER_Y 150
#define G_SLIDER_Y 170
#define B_SLIDER_Y 190
#define PAL_Y 220
#define COLOR_ADDR 0x800
#define RED_ADDR 0x804
#define GREEN_ADDR 0x808
#define BLUE_ADDR 0x80C
#define STR_ADDR 0x900

start:
    ; Initialize color to red
    LDI r1, 0xFF0000
    LDI r21, COLOR_ADDR
    STORE r1, r21
    LDI r1, 255
    LDI r21, RED_ADDR
    STORE r1, r21
    LDI r1, 0
    LDI r21, GREEN_ADDR
    STORE r1, r21
    LDI r21, BLUE_ADDR
    STORE r1, r21

frame_loop:
    ; Clear screen
    LDI r5, 0x1A1A2E
    FILL r5

    ; Title
    LDI r1, STR_ADDR
    STRO r1, "Color Picker"
    LDI r1, 90
    LDI r2, 8
    LDI r3, STR_ADDR
    TEXT r1, r2, r3

    ; Preview box - outline using RECT
    LDI r1, PREVIEW_X
    LDI r2, PREVIEW_Y
    LDI r3, PREVIEW_SZ
    LDI r4, PREVIEW_SZ
    LDI r5, 0xAAAAAA
    RECT r1, r2, r3, r4, r5

    ; Preview fill using RECTF
    LDI r21, COLOR_ADDR
    LOAD r5, r21
    RECTF r1, r2, r3, r4, r5

    ; Re-draw outline on top of fill
    LDI r5, 0xAAAAAA
    RECT r1, r2, r3, r4, r5

    ; --- Red slider ---
    LDI r1, SLIDER_X
    LDI r2, R_SLIDER_Y
    LDI r3, SLIDER_W
    LDI r4, SLIDER_H
    LDI r5, 0xFF0000
    RECT r1, r2, r3, r4, r5

    LDI r1, STR_ADDR
    STRO r1, "R"
    LDI r1, 15
    LDI r2, R_SLIDER_Y
    LDI r3, STR_ADDR
    TEXT r1, r2, r3

    ; Red slider fill
    LDI r21, RED_ADDR
    LOAD r1, r21
    LDI r2, 255
    MUL r1, r2
    LDI r2, SLIDER_W
    DIV r1, r2
    LDI r3, SLIDER_X
    LDI r6, 30
    ADD r3, r6
    LDI r4, R_SLIDER_Y
    LDI r6, 1
    ADD r4, r6
    LDI r5, 0xFF4444
    RECTF r3, r4, r1, r5, r5

    ; --- Green slider ---
    LDI r1, SLIDER_X
    LDI r2, G_SLIDER_Y
    LDI r3, SLIDER_W
    LDI r4, SLIDER_H
    LDI r5, 0x00FF00
    RECT r1, r2, r3, r4, r5

    LDI r1, STR_ADDR
    STRO r1, "G"
    LDI r1, 15
    LDI r2, G_SLIDER_Y
    LDI r3, STR_ADDR
    TEXT r1, r2, r3

    ; Green slider fill
    LDI r21, GREEN_ADDR
    LOAD r1, r21
    LDI r2, 255
    MUL r1, r2
    LDI r2, SLIDER_W
    DIV r1, r2
    LDI r3, SLIDER_X
    ADD r3, r6
    LDI r4, G_SLIDER_Y
    ADD r4, r6
    LDI r5, 0x44FF44
    RECTF r3, r4, r1, r5, r5

    ; --- Blue slider ---
    LDI r1, SLIDER_X
    LDI r2, B_SLIDER_Y
    LDI r3, SLIDER_W
    LDI r4, SLIDER_H
    LDI r5, 0x0000FF
    RECT r1, r2, r3, r4, r5

    LDI r1, STR_ADDR
    STRO r1, "B"
    LDI r1, 15
    LDI r2, B_SLIDER_Y
    LDI r3, STR_ADDR
    TEXT r1, r2, r3

    ; Blue slider fill
    LDI r21, BLUE_ADDR
    LOAD r1, r21
    LDI r2, 255
    MUL r1, r2
    LDI r2, SLIDER_W
    DIV r1, r2
    LDI r3, SLIDER_X
    ADD r3, r6
    LDI r4, B_SLIDER_Y
    ADD r4, r6
    LDI r5, 0x4444FF
    RECTF r3, r4, r1, r5, r5

    ; --- Palette (8 swatches) ---
    LDI r10, 10
    LDI r11, PAL_Y
    LDI r12, 20
    LDI r13, 20
    LDI r20, 0

pal_loop:
    CMPI r20, 0
    JNZ r0, not_p0
    LDI r5, 0xFF0000
    JMP pal_draw
not_p0:
    CMPI r20, 1
    JNZ r0, not_p1
    LDI r5, 0x00FF00
    JMP pal_draw
not_p1:
    CMPI r20, 2
    JNZ r0, not_p2
    LDI r5, 0x0000FF
    JMP pal_draw
not_p2:
    CMPI r20, 3
    JNZ r0, not_p3
    LDI r5, 0xFFFF00
    JMP pal_draw
not_p3:
    CMPI r20, 4
    JNZ r0, not_p4
    LDI r5, 0xFF00FF
    JMP pal_draw
not_p4:
    CMPI r20, 5
    JNZ r0, not_p5
    LDI r5, 0x00FFFF
    JMP pal_draw
not_p5:
    CMPI r20, 6
    JNZ r0, not_p6
    LDI r5, 0xFFFFFF
    JMP pal_draw
not_p6:
    CMPI r20, 7
    JNZ r0, pal_done
    LDI r5, 0xFF8800
    JMP pal_draw

pal_draw:
    RECTF r10, r11, r12, r13, r5
    LDI r14, 0x888888
    RECT r10, r11, r12, r13, r14
    LDI r6, 25
    ADD r10, r6
    LDI r6, 1
    ADD r20, r6
    CMPI r20, 8
    BLT r0, pal_loop

pal_done:
    ; --- Hit regions for palette ---
    LDI r10, 10
    LDI r11, PAL_Y
    LDI r12, 20
    LDI r13, 20
    HITSET r10, r11, r12, r13, 1
    LDI r6, 25
    ADD r10, r6
    HITSET r10, r11, r12, r13, 2
    ADD r10, r6
    HITSET r10, r11, r12, r13, 3
    ADD r10, r6
    HITSET r10, r11, r12, r13, 4
    ADD r10, r6
    HITSET r10, r11, r12, r13, 5
    ADD r10, r6
    HITSET r10, r11, r12, r13, 6
    ADD r10, r6
    HITSET r10, r11, r12, r13, 7
    ADD r10, r6
    HITSET r10, r11, r12, r13, 8

    ; Hit regions for sliders
    LDI r10, SLIDER_X
    LDI r12, SLIDER_W
    LDI r13, SLIDER_H
    LDI r11, R_SLIDER_Y
    HITSET r10, r11, r12, r13, 10
    LDI r11, G_SLIDER_Y
    HITSET r10, r11, r12, r13, 11
    LDI r11, B_SLIDER_Y
    HITSET r10, r11, r12, r13, 12

    ; Query mouse and hits
    MOUSEQ r14
    HITQ r15

    ; Handle palette clicks (IDs 1-8)
    CMPI r15, 1
    JNZ r0, chk_p2
    LDI r1, 0xFF0000
    LDI r21, COLOR_ADDR
    STORE r1, r21
    LDI r1, 255
    LDI r21, RED_ADDR
    STORE r1, r21
    LDI r1, 0
    LDI r21, GREEN_ADDR
    STORE r1, r21
    LDI r21, BLUE_ADDR
    STORE r1, r21
    JMP click_done

chk_p2:
    CMPI r15, 2
    JNZ r0, chk_p3
    LDI r1, 0x00FF00
    LDI r21, COLOR_ADDR
    STORE r1, r21
    LDI r1, 0
    LDI r21, RED_ADDR
    STORE r1, r21
    LDI r1, 255
    LDI r21, GREEN_ADDR
    STORE r1, r21
    LDI r21, BLUE_ADDR
    STORE r1, r21
    JMP click_done

chk_p3:
    CMPI r15, 3
    JNZ r0, chk_p4
    LDI r1, 0x0000FF
    LDI r21, COLOR_ADDR
    STORE r1, r21
    LDI r1, 0
    LDI r21, RED_ADDR
    STORE r1, r21
    LDI r21, GREEN_ADDR
    STORE r1, r21
    LDI r1, 255
    LDI r21, BLUE_ADDR
    STORE r1, r21
    JMP click_done

chk_p4:
    CMPI r15, 4
    JNZ r0, chk_p5
    LDI r1, 0xFFFF00
    LDI r21, COLOR_ADDR
    STORE r1, r21
    LDI r1, 255
    LDI r21, RED_ADDR
    STORE r1, r21
    LDI r21, GREEN_ADDR
    STORE r1, r21
    LDI r1, 0
    LDI r21, BLUE_ADDR
    STORE r1, r21
    JMP click_done

chk_p5:
    CMPI r15, 5
    JNZ r0, chk_p6
    LDI r1, 0xFF00FF
    LDI r21, COLOR_ADDR
    STORE r1, r21
    LDI r1, 255
    LDI r21, RED_ADDR
    STORE r1, r21
    LDI r1, 0
    LDI r21, GREEN_ADDR
    STORE r1, r21
    LDI r1, 255
    LDI r21, BLUE_ADDR
    STORE r1, r21
    JMP click_done

chk_p6:
    CMPI r15, 6
    JNZ r0, chk_p7
    LDI r1, 0x00FFFF
    LDI r21, COLOR_ADDR
    STORE r1, r21
    LDI r1, 0
    LDI r21, RED_ADDR
    STORE r1, r21
    LDI r1, 255
    LDI r21, GREEN_ADDR
    STORE r1, r21
    LDI r1, 255
    LDI r21, BLUE_ADDR
    STORE r1, r21
    JMP click_done

chk_p7:
    CMPI r15, 7
    JNZ r0, chk_p8
    LDI r1, 0xFFFFFF
    LDI r21, COLOR_ADDR
    STORE r1, r21
    LDI r1, 255
    LDI r21, RED_ADDR
    STORE r1, r21
    LDI r21, GREEN_ADDR
    STORE r1, r21
    LDI r21, BLUE_ADDR
    STORE r1, r21
    JMP click_done

chk_p8:
    CMPI r15, 8
    JNZ r0, click_done
    LDI r1, 0xFF8800
    LDI r21, COLOR_ADDR
    STORE r1, r21
    LDI r1, 255
    LDI r21, RED_ADDR
    STORE r1, r21
    LDI r1, 136
    LDI r21, GREEN_ADDR
    STORE r1, r21
    LDI r1, 0
    LDI r21, BLUE_ADDR
    STORE r1, r21

click_done:
    FRAME
    JMP frame_loop
