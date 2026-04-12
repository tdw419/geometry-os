; sprite_demo.asm -- Demonstrate SPRITE opcode
; Draws a 4x3 red rectangle sprite at (10, 10)
; Uses STORE to write sprite data into RAM, then blits it

; Write sprite data to RAM starting at 0x200
; Row 0: 4 red pixels
LDI r0, 0xFF0000
LDI r6, 0x200
STORE r6, r0
LDI r6, 0x201
STORE r6, r0
LDI r6, 0x202
STORE r6, r0
LDI r6, 0x203
STORE r6, r0

; Row 1: 4 red pixels
LDI r6, 0x204
STORE r6, r0
LDI r6, 0x205
STORE r6, r0
LDI r6, 0x206
STORE r6, r0
LDI r6, 0x207
STORE r6, r0

; Row 2: 4 red pixels
LDI r6, 0x208
STORE r6, r0
LDI r6, 0x209
STORE r6, r0
LDI r6, 0x20A
STORE r6, r0
LDI r6, 0x20B
STORE r6, r0

; Load sprite parameters
LDI r1, 10       ; x position
LDI r2, 10       ; y position
LDI r3, 0x200    ; sprite data address
LDI r4, 4        ; width
LDI r5, 3        ; height

; Blit the sprite
SPRITE r1, r2, r3, r4, r5

HALT
