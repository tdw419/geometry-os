; BORDER -- Draw a colored border around the screen edges
; Uses RECTF to draw borders on all four sides
; Test: border pixels should be green (0x00FF00), center should be black

; Top border: full width, 4 pixels tall
LDI r0, 0
LDI r1, 0
LDI r2, 256
LDI r3, 4
LDI r4, 0x00FF00
RECTF r0, r1, r2, r3, r4

; Bottom border
LDI r0, 0
LDI r1, 252
LDI r2, 256
LDI r3, 4
RECTF r0, r1, r2, r3, r4

; Left border
LDI r0, 0
LDI r1, 4
LDI r2, 4
LDI r3, 248
RECTF r0, r1, r2, r3, r4

; Right border
LDI r0, 252
LDI r1, 4
LDI r2, 4
LDI r3, 248
RECTF r0, r1, r2, r3, r4

HALT
