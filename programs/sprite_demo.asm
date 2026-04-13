; sprite_demo.asm -- Interactive SPRITE demo
;
; 8x8 pixel-art character moves with WASD; simple gravity pulls down.
; Sprite pixel data stored at 0x3000 (64 words, color 0 = transparent).
;
; Registers:
;   r1 = player x
;   r2 = player y
;   r3 = sprite addr (0x3000)
;   r4 = sprite width  (8)
;   r5 = sprite height (8)
;   r10 = vy (vertical velocity, signed two's-complement)
;   r11 = grounded flag

; ── build sprite data at 0x3000 ─────────────────────────────────
; Use a loop to fill a base color, then patch specific pixels.
;
; Pixel layout (. = transparent, H = head/skin, E = eye, B = body, L = leg):
;   Row 0: . H H H H H H .
;   Row 1: H E H H H H E H
;   Row 2: H H H H H H H H
;   Row 3: H H H H H H H H
;   Row 4: B B B B B B B B
;   Row 5: B B B B B B B B
;   Row 6: L L . . . . L L
;   Row 7: L L . . . . L L

  LDI r20, 0xFFBB77  ; skin tone
  LDI r21, 0x00AAFF  ; eye color
  LDI r22, 0x3355AA  ; shirt blue
  LDI r23, 0x223344  ; trouser dark
  LDI r24, 0         ; transparent

  ; fill all 64 slots with skin first
  LDI r9, 0x3000
  LDI r8, 64
fill_skin:
  STORE r9, r20
  LDI r7, 1
  ADD r9, r7
  SUB r8, r7
  JNZ r8, fill_skin

  ; patch row 0 corners to transparent
  LDI r9, 0x3000
  STORE r9, r24          ; (0,0)
  LDI r9, 0x3007
  STORE r9, r24          ; (7,0)

  ; patch eyes at (1,1) and (6,1)
  LDI r9, 0x3009
  STORE r9, r21          ; (1,1)
  LDI r9, 0x300E
  STORE r9, r21          ; (6,1)

  ; fill rows 4-5 with shirt (offsets 32-47)
  LDI r9, 0x3020
  LDI r8, 16
fill_shirt:
  STORE r9, r22
  LDI r7, 1
  ADD r9, r7
  SUB r8, r7
  JNZ r8, fill_shirt

  ; fill rows 6-7 with trousers (offsets 48-63)
  LDI r9, 0x3030
  LDI r8, 16
fill_trousers:
  STORE r9, r23
  LDI r7, 1
  ADD r9, r7
  SUB r8, r7
  JNZ r8, fill_trousers

  ; make trouser inner pixels transparent (middle 4 of each row)
  LDI r7, 0
  LDI r9, 0x3032
  STORE r9, r7           ; (2,6)
  LDI r9, 0x3033
  STORE r9, r7           ; (3,6)
  LDI r9, 0x3034
  STORE r9, r7           ; (4,6)
  LDI r9, 0x3035
  STORE r9, r7           ; (5,6)
  LDI r9, 0x303A
  STORE r9, r7           ; (2,7)
  LDI r9, 0x303B
  STORE r9, r7           ; (3,7)
  LDI r9, 0x303C
  STORE r9, r7           ; (4,7)
  LDI r9, 0x303D
  STORE r9, r7           ; (5,7)

; ── init player state ────────────────────────────────────────────
  LDI r1, 124         ; player x
  LDI r2, 100         ; player y
  LDI r3, 0x3000      ; sprite address
  LDI r4, 8           ; width
  LDI r5, 8           ; height
  LDI r10, 0          ; vy = 0
  LDI r11, 0          ; not grounded

; ── main loop ────────────────────────────────────────────────────
game_loop:
  ; clear
  LDI r6, 0x0A0818
  FILL r6

  ; draw floor at y=220 (a bright line)
  LDI r6, 0x448844
  LDI r7, 0
  LDI r8, 255
  LDI r9, 220
  LINE r7, r9, r8, r9, r6

  ; read key
  IKEY r6

  ; A/a: move left
  LDI r7, 65
  CMP r6, r7
  JZ r0, move_left
  LDI r7, 97
  CMP r6, r7
  JZ r0, move_left

  ; D/d: move right
  LDI r7, 68
  CMP r6, r7
  JZ r0, move_right
  LDI r7, 100
  CMP r6, r7
  JZ r0, move_right

  ; W/w or Space (32): jump (only if grounded)
  LDI r7, 87
  CMP r6, r7
  JZ r0, try_jump
  LDI r7, 119
  CMP r6, r7
  JZ r0, try_jump
  LDI r7, 32
  CMP r6, r7
  JZ r0, try_jump

after_input:
  ; apply gravity: vy += 1, capped at 8
  LDI r7, 1
  ADD r10, r7
  LDI r7, 8
  CMP r10, r7
  BLT r0, apply_vy
  LDI r10, 8

apply_vy:
  ; y += vy
  ADD r2, r10

  ; floor collision at y=212 (sprite 8px tall, floor at 220)
  LDI r7, 212
  CMP r2, r7
  BLT r0, no_floor
  LDI r2, 212
  LDI r10, 0           ; vy = 0
  LDI r11, 1           ; grounded = true
  JMP clamp_x

no_floor:
  LDI r11, 0           ; airborne

clamp_x:
  LDI r7, 0
  CMP r1, r7
  BLT r0, cx_low
  LDI r7, 248
  CMP r1, r7
  BGE r0, cx_hi
  JMP draw_player
cx_low:
  LDI r1, 0
  JMP draw_player
cx_hi:
  LDI r1, 248

draw_player:
  SPRITE r1, r2, r3, r4, r5
  FRAME
  JMP game_loop

move_left:
  LDI r7, 2
  SUB r1, r7
  JMP after_input

move_right:
  LDI r7, 2
  ADD r1, r7
  JMP after_input

try_jump:
  LDI r7, 0
  CMP r11, r7
  JZ r0, after_input   ; not grounded, skip
  LDI r10, 0
  LDI r7, 6
  NEG r7
  ADD r10, r7          ; vy = -6
  LDI r11, 0           ; airborne
  JMP after_input
