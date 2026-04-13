; window_manager.asm -- Geometric Window Manager demo
;
; Demonstrates the Window Bounds Protocol:
;   RAM[0xF00] = win_x  (left edge, updated each frame)
;   RAM[0xF01] = win_y  (top edge,  fixed)
;   RAM[0xF02] = win_w  (width,     fixed)
;   RAM[0xF03] = win_h  (height,    fixed)
;
; Primary process: animates a green window border left/right across the screen.
; Child  process:  bounces a red ball INSIDE the window, reading bounds from
;                  shared RAM each iteration so it tracks the moving window.
;
; ── Primary register layout ──────────────────────────────────────────────────
;   r1  = win_x  (animated, oscillates 5..150)
;   r2  = win_y  (fixed: 30)
;   r3  = win_w  (fixed: 100)
;   r4  = win_h  (fixed: 80)
;   r5  = dx     (+1 or -1 for animation direction)
;   r6  = scratch
;   r7  = border color (green 0x00FF00)
;   r8  = x2 = win_x + win_w - 1
;   r9  = y2 = win_y + win_h - 1
;   r10 = constant 1

; ── Primary init ─────────────────────────────────────────────────────────────
  LDI r1, 20          ; initial win_x
  LDI r2, 30          ; win_y (fixed)
  LDI r3, 100         ; win_w
  LDI r4, 80          ; win_h
  LDI r5, 1           ; dx = +1

  ; write initial bounds to shared RAM before spawning child
  LDI r6, 0xF00
  STORE r6, r1
  LDI r6, 0xF01
  STORE r6, r2
  LDI r6, 0xF02
  STORE r6, r3
  LDI r6, 0xF03
  STORE r6, r4

  ; spawn the child
  LDI r6, child
  SPAWN r6

; ── Primary frame loop ───────────────────────────────────────────────────────
frame:
  ; animate win_x
  ADD r1, r5

  ; bounce left: if win_x == 5, dx = +1
  MOV r6, r1
  LDI r7, 5
  CMP r6, r7          ; r6 = sign(win_x - 5): 0 means equal
  JZ r6, set_right

  ; bounce right: if win_x + win_w == 251 (right edge = 250), dx = -1
  MOV r6, r1
  ADD r6, r3          ; r6 = win_x + win_w
  LDI r7, 251
  CMP r6, r7
  JZ r6, set_left
  JMP do_frame

set_right:
  LDI r5, 1
  JMP do_frame

set_left:
  LDI r5, 1
  NEG r5              ; r5 = -1

do_frame:
  ; update shared bounds (win_x may have changed)
  LDI r6, 0xF00
  STORE r6, r1

  ; clear screen to black
  LDI r6, 0
  FILL r6

  ; compute border corners
  ;   r8 = x2 = win_x + win_w - 1
  ;   r9 = y2 = win_y + win_h - 1
  LDI r10, 1

  MOV r8, r1
  ADD r8, r3
  SUB r8, r10         ; r8 = x2

  MOV r9, r2
  ADD r9, r4
  SUB r9, r10         ; r9 = y2

  LDI r7, 0x00FF00    ; green border

  ; draw window border (4 sides)
  LINE r1, r2, r8, r2, r7    ; top
  LINE r1, r9, r8, r9, r7    ; bottom
  LINE r1, r2, r1, r9, r7    ; left
  LINE r8, r2, r8, r9, r7    ; right

  FRAME
  JMP frame

; ── Child process (spawned at 0x400) ─────────────────────────────────────────
; Starts with all registers zero. Bounces a red ball inside the window.
;
; Child register layout:
;   r1  = ball x-offset from win_x (offset into window, not absolute)
;   r2  = ball y-offset from win_y
;   r3  = vx  (+1 or -1)
;   r4  = vy  (+1 or -1)
;   r5  = ball color
;   r6  = scratch / addr
;   r7  = scratch
;   r8  = scratch
;   r9  = constant 31 (for SAR sign-extension)
;   r10 = win_x (loaded from shared RAM)
;   r11 = win_y
;   r12 = win_w
;   r13 = win_h
;   r14 = abs_x = win_x + r1
;   r15 = abs_y = win_y + r2

.org 0x400

child:
  LDI r1, 8           ; initial x offset
  LDI r2, 8           ; initial y offset
  LDI r3, 1           ; vx = +1
  LDI r4, 1           ; vy = +1
  LDI r5, 0xFF4444    ; red-orange ball
  LDI r9, 31          ; constant for SAR sign-extension

child_loop:
  ; read current window bounds from shared RAM
  LDI r6, 0xF00
  LOAD r10, r6        ; win_x
  LDI r6, 0xF01
  LOAD r11, r6        ; win_y
  LDI r6, 0xF02
  LOAD r12, r6        ; win_w
  LDI r6, 0xF03
  LOAD r13, r6        ; win_h

  ; move ball
  ADD r1, r3
  ADD r2, r4

  ; ── x bounce ─────────────────────────────────────────
  ; left wall: if x_offset < 2, flip vx = +1 and clamp
  MOV r6, r1
  LDI r7, 2
  SUB r6, r7          ; r6 = x_offset - 2 (negative if x < 2)
  SAR r6, r9          ; r6 = 0xFFFFFFFF if negative, 0 if non-negative
  JZ r6, chk_rwall    ; non-negative → in bounds, skip
  LDI r3, 1           ; bounce right
  LDI r1, 2           ; clamp

chk_rwall:
  ; right wall: if x_offset >= win_w - 3, flip vx = -1 and clamp
  MOV r6, r12
  LDI r7, 3
  SUB r6, r7          ; r6 = win_w - 3  (max safe offset)
  MOV r7, r1
  SUB r7, r6          ; r7 = x_offset - (win_w - 3), positive if overflow
  SAR r7, r9          ; r7 = 0xFFFFFFFF if in bounds, 0 if at/past wall
  JNZ r7, chk_twall   ; 0xFFFFFFFF (nonzero) = in bounds, skip
  LDI r3, 1
  NEG r3              ; vx = -1
  MOV r1, r6          ; clamp to win_w - 3

  ; ── y bounce ─────────────────────────────────────────
chk_twall:
  ; top wall: if y_offset < 2, flip vy = +1 and clamp
  MOV r6, r2
  LDI r7, 2
  SUB r6, r7
  SAR r6, r9
  JZ r6, chk_bwall
  LDI r4, 1
  LDI r2, 2

chk_bwall:
  ; bottom wall: if y_offset >= win_h - 3, flip vy = -1 and clamp
  MOV r6, r13
  LDI r7, 3
  SUB r6, r7          ; r6 = win_h - 3
  MOV r7, r2
  SUB r7, r6
  SAR r7, r9
  JNZ r7, draw_ball
  LDI r4, 1
  NEG r4              ; vy = -1
  MOV r2, r6          ; clamp

  ; ── draw ball ────────────────────────────────────────
draw_ball:
  MOV r14, r10
  ADD r14, r1         ; abs_x = win_x + x_offset
  MOV r15, r11
  ADD r15, r2         ; abs_y = win_y + y_offset
  PSET r14, r15, r5
  JMP child_loop
