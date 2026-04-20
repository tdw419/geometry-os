; concentric.asm -- Draw 4 concentric rings from center (128,128)
;
; Intended output:
;   Ring 1 (r=100): blue   0x0000FF
;   Ring 2 (r=75):  red    0xFF0000
;   Ring 3 (r=50):  green  0x00FF00
;   Ring 4 (r=25):  yellow 0xFFFF00
;
; Actual output: Ring 3 is wrong color. Agent must diagnose why.

LDI r10, 128      ; center x
LDI r11, 128      ; center y

; Ring 1: radius 100, blue
LDI r12, 100
LDI r13, 0x0000FF
CIRCLE r10, r11, r12, r13

; Ring 2: radius 75, red
LDI r12, 75
LDI r13, 0xFF0000
CIRCLE r10, r11, r12, r13

; Ring 3: radius 50, SHOULD BE green (0x00FF00)
; Bug: color is computed by shifting instead of loading immediate.
; SHL 1 on 0x7F80 gives 0xFF00, not 0x00FF00.
LDI r12, 50
LDI r13, 0x7F80
LDI r14, 1
SHL r13, r14
CIRCLE r10, r11, r12, r13

; Ring 4: radius 25, yellow
LDI r12, 25
LDI r13, 0xFFFF00
CIRCLE r10, r11, r12, r13

HALT
