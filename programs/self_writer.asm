; self_writer.asm
; A program that writes a NEW program to the canvas, compiles it, and runs it.
; Generation 1: Writes "LDI r1, 42 \n HALT" to canvas.
; Successor: Sets r1 to 42 and halts.

  LDI r1, 0x8000    ; Start of canvas RAM
  LDI r3, 1         ; Increment
  
  ; Write "LDI r1, 42" to canvas
  LDI r2, 0x4C      ; 'L'
  STORE r1, r2
  ADD r1, r3
  LDI r2, 0x44      ; 'D'
  STORE r1, r2
  ADD r1, r3
  LDI r2, 0x49      ; 'I'
  STORE r1, r2
  ADD r1, r3
  LDI r2, 0x20      ; ' '
  STORE r1, r2
  ADD r1, r3
  LDI r2, 0x72      ; 'r'
  STORE r1, r2
  ADD r1, r3
  LDI r2, 0x31      ; '1'
  STORE r1, r2
  ADD r1, r3
  LDI r2, 0x2C      ; ','
  STORE r1, r2
  ADD r1, r3
  LDI r2, 0x20      ; ' '
  STORE r1, r2
  ADD r1, r3
  LDI r2, 0x34      ; '4'
  STORE r1, r2
  ADD r1, r3
  LDI r2, 0x32      ; '2'
  STORE r1, r2
  ADD r1, r3
  LDI r2, 0x0A      ; '\n'
  STORE r1, r2
  ADD r1, r3
  
  ; Write "HALT"
  LDI r2, 0x48      ; 'H'
  STORE r1, r2
  ADD r1, r3
  LDI r2, 0x41      ; 'A'
  STORE r1, r2
  ADD r1, r3
  LDI r2, 0x4C      ; 'L'
  STORE r1, r2
  ADD r1, r3
  LDI r2, 0x54      ; 'T'
  STORE r1, r2
  ADD r1, r3
  
  ; Null-terminate
  LDI r2, 0
  STORE r1, r2

  ; Assemble the canvas text
  ASMSELF
  
  ; Run the newly assembled code at 0x1000
  RUNNEXT
