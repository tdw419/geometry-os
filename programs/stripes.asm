; HORIZONTAL_STRIPES -- Draw alternating horizontal stripes
; Each stripe is 16 pixels tall, alternating red (0xFF0000) and blue (0x0000FF)
; Uses PSET (register-based) for all drawing
; Test: rows 0-15 = red, rows 16-31 = blue, rows 32-47 = red, etc.

LDI r0, 0            ; r0 = y counter
LDI r1, 1            ; increment
LDI r3, 256          ; limit
LDI r6, 0xFF00FF     ; toggle mask (red XOR blue)
LDI r7, 16           ; stripe height
LDI r8, 0            ; stripe pixel counter
LDI r9, 0xFF0000     ; current color (start red)

y_loop:
  LDI r2, 0          ; r2 = x counter

x_loop:
  PSET r2, r0, r9    ; pixel at (x, y) with current color
  ADD r2, r1          ; x++
  LDI r4, 0
  ADD r4, r2
  SUB r4, r3
  JZ r4, next_row
  JMP x_loop

next_row:
  ADD r0, r1          ; y++
  ADD r8, r1          ; stripe pixel counter++
  LDI r4, 0
  ADD r4, r8
  SUB r4, r7
  JNZ r4, check_y
  ; Toggle color and reset counter
  LDI r8, 0
  LDI r4, 0
  ADD r4, r9
  XOR r4, r6
  LDI r9, 0
  ADD r9, r4

check_y:
  LDI r4, 0
  ADD r4, r0
  SUB r4, r3
  JZ r4, done
  JMP y_loop

done:
HALT
