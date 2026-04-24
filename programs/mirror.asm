; mirror.asm - Self-Modifying Demo: Screen Pixel Mirror
;
; Draws 4 colored dots on screen, reads their pixels via SCREENP,
; generates PSETI instructions on canvas, then self-assembles
; and runs the generated code to reproduce the dots at new positions.
;
; Demonstrates: SCREENP (screen readback), canvas text generation,
;   ASMSELF (self-assembly), RUNNEXT (execute generated code)

LDI r7, 1
LDI r14, 1

; ===== Phase 0: Draw colored dots =====
LDI r1, 0xFF0000
LDI r2, 32
LDI r3, 32
PSET r2, r3, r1

LDI r1, 0x00FF00
LDI r2, 224
PSET r2, r3, r1

LDI r3, 224
LDI r1, 0x0000FF
LDI r2, 32
PSET r2, r3, r1

LDI r1, 0xFFFF00
LDI r2, 224
PSET r2, r3, r1

; ===== Phase 1: Read pixel and generate code =====
LDI r20, 32
LDI r21, 32
SCREENP r10, r20, r21   ; r10 = red pixel

LDI r8, 0x8000          ; canvas buffer base

; Write "PSETI 40, 40, 0x" 
LDI r2, 80              ; 'P'
STORE r8, r2
ADD r8, r14
LDI r2, 83              ; 'S'
STORE r8, r2
ADD r8, r14
LDI r2, 69              ; 'E'
STORE r8, r2
ADD r8, r14
LDI r2, 84              ; 'T'
STORE r8, r2
ADD r8, r14
LDI r2, 73              ; 'I'
STORE r8, r2
ADD r8, r14
LDI r2, 32              ; ' '
STORE r8, r2
ADD r8, r14
LDI r2, 52              ; '4'
STORE r8, r2
ADD r8, r14
LDI r2, 48              ; '0'
STORE r8, r2
ADD r8, r14
LDI r2, 44              ; ','
STORE r8, r2
ADD r8, r14
LDI r2, 32              ; ' '
STORE r8, r2
ADD r8, r14
LDI r2, 52              ; '4'
STORE r8, r2
ADD r8, r14
LDI r2, 48              ; '0'
STORE r8, r2
ADD r8, r14
LDI r2, 44              ; ','
STORE r8, r2
ADD r8, r14
LDI r2, 32              ; ' '
STORE r8, r2
ADD r8, r14
LDI r2, 48              ; '0'
STORE r8, r2
ADD r8, r14
LDI r2, 120             ; 'x'
STORE r8, r2
ADD r8, r14

; Convert r10 (pixel color) to 8 hex digits (full 32-bit value)
LDI r11, 28
LDI r15, 8

hex_loop:
  MOV r2, r10
  MOV r3, r11
  SHR r2, r3
  LDI r3, 15
  AND r2, r3            ; r2 = nibble (0-15)
  LDI r3, 10
  CMP r2, r3
  BGE r0, hex_alpha
  LDI r3, 48
  ADD r2, r3            ; 0-9 -> '0'-'9'
  JMP hex_write
hex_alpha:
  LDI r3, 55
  ADD r2, r3            ; 10-15 -> 'A'-'F'
hex_write:
  STORE r8, r2
  ADD r8, r14
  LDI r3, 4
  SUB r11, r3
  SUB r15, r7
  JNZ r15, hex_loop

; Write newline
LDI r2, 10
STORE r8, r2
ADD r8, r14

; Write HALT
LDI r2, 72              ; 'H'
STORE r8, r2
ADD r8, r14
LDI r2, 65              ; 'A'
STORE r8, r2
ADD r8, r14
LDI r2, 76              ; 'L'
STORE r8, r2
ADD r8, r14
LDI r2, 84              ; 'T'
STORE r8, r2

; ===== Phase 2: Clear screen, self-assemble, run =====
LDI r1, 0
FILL r1

ASMSELF
RUNNEXT

HALT
