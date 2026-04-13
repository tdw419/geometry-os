; RINGS -- Concentric colored rings emanating from center (128,128)
; For each pixel, compute distance from center, map to color
; Distance approximated with Manhattan distance (|dx|+|dy|)
; Ring color cycles through: red, yellow, green, cyan, blue, magenta

LDI r21, 0           ; r21 = y
LDI r1, 1            ; r1 = increment
LDI r2, 256          ; r2 = limit
LDI r3, 128          ; r3 = center (128)

y_loop:
  LDI r4, 0          ; r4 = x

x_loop:
  ; Compute |x - 128|
  LDI r5, 0
  ADD r5, r4         ; r5 = x
  SUB r5, r3         ; r5 = x - 128
  ; If negative (wrapped), negate: 0 - r5
  LDI r6, 0
  CMP r5, r6         ; r0 = sign comparison
  ; r0 > 0 means r5 < r6 (negative)
  ; We can't branch on sign of r0 directly... 
  ; Use BLT: if r5 < 0, negate
  LDI r20, 0
  CMP r5, r20
  BLT r0, abs_x_neg
  JMP abs_x_done
abs_x_neg:
  LDI r6, 0
  SUB r6, r5         ; r6 = -r5 = |x-128|
  LDI r5, 0
  ADD r5, r6         ; r5 = |x-128|
abs_x_done:
  ; r5 = |x - 128|

  ; Compute |y - 128|
  LDI r7, 0
  ADD r7, r21        ; r7 = y
  SUB r7, r3         ; r7 = y - 128
  LDI r6, 0
  CMP r7, r6
  LDI r20, 0
  CMP r7, r20
  BLT r0, abs_y_neg
  JMP abs_y_done
abs_y_neg:
  LDI r6, 0
  SUB r6, r7
  LDI r7, 0
  ADD r7, r6
abs_y_done:
  ; r7 = |y - 128|

  ; distance = |x-128| + |y-128| (Manhattan)
  ADD r5, r7         ; r5 = distance

  ; Ring index = distance / 16 (integer division by repeated subtraction)
  LDI r8, 0          ; r8 = ring index
  LDI r9, 16         ; r9 = ring width
div_loop:
  SUB r5, r9
  LDI r6, 0
  CMP r5, r6
  ; If r5 >= 0, increment ring index and continue
  ; BLT r5 < 0 means we overshot
  BLT r0, div_done
  LDI r10, 1
  ADD r8, r10
  JMP div_loop
div_done:
  ; r8 = ring index (0, 1, 2, ...)

  ; Map ring index to color using mod 6
  ; 0=red, 1=yellow, 2=green, 3=cyan, 4=blue, 5=magenta
  LDI r9, 6
mod_loop:
  SUB r8, r9
  LDI r6, 0
  CMP r8, r6
  BLT r0, mod_done
  JMP mod_loop
mod_done:
  ADD r8, r9         ; undo last subtraction
  ; r8 = color index (0..5)

  ; Look up color
  JNZ r8, not_red
  LDI r5, 0xFF0000   ; red
  PSET r4, r21, r5
  JMP next_pixel
not_red:
  LDI r6, 1
  SUB r8, r6
  JNZ r8, not_yellow
  LDI r5, 0xFFFF00   ; yellow
  PSET r4, r21, r5
  JMP next_pixel
not_yellow:
  LDI r6, 1
  SUB r8, r6
  JNZ r8, not_green
  LDI r5, 0x00FF00   ; green
  PSET r4, r21, r5
  JMP next_pixel
not_green:
  LDI r6, 1
  SUB r8, r6
  JNZ r8, not_cyan
  LDI r5, 0x00FFFF   ; cyan
  PSET r4, r21, r5
  JMP next_pixel
not_cyan:
  LDI r6, 1
  SUB r8, r6
  JNZ r8, not_blue
  LDI r5, 0x0000FF   ; blue
  PSET r4, r21, r5
  JMP next_pixel
not_blue:
  LDI r5, 0xFF00FF   ; magenta (default)
  PSET r4, r21, r5

next_pixel:
  ; x++
  LDI r6, 1
  ADD r4, r6
  ; If x == 256, next row
  LDI r6, 0
  ADD r6, r4
  SUB r6, r2
  JZ r6, next_row
  JMP x_loop

next_row:
  ; y++
  LDI r6, 1
  ADD r21, r6
  ; If y == 256, done
  LDI r6, 0
  ADD r6, r21
  SUB r6, r2
  JZ r6, done
  JMP y_loop

done:
HALT
