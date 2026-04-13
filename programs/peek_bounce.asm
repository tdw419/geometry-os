; peek_bounce.asm -- PEEK-based collision demo
;
; Demonstrates PEEK (0x4F): a white ball bounces off drawn obstacles
; by reading pixel colors from the screen buffer directly.
; No RAM-based collision map -- the screen IS the state.
;
; Registers:
;   r1  = ball x
;   r2  = ball y
;   r3  = vx (+1 or -1)
;   r4  = vy (+1 or -1)
;   r5  = ball color (white 0xFFFFFF)
;   r7  = bg color (0x001020)
;   r6, r8 = scratch

restart:
  LDI r5, 0xFFFFFF     ; ball color
  LDI r7, 0x001020     ; bg color

  ; initial ball position
  LDI r1, 128
  LDI r2, 128
  LDI r3, 1            ; vx = +1
  LDI r4, 1            ; vy = +1

  ; draw obstacles
  CALL draw_obstacles

; ── Frame loop ──────────────────────────────────────────────────────
frame_loop:
  ; clear old ball
  PSET r1, r2, r7

  ; ── X collision: peek pixel at (x+vx, y) ─────────────────────
  MOV r6, r1
  ADD r6, r3           ; r6 = x + vx
  PEEK r6, r2, r8      ; r8 = color at (x+vx, y)
  MOV r6, r8
  CMP r6, r7           ; compare with bg
  JZ r6, do_move_x     ; equal → background → safe to move
  NEG r3               ; not bg → obstacle → flip vx
  JMP check_y

do_move_x:
  ADD r1, r3           ; move x

check_y:
  ; ── Y collision: peek pixel at (x, y+vy) ─────────────────────
  MOV r6, r2
  ADD r6, r4           ; r6 = y + vy
  PEEK r1, r6, r8      ; r8 = color at (x, y+vy)
  MOV r6, r8
  CMP r6, r7
  JZ r6, do_move_y     ; bg → safe
  NEG r4               ; obstacle → flip vy
  JMP draw_ball

do_move_y:
  ADD r2, r4           ; move y

draw_ball:
  PSET r1, r2, r5
  FRAME
  JMP frame_loop

; ── draw_obstacles -- border walls + interior colored blocks ────────
draw_obstacles:
  LDI r1, 0x001020
  FILL r1

  ; border walls (blue-gray 0x4466AA)
  LDI r24, 0x4466AA

  ; top
  LDI r1, 0
  LDI r2, 0
  LDI r22, 256
  LDI r23, 4
  RECTF r1, r2, r22, r23, r24

  ; bottom
  LDI r2, 252
  RECTF r1, r2, r22, r23, r24

  ; left
  LDI r22, 4
  LDI r23, 256
  RECTF r1, r2, r22, r23, r24

  ; right
  LDI r1, 252
  RECTF r1, r2, r22, r23, r24

  ; red block top-left
  LDI r1, 60
  LDI r2, 60
  LDI r22, 40
  LDI r23, 10
  LDI r24, 0xCC4444
  RECTF r1, r2, r22, r23, r24

  ; green pillar center
  LDI r1, 150
  LDI r2, 100
  LDI r22, 10
  LDI r23, 60
  LDI r24, 0x44CC44
  RECTF r1, r2, r22, r23, r24

  ; blue bar bottom
  LDI r1, 80
  LDI r2, 180
  LDI r22, 80
  LDI r23, 10
  LDI r24, 0x4444CC
  RECTF r1, r2, r22, r23, r24

  ; gold column right
  LDI r1, 200
  LDI r2, 30
  LDI r22, 15
  LDI r23, 50
  LDI r24, 0xCCAA44
  RECTF r1, r2, r22, r23, r24

  ; restore ball registers
  LDI r1, 128
  LDI r2, 128
  LDI r3, 1
  LDI r4, 1
  RET
