; paint.asm -- Mouse-Driven Paint App for Geometry OS
;
; Proves: MOUSEQ opcode (0x85), PSET pixel drawing, HITSET/HITQ buttons,
;         FRAME loop, color palette selection, clear canvas.
;
; Controls:
;   Mouse movement + left-click area = paint
;   Bottom palette = click to select color (8 colors)
;   [X] button = clear canvas
;
; RAM Layout:
;   0x400        current color (u32, starts red 0xFF0000)
;   0x404        previous mouse X (for line drawing between frames)
;   0x408        previous mouse Y
;   0x40C        paint flag (0=not painting, 1=painting)
;
; String buffers for TEXT opcode:
;   0x500        "CLR" string (4 chars: C, L, R, null)
;   0x520        "PAINT" string (6 chars: P, A, I, N, T, null)
;
; Register allocation:
;   r1  - constant 1
;   r2  - scratch
;   r3  - scratch
;   r5  - color
;   r10 - mouse X from MOUSEQ
;   r11 - mouse Y from MOUSEQ
;   r12 - hit query result
;   r20 - RAM pointer
;   r21 - scratch for color index
;
; NOTE: Avoid colons in comments (assembler pitfall)

#define COLOR   0x400
#define PREV_X  0x404
#define PREV_Y  0x408
#define PAINT   0x40C

; ── INIT ──────────────────────────────────────
LDI r1, 1

; Default color = red
LDI r20, COLOR
LDI r2, 0xFF0000
STORE r20, r2

; Paint flag = 0
LDI r20, PAINT
LDI r2, 0
STORE r20, r2

; Fill screen dark (near-black)
LDI r2, 0x111111
FILL r2

; ── Register hit regions for palette buttons ──
; Palette is at bottom: y=240, 8 colors, each 30x14
; Color 0: Red at x=2
LDI r1, 2
LDI r2, 240
LDI r3, 30
LDI r4, 14
HITSET r1, r2, r3, r4, 1

; Color 1: Green at x=34
LDI r1, 34
LDI r2, 240
LDI r3, 30
LDI r4, 14
HITSET r1, r2, r3, r4, 2

; Color 2: Blue at x=66
LDI r1, 66
LDI r2, 240
LDI r3, 30
LDI r4, 14
HITSET r1, r2, r3, r4, 3

; Color 3: Yellow at x=98
LDI r1, 98
LDI r2, 240
LDI r3, 30
LDI r4, 14
HITSET r1, r2, r3, r4, 4

; Color 4: Cyan at x=130
LDI r1, 130
LDI r2, 240
LDI r3, 30
LDI r4, 14
HITSET r1, r2, r3, r4, 5

; Color 5: Magenta at x=162
LDI r1, 162
LDI r2, 240
LDI r3, 30
LDI r4, 14
HITSET r1, r2, r3, r4, 6

; Color 6: White at x=194
LDI r1, 194
LDI r2, 240
LDI r3, 30
LDI r4, 14
HITSET r1, r2, r3, r4, 7

; Color 7: Orange at x=226
LDI r1, 226
LDI r2, 240
LDI r3, 26
LDI r4, 14
HITSET r1, r2, r3, r4, 8

; Clear button at x=2, y=220, 40x16, id=99
LDI r1, 2
LDI r2, 220
LDI r3, 40
LDI r4, 16
HITSET r1, r2, r3, r4, 99

; ── MAIN LOOP ─────────────────────────────────
main_loop:
    LDI r1, 1

    ; ── Read mouse position ──
    MOUSEQ r10
    ; r10 = mouse_x, r11 = mouse_y

    ; ── Check palette hit regions ──
    HITQ r12
    JZ r12, check_paint

    ; r12 = hit region id
    CMPI r12, 1
    JZ r0, set_red
    CMPI r12, 2
    JZ r0, set_green
    CMPI r12, 3
    JZ r0, set_blue
    CMPI r12, 4
    JZ r0, set_yellow
    CMPI r12, 5
    JZ r0, set_cyan
    CMPI r12, 6
    JZ r0, set_magenta
    CMPI r12, 7
    JZ r0, set_white
    CMPI r12, 8
    JZ r0, set_orange
    CMPI r12, 99
    JZ r0, do_clear
    JMP check_paint

set_red:
    LDI r20, COLOR
    LDI r2, 0xFF0000
    STORE r20, r2
    JMP draw_frame

set_green:
    LDI r20, COLOR
    LDI r2, 0x00FF00
    STORE r20, r2
    JMP draw_frame

set_blue:
    LDI r20, COLOR
    LDI r2, 0x0000FF
    STORE r20, r2
    JMP draw_frame

set_yellow:
    LDI r20, COLOR
    LDI r2, 0xFFFF00
    STORE r20, r2
    JMP draw_frame

set_cyan:
    LDI r20, COLOR
    LDI r2, 0x00FFFF
    STORE r20, r2
    JMP draw_frame

set_magenta:
    LDI r20, COLOR
    LDI r2, 0xFF00FF
    STORE r20, r2
    JMP draw_frame

set_white:
    LDI r20, COLOR
    LDI r2, 0xFFFFFF
    STORE r20, r2
    JMP draw_frame

set_orange:
    LDI r20, COLOR
    LDI r2, 0xFF8800
    STORE r20, r2
    JMP draw_frame

do_clear:
    ; Clear canvas area (keep palette)
    LDI r2, 0x111111
    FILL r2
    LDI r20, PAINT
    LDI r2, 0
    STORE r20, r2
    LDI r20, PREV_X
    LDI r2, 0
    STORE r20, r2
    LDI r20, PREV_Y
    LDI r2, 0
    STORE r20, r2
    JMP draw_frame

check_paint:
    LDI r1, 1
    ; Check if mouse is in paint area (y < 220)
    CMPI r11, 220
    BGE r0, draw_frame

    ; Mouse is in paint area -- draw pixel at mouse pos
    LDI r20, COLOR
    LOAD r5, r20

    ; Paint pixel at current mouse position
    PSET r10, r11, r5

    ; Also paint pixels between previous and current pos (line fill)
    ; Simple approach -- just paint prev pos too for continuity
    LDI r20, PREV_X
    LOAD r2, r20
    CMPI r2, 0
    JZ r0, save_pos

    LDI r20, PREV_Y
    LOAD r3, r20
    PSET r2, r3, r5

save_pos:
    LDI r20, PREV_X
    STORE r20, r10
    LDI r20, PREV_Y
    STORE r20, r11

    ; Set paint flag
    LDI r20, PAINT
    LDI r2, 1
    STORE r20, r2

    JMP draw_frame

draw_frame:
    LDI r1, 1

    ; ── Draw palette background bar ──
    LDI r1, 0
    LDI r2, 238
    LDI r3, 256
    LDI r4, 18
    LDI r5, 0x222222
    RECTF r1, r2, r3, r4, r5

    ; ── Highlight selected color (drawn BEFORE swatches so they appear on top) ──
    LDI r20, COLOR
    LOAD r5, r20
    CMPI r5, 0xFF0000
    JNZ r0, chk_grn
    LDI r1, 1
    LDI r2, 239
    LDI r3, 32
    LDI r4, 16
    LDI r6, 0xFFFFFF
    RECTF r1, r2, r3, r4, r6
    JMP draw_swatches

chk_grn:
    CMPI r5, 0x00FF00
    JNZ r0, chk_blu
    LDI r1, 33
    LDI r2, 239
    LDI r3, 32
    LDI r4, 16
    LDI r6, 0xFFFFFF
    RECTF r1, r2, r3, r4, r6
    JMP draw_swatches

chk_blu:
    CMPI r5, 0x0000FF
    JNZ r0, chk_yel
    LDI r1, 65
    LDI r2, 239
    LDI r3, 32
    LDI r4, 16
    LDI r6, 0xFFFFFF
    RECTF r1, r2, r3, r4, r6
    JMP draw_swatches

chk_yel:
    CMPI r5, 0xFFFF00
    JNZ r0, chk_cya
    LDI r1, 97
    LDI r2, 239
    LDI r3, 32
    LDI r4, 16
    LDI r6, 0xFFFFFF
    RECTF r1, r2, r3, r4, r6
    JMP draw_swatches

chk_cya:
    CMPI r5, 0x00FFFF
    JNZ r0, chk_mag
    LDI r1, 129
    LDI r2, 239
    LDI r3, 32
    LDI r4, 16
    LDI r6, 0xFFFFFF
    RECTF r1, r2, r3, r4, r6
    JMP draw_swatches

chk_mag:
    CMPI r5, 0xFF00FF
    JNZ r0, chk_wht
    LDI r1, 161
    LDI r2, 239
    LDI r3, 32
    LDI r4, 16
    LDI r6, 0xFFFFFF
    RECTF r1, r2, r3, r4, r6
    JMP draw_swatches

chk_wht:
    CMPI r5, 0xFFFFFF
    JNZ r0, chk_org
    LDI r1, 193
    LDI r2, 239
    LDI r3, 32
    LDI r4, 16
    LDI r6, 0x888888
    RECTF r1, r2, r3, r4, r6
    JMP draw_swatches

chk_org:
    CMPI r5, 0xFF8800
    JNZ r0, draw_swatches
    LDI r1, 225
    LDI r2, 239
    LDI r3, 28
    LDI r4, 16
    LDI r6, 0xFFFFFF
    RECTF r1, r2, r3, r4, r6

draw_swatches:
    ; ── Draw color swatches (on top of highlight) ──
    ; Red (id=1)
    LDI r1, 2
    LDI r2, 240
    LDI r3, 30
    LDI r4, 14
    LDI r5, 0xFF0000
    RECTF r1, r2, r3, r4, r5

    ; Green (id=2)
    LDI r1, 34
    LDI r5, 0x00FF00
    RECTF r1, r2, r3, r4, r5

    ; Blue (id=3)
    LDI r1, 66
    LDI r5, 0x0000FF
    RECTF r1, r2, r3, r4, r5

    ; Yellow (id=4)
    LDI r1, 98
    LDI r5, 0xFFFF00
    RECTF r1, r2, r3, r4, r5

    ; Cyan (id=5)
    LDI r1, 130
    LDI r5, 0x00FFFF
    RECTF r1, r2, r3, r4, r5

    ; Magenta (id=6)
    LDI r1, 162
    LDI r5, 0xFF00FF
    RECTF r1, r2, r3, r4, r5

    ; White (id=7)
    LDI r1, 194
    LDI r5, 0xFFFFFF
    RECTF r1, r2, r3, r4, r5

    ; Orange (id=8)
    LDI r1, 226
    LDI r3, 26
    LDI r5, 0xFF8800
    RECTF r1, r2, r3, r4, r5

    ; ── Draw Clear button ──
    LDI r1, 2
    LDI r2, 220
    LDI r3, 40
    LDI r4, 16
    LDI r5, 0x555555
    RECTF r1, r2, r3, r4, r5

    ; "CLR" label on clear button
    LDI r20, 0x500
    LDI r2, 67
    STORE r20, r2
    LDI r2, 76
    ADD r20, r1
    STORE r20, r2
    LDI r2, 82
    ADD r20, r1
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 10
    LDI r2, 224
    LDI r3, 0x500
    TEXT r1, r2, r3

    ; ── Draw title ──
    LDI r1, 1
    LDI r20, 0x520
    LDI r2, 80
    STORE r20, r2
    LDI r2, 65
    ADD r20, r1
    STORE r20, r2
    LDI r2, 73
    ADD r20, r1
    STORE r20, r2
    LDI r2, 78
    ADD r20, r1
    STORE r20, r2
    LDI r2, 84
    ADD r20, r1
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 2
    LDI r2, 2
    LDI r3, 0x520
    TEXT r1, r2, r3

do_frame:
    FRAME
    JMP main_loop
