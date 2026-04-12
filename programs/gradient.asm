; GRADIENT -- Draw a horizontal blue gradient across the screen
; Each column x gets color = x (blue channel only)
; Uses PSET (register-based) for dynamic coordinates
; Test: column 0 = 0x00, column 255 = 0xFF

LDI r0, 0            ; r0 = x counter
LDI r1, 1            ; increment
LDI r3, 256          ; limit

x_loop:
  LDI r4, 0          ; r4 = y counter

y_loop:
  PSET r0, r4, r0    ; pixel at (x, y) with color = x (r0 serves as both x and color)
  ADD r4, r1          ; y++
  LDI r5, 0
  ADD r5, r4
  SUB r5, r3
  JZ r5, next_x
  JMP y_loop

next_x:
  ADD r0, r1          ; x++
  LDI r5, 0
  ADD r5, r0
  SUB r5, r3
  JZ r5, done
  JMP x_loop

done:
HALT
