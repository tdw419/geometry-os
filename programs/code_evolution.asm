; code_evolution.asm
; Generation 0: The Ancestor.
; Writes Generation 1 to the canvas and executes it.

  LDI r1, 0x10      ; x
  LDI r2, 0x10      ; y
  STRO r1, "GEN 0: ANCESTOR"
  
  LDI r1, 0x8000    ; Canvas start
  LDI r3, 1
  
  ; Write Gen 1 code to canvas
  ; Gen 1: STRO 0x10, 0x20, "GEN 1: SUCCESSOR" \n HALT
  
  LDI r2, 0x4C ; 'L'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x44 ; 'D'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x49 ; 'I'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x20 ; ' '
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x72 ; 'r'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x31 ; '1'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x2C ; ','
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x20 ; ' '
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x31 ; '1'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x36 ; '6'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x0A ; '\n'
  STORE r1, r2; ADD r1, r3
  
  LDI r2, 0x4C ; 'L'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x44 ; 'D'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x49 ; 'I'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x20 ; ' '
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x72 ; 'r'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x32 ; '2'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x2C ; ','
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x20 ; ' '
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x33 ; '3'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x32 ; '2'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x0A ; '\n'
  STORE r1, r2; ADD r1, r3
  
  LDI r2, 0x53 ; 'S'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x54 ; 'T'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x52 ; 'R'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x4F ; 'O'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x20 ; ' '
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x72 ; 'r'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x31 ; '1'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x2C ; ','
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x20 ; ' '
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x22 ; '"'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x47 ; 'G'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x45 ; 'E'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x4E ; 'N'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x20 ; ' '
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x31 ; '1'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x22 ; '"'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x0A ; '\n'
  STORE r1, r2; ADD r1, r3
  
  LDI r2, 0x48 ; 'H'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x41 ; 'A'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x4C ; 'L'
  STORE r1, r2; ADD r1, r3
  LDI r2, 0x54 ; 'T'
  STORE r1, r2; ADD r1, r3
  
  LDI r2, 0
  STORE r1, r2
  
  ASMSELF
  RUNNEXT
