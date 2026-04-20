; imgview.asm -- Image Viewer Demo
;
; Generates a gradient pattern (simulating a loaded image),
; draws UI chrome (title bar, status bar), and enters a main loop.
; Press S to save screenshot. Press Q to quit.

; ── Constants ──
LDI r7, 1
LDI r8, 0          ; overlay timer
LDI r9, 256        ; screen dimension

; ── Draw gradient pattern (simulated loaded image) ──
LDI r0, 0          ; y = 0
dg_y:
    LDI r10, 0         ; x = 0
dg_x:
    MOV r11, r10
    LDI r12, 16
    SHL r11, r12        ; R = x << 16
    MOV r13, r0
    LDI r12, 8
    SHL r13, r12        ; G = y << 8
    OR r11, r13
    LDI r13, 128
    OR r11, r13         ; B = 128
    PSET r10, r0, r11
    ADD r10, r7
    LDI r12, 0
    ADD r12, r10
    SUB r12, r9
    JZ r12, dg_next_y
    JMP dg_x
dg_next_y:
    ADD r0, r7
    LDI r12, 0
    ADD r12, r0
    SUB r12, r9
    JZ r12, dg_done
    JMP dg_y
dg_done:

; ── Draw title bar ──
LDI r0, 0
LDI r1, 0
LDI r2, 256
LDI r3, 14
LDI r4, 0x222222
RECTF r0, r1, r2, r3, r4

; Highlight bar
LDI r0, 2
LDI r1, 2
LDI r2, 100
LDI r3, 10
LDI r4, 0x8888FF
RECTF r0, r1, r2, r3, r4

; Status bar at bottom
LDI r0, 0
LDI r1, 244
LDI r2, 256
LDI r3, 12
LDI r4, 0x222222
RECTF r0, r1, r2, r3, r4

; ── Screenshot counter at RAM[0x7000] ──
LDI r15, 0x7000
LDI r16, 0
STORE r15, r16

; ── Main loop ──
main_loop:
    IKEY r10
    ; S key = save screenshot
    LDI r20, 83
    CMP r10, r20
    JZ r0, do_save
    ; Q key = quit
    LDI r20, 81
    CMP r10, r20
    JZ r0, do_quit
    JMP check_overlay

do_save:
    CALL write_filename
    SCRSHOT r1
    ; Increment counter
    LDI r15, 0x7000
    LOAD r16, r15
    ADD r16, r7
    STORE r15, r16
    LDI r8, 60
    JMP check_overlay

check_overlay:
    LDI r20, 0
    CMP r8, r20
    JZ r0, do_frame
    ; Draw saved indicator
    LDI r0, 220
    LDI r1, 2
    LDI r2, 34
    LDI r3, 10
    LDI r4, 0x00CC00
    RECTF r0, r1, r2, r3, r4
    SUB r8, r7
    JMP do_frame

do_quit:
    HALT

do_frame:
    FRAME
    JMP main_loop

; ── Write filename "view_N.img" to RAM at 0x2000 ──
write_filename:
    PUSH r31
    LDI r1, 0x2000
    LDI r12, 118       ; 'v'
    STORE r1, r12
    ADD r1, r7
    LDI r12, 105       ; 'i'
    STORE r1, r12
    ADD r1, r7
    LDI r12, 101       ; 'e'
    STORE r1, r12
    ADD r1, r7
    LDI r12, 119       ; 'w'
    STORE r1, r12
    ADD r1, r7
    LDI r12, 95        ; '_'
    STORE r1, r12
    ADD r1, r7
    ; Append counter as digit
    LDI r15, 0x7000
    LOAD r16, r15
    LDI r12, 48
    ADD r12, r16
    STORE r1, r12
    ADD r1, r7
    LDI r12, 46        ; '.'
    STORE r1, r12
    ADD r1, r7
    LDI r12, 105       ; 'i'
    STORE r1, r12
    ADD r1, r7
    LDI r12, 109       ; 'm'
    STORE r1, r12
    ADD r1, r7
    LDI r12, 103       ; 'g'
    STORE r1, r12
    ADD r1, r7
    LDI r12, 0
    STORE r1, r12
    LDI r1, 0x2000
    POP r31
    RET
