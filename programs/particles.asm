; particles.asm -- 100 colored pixels that drift, bounce, and fade
;
; Proves FRAME + pixel write performance for real-time effects.
; Each particle has position (x,y), velocity (vx,vy), color, and life.
; Particles bounce off screen edges and fade over time. When life runs
; out, the particle respawns at a random position with fresh color/velocity.
;
; Memory layout (100 particles, 6 words each):
;   RAM[0x8000 + N*6 + 0] = x      (0..255)
;   RAM[0x8000 + N*6 + 1] = y      (0..255)
;   RAM[0x8000 + N*6 + 2] = vx     (signed, -2..+2, nonzero)
;   RAM[0x8000 + N*6 + 3] = vy     (signed, -2..+2, nonzero)
;   RAM[0x8000 + N*6 + 4] = color  (0xRRGGBB)
;   RAM[0x8000 + N*6 + 5] = life   (128..255, decrements each frame)

; ===== Constants =====
LDI r20, 6           ; stride per particle
LDI r21, 100         ; particle count
LDI r22, 256         ; screen size
LDI r23, 0x8000      ; particle base address
LDI r24, 0xFF        ; byte mask
LDI r25, 1           ; constant 1
LDI r26, 0           ; background color (black)

; ===== Initialize 100 particles =====
LDI r1, 0

init_loop:
  CALL calc_addr
  CALL randomize
  ADD r1, r25
  CMP r1, r21
  BLT r0, init_loop

; ===== Main frame loop =====
frame_loop:
  FILL r26            ; clear screen to black
  LDI r1, 0

update_loop:
  CALL calc_addr      ; r2 = base of particle[i]

  ; -- Load x and vx --
  LOAD r3, r2         ; r3 = x
  LDI r5, 2
  ADD r2, r5          ; r2 -> vx
  LOAD r6, r2         ; r6 = vx
  ADD r3, r6          ; x += vx

  ; Bounce X: if x >= 256, it went off screen
  CMP r3, r22
  BLT r0, x_ok
  ; Clamp x and negate vx
  LDI r3, 2
  LOAD r6, r2
  NEG r6
  STORE r2, r6        ; save negated vx
x_ok:
  ; Save x back
  CALL calc_addr
  STORE r2, r3

  ; -- Load y and vy --
  CALL calc_addr
  LDI r5, 1
  ADD r2, r5          ; r2 -> y
  LOAD r4, r2         ; r4 = y
  LDI r5, 1
  ADD r2, r5          ; r2 -> vy
  LOAD r6, r2         ; r6 = vy
  ADD r4, r6          ; y += vy

  ; Bounce Y
  CMP r4, r22
  BLT r0, y_ok
  LDI r4, 2
  LOAD r6, r2
  NEG r6
  STORE r2, r6        ; save negated vy
y_ok:
  ; Save y back
  CALL calc_addr
  LDI r5, 1
  ADD r2, r5
  STORE r2, r4

  ; -- Decrement life --
  CALL calc_addr
  LDI r5, 5
  ADD r2, r5          ; r2 -> life
  LOAD r5, r2
  SUBI r5, 1
  STORE r2, r5

  ; If life > 0, draw
  CMP r5, r26
  BGE r0, do_draw

  ; Life hit 0: respawn
  CALL calc_addr
  CALL randomize
  JMP next_particle

do_draw:
  ; -- Draw with fade --
  CALL calc_addr
  LOAD r3, r2         ; x
  LDI r5, 1
  ADD r2, r5
  LOAD r4, r2         ; y
  LDI r5, 1
  ADD r2, r5          ; skip vy
  LDI r5, 1
  ADD r2, r5          ; -> color
  LOAD r7, r2         ; r7 = color
  LDI r5, 1
  ADD r2, r5          ; -> life
  LOAD r8, r2         ; r8 = life

  ; Compute fade: shift = (255 - life) >> 5 (0..7)
  LDI r5, 255
  SUB r5, r8
  SHRI r5, 5          ; r5 = fade_shift (0..7)
  MOV r8, r5

  ; Fade red
  MOV r9, r7
  SHRI r9, 16
  SHR r9, r8
  SHLI r9, 16
  MOV r5, r9

  ; Fade green
  MOV r9, r7
  SHRI r9, 8
  ANDI r9, 0xFF
  SHR r9, r8
  SHLI r9, 8
  OR r5, r9

  ; Fade blue
  MOV r9, r7
  ANDI r9, 0xFF
  SHR r9, r8
  OR r5, r9           ; r5 = final faded color

  ; Skip if fully faded
  CMP r5, r26
  JZ r0, next_particle

  PSET r3, r4, r5

next_particle:
  ADD r1, r25
  CMP r1, r21
  BLT r0, update_loop

  FRAME
  JMP frame_loop

; ===== Subroutine: calc_addr =====
; Sets r2 = 0x8000 + r1 * 6
calc_addr:
  MOV r2, r1
  MUL r2, r20
  ADD r2, r23
  RET

; ===== Subroutine: randomize =====
; Randomizes particle at address r2
randomize:
  ; x = RAND & 0xFF
  RAND r3
  AND r3, r24
  STORE r2, r3
  ADD r2, r25

  ; y = RAND & 0xFF
  RAND r3
  AND r3, r24
  STORE r2, r3
  ADD r2, r25

  ; vx = (RAND & 3) + 1, randomly negated
  RAND r3
  ANDI r3, 3
  ADDI r3, 1           ; 1..4
  RAND r5
  ANDI r5, 1
  JZ r5, vx_pos
  NEG r3
vx_pos:
  STORE r2, r3
  ADD r2, r25

  ; vy = same
  RAND r3
  ANDI r3, 3
  ADDI r3, 1
  RAND r5
  ANDI r5, 1
  JZ r5, vy_pos
  NEG r3
vy_pos:
  STORE r2, r3
  ADD r2, r25

  ; color = RAND | 0x404040 (ensure bright enough)
  RAND r3
  LDI r5, 0x404040
  OR r3, r5
  STORE r2, r3
  ADD r2, r25

  ; life = 128 + (RAND & 127)
  RAND r3
  ANDI r3, 127
  ADDI r3, 128
  STORE r2, r3

  RET
