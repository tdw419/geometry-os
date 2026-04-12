; CHECKERBOARD -- 8x8 pixel squares, alternating black and white
; cell_row = y/8, cell_col = x/8, tracked with counters
; Color: white if (cell_row+cell_col) is even, black if odd

LDI r0, 0            ; r0 = y
LDI r2, 256          ; r2 = limit
LDI r3, 8            ; r3 = cell size
LDI r10, 0x000000    ; r10 = black
LDI r11, 0xFFFFFF    ; r11 = white
LDI r12, 0           ; r12 = cell_row
LDI r13, 0           ; r13 = y-in-cell counter (0..7)

y_loop:
  LDI r14, 0         ; r14 = cell_col (reset each row)
  LDI r15, 0         ; r15 = x-in-cell counter (0..7)
  LDI r1, 0          ; r1 = x

x_loop:
  ; Color = (cell_row + cell_col) & 1
  LDI r6, 0
  ADD r6, r12
  ADD r6, r14        ; r6 = cell_row + cell_col
  LDI r7, 1
  AND r6, r7         ; r6 = parity (0 or 1)

  ; Draw pixel
  JZ r6, draw_white
  LDI r6, 0
  ADD r6, r10
  PSET r1, r0, r6
  JMP advance_x

draw_white:
  LDI r6, 0
  ADD r6, r11
  PSET r1, r0, r6

advance_x:
  ; x++
  LDI r7, 1
  ADD r1, r7
  ; x-in-cell++
  ADD r15, r7

  ; If x-in-cell == 8, advance cell_col, reset counter
  LDI r6, 0
  ADD r6, r15
  SUB r6, r3
  JNZ r6, check_x_done
  ADD r14, r7         ; cell_col++ (r7=1)
  LDI r15, 0          ; reset x-in-cell

check_x_done:
  ; If x == 256, next row
  LDI r6, 0
  ADD r6, r1
  SUB r6, r2
  JZ r6, advance_y
  JMP x_loop

advance_y:
  ; y++
  LDI r7, 1
  ADD r0, r7
  ; y-in-cell++
  ADD r13, r7

  ; If y-in-cell == 8, advance cell_row, reset counter
  LDI r6, 0
  ADD r6, r13
  SUB r6, r3
  JNZ r6, check_y_done
  ADD r12, r7         ; cell_row++ (r7=1)
  LDI r13, 0          ; reset y-in-cell

check_y_done:
  ; If y == 256, done
  LDI r6, 0
  ADD r6, r0
  SUB r6, r2
  JZ r6, done
  JMP y_loop

done:
HALT
