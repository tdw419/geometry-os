; invert_demo.asm -- Visual demo cycling between normal and inverted screen
; Phase 67: uses INV opcode
; Press any key to exit

; Draw colored stripes using RECTF
LDI r0, 0
LDI r1, 0
LDI r2, 256
LDI r3, 42
LDI r4, 0x00FF0000
RECTF r0, r1, r2, r3, r4

LDI r1, 42
LDI r4, 0x0000FF00
RECTF r0, r1, r2, r3, r4

LDI r1, 84
LDI r4, 0x000000FF
RECTF r0, r1, r2, r3, r4

LDI r1, 126
LDI r4, 0x00FFFF00
RECTF r0, r1, r2, r3, r4

LDI r1, 168
LDI r4, 0x00FF00FF
RECTF r0, r1, r2, r3, r4

LDI r1, 210
LDI r3, 46
LDI r4, 0x0000FFFF
RECTF r0, r1, r2, r3, r4

FRAME

; Main loop: wait, invert, wait, invert back
main_loop:
    IKEY r0
    CMPI r0, 0
    JNZ r0, exit

    LDI r10, 30
wait1:
    FRAME
    SUBI r10, 1
    JNZ r10, wait1

    INV
    FRAME

    LDI r10, 30
wait2:
    FRAME
    SUBI r10, 1
    JNZ r10, wait2

    INV
    FRAME

    JNZ r21, main_loop

exit:
    HALT
