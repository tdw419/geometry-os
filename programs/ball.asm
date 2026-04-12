; ball.asm -- bouncing ball with keyboard control
;
; Left arrow (ASCII 68 = not available... use WASD or arrow keys)
; The VM keyboard port sends ASCII codes for printable keys.
; IKEY reads and clears the port each frame.
;
; Controls (WASD):
;   A (65) = push ball left    D (68) = push ball right
;   W (87) = push ball up      S (83) = push ball down
;   R (82) = reset ball to center
;
; Registers:
;   r1 = ball_x (fixed point: actual x = r1, but treated as signed)
;   r2 = ball_y
;   r3 = dx (velocity x, signed two's complement)
;   r4 = dy (velocity y)
;   r5 = ball color (white)
;   r6 = scratch
;   r7 = key code
;   r8 = ball radius (for CIRCLE)
;   r9 = 0 (black, for clearing)

; ── init ────────────────────────────────────────────────────────
LDI r1, 128     ; ball_x = center
LDI r2, 128     ; ball_y = center
LDI r3, 2       ; dx = 2
LDI r4, 3       ; dy = 3
LDI r5, 0xFFFFFF ; white
LDI r8, 8       ; radius 8
LDI r9, 0       ; black

; ── main loop ────────────────────────────────────────────────────
loop:
  ; clear screen
  FILL r9

  ; read keyboard
  IKEY r7

  ; A (97 lowercase or 65 uppercase) = kick left
  LDI r6, 65
  CMP r7, r6
  JZ r0, kick_left
  LDI r6, 97
  CMP r7, r6
  JZ r0, kick_left

  ; D (100 or 68) = kick right
  LDI r6, 68
  CMP r7, r6
  JZ r0, kick_right
  LDI r6, 100
  CMP r7, r6
  JZ r0, kick_right

  ; W (119 or 87) = kick up
  LDI r6, 87
  CMP r7, r6
  JZ r0, kick_up
  LDI r6, 119
  CMP r7, r6
  JZ r0, kick_up

  ; S (115 or 83) = kick down
  LDI r6, 83
  CMP r7, r6
  JZ r0, kick_down
  LDI r6, 115
  CMP r7, r6
  JZ r0, kick_down

  ; R (82 or 114) = reset
  LDI r6, 82
  CMP r7, r6
  JZ r0, do_reset
  LDI r6, 114
  CMP r7, r6
  JZ r0, do_reset

after_input:
  ; move ball
  ADD r1, r3
  ADD r2, r4

  ; ── bounce x walls ──────────────────────────────────────────
  ; if x >= 248 (255 - radius), negate dx
  LDI r6, 248
  CMP r1, r6
  BLT r0, check_x_low
  LDI r1, 248
  NEG r3
  JMP check_y

check_x_low:
  ; if x <= 8 (radius), negate dx
  LDI r6, 8
  CMP r1, r6
  BGE r0, check_y
  LDI r1, 8
  NEG r3

  ; ── bounce y walls ──────────────────────────────────────────
check_y:
  LDI r6, 248
  CMP r2, r6
  BLT r0, check_y_low
  LDI r2, 248
  NEG r4
  JMP draw_ball

check_y_low:
  LDI r6, 8
  CMP r2, r6
  BGE r0, draw_ball
  LDI r2, 8
  NEG r4

  ; ── draw ball ───────────────────────────────────────────────
draw_ball:
  CIRCLE r1, r2, r8, r5
  FRAME
  JMP loop

  ; ── kick handlers ───────────────────────────────────────────
kick_left:
  LDI r6, 3
  NEG r6
  LDI r3, 0
  ADD r3, r6
  JMP after_input

kick_right:
  LDI r3, 3
  JMP after_input

kick_up:
  LDI r6, 3
  NEG r6
  LDI r4, 0
  ADD r4, r6
  JMP after_input

kick_down:
  LDI r4, 3
  JMP after_input

do_reset:
  LDI r1, 128
  LDI r2, 128
  LDI r3, 2
  LDI r4, 3
  JMP after_input
