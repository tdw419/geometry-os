; snake.asm -- Snake game on a 32x32 cell grid (8px per cell = 256x256)
;
; Memory layout (all RAM addresses):
;   0x2000..0x21FF  body ring buffer: each cell = (x<<8)|y, 512 slots
;   0x2200          head index (into ring buffer)
;   0x2201          tail index
;   0x2202          length
;   0x2203          apple_x (cell coords, 0-31)
;   0x2204          apple_y
;   0x2205          direction (0=up 1=right 2=down 3=left)
;   0x2206          game_over flag
;
; Stack: r30 = SP, initialized to 0x8000 (grows down into safe RAM)
;
; Controls: WASD (also lowercase)

; ── restart / init ──────────────────────────────────────────────
restart:
  LDI r30, 0x8000     ; stack pointer

  ; write initial snake: (14,16) (15,16) (16,16) at indices 0,1,2
  LDI r4, 0x2000
  LDI r9, 8

  LDI r1, 14
  SHL r1, r9
  LDI r2, 16
  OR r1, r2
  STORE r4, r1        ; body[0] = (14,16)

  LDI r1, 15
  SHL r1, r9
  OR r1, r2
  LDI r4, 0x2001
  STORE r4, r1        ; body[1] = (15,16)

  LDI r1, 16
  SHL r1, r9
  OR r1, r2
  LDI r4, 0x2002
  STORE r4, r1        ; body[2] = (16,16)

  LDI r1, 2
  LDI r4, 0x2200
  STORE r4, r1        ; head = 2
  LDI r1, 0
  LDI r4, 0x2201
  STORE r4, r1        ; tail = 0
  LDI r1, 3
  LDI r4, 0x2202
  STORE r4, r1        ; length = 3

  LDI r1, 1           ; direction = right
  LDI r4, 0x2205
  STORE r4, r1

  LDI r1, 0           ; game_over = 0
  LDI r4, 0x2206
  STORE r4, r1

  CALL place_apple

; ── main loop ────────────────────────────────────────────────────
game_loop:
  ; check game over
  LDI r4, 0x2206
  LOAD r1, r4
  JNZ r1, game_over_screen

  ; read key and update direction (cannot reverse)
  IKEY r7

  LDI r6, 87          ; W / w = up
  CMP r7, r6
  JZ r0, try_up
  LDI r6, 119
  CMP r7, r6
  JZ r0, try_up

  LDI r6, 68          ; D / d = right
  CMP r7, r6
  JZ r0, try_right
  LDI r6, 100
  CMP r7, r6
  JZ r0, try_right

  LDI r6, 83          ; S / s = down
  CMP r7, r6
  JZ r0, try_down
  LDI r6, 115
  CMP r7, r6
  JZ r0, try_down

  LDI r6, 65          ; A / a = left
  CMP r7, r6
  JZ r0, try_left
  LDI r6, 97
  CMP r7, r6
  JZ r0, try_left

after_input:
  CALL move_snake
  CALL draw_frame
  FRAME
  JMP game_loop

try_up:
  LDI r4, 0x2205
  LOAD r6, r4
  LDI r9, 2
  CMP r6, r9
  JZ r0, after_input  ; refuse if going down
  LDI r1, 0
  STORE r4, r1
  JMP after_input

try_right:
  LDI r4, 0x2205
  LOAD r6, r4
  LDI r9, 3
  CMP r6, r9
  JZ r0, after_input  ; refuse if going left
  LDI r1, 1
  STORE r4, r1
  JMP after_input

try_down:
  LDI r4, 0x2205
  LOAD r6, r4
  LDI r9, 0
  CMP r6, r9
  JZ r0, after_input  ; refuse if going up
  LDI r1, 2
  STORE r4, r1
  JMP after_input

try_left:
  LDI r4, 0x2205
  LOAD r6, r4
  LDI r9, 1
  CMP r6, r9
  JZ r0, after_input  ; refuse if going right
  LDI r1, 3
  STORE r4, r1
  JMP after_input

; ── game over screen ─────────────────────────────────────────────
game_over_screen:
  LDI r1, 0x550000
  FILL r1
  FRAME
  IKEY r7
  JZ r7, game_over_screen
  JMP restart

; ─────────────────────────────────────────────────────────────────
; SUBROUTINE: move_snake
;   Reads direction, computes new head, checks walls + apple.
;   PUSH/POP r31 because we may nested-call place_apple.
; ─────────────────────────────────────────────────────────────────
move_snake:
  PUSH r31

  ; load head cell
  LDI r4, 0x2200
  LOAD r10, r4         ; r10 = head index
  LDI r4, 0x2000
  ADD r4, r10
  LOAD r11, r4         ; r11 = head cell = (x<<8)|y

  ; decode head x (r12), y (r13)
  LDI r9, 8
  LDI r12, 0
  ADD r12, r11
  SHR r12, r9          ; r12 = x
  LDI r9, 0xFF
  LDI r13, 0
  ADD r13, r11
  AND r13, r9          ; r13 = y

  ; load direction
  LDI r4, 0x2205
  LOAD r15, r4

  LDI r9, 0
  CMP r15, r9
  JZ r0, dir_up
  LDI r9, 1
  CMP r15, r9
  JZ r0, dir_right
  LDI r9, 2
  CMP r15, r9
  JZ r0, dir_down
  ; left: x -= 1
  LDI r9, 1
  SUB r12, r9
  JMP wall_check

dir_up:
  LDI r9, 1
  SUB r13, r9
  JMP wall_check

dir_right:
  LDI r9, 1
  ADD r12, r9
  JMP wall_check

dir_down:
  LDI r9, 1
  ADD r13, r9

wall_check:
  ; x >= 32 -> hit
  LDI r9, 32
  CMP r12, r9
  BGE r0, do_wall_hit
  ; x < 0 (signed underflow -> large u32) -> hit
  LDI r9, 0
  CMP r12, r9
  BLT r0, do_wall_hit
  ; y < 0
  CMP r13, r9
  BLT r0, do_wall_hit
  ; y >= 32
  LDI r9, 32
  CMP r13, r9
  BGE r0, do_wall_hit

  ; encode new head: (x<<8)|y
  LDI r9, 8
  LDI r11, 0
  ADD r11, r12
  SHL r11, r9
  OR r11, r13

  ; advance head index (mod 512)
  LDI r4, 0x2200
  LOAD r10, r4
  LDI r9, 1
  ADD r10, r9
  LDI r9, 0x1FF
  AND r10, r9
  STORE r4, r10

  ; store new head cell
  LDI r4, 0x2000
  ADD r4, r10
  STORE r4, r11

  ; check apple: compare new head_x with apple_x
  LDI r4, 0x2203
  LOAD r20, r4          ; apple_x
  LDI r4, 0x2204
  LOAD r21, r4          ; apple_y
  CMP r12, r20
  JZ r0, check_apple_y
  JMP advance_tail

check_apple_y:
  CMP r13, r21
  JZ r0, do_eat_apple

advance_tail:
  ; not eating: advance tail
  LDI r4, 0x2201
  LOAD r22, r4
  LDI r9, 1
  ADD r22, r9
  LDI r9, 0x1FF
  AND r22, r9
  LDI r4, 0x2201
  STORE r4, r22
  POP r31
  RET

do_eat_apple:
  ; grow: increment length
  LDI r4, 0x2202
  LOAD r9, r4
  LDI r20, 1
  ADD r9, r20
  STORE r4, r9
  CALL place_apple
  POP r31
  RET

do_wall_hit:
  LDI r1, 1
  LDI r4, 0x2206
  STORE r4, r1
  POP r31
  RET

; ─────────────────────────────────────────────────────────────────
; SUBROUTINE: place_apple -- pick random cell (0-31, 0-31)
; ─────────────────────────────────────────────────────────────────
place_apple:
  RAND r1
  LDI r2, 31
  AND r1, r2
  LDI r4, 0x2203
  STORE r4, r1
  RAND r1
  AND r1, r2
  LDI r4, 0x2204
  STORE r4, r1
  RET

; ─────────────────────────────────────────────────────────────────
; SUBROUTINE: draw_frame -- clear, draw apple, draw snake body
;   No nested calls, r31 safe.
; ─────────────────────────────────────────────────────────────────
draw_frame:
  ; clear dark background
  LDI r1, 0x001008
  FILL r1

  ; draw apple (red 8x8 block)
  LDI r4, 0x2203
  LOAD r2, r4           ; apple_x
  LDI r4, 0x2204
  LOAD r3, r4           ; apple_y
  LDI r9, 8
  MUL r2, r9            ; px_x
  MUL r3, r9            ; px_y
  LDI r22, 8
  LDI r23, 8
  LDI r24, 0xFF2200
  RECTF r2, r3, r22, r23, r24

  ; draw snake body: iterate ring buffer from tail, length times
  LDI r4, 0x2201
  LOAD r10, r4           ; r10 = tail index
  LDI r4, 0x2202
  LOAD r25, r4           ; r25 = length (loop counter)

draw_loop:
  LDI r4, 0x2000
  ADD r4, r10
  LOAD r1, r4            ; r1 = cell = (x<<8)|y

  ; x = r1 >> 8
  LDI r9, 8
  LDI r2, 0
  ADD r2, r1
  SHR r2, r9             ; r2 = x

  ; y = r1 & 0xFF
  LDI r3, 0xFF
  AND r3, r1             ; r3 = y

  ; pixel coords
  LDI r9, 8
  MUL r2, r9
  MUL r3, r9

  ; draw 7x7 green cell (1px gap gives grid look)
  LDI r22, 7
  LDI r23, 7
  LDI r24, 0x00CC44
  RECTF r2, r3, r22, r23, r24

  ; advance ring buffer index
  LDI r9, 1
  ADD r10, r9
  LDI r9, 0x1FF
  AND r10, r9

  ; decrement counter and loop
  LDI r9, 1
  SUB r25, r9
  JZ r25, draw_done
  JMP draw_loop

draw_done:
  RET
