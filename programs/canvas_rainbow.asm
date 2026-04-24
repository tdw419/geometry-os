; canvas_rainbow.asm
; Self-modifying demo - writes a rainbow diagonal generator to canvas
; Writes a program to the canvas at 0x8000, then ASMSELF+RUNNEXT.

  LDI r7, 0x8000    ; canvas buffer start
  LDI r3, 1         ; increment
  LDI r2, 76      ; 'L'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 68      ; 'D'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 73      ; 'I'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 32
  STORE r7, r2
  ADD r7, r3
  LDI r2, 114      ; 'r'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 49      ; '1'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 48      ; '0'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 44
  STORE r7, r2
  ADD r7, r3
  LDI r2, 32
  STORE r7, r2
  ADD r7, r3
  LDI r2, 48      ; '0'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 10
  STORE r7, r2
  ADD r7, r3
  LDI r2, 76      ; 'L'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 68      ; 'D'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 73      ; 'I'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 32
  STORE r7, r2
  ADD r7, r3
  LDI r2, 114      ; 'r'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 49      ; '1'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 49      ; '1'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 44
  STORE r7, r2
  ADD r7, r3
  LDI r2, 32
  STORE r7, r2
  ADD r7, r3
  LDI r2, 49      ; '1'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 10
  STORE r7, r2
  ADD r7, r3
  LDI r2, 76      ; 'L'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 68      ; 'D'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 73      ; 'I'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 32
  STORE r7, r2
  ADD r7, r3
  LDI r2, 114      ; 'r'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 49      ; '1'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 50      ; '2'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 44
  STORE r7, r2
  ADD r7, r3
  LDI r2, 32
  STORE r7, r2
  ADD r7, r3
  LDI r2, 50      ; '2'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 53      ; '5'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 54      ; '6'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 10
  STORE r7, r2
  ADD r7, r3
  LDI r2, 108      ; 'l'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 111      ; 'o'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 111      ; 'o'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 112      ; 'p'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 58
  STORE r7, r2
  ADD r7, r3
  LDI r2, 10
  STORE r7, r2
  ADD r7, r3
  LDI r2, 80      ; 'P'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 83      ; 'S'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 69      ; 'E'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 84      ; 'T'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 73      ; 'I'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 32
  STORE r7, r2
  ADD r7, r3
  LDI r2, 114      ; 'r'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 49      ; '1'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 48      ; '0'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 44
  STORE r7, r2
  ADD r7, r3
  LDI r2, 32
  STORE r7, r2
  ADD r7, r3
  LDI r2, 114      ; 'r'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 49      ; '1'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 48      ; '0'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 44
  STORE r7, r2
  ADD r7, r3
  LDI r2, 32
  STORE r7, r2
  ADD r7, r3
  LDI r2, 114      ; 'r'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 49      ; '1'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 50      ; '2'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 10
  STORE r7, r2
  ADD r7, r3
  LDI r2, 65      ; 'A'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 68      ; 'D'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 68      ; 'D'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 32
  STORE r7, r2
  ADD r7, r3
  LDI r2, 114      ; 'r'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 49      ; '1'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 48      ; '0'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 44
  STORE r7, r2
  ADD r7, r3
  LDI r2, 32
  STORE r7, r2
  ADD r7, r3
  LDI r2, 114      ; 'r'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 49      ; '1'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 49      ; '1'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 10
  STORE r7, r2
  ADD r7, r3
  LDI r2, 67      ; 'C'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 77      ; 'M'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 80      ; 'P'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 32
  STORE r7, r2
  ADD r7, r3
  LDI r2, 114      ; 'r'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 49      ; '1'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 48      ; '0'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 44
  STORE r7, r2
  ADD r7, r3
  LDI r2, 32
  STORE r7, r2
  ADD r7, r3
  LDI r2, 114      ; 'r'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 49      ; '1'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 50      ; '2'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 10
  STORE r7, r2
  ADD r7, r3
  LDI r2, 66      ; 'B'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 76      ; 'L'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 84      ; 'T'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 32
  STORE r7, r2
  ADD r7, r3
  LDI r2, 114      ; 'r'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 48      ; '0'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 44
  STORE r7, r2
  ADD r7, r3
  LDI r2, 32
  STORE r7, r2
  ADD r7, r3
  LDI r2, 108      ; 'l'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 111      ; 'o'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 111      ; 'o'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 112      ; 'p'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 10
  STORE r7, r2
  ADD r7, r3
  LDI r2, 72      ; 'H'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 65      ; 'A'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 76      ; 'L'
  STORE r7, r2
  ADD r7, r3
  LDI r2, 84      ; 'T'
  STORE r7, r2
  ADD r7, r3
  ; Null-terminate
  LDI r2, 0
  STORE r7, r2

  ; Assemble the canvas text and run it
  ASMSELF
  RUNNEXT