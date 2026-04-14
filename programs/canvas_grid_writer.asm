; canvas_grid_writer.asm
; Writes "PIXELS DRIVE PIXELS" to the canvas grid using STORE to 0x8000+

  LDI r1, 0x8000    ; Start of canvas RAM
  LDI r2, string    ; Address of our string
  LDI r3, 1         ; Increment
  
loop:
  LOAD r4, r2       ; Load char from string
  JZ r4, done       ; End of string?
  STORE r1, r4      ; Store to canvas RAM
  ADD r1, r3        ; Next canvas cell
  ADD r2, r3        ; Next char in string
  JMP loop

done:
  HALT

.org 0x100
string:
  .db "PIXELS DRIVE PIXELS", 0
