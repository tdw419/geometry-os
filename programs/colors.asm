; Colors -- fill screen with horizontal color bands
; Uses FILL in a loop to draw 4 color bands across the screen

LDI r0, 0xFF0000
FILL r0
LDI r0, 0x00FF00
FILL r0
LDI r0, 0x0000FF
FILL r0
LDI r0, 0xFFFF00
FILL r0
HALT
