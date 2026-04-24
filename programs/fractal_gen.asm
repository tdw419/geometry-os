; fractal_gen.asm - Self-Modifying Mandelbrot Fractal Generator
;
; Generates a 4x4 Mandelbrot visualization via self-modification:
; 1. Compute iteration counts for 16 points in the complex plane
; 2. Map iterations to colors
; 3. Generate PSETI instructions on canvas as assembly text
; 4. Self-assemble and run the generated code
;
; Demonstrates: fixed-point math, iteration-to-color mapping,
;   code generation, ASMSELF, RUNNEXT

LDI r7, 1
LDI r14, 1
LDI r30, 0xFF00

; ===== Pre-computed cr/ci values in RAM =====
; 16 complex numbers sampling the Mandelbrot set
; cr (real part) stored at 0x3000-0x300F, ci (imag) at 0x3010-0x301F
; Using values near the main cardioid for interesting variation
; cr values: -2.0, -1.0, -0.5, 0.0 (scaled by 10000)
; ci values: -1.5, -0.5, 0.3, 1.0 (scaled by 10000)

; cr[0..3] = -20000, -10000, -5000, 0
LDI r1, 0x3000
LDI r2, 4294947296       ; -20000 as u32
STORE r1, r2
ADD r1, r7
LDI r2, 4294957296       ; -10000
STORE r1, r2
ADD r1, r7
LDI r2, 4294962296       ; -5000
STORE r1, r2
ADD r1, r7
LDI r2, 0
STORE r1, r2

; ci[0..3] = -15000, -5000, 3000, 10000
LDI r1, 0x3010
LDI r2, 4294952296       ; -15000
STORE r1, r2
ADD r1, r7
LDI r2, 4294962296       ; -5000
STORE r1, r2
ADD r1, r7
LDI r2, 3000
STORE r1, r2
ADD r1, r7
LDI r2, 10000
STORE r1, r2

; Canvas write pointer
LDI r8, 0x8000

; Outer loop: y tile (0-3)
LDI r25, 0

y_loop:
  LDI r26, 0

  x_loop:
    ; Load cr
    LDI r1, 0x3000
    ADD r1, r26
    LOAD r22, r1

    ; Load ci
    LDI r1, 0x3010
    ADD r1, r25
    LOAD r23, r1

    ; Simple iteration counter
    ; z = 0+0i, iterate z = z^2 + c
    ; Use absolute value check: if |zr|>20000 or |zi|>20000, escaped
    LDI r18, 0             ; z_real
    LDI r19, 0             ; z_imag
    LDI r10, 0             ; count

    iter_loop:
      ; Escape check using SAR to detect sign
      ; If zr > 20000 or zr < -20000, escape
      MOV r20, r18
      LDI r21, 20000
      ADD r20, r21         ; zr + 20000 (should be < 40000 if not escaped)
      LDI r21, 40000
      CMP r20, r21
      BGE r0, escaped

      MOV r20, r19
      LDI r21, 20000
      ADD r20, r21
      LDI r21, 40000
      CMP r20, r21
      BGE r0, escaped

      ; z_new_real = (zr*zr - zi*zi)/10000 + cr
      MOV r20, r18
      MUL r20, r18         ; zr^2 (scaled by 10^8)
      LDI r21, 10000
      DIV r20, r21         ; zr^2/10000
      MOV r21, r19
      MUL r21, r19
      LDI r3, 10000
      DIV r21, r3
      SUB r20, r21         ; zr^2/10000 - zi^2/10000
      ADD r20, r22         ; + cr
      ; r20 = new z_real

      ; z_new_imag = 2*zr*zi/10000 + ci
      MOV r21, r18
      MUL r21, r19
      LDI r3, 5000
      DIV r21, r3          ; 2*zr*zi/10000
      ADD r21, r23

      MOV r18, r20
      MOV r19, r21

      ADD r10, r7

      ; Max 10 iterations
      LDI r3, 10
      CMP r10, r3
      BLT r0, iter_loop

    escaped:
    ; Map iteration to color
    ; 0-1 = in set (black), 2 = dark, 3-4 = blue, 5-6 = cyan,
    ; 7-8 = green, 9 = yellow, 10 = red
    LDI r24, 0

    LDI r3, 2
    CMP r10, r3
    BLT r0, gen_code
    LDI r24, 0x330033

    LDI r3, 3
    CMP r10, r3
    BLT r0, gen_code
    LDI r24, 0x0000FF

    LDI r3, 5
    CMP r10, r3
    BLT r0, gen_code
    LDI r24, 0x00FFFF

    LDI r3, 7
    CMP r10, r3
    BLT r0, gen_code
    LDI r24, 0x00FF00

    LDI r3, 9
    CMP r10, r3
    BLT r0, gen_code
    LDI r24, 0xFFFF00

    LDI r3, 10
    CMP r10, r3
    BLT r0, gen_code
    LDI r24, 0xFF0000

    gen_code:
    ; Write "PSETI " to canvas
    CALL write_pseti_prefix

    ; Write x coordinate
    MOV r11, r26
    LDI r3, 64
    MUL r11, r3
    CALL write_coord

    ; Write ", "
    LDI r2, 44
    STORE r8, r2
    ADD r8, r14
    LDI r2, 32
    STORE r8, r2
    ADD r8, r14

    ; Write y coordinate
    MOV r11, r25
    LDI r3, 64
    MUL r11, r3
    CALL write_coord

    ; Write ", 0x"
    LDI r2, 44
    STORE r8, r2
    ADD r8, r14
    LDI r2, 32
    STORE r8, r2
    ADD r8, r14
    LDI r2, 48
    STORE r8, r2
    ADD r8, r14
    LDI r2, 120
    STORE r8, r2
    ADD r8, r14

    ; Write 8 hex digits of color
    MOV r10, r24
    CALL write_hex8

    ; Newline
    LDI r2, 10
    STORE r8, r2
    ADD r8, r14

    ; Next tile
    ADD r26, r7
    LDI r3, 4
    CMP r26, r3
    BLT r0, x_loop

  ADD r25, r7
  LDI r3, 4
  CMP r25, r3
  BLT r0, y_loop

; HALT
LDI r2, 72
STORE r8, r2
ADD r8, r14
LDI r2, 65
STORE r8, r2
ADD r8, r14
LDI r2, 76
STORE r8, r2
ADD r8, r14
LDI r2, 84
STORE r8, r2

; Clear, assemble, run
LDI r1, 0
FILL r1
ASMSELF
RUNNEXT
HALT

; ===== Subroutines =====

write_pseti_prefix:
  PUSH r31
  LDI r2, 80
  STORE r8, r2
  ADD r8, r14
  LDI r2, 83
  STORE r8, r2
  ADD r8, r14
  LDI r2, 69
  STORE r8, r2
  ADD r8, r14
  LDI r2, 84
  STORE r8, r2
  ADD r8, r14
  LDI r2, 73
  STORE r8, r2
  ADD r8, r14
  LDI r2, 32
  STORE r8, r2
  ADD r8, r14
  POP r31
  RET

; write_coord: writes r11 (0-255) as 3 decimal digits
write_coord:
  PUSH r31
  PUSH r5
  ; Hundreds
  LDI r3, 100
  MOV r5, r11
  DIV r5, r3
  LDI r4, 48
  ADD r5, r4
  STORE r8, r5
  ADD r8, r14
  ; Remove hundreds
  LDI r3, 100
  MOV r5, r11
  DIV r5, r3
  MUL r5, r3
  MOV r4, r11
  SUB r4, r5
  MOV r11, r4
  ; Tens
  LDI r3, 10
  MOV r5, r11
  DIV r5, r3
  LDI r4, 48
  ADD r5, r4
  STORE r8, r5
  ADD r8, r14
  ; Remove tens
  LDI r3, 10
  MOV r5, r11
  DIV r5, r3
  MUL r5, r3
  MOV r4, r11
  SUB r4, r5
  ; Ones
  LDI r5, 48
  ADD r4, r5
  STORE r8, r4
  ADD r8, r14
  POP r5
  POP r31
  RET

; write_hex8: writes r10 as 8 hex digits
write_hex8:
  PUSH r31
  PUSH r11
  PUSH r15
  LDI r11, 28
  LDI r15, 8

hex8_loop:
  MOV r2, r10
  MOV r3, r11
  SHR r2, r3
  LDI r3, 15
  AND r2, r3
  LDI r3, 10
  CMP r2, r3
  BGE r0, hex8_alpha
  LDI r3, 48
  ADD r2, r3
  JMP hex8_write
hex8_alpha:
  LDI r3, 55
  ADD r2, r3
hex8_write:
  STORE r8, r2
  ADD r8, r14
  LDI r5, 4
  SUB r11, r5
  SUB r15, r7
  JNZ r15, hex8_loop
  POP r15
  POP r11
  POP r31
  RET
