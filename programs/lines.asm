; lines.asm -- draw a star burst of lines from center
; Uses LINE opcode (0x45)

; color: white
LDI r5, 0xFFFFFF

; center
LDI r0, 128
LDI r1, 128

; corners and edges to draw lines to
LDI r2, 0
LDI r3, 0
LINE r0, r1, r2, r3, r5

LDI r2, 255
LDI r3, 0
LINE r0, r1, r2, r3, r5

LDI r2, 255
LDI r3, 255
LINE r0, r1, r2, r3, r5

LDI r2, 0
LDI r3, 255
LINE r0, r1, r2, r3, r5

LDI r2, 128
LDI r3, 0
LINE r0, r1, r2, r3, r5

LDI r2, 255
LDI r3, 128
LINE r0, r1, r2, r3, r5

LDI r2, 128
LDI r3, 255
LINE r0, r1, r2, r3, r5

LDI r2, 0
LDI r3, 128
LINE r0, r1, r2, r3, r5

HALT
