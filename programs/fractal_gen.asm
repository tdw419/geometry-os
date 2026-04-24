; fractal_gen.asm - Self-Modifying Fractal Generator
;
; Generates a Sierpinski triangle by computing which pixels to color
; and writing PSETI assembly instructions to the canvas buffer.
; Then self-assembles and runs the generated code.
;
; Algorithm - pixel (x,y) is colored if (x AND y) equals 0
; 8x8 grid, each cell 32x32 pixels. About 36 colored pixels.

LDI r15, 1
LDI r14, 0x8000
LDI r30, 0xFF00

; Main loop - iterate 8x8 grid
LDI r11, 0

y_loop:
  LDI r10, 0

  x_loop:
    MOV r12, r10
    AND r12, r11
    JNZ r12, skip_pixel

    ; Color - red if y==0, yellow if x==0, green otherwise
    MOV r7, r11
    JNZ r7, not_top
    LDI r7, 0xFF0000
    JMP have_color
    not_top:
    MOV r7, r10
    JNZ r7, not_left
    LDI r7, 0xFFFF00
    JMP have_color
    not_left:
    LDI r7, 0x00FF00
    have_color:

    ; Write "PSETI XX,YY,0xHHHHHH\n" to canvas
    LDI r13, 80
    STORE r14, r13
    ADD r14, r15
    LDI r13, 83
    STORE r14, r13
    ADD r14, r15
    LDI r13, 69
    STORE r14, r13
    ADD r14, r15
    LDI r13, 84
    STORE r14, r13
    ADD r14, r15
    LDI r13, 73
    STORE r14, r13
    ADD r14, r15
    LDI r13, 32
    STORE r14, r13
    ADD r14, r15

    ; X coord = x * 32 as decimal
    MOV r13, r10
    LDI r16, 32
    MUL r13, r16
    CALL write_dec

    LDI r13, 44
    STORE r14, r13
    ADD r14, r15

    ; Y coord = y * 32 as decimal
    MOV r13, r11
    LDI r16, 32
    MUL r13, r16
    CALL write_dec

    LDI r13, 44
    STORE r14, r13
    ADD r14, r15

    ; Color as hex "0xHHHHHH"
    LDI r13, 48
    STORE r14, r13
    ADD r14, r15
    LDI r13, 120
    STORE r14, r13
    ADD r14, r15

    ; Write 6 hex nibbles from r7
    PUSH r31
    PUSH r7
    PUSH r6
    LDI r6, 20
    hex_loop:
      MOV r13, r7
      MOV r16, r6
      SHR r13, r16
      LDI r16, 15
      AND r13, r16
      LDI r16, 10
      CMP r13, r16
      BGE r0, hex_alpha
      LDI r16, 48
      ADD r13, r16
      JMP hex_write
      hex_alpha:
      LDI r16, 55
      ADD r13, r16
      hex_write:
      STORE r14, r13
      ADD r14, r15
      LDI r13, 4
      SUB r6, r13
      LDI r13, 31
      MOV r16, r6
      SAR r16, r13
      JNZ r16, hex_done
      JMP hex_loop
    hex_done:
    POP r6
    POP r7
    POP r31

    LDI r13, 10
    STORE r14, r13
    ADD r14, r15

    skip_pixel:
    ADD r10, r15
    LDI r13, 8
    CMP r10, r13
    BLT r0, x_loop

  ADD r11, r15
  LDI r13, 8
  CMP r11, r13
  BLT r0, y_loop

; Write HALT
LDI r13, 72
STORE r14, r13
ADD r14, r15
LDI r13, 65
STORE r14, r13
ADD r14, r15
LDI r13, 76
STORE r14, r13
ADD r14, r15
LDI r13, 84
STORE r14, r13
ADD r14, r15
LDI r13, 0
STORE r14, r13

; Self-assemble and run
ASMSELF
RUNNEXT

; Subroutine - write decimal number to canvas
; Input r13 = number (0-255)
;       r14 = canvas position (advances)
;       r15 = 1
; Clobbers r16, r17, r18
write_dec:
  PUSH r31
  JNZ r13, wd_nonzero
  LDI r13, 48
  STORE r14, r13
  ADD r14, r15
  JMP wd_done
  wd_nonzero:
  LDI r16, 100
  MOV r17, r13
  DIV r17, r16
  JZ r17, wd_tens
  LDI r18, 48
  ADD r17, r18
  STORE r14, r17
  ADD r14, r15
  MOV r17, r13
  DIV r17, r16
  MUL r17, r16
  SUB r13, r17
  wd_tens:
  LDI r16, 10
  MOV r17, r13
  DIV r17, r16
  LDI r18, 48
  ADD r17, r18
  STORE r14, r17
  ADD r14, r15
  MOV r17, r13
  DIV r17, r16
  MUL r17, r16
  SUB r13, r17
  LDI r16, 48
  ADD r13, r16
  STORE r14, r13
  ADD r14, r15
  wd_done:
  POP r31
  RET
