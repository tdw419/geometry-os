; screenshot.asm -- Screenshot capture utility
;
; Press S to capture a screenshot, saves to VFS as "shot_N.img".
; Shows a green indicator in top-right corner for ~1 second after capture.
; The screen starts with a demo pattern (colored rectangles).

; ── Draw demo pattern first ──
; Blue background
LDI r0, 0x000044
FILL r0
; Red rectangle top-left
LDI r0, 10
LDI r1, 10
LDI r2, 60
LDI r3, 40
LDI r4, 0xFF0000
RECTF r0, r1, r2, r3, r4
; Green rectangle top-right
LDI r0, 180
LDI r1, 10
LDI r2, 60
LDI r3, 40
LDI r4, 0x00FF00
RECTF r0, r1, r2, r3, r4
; Yellow rectangle center
LDI r0, 100
LDI r1, 100
LDI r2, 56
LDI r3, 56
LDI r4, 0xFFFF00
RECTF r0, r1, r2, r3, r4

; ── Screenshot counter in RAM[0x7000] ──
LDI r7, 1
LDI r15, 0x7000
LDI r16, 0
STORE r15, r16

; ── Overlay timer in r8 ──
LDI r8, 0

; ── Main loop ──
main_loop:
    IKEY r10           ; read key
    LDI r20, 83        ; 'S' key code
    CMP r10, r20
    JZ r0, do_screenshot
    JMP check_overlay

do_screenshot:
    ; Build filename "shot_N.img" in RAM at 0x2000
    CALL write_filename

    ; SCRSHOT - path addr in r1 (set by write_filename)
    SCRSHOT r1

    ; Increment shot counter
    LDI r15, 0x7000
    LOAD r16, r15
    ADD r16, r7
    STORE r15, r16

    ; Set overlay timer (60 frames ~ 1 second)
    LDI r8, 60
    JMP check_overlay

check_overlay:
    ; Show green indicator if timer > 0
    LDI r20, 0
    CMP r8, r20
    JZ r0, skip_overlay

    ; Draw green rectangle in top-right as "saved" indicator
    LDI r0, 230
    LDI r1, 2
    LDI r2, 24
    LDI r3, 8
    LDI r4, 0x00FF00
    RECTF r0, r1, r2, r3, r4
    SUB r8, r7         ; timer--

    JMP do_frame

skip_overlay:
    ; Redraw demo area where overlay was (restore blue background)
    LDI r20, 0
    CMP r8, r20
    JNZ r0, do_frame   ; still showing overlay
    ; Clear overlay area with background color
    LDI r0, 230
    LDI r1, 2
    LDI r2, 24
    LDI r3, 8
    LDI r4, 0x000044
    RECTF r0, r1, r2, r3, r4

do_frame:
    FRAME
    JMP main_loop

; ── Write filename "shot_N.img" to RAM at 0x2000 ──
write_filename:
    PUSH r31
    LDI r1, 0x2000
    ; "shot_"
    LDI r12, 115       ; 's'
    STORE r1, r12
    ADD r1, r7
    LDI r12, 104       ; 'h'
    STORE r1, r12
    ADD r1, r7
    LDI r12, 111       ; 'o'
    STORE r1, r12
    ADD r1, r7
    LDI r12, 116       ; 't'
    STORE r1, r12
    ADD r1, r7
    LDI r12, 95        ; '_'
    STORE r1, r12
    ADD r1, r7
    ; Append shot counter as digit
    LDI r15, 0x7000
    LOAD r16, r15
    LDI r12, 48         ; '0'
    ADD r12, r16
    STORE r1, r12
    ADD r1, r7
    ; ".img"
    LDI r12, 46         ; '.'
    STORE r1, r12
    ADD r1, r7
    LDI r12, 105        ; 'i'
    STORE r1, r12
    ADD r1, r7
    LDI r12, 109        ; 'm'
    STORE r1, r12
    ADD r1, r7
    LDI r12, 103        ; 'g'
    STORE r1, r12
    ADD r1, r7
    LDI r12, 0          ; null terminator
    STORE r1, r12
    ; Restore r1 to point to start of filename
    LDI r1, 0x2000
    POP r31
    RET
