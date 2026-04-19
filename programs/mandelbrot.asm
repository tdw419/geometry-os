; mandelbrot.asm -- Mandelbrot set fractal renderer
;
; Renders the classic Mandelbrot set to the full 256x256 screen using
; fixed-point arithmetic (8.8 format, scale factor = 256).
;
; Coordinate mapping (centered on the classic Mandelbrot view):
;   cx = px * 3 - 640  (real part, range: -2.50 to 0.49)
;   cy = py * 3 - 384  (imaginary part, range: -1.50 to 1.49)
;
; Iteration: z = z^2 + c, escape when |z|^2 > 4
; Max iterations: 64 (points inside set render as black)
;
; Color scheme: psychedelic palette based on iteration count
;   R = (iter * 37) & 0xFF
;   G = (iter * 67 + 100) & 0xFF
;   B = (iter * 97 + 200) & 0xFF
;
; Register allocation:
;   r1  = px (x pixel counter, 0-255)
;   r2  = py (y pixel counter, 0-255)
;   r3  = iter (iteration counter)
;   r4  = c_r (real part of c, fixed-point)
;   r5  = c_i (imaginary part of c, fixed-point)
;   r6  = z_r (real part of z, fixed-point)
;   r7  = z_i (imaginary part of z, fixed-point)
;   r8  = z_r^2 (scratch, scale S^2)
;   r9  = z_i^2 (scratch, scale S^2)
;   r10-r12 = scratch / color

; === CONSTANTS ===
LDI r13, 64       ; max_iter = 64
LDI r14, 0x40000  ; escape_radius_sq = 4 * 256^2 = 262144
LDI r2, 0         ; py = 0

; === MAIN Y LOOP ===
y_loop:
  LDI r1, 0       ; px = 0

  ; === MAIN X LOOP ===
  x_loop:
    ; Compute c_r = px * 3 - 640
    MOV r4, r1
    LDI r10, 3
    MUL r4, r10
    SUBI r4, 640

    ; Compute c_i = py * 3 - 384
    MOV r5, r2
    LDI r10, 3
    MUL r5, r10
    SUBI r5, 384

    ; z = 0, iter = 0
    LDI r6, 0
    LDI r7, 0
    LDI r3, 0

  ; === ITERATION LOOP ===
  iter_loop:
    ; z_r^2
    MOV r8, r6
    MUL r8, r6

    ; z_i^2
    MOV r9, r7
    MUL r9, r7

    ; Escape check: z_r^2 + z_i^2 > escape_radius_sq
    ADD r8, r9
    CMP r8, r14
    BLT r0, not_escaped
    JZ r0, not_escaped
    JMP escaped

  not_escaped:
    ; Max iterations check
    CMP r3, r13
    BLT r0, do_iter
    JMP inside_set

  do_iter:
    ; z_i_new = 2 * z_r * z_i / S + c_i = (z_r * z_i) >> 7 + c_i
    MOV r10, r6
    MUL r10, r7
    SARI r10, 7
    ADD r10, r5
    MOV r7, r10

    ; z_r_new = (z_r^2 - z_i^2) / S + c_r
    SUB r8, r9
    SARI r8, 8
    ADD r8, r4
    MOV r6, r8

    ; iter++
    ADDI r3, 1
    JMP iter_loop

  ; === COLOR: inside set (black) ===
  inside_set:
    LDI r12, 0
    JMP draw_pixel

  ; === COLOR: escaped (palette based on iter) ===
  escaped:
    ; R = (iter * 37) & 0xFF
    MOV r12, r3
    LDI r10, 37
    MUL r12, r10
    LDI r10, 0xFF
    AND r12, r10

    ; G = (iter * 67 + 100) & 0xFF, shifted left 8
    MOV r10, r3
    LDI r11, 67
    MUL r10, r11
    ADDI r10, 100
    LDI r11, 0xFF
    AND r10, r11
    SHLI r10, 8
    OR r12, r10

    ; B = (iter * 97 + 200) & 0xFF, shifted left 16
    MOV r10, r3
    LDI r11, 97
    MUL r10, r11
    ADDI r10, 200
    LDI r11, 0xFF
    AND r10, r11
    SHLI r10, 8
    SHLI r10, 8
    OR r12, r10

  draw_pixel:
    PSET r1, r2, r12

    ; px++
    ADDI r1, 1
    LDI r10, 256
    CMP r1, r10
    JZ r0, end_row
    JMP x_loop

  end_row:
    ; py++
    ADDI r2, 1
    LDI r10, 256
    CMP r2, r10
    JZ r0, done
    JMP y_loop

done:
    HALT
