; NESTED_RECTS -- Draw concentric rectangles from outside in
; Outer = red, inner = green, innermost = blue
; Each rectangle shrinks by 20 pixels on each side
; Test: visible nested colored rectangles

; Outer red rectangle
LDI r0, 0
LDI r1, 0
LDI r2, 256
LDI r3, 256
LDI r4, 0xFF0000
RECTF r0, r1, r2, r3, r4

; Green rectangle (20px inset)
LDI r0, 20
LDI r1, 20
LDI r2, 216
LDI r3, 216
LDI r4, 0x00FF00
RECTF r0, r1, r2, r3, r4

; Blue rectangle (40px inset)
LDI r0, 40
LDI r1, 40
LDI r2, 176
LDI r3, 176
LDI r4, 0x0000FF
RECTF r0, r1, r2, r3, r4

; Yellow rectangle (60px inset)
LDI r0, 60
LDI r1, 60
LDI r2, 136
LDI r3, 136
LDI r4, 0xFFFF00
RECTF r0, r1, r2, r3, r4

; Magenta rectangle (80px inset)
LDI r0, 80
LDI r1, 80
LDI r2, 96
LDI r3, 96
LDI r4, 0xFF00FF
RECTF r0, r1, r2, r3, r4

; Cyan rectangle (100px inset)
LDI r0, 100
LDI r1, 100
LDI r2, 56
LDI r3, 56
LDI r4, 0x00FFFF
RECTF r0, r1, r2, r3, r4

; White center (120px inset)
LDI r0, 120
LDI r1, 120
LDI r2, 16
LDI r3, 16
LDI r4, 0xFFFFFF
RECTF r0, r1, r2, r3, r4

HALT
