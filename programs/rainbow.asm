; RAINBOW -- Diagonal pattern
LDI r1, 0      ; x
LDI r2, 0      ; y
LDI r3, 256    ; limit
LDI r4, 0      ; color (temp)
LDI r5, 1      ; increment
LDI r6, 6      ; color count

loop_y:
  LDI r1, 0    ; x = 0
loop_x:
  ; color = (x + y) % 6
  LDI r7, 0
  ADD r7, r1   ; temp = x
  ADD r7, r2   ; temp = x + y
  MOD r7, r6   ; temp = (x + y) % 6
  
  ; select color from index in r7
  ; 0: red, 1: orange, 2: yellow, 3: green, 4: blue, 5: purple
  LDI r8, 0
  CMP r7, r8
  JZ r0, set_red
  
  LDI r8, 1
  CMP r7, r8
  JZ r0, set_orange
  
  LDI r8, 2
  CMP r7, r8
  JZ r0, set_yellow
  
  LDI r8, 3
  CMP r7, r8
  JZ r0, set_green
  
  LDI r8, 4
  CMP r7, r8
  JZ r0, set_blue
  
  ; else purple
  LDI r4, 0x800080
  JMP draw

set_red:
  LDI r4, 0xFF0000
  JMP draw
set_orange:
  LDI r4, 0xFFA500
  JMP draw
set_yellow:
  LDI r4, 0xFFFF00
  JMP draw
set_green:
  LDI r4, 0x00FF00
  JMP draw
set_blue:
  LDI r4, 0x0000FF

draw:
  PSET r1, r2, r4
  ADD r1, r5   ; x++
  CMP r1, r3
  JZ r0, next_y
  JMP loop_x

next_y:
  ADD r2, r5   ; y++
  CMP r2, r3
  JZ r0, done
  JMP loop_y

done:
  HALT
