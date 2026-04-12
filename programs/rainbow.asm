; RAINBOW -- Diagonal rainbow stripes across the screen
; Each pixel (x,y) gets color based on (x+y) mod 6
; Colors: red, orange, yellow, green, blue, purple

LDI r0, 0            ; r0 = y
LDI r2, 256          ; r2 = limit

y_loop:
  LDI r1, 0          ; r1 = x

x_loop:
  ; color_index = (x + y) % 6
  LDI r5, 0
  ADD r5, r1         ; r5 = x
  ADD r5, r0         ; r5 = x + y

  ; Mod 6 by repeated subtraction
  LDI r9, 6
mod_loop:
  SUB r5, r9
  LDI r6, 0
  CMP r5, r6
  BLT r5, r6, mod_done
  JMP mod_loop
mod_done:
  ADD r5, r9         ; undo last sub
  ; r5 = color index (0..5)

  ; Color lookup
  JNZ r5, not_red
  LDI r3, 0xFF0000
  PSET r1, r0, r3
  JMP next_pixel
not_red:
  LDI r6, 1
  SUB r5, r6
  JNZ r5, not_orange
  LDI r3, 0xFF8800
  PSET r1, r0, r3
  JMP next_pixel
not_orange:
  LDI r6, 1
  SUB r5, r6
  JNZ r5, not_yellow
  LDI r3, 0xFFFF00
  PSET r1, r0, r3
  JMP next_pixel
not_yellow:
  LDI r6, 1
  SUB r5, r6
  JNZ r5, not_green
  LDI r3, 0x00FF00
  PSET r1, r0, r3
  JMP next_pixel
not_green:
  LDI r6, 1
  SUB r5, r6
  JNZ r5, not_blue
  LDI r3, 0x0088FF
  PSET r1, r0, r3
  JMP next_pixel
not_blue:
  LDI r3, 0x8800FF
  PSET r1, r0, r3

next_pixel:
  LDI r6, 1
  ADD r1, r6
  LDI r6, 0
  ADD r6, r1
  SUB r6, r2
  JZ r6, next_row
  JMP x_loop

next_row:
  LDI r6, 1
  ADD r0, r6
  LDI r6, 0
  ADD r6, r0
  SUB r6, r2
  JZ r6, done
  JMP y_loop

done:
HALT
