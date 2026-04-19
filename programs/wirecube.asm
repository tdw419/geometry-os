; wirecube.asm -- 3D rotating wireframe cube
;
; A wireframe cube tumbles in 3D space with perspective projection
; and depth-based edge coloring. Uses parabolic sin/cos approximation
; for rotation math -- no lookup table needed.
;
; Math:
;   sin(a) ≈ t*(32768-t) >> 20 where t = a folded to [0, 32767]
;   cos(a) = sin(a + 16384)
;   Y rotation: rx = (vx*cos - vz*sin) >> 8, rz = (vx*sin + vz*cos) >> 8
;   X rotation: ry = (vy*cos - rz*sin) >> 8, rz2 = (vy*sin + rz*cos) >> 8
;   Perspective: scale = 200 - (rz2 >> 3)
;                sx = (rx * scale) >> 8 + 128
;                sy = (ry * scale) >> 8 + 128
;
; Vertices stored as signed 8.8 fixed-point at 0x1000-0x1017.
; Coordinates are +/- 120 (fits in 8 bits, no overflow in multiply).
;
; Memory layout (word addresses):
;   0x1000-0x1017: 8 cube vertices (vx, vy, vz) in 8.8 fixed-point
;   0x1020-0x102F: 8 projected 2D points (sx, sy)
;   0x1030-0x1037: 8 rotated Z values (depth coloring)
;   0x1040-0x1057: 12 edge index pairs (va, vb)
;
; Register allocation:
;   r1  = angle_y
;   r2  = angle_x
;   r3  = loop counter
;   r4-r7 = scratch / pointers
;   r8-r11 = sin/cos values
;   r12-r19 = rotation scratch
;   r20 = edge counter
;   r21 = edge list pointer
;   r22-r23 = edge vertex indices
;   r24-r27 = LINE args (sx_a, sy_a, sx_b, sy_b)
;   r28 = color
;   r29 = temp

; ============================================================
; INITIALIZATION
; ============================================================

; --- Store 8 cube vertices at 0x1000 ---
; Vertex i: x = bit2 ? -120 : 120, y = bit1 ? -120 : 120, z = bit0 ? -120 : 120
; Using 120 keeps all multiply results within u32 range
LDI r3, 0
LDI r4, 0x1000

vert_init:
  MOV r5, r3
  ANDI r5, 4
  JNZ r5, xi_neg
  LDI r5, 120
  JMP xi_store
xi_neg:
  LDI r5, 0xFFFFFF88  ; -120 in u32
xi_store:
  STORE r4, r5
  ADDI r4, 1

  MOV r5, r3
  ANDI r5, 2
  JNZ r5, yi_neg
  LDI r5, 120
  JMP yi_store
yi_neg:
  LDI r5, 0xFFFFFF88
yi_store:
  STORE r4, r5
  ADDI r4, 1

  MOV r5, r3
  ANDI r5, 1
  JNZ r5, zi_neg
  LDI r5, 120
  JMP zi_store
zi_neg:
  LDI r5, 0xFFFFFF88
zi_store:
  STORE r4, r5
  ADDI r4, 1

  ADDI r3, 1
  LDI r29, 8
  CMP r3, r29
  BLT r0, vert_init

; --- Store 12 edge index pairs at 0x1040 ---
LDI r3, 0
LDI r4, 0x1040

edge_init:
  MOV r5, r3
  ANDI r5, 1
  JNZ r5, eisz
  STORE r4, r3
  ADDI r4, 1
  MOV r5, r3
  ORI r5, 1
  STORE r4, r5
  ADDI r4, 1
eisz:
  MOV r5, r3
  ANDI r5, 2
  JNZ r5, eisy
  STORE r4, r3
  ADDI r4, 1
  MOV r5, r3
  ORI r5, 2
  STORE r4, r5
  ADDI r4, 1
eisy:
  MOV r5, r3
  ANDI r5, 4
  JNZ r5, eisx
  STORE r4, r3
  ADDI r4, 1
  MOV r5, r3
  ORI r5, 4
  STORE r4, r5
  ADDI r4, 1
eisx:
  ADDI r3, 1
  LDI r29, 8
  CMP r3, r29
  BLT r0, edge_init

; Initialize rotation angles
LDI r1, 0
LDI r2, 0

; ============================================================
; MAIN ANIMATION LOOP
; ============================================================
main_loop:
  ; Clear screen to black
  LDI r28, 0
  FILL r28

  ; Update angles every 3rd frame
  LDI r29, 0xFFE
  LOAD r29, r29
  ANDI r29, 3
  JNZ r29, skip_angle
  ADDI r1, 800
  ANDI r1, 0xFFFF
  ADDI r2, 550
  ANDI r2, 0xFFFF
skip_angle:

  ; ============================================================
  ; COMPUTE sin_y, cos_y, sin_x, cos_x
  ; parabolic_sin(a): fold a to [0,32767], compute t*(32768-t)>>20
  ; ============================================================

  ; sin_y = parabolic_sin(angle_y)
  MOV r15, r1
  LDI r16, 0
  LDI r17, 32768
  CMP r15, r17
  BLT r0, sy_first
  LDI r16, 1
  SUB r15, r17
sy_first:
  MOV r17, r15
  LDI r8, 32768
  SUB r8, r17
  MUL r15, r8
  SHRI r15, 20
  JZ r16, sy_done
  NEG r15
sy_done:
  MOV r8, r15

  ; cos_y = parabolic_sin(angle_y + 16384)
  MOV r15, r1
  ADDI r15, 16384
  LDI r17, 65536
  CMP r15, r17
  BLT r0, cy_wrap
  SUB r15, r17
cy_wrap:
  LDI r16, 0
  LDI r17, 32768
  CMP r15, r17
  BLT r0, cy_first
  LDI r16, 1
  SUB r15, r17
cy_first:
  MOV r17, r15
  LDI r9, 32768
  SUB r9, r17
  MUL r15, r9
  SHRI r15, 20
  JZ r16, cy_done
  NEG r15
cy_done:
  MOV r9, r15

  ; sin_x = parabolic_sin(angle_x)
  MOV r15, r2
  LDI r16, 0
  LDI r17, 32768
  CMP r15, r17
  BLT r0, sx_first
  LDI r16, 1
  SUB r15, r17
sx_first:
  MOV r17, r15
  LDI r10, 32768
  SUB r10, r17
  MUL r15, r10
  SHRI r15, 20
  JZ r16, sx_done
  NEG r15
sx_done:
  MOV r10, r15

  ; cos_x = parabolic_sin(angle_x + 16384)
  MOV r15, r2
  ADDI r15, 16384
  LDI r17, 65536
  CMP r15, r17
  BLT r0, cx_wrap
  SUB r15, r17
cx_wrap:
  LDI r16, 0
  LDI r17, 32768
  CMP r15, r17
  BLT r0, cx_first
  LDI r16, 1
  SUB r15, r17
cx_first:
  MOV r17, r15
  LDI r11, 32768
  SUB r11, r17
  MUL r15, r11
  SHRI r15, 20
  JZ r16, cx_done
  NEG r15
cx_done:
  MOV r11, r15

  ; ============================================================
  ; ROTATE AND PROJECT ALL 8 VERTICES
  ; ============================================================
  LDI r3, 0
  LDI r29, 8

vert_loop:
  ; Compute vertex RAM address: 0x1000 + i*3
  MOV r4, r3
  SHLI r4, 1
  ADD r4, r3
  ADDI r4, 0x1000

  ; Load vertex (vx, vy, vz)
  LOAD r5, r4       ; vx
  ADDI r4, 1
  LOAD r6, r4       ; vy
  ADDI r4, 1
  LOAD r7, r4       ; vz

  ; Y-axis rotation:
  ; rx = (vx * cos_y - vz * sin_y) >> 8
  ; rz = (vx * sin_y + vz * cos_y) >> 8
  ; With 8-bit coords and 8.8 sin/cos (max 256), products fit in u32
  MOV r12, r5
  MUL r12, r9       ; vx * cos_y
  MOV r13, r7
  MUL r13, r8       ; vz * sin_y
  SUB r12, r13      ; vx*cos_y - vz*sin_y
  SARI r12, 8       ; rx

  MOV r14, r5
  MUL r14, r8       ; vx * sin_y
  MOV r17, r7
  MUL r17, r9       ; vz * cos_y
  ADD r14, r17      ; vx*sin_y + vz*cos_y
  SARI r14, 8       ; rz

  ; X-axis rotation:
  ; ry = (vy * cos_x - rz * sin_x) >> 8
  ; rz2 = (vy * sin_x + rz * cos_x) >> 8
  MOV r16, r6
  MUL r16, r11      ; vy * cos_x
  MOV r17, r14
  MUL r17, r10      ; rz * sin_x
  SUB r16, r17      ; vy*cos_x - rz*sin_x
  SARI r16, 8       ; ry

  MOV r17, r6
  MUL r17, r10      ; vy * sin_x
  MOV r15, r14
  MUL r15, r11      ; rz * cos_x
  ADD r17, r15      ; vy*sin_x + rz*cos_x
  SARI r17, 8       ; rz2

  ; Store rotated Z at 0x1030 + i
  MOV r4, r3
  ADDI r4, 0x1030
  STORE r4, r17

  ; Perspective: scale = 200 - (rz2 >> 3)
  ; rz2 ranges from about -170 to 170 (after rotation of +/-120)
  ; So rz2>>3 ranges from -21 to 21, scale ranges from 179 to 221
  MOV r19, r17
  SARI r19, 3
  LDI r4, 200
  SUB r19, r4       ; scale = 200 - (rz2>>3)

  ; sx = (rx * scale) >> 8 + 128
  MOV r18, r12
  MUL r18, r19
  SARI r18, 8
  ADDI r18, 128

  ; Store sx at 0x1020 + i*2
  MOV r4, r3
  SHLI r4, 1
  ADDI r4, 0x1020
  STORE r4, r18

  ; sy = (ry * scale) >> 8 + 128
  MOV r18, r16
  MUL r18, r19
  SARI r18, 8
  ADDI r18, 128

  ; Store sy at 0x1020 + i*2 + 1
  ADDI r4, 1
  STORE r4, r18

  ; Next vertex
  ADDI r3, 1
  CMP r3, r29
  BLT r0, vert_loop

  ; ============================================================
  ; DRAW 12 EDGES WITH DEPTH-BASED COLORING
  ; ============================================================
  LDI r20, 0
  LDI r4, 0x1040

edge_loop:
  ; Load edge vertex indices
  LOAD r22, r4
  ADDI r4, 1
  LOAD r23, r4
  ADDI r4, 1

  ; Load sx_a: 0x1020 + va*2
  MOV r5, r22
  SHLI r5, 1
  ADDI r5, 0x1020
  LOAD r24, r5

  ; Load sy_a
  ADDI r5, 1
  LOAD r25, r5

  ; Load sx_b: 0x1020 + vb*2
  MOV r5, r23
  SHLI r5, 1
  ADDI r5, 0x1020
  LOAD r26, r5

  ; Load sy_b
  ADDI r5, 1
  LOAD r27, r5

  ; Depth-based coloring: blue channel + green based on avg Z
  ; Load z_a and z_b
  MOV r5, r22
  ADDI r5, 0x1030
  LOAD r29, r5
  MOV r5, r23
  ADDI r5, 0x1030
  LOAD r7, r5

  ; avg_z = (z_a + z_b) >> 1
  ADD r29, r7
  SARI r29, 1

  ; green = clamp(128 + (avg_z >> 1), 0, 255) -- shifted for more variation
  MOV r7, r29
  SARI r7, 1
  LDI r5, 128
  ADD r7, r5
  ANDI r7, 0xFF

  ; Build color: R=0x40, G=green, B=0xFF
  ; color = (0xFF << 16) | (green << 8) | 0x40
  LDI r28, 0xFF
  SHLI r28, 8
  SHLI r28, 8
  SHLI r7, 8
  OR r28, r7
  LDI r5, 0x40
  OR r28, r5

  ; Draw the edge
  LINE r24, r25, r26, r27, r28

  ; Next edge
  ADDI r20, 1
  LDI r5, 12
  CMP r20, r5
  BLT r0, edge_loop

  ; Yield to renderer
  FRAME
  JMP main_loop
