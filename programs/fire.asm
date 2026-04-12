; fire.asm -- scrolling fire animation using SCROLL + FRAME
;
; Each frame:
;   1. SCROLL screen up 1 pixel
;   2. Draw a new ember row at y=254 with position-varied orange/red
;   3. FRAME -- display this state and continue
;
; r1 = x counter  r2 = y (254)  r4 = frame counter
; r5 = scroll amt  r6 = 256

LDI r2, 254
LDI r5, 1
LDI r6, 256

frame_loop:
  SCROLL r5

  LDI r1, 0

ember_loop:
  ; brightness = (x * 7 + frame) & 0xFF
  LDI r3, 7
  MUL r3, r1
  ADD r3, r4
  LDI r9, 0xFF
  AND r3, r9

  ; threshold: skip if brightness < 64
  LDI r9, 64
  CMP r3, r9
  BLT r0, next_x

  ; green = brightness >> 1, red = 0xFF
  LDI r9, 2
  DIV r3, r9
  LDI r9, 8
  SHL r3, r9
  LDI r9, 0xFF0000
  OR r3, r9

  PSET r1, r2, r3

next_x:
  LDI r9, 1
  ADD r1, r9
  CMP r1, r6
  JZ r0, end_row
  JMP ember_loop

end_row:
  LDI r9, 1
  ADD r4, r9
  FRAME
  JMP frame_loop
