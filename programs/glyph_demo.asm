; glyph_demo.asm -- Auto-generated from GlyphLang: 3 4 + . @
; Demonstrates GlyphLang compilation to Geometry OS assembly.
; Source: programs/glyph_demo.glyph
; Expected: draws pixel at (7, 0) with color 7, then halts.

LDI r30, 0x8000     ; Initialize Stack Pointer
LDI r29, 0          ; output x counter
LDI r28, 0          ; output y counter

; 3 4 + .
LDI r27, 3
PUSH r27
LDI r27, 4
PUSH r27
POP r28             ; b = 4
POP r27             ; a = 3
ADD r27, r28        ; r27 = 7
PUSH r27
POP r27             ; value to output = 7
PSET r27, r28, r27  ; draw pixel at (7, 0) with color 7

HALT
