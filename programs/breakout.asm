; breakout.asm -- Breakout / Brick Breaker for Geometry OS
;
; Controls:
;   A/D or Left/Right arrows -- move paddle
;   W or Space -- launch ball
;   R -- restart game
;
; Layout (256x256 screen):
;   y=0..7:   score area (text)
;   y=8..39:  bricks (8 cols x 4 rows, 32x8 px each)
;   y=40..243: play field
;   y=244..249: paddle (48x6 px)
;
; Memory map:
;   0x3000..0x301F  brick[0..31]  (0=dead, color=alive)
;   0x3020          paddle_x      (0..208)
;   0x3021          ball_x
;   0x3022          ball_y
;   0x3023          ball_dx       (signed u32)
;   0x3024          ball_dy       (signed u32)
;   0x3025          score
;   0x3026          lives         (3)
;   0x3027          game_over     (0=play, 1=win, 2=lose)
;   0x3028          ball_launched (0=on paddle, 1=flying)
;   0x3029          bricks_left
;   0x3100..0x3106  score text buf (7 chars + null)

; ── init / restart ──────────────────────────────────────────────
restart:
  LDI r30, 0x8000

  ; write "SCORE:" text to buffer (stays constant)
  LDI r4, 0x3100
  LDI r1, 83
  STORE r4, r1        ; S
  LDI r4, 0x3101
  LDI r1, 67
  STORE r4, r1        ; C
  LDI r4, 0x3102
  LDI r1, 79
  STORE r4, r1        ; O
  LDI r4, 0x3103
  LDI r1, 82
  STORE r4, r1        ; R
  LDI r4, 0x3104
  LDI r1, 69
  STORE r4, r1        ; E
  LDI r4, 0x3105
  LDI r1, 58
  STORE r4, r1        ; :

  ; init bricks -- row 0=red, row 1=orange, row 2=yellow, row 3=green
  LDI r22, 0xFF0000   ; red
  LDI r23, 0xFF8800   ; orange
  LDI r24, 0xFFDD00   ; yellow
  LDI r25, 0x00CC44   ; green
  LDI r10, 0          ; brick index

ib_loop:
  LDI r5, 0
  ADD r5, r10
  LDI r6, 3
  SHR r5, r6          ; r5 = row (0-3)
  ; default green (row 3)
  LDI r1, 0
  ADD r1, r25
  ; check row 0
  LDI r6, 0
  CMP r5, r6
  JZ r0, ib_r0
  ; check row 1
  LDI r6, 1
  CMP r5, r6
  JZ r0, ib_r1
  ; check row 2
  LDI r6, 2
  CMP r5, r6
  JZ r0, ib_r2
  JMP ib_store        ; row 3 = green (already in r1)

ib_r0:
  LDI r1, 0
  ADD r1, r22         ; red
  JMP ib_store

ib_r1:
  LDI r1, 0
  ADD r1, r23         ; orange
  JMP ib_store

ib_r2:
  LDI r1, 0
  ADD r1, r24         ; yellow

ib_store:
  LDI r4, 0x3000
  ADD r4, r10
  STORE r4, r1
  LDI r5, 1
  ADD r10, r5
  LDI r6, 32
  CMP r10, r6
  BLT r0, ib_loop

  ; init game state
  LDI r1, 104
  LDI r4, 0x3020
  STORE r4, r1        ; paddle_x = 104 (centered)
  LDI r1, 0
  LDI r4, 0x3025
  STORE r4, r1        ; score = 0
  LDI r1, 3
  LDI r4, 0x3026
  STORE r4, r1        ; lives = 3
  LDI r1, 0
  LDI r4, 0x3027
  STORE r4, r1        ; game_over = 0
  LDI r1, 32
  LDI r4, 0x3029
  STORE r4, r1        ; bricks_left = 32

  CALL reset_ball

; ── main loop ────────────────────────────────────────────────────
game_loop:
  ; check game over
  LDI r4, 0x3027
  LOAD r1, r4
  JNZ r1, game_over_screen

  ; read keyboard
  IKEY r7

  ; A/a = paddle left
  LDI r6, 65
  CMP r7, r6
  JZ r0, pad_left
  LDI r6, 97
  CMP r7, r6
  JZ r0, pad_left

  ; D/d = paddle right
  LDI r6, 68
  CMP r7, r6
  JZ r0, pad_right
  LDI r6, 100
  CMP r7, r6
  JZ r0, pad_right

  ; W/w/Space = launch ball
  LDI r6, 87
  CMP r7, r6
  JZ r0, do_launch
  LDI r6, 119
  CMP r7, r6
  JZ r0, do_launch
  LDI r6, 32
  CMP r7, r6
  JZ r0, do_launch

  ; R/r = restart
  LDI r6, 82
  CMP r7, r6
  JZ r0, restart
  LDI r6, 114
  CMP r7, r6
  JZ r0, restart

after_input:
  ; if ball not launched, stick to paddle
  LDI r4, 0x3028
  LOAD r1, r4
  JNZ r1, update_ball

  ; ball sits on paddle
  LDI r4, 0x3020
  LOAD r1, r4          ; paddle_x
  LDI r5, 22
  ADD r1, r5           ; center ball on paddle
  LDI r4, 0x3021
  STORE r4, r1         ; ball_x
  LDI r1, 240
  LDI r4, 0x3022
  STORE r4, r1         ; ball_y = 240
  JMP draw_and_loop

update_ball:
  ; move ball x
  LDI r4, 0x3021
  LOAD r1, r4
  LDI r4, 0x3023
  LOAD r2, r4
  ADD r1, r2
  LDI r4, 0x3021
  STORE r4, r1

  ; move ball y
  LDI r4, 0x3022
  LOAD r1, r4
  LDI r4, 0x3024
  LOAD r2, r4
  ADD r1, r2
  LDI r4, 0x3022
  STORE r4, r1

  ; ── left wall bounce ──
  LDI r4, 0x3021
  LOAD r1, r4
  LDI r6, 0
  CMP r1, r6
  BGE r0, chk_rw
  LDI r1, 0
  LDI r4, 0x3021
  STORE r4, r1
  LDI r4, 0x3023
  LOAD r1, r4
  NEG r1
  LDI r4, 0x3023
  STORE r4, r1

chk_rw:
  ; ── right wall bounce ──
  LDI r4, 0x3021
  LOAD r1, r4
  LDI r6, 252          ; 256 - 4 (ball width)
  CMP r1, r6
  BLT r0, chk_tw
  LDI r1, 252
  LDI r4, 0x3021
  STORE r4, r1
  LDI r4, 0x3023
  LOAD r1, r4
  NEG r1
  LDI r4, 0x3023
  STORE r4, r1

chk_tw:
  ; ── top wall bounce ──
  LDI r4, 0x3022
  LOAD r1, r4
  LDI r6, 0
  CMP r1, r6
  BGE r0, chk_bot
  LDI r1, 0
  LDI r4, 0x3022
  STORE r4, r1
  LDI r4, 0x3024
  LOAD r1, r4
  NEG r1
  LDI r4, 0x3024
  STORE r4, r1

chk_bot:
  ; ── ball lost (fell off bottom) ──
  LDI r4, 0x3022
  LOAD r1, r4
  LDI r6, 256
  CMP r1, r6
  BLT r0, chk_brick
  CALL lose_life
  JMP draw_and_loop

chk_brick:
  ; ── brick collision ──
  LDI r4, 0x3022
  LOAD r1, r4          ; ball_y
  ; ball must be in brick area: y >= 8 and y < 40
  LDI r6, 8
  CMP r1, r6
  BLT r0, chk_paddle
  LDI r6, 40
  CMP r1, r6
  BGE r0, chk_paddle

  ; compute brick row = (ball_y - 8) >> 3
  LDI r5, 8
  SUB r1, r5
  LDI r5, 3
  SHR r1, r5           ; r1 = row (0-3)

  ; compute brick col = ball_x >> 5
  LDI r4, 0x3021
  LOAD r2, r4          ; ball_x
  LDI r5, 5
  SHR r2, r5           ; r2 = col (0-7)

  ; bounds check
  LDI r6, 4
  CMP r1, r6
  BGE r0, chk_paddle
  LDI r6, 8
  CMP r2, r6
  BGE r0, chk_paddle

  ; brick index = row * 8 + col
  LDI r3, 0
  ADD r3, r1
  LDI r5, 3
  SHL r3, r5           ; row * 8
  ADD r3, r2           ; + col

  ; check if brick alive
  LDI r4, 0x3000
  ADD r4, r3
  LOAD r1, r4
  JZ r1, chk_paddle   ; dead brick, skip

  ; kill brick
  LDI r1, 0
  STORE r4, r1

  ; decrement bricks_left
  LDI r4, 0x3029
  LOAD r1, r4
  LDI r5, 1
  SUB r1, r5
  LDI r4, 0x3029
  STORE r4, r1

  ; check win
  JNZ r1, brick_bounce
  LDI r1, 1
  LDI r4, 0x3027
  STORE r4, r1         ; game_over = 1 (won!)
  LDI r5, 880
  LDI r6, 200
  BEEP r5, r6
  JMP draw_and_loop

brick_bounce:
  ; reverse ball_dy
  LDI r4, 0x3024
  LOAD r1, r4
  NEG r1
  LDI r4, 0x3024
  STORE r4, r1
  ; score += 10
  LDI r4, 0x3025
  LOAD r1, r4
  LDI r5, 10
  ADD r1, r5
  LDI r4, 0x3025
  STORE r4, r1
  ; brick hit sound
  LDI r5, 660
  LDI r6, 30
  BEEP r5, r6

chk_paddle:
  ; ── paddle collision ──
  ; ball must be at paddle level (y >= 240) and moving down
  LDI r4, 0x3022
  LOAD r1, r4          ; ball_y
  LDI r6, 240
  CMP r1, r6
  BLT r0, draw_and_loop
  ; check ball moving down (dy > 0)
  LDI r4, 0x3024
  LOAD r1, r4
  LDI r6, 0
  CMP r1, r6
  BLT r0, draw_and_loop ; moving up, skip
  ; check x overlap (ball center vs paddle bounds)
  LDI r4, 0x3021
  LOAD r1, r4          ; ball_x
  LDI r4, 0x3020
  LOAD r2, r4          ; paddle_x
  LDI r5, 2
  ADD r1, r5           ; ball center x
  CMP r1, r2
  BLT r0, draw_and_loop ; left of paddle
  LDI r5, 48
  ADD r2, r5           ; paddle right edge
  CMP r1, r2
  BGE r0, draw_and_loop ; right of paddle

  ; paddle hit! reverse dy
  LDI r4, 0x3024
  LOAD r1, r4
  NEG r1
  LDI r4, 0x3024
  STORE r4, r1

  ; set dx from hit position (offset -24..24, clamped to -3..3)
  LDI r4, 0x3021
  LOAD r1, r4          ; ball_x
  LDI r5, 2
  ADD r1, r5           ; ball center x
  LDI r4, 0x3020
  LOAD r2, r4          ; paddle_x
  SUB r1, r2           ; offset (0-48)
  LDI r5, 24
  SUB r1, r5           ; -24..24

  ; clamp to [-3, 3]
  LDI r6, 0xFFFFFFFD   ; -3 unsigned
  CMP r1, r6
  BLT r0, dx_neg
  LDI r6, 3
  CMP r1, r6
  BGE r0, dx_pos
  JMP dx_store

dx_neg:
  LDI r1, 0xFFFFFFFD
  JMP dx_store

dx_pos:
  LDI r1, 3

dx_store:
  ; ensure dx is never 0 (would be boring)
  JNZ r1, dx_ok
  LDI r1, 1
dx_ok:
  LDI r4, 0x3023
  STORE r4, r1
  ; paddle hit sound
  LDI r5, 440
  LDI r6, 20
  BEEP r5, r6

draw_and_loop:
  CALL draw_frame
  FRAME
  JMP game_loop

; ── input handlers ───────────────────────────────────────────────
pad_left:
  LDI r4, 0x3020
  LOAD r1, r4
  LDI r5, 5
  SUB r1, r5
  LDI r6, 0
  CMP r1, r6
  BGE r0, pl_ok
  LDI r1, 0
pl_ok:
  LDI r4, 0x3020
  STORE r4, r1
  JMP after_input

pad_right:
  LDI r4, 0x3020
  LOAD r1, r4
  LDI r5, 5
  ADD r1, r5
  LDI r6, 208           ; 256 - 48
  CMP r1, r6
  BLT r0, pr_ok
  LDI r1, 208
pr_ok:
  LDI r4, 0x3020
  STORE r4, r1
  JMP after_input

do_launch:
  LDI r4, 0x3028
  LOAD r1, r4
  JNZ r1, after_input  ; already launched
  LDI r1, 1
  LDI r4, 0x3028
  STORE r4, r1
  JMP after_input

; ── subroutines ──────────────────────────────────────────────────
reset_ball:
  LDI r1, 0
  LDI r4, 0x3028
  STORE r4, r1         ; ball_launched = 0
  LDI r1, 2
  LDI r4, 0x3023
  STORE r4, r1         ; ball_dx = 2
  LDI r1, 0xFFFFFFFD   ; -3
  LDI r4, 0x3024
  STORE r4, r1         ; ball_dy = -3
  RET

lose_life:
  PUSH r31
  LDI r4, 0x3026
  LOAD r1, r4
  LDI r5, 1
  SUB r1, r5
  LDI r4, 0x3026
  STORE r4, r1
  JNZ r1, ll_reset
  ; game over (lost)
  LDI r1, 2
  LDI r4, 0x3027
  STORE r4, r1
  LDI r5, 110
  LDI r6, 300
  BEEP r5, r6
  POP r31
  RET

ll_reset:
  CALL reset_ball
  POP r31
  RET

; ── game over screen ─────────────────────────────────────────────
game_over_screen:
  LDI r4, 0x3027
  LOAD r1, r4
  LDI r6, 1
  CMP r1, r6
  JZ r0, win_screen

  ; lose screen (dark red)
  LDI r1, 0x330000
  FILL r1
  FRAME
  IKEY r7
  JZ r7, game_over_screen
  JMP restart

win_screen:
  ; win screen (dark green)
  LDI r1, 0x003300
  FILL r1
  FRAME
  IKEY r7
  JZ r7, win_screen
  JMP restart

; ─────────────────────────────────────────────────────────────────
; draw_frame -- render entire screen
;   Clobbers r1-r9, r10. Preserves r20-r25.
; ─────────────────────────────────────────────────────────────────
draw_frame:
  ; dark background
  LDI r1, 0x000811
  FILL r1

  ; ── draw bricks ──
  LDI r10, 0           ; brick index
  LDI r20, 0x3000      ; brick base

df_bloop:
  ; load brick color
  LDI r4, 0
  ADD r4, r20
  ADD r4, r10
  LOAD r1, r4          ; r1 = color (0 = dead)
  JZ r1, df_bnext

  ; compute pixel position
  LDI r5, 0
  ADD r5, r10
  LDI r6, 7
  AND r5, r6           ; r5 = col (0-7)
  LDI r8, 5
  SHL r5, r8           ; r5 = col * 32

  LDI r6, 0
  ADD r6, r10
  LDI r8, 3
  SHR r6, r8           ; r6 = row (0-3)
  LDI r8, 3
  SHL r6, r8           ; r6 = row * 8
  LDI r8, 9
  ADD r6, r8           ; r6 = row * 8 + 9 (1px top gap)

  ; draw brick: RECTF(x+1, y+1, 30, 6, color)
  LDI r7, 1
  ADD r5, r7           ; x + 1
  ADD r6, r7           ; y + 1
  LDI r7, 30           ; width
  LDI r8, 6            ; height
  RECTF r5, r6, r7, r8, r1

df_bnext:
  LDI r5, 1
  ADD r10, r5
  LDI r6, 32
  CMP r10, r6
  BLT r0, df_bloop

  ; ── draw paddle ──
  LDI r4, 0x3020
  LOAD r5, r4          ; r5 = paddle_x
  LDI r6, 244           ; y
  LDI r7, 48            ; width
  LDI r8, 6             ; height
  LDI r9, 0xCCCCCC      ; light gray
  RECTF r5, r6, r7, r8, r9

  ; ── draw ball ──
  LDI r4, 0x3021
  LOAD r5, r4          ; ball_x
  LDI r4, 0x3022
  LOAD r6, r4          ; ball_y
  LDI r7, 4            ; width
  LDI r8, 4            ; height
  LDI r9, 0xFFFFFF     ; white
  RECTF r5, r6, r7, r8, r9

  ; ── draw score text ──
  CALL draw_score

  ; ── draw lives (small rects at bottom-right) ──
  LDI r4, 0x3026
  LOAD r1, r4          ; lives
  LDI r9, 0            ; loop counter
  LDI r10, 240         ; x start
df_lloop:
  CMP r9, r1
  BGE r0, df_ldone
  LDI r5, 0
  ADD r5, r10
  LDI r6, 252           ; y
  LDI r7, 4             ; w
  LDI r8, 4             ; h
  LDI r2, 0x00CC44     ; green
  RECTF r5, r6, r7, r8, r2
  LDI r5, 6
  ADD r10, r5           ; next life dot x += 6
  LDI r5, 1
  ADD r9, r5
  JMP df_lloop

df_ldone:
  RET

; ─────────────────────────────────────────────────────────────────
; draw_score -- convert score to 3-digit ASCII and display with TEXT
;   Writes digits to RAM[0x3106..0x3108], null at 0x3109
; ─────────────────────────────────────────────────────────────────
draw_score:
  LDI r4, 0x3025
  LOAD r1, r4          ; r1 = score

  ; hundreds digit: score / 100
  LDI r2, 0
  ADD r2, r1           ; r2 = score copy
  LDI r3, 100
  DIV r2, r3           ; r2 = hundreds
  LDI r5, 48
  ADD r2, r5           ; r2 = '0' + hundreds
  LDI r4, 0x3106
  STORE r4, r2

  ; tens digit: (score - hundreds*100) / 10
  LDI r3, 100
  LDI r6, 0
  ADD r6, r2           ; r6 = ASCII hundreds
  LDI r5, 48
  SUB r6, r5           ; r6 = numeric hundreds
  MUL r6, r3           ; r6 = hundreds * 100
  LDI r2, 0
  ADD r2, r1           ; r2 = score
  SUB r2, r6           ; r2 = remainder (< 100)
  LDI r3, 10
  DIV r2, r3           ; r2 = tens
  LDI r5, 48
  ADD r2, r5           ; r2 = '0' + tens
  LDI r4, 0x3107
  STORE r4, r2

  ; ones digit: remainder - tens*10
  LDI r3, 10
  LDI r6, 0
  ADD r6, r2           ; r6 = ASCII tens
  LDI r5, 48
  SUB r6, r5           ; r6 = numeric tens
  MUL r6, r3           ; r6 = tens * 10
  SUB r2, r6           ; r2 = ones
  LDI r5, 48
  ADD r2, r5           ; r2 = '0' + ones
  LDI r4, 0x3108
  STORE r4, r2

  ; null terminator
  LDI r1, 0
  LDI r4, 0x3109
  STORE r4, r1

  ; TEXT opcode to render "SCORE:XXX"
  LDI r5, 2             ; x
  LDI r6, 0             ; y
  LDI r7, 0x3100        ; string address
  TEXT r5, r6, r7

  RET
