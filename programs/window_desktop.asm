; window_desktop.asm -- Multi-window desktop demo (Phase 68b)
; Demonstrates WINSYS + MOUSEQ integration: drag, focus, multiple windows.
; Each window draws its own content into its offscreen buffer.
;
; Controls: Move mouse to interact. Click title bar to drag, click body to focus.
; Press Q to quit.

; ── Constants ──
LDI r7, 1
LDI r20, 0         ; zero constant

; ── Store title strings in RAM ──
LDI r6, 0x2000
STRO r6, "Hello"
LDI r6, 0x2020
STRO r6, "Counter"
LDI r6, 0x2040
STRO r6, "Colors"

; ── Create 3 windows ──

; Window 1: "Hello" at (20, 20), 100x80
LDI r1, 20
LDI r2, 20
LDI r3, 100
LDI r4, 80
LDI r5, 0x2000     ; title addr
LDI r6, 0          ; op=0 (create)
WINSYS r6
MOV r10, r0        ; r10 = win1_id

; Window 2: "Counter" at (140, 30), 100x80
LDI r1, 140
LDI r2, 30
LDI r3, 100
LDI r4, 80
LDI r5, 0x2020
LDI r6, 0
WINSYS r6
MOV r11, r0        ; r11 = win2_id

; Window 3: "Colors" at (60, 120), 100x80
LDI r1, 60
LDI r2, 120
LDI r3, 100
LDI r4, 80
LDI r5, 0x2040
LDI r6, 0
WINSYS r6
MOV r12, r0        ; r12 = win3_id

; ── Draw window contents ──

; Window 1: Green background with welcome text
CALL draw_win1

; Window 2: Blue background with counter
CALL draw_win2

; Window 3: Color stripes
CALL draw_win3

; ── Drag state ──
; r15 = dragging window id (0 = none)
; r16 = drag offset X
; r17 = drag offset Y
LDI r15, 0
LDI r16, 0
LDI r17, 0

; ── Counter for window 2 ──
LDI r18, 0         ; counter value

; ── Main loop ──
main_loop:
  FILL r20          ; clear screen to black

  ; Read mouse state: r21=mx, r22=my, r23=btn
  MOUSEQ r21

  ; Read keyboard
  IKEY r24
  LDI r25, 81       ; 'Q'
  CMP r24, r25
  JZ r0, quit

  ; ── Handle mouse button ──
  LDI r25, 2
  CMP r23, r25      ; click?
  BGE r0, check_drag

  ; Fresh click -- do hittest
  LDI r6, 4         ; op=4 (hittest)
  WINSYS r6         ; r0=win_id, r1=hit_type

  ; If no hit, skip
  JZ r0, no_click_action

  ; Bring clicked window to front
  LDI r6, 2         ; op=2 (bring to front)
  WINSYS r6

  ; Check hit type -- title bar = start drag
  LDI r25, 1
  CMP r1, r25       ; r1 = hit_type from hittest
  JNZ r0, no_click_action

  ; Start dragging (title bar hit)
  MOV r15, r0       ; r15 = dragging win_id

  ; Get window info for drag offset
  MOV r0, r15
  LDI r1, 0x9000    ; winfo dest addr
  LDI r6, 6         ; op=6 (winfo)
  WINSYS r6

  ; drag_ox = mouse_x - win_x
  LOAD r25, 0x9000  ; win_x
  MOV r26, r21
  SUB r26, r25      ; mx - win_x
  MOV r16, r26

  ; drag_oy = mouse_y - win_y
  LOAD r25, 0x9001  ; win_y
  MOV r26, r22
  SUB r26, r25      ; my - win_y
  MOV r17, r26

  JMP no_click_action

check_drag:
  ; If not dragging, skip
  JZ r15, no_click_action

  ; If button released (btn=0), stop dragging
  JZ r23, stop_drag

  ; Still dragging -- move window to mouse position
  MOV r0, r15       ; win_id
  MOV r1, r21
  SUB r1, r16       ; new_x = mx - drag_ox
  MOV r2, r22
  SUB r2, r17       ; new_y = my - drag_oy
  LDI r6, 5         ; op=5 (moveto)
  WINSYS r6

  JMP no_click_action

stop_drag:
  LDI r15, 0        ; stop dragging
  JMP no_click_action

no_click_action:

  ; ── Update counter (throttled to ~2/sec) ──
  LDI r25, 0xFFE
  LOAD r26, r25     ; TICKS
  LDI r27, 31
  AND r26, r27      ; TICKS & 31
  JNZ r26, skip_counter
  ADD r18, r7       ; counter++
  CALL draw_win2    ; redraw with new counter
skip_counter:

  ; ── Animate colors window (~4/sec) ──
  LDI r25, 0xFFE
  LOAD r26, r25
  LDI r27, 15
  AND r26, r27
  JNZ r26, skip_colors
  CALL draw_win3
skip_colors:

  ; ── Draw window borders and title bars on screen ──
  ; Window 1 border
  MOV r0, r10
  CALL draw_window_frame

  ; Window 2 border
  MOV r0, r11
  CALL draw_window_frame

  ; Window 3 border
  MOV r0, r12
  CALL draw_window_frame

  FRAME
  JMP main_loop

quit:
  HALT

; ── Draw Window 1: Green background ──
draw_win1:
  PUSH r31
  PUSH r0
  MOV r0, r10       ; win_id

  ; Fill with dark green
  LDI r3, 0x003300  ; dark green
  LDI r4, 0         ; y
fill1_loop:
  LDI r6, 0         ; x
fill1_x:
  WPIXEL r0, r6, r4, r3
  ADD r6, r7        ; x++
  LDI r25, 100
  CMP r6, r25
  BLT r0, fill1_x
  ADD r4, r7        ; y++
  LDI r25, 80
  CMP r4, r25
  BLT r0, fill1_loop

  ; Draw a bright green H in the center
  LDI r3, 0x00FF00  ; bright green
  ; Left vertical of H
  LDI r4, 30
  LDI r6, 40
h_left:
  WPIXEL r0, r6, r4, r3
  WPIXEL r0, r6, r4, r3
  ADD r4, r7
  LDI r25, 60
  CMP r4, r25
  BLT r0, h_left

  ; Right vertical of H
  LDI r4, 30
  LDI r6, 48
h_right:
  WPIXEL r0, r6, r4, r3
  ADD r4, r7
  LDI r25, 60
  CMP r4, r25
  BLT r0, h_right

  ; Crossbar of H
  LDI r4, 44
  LDI r6, 40
h_bar:
  WPIXEL r0, r6, r4, r3
  ADD r6, r7
  LDI r25, 49
  CMP r6, r25
  BLT r0, h_bar

  POP r0
  POP r31
  RET

; ── Draw Window 2: Blue background with counter bar ──
draw_win2:
  PUSH r31
  PUSH r0

  MOV r0, r11       ; win_id

  ; Fill with dark blue
  LDI r3, 0x000033  ; dark blue
  LDI r4, 0
fill2_loop:
  LDI r6, 0
fill2_x:
  WPIXEL r0, r6, r4, r3
  ADD r6, r7
  LDI r25, 100
  CMP r6, r25
  BLT r0, fill2_x
  ADD r4, r7
  LDI r25, 80
  CMP r4, r25
  BLT r0, fill2_loop

  ; Draw counter bar (white bar, length = counter % 90 + 5)
  LDI r3, 0xFFFFFF  ; white
  LDI r4, 40        ; y position
  MOV r5, r18
  LDI r25, 90
  MOD r5, r25
  ADD r5, r7        ; +1 = at least 1 pixel
  LDI r6, 5         ; x offset
bar_loop:
  CMP r6, r5
  BGE r0, bar_done
  WPIXEL r0, r6, r4, r3
  ADD r6, r7
  JMP bar_loop
bar_done:

  POP r0
  POP r31
  RET

; ── Draw Window 3: Animated color stripes ──
draw_win3:
  PUSH r31
  PUSH r0
  PUSH r1
  PUSH r2

  MOV r0, r12       ; win_id

  ; Get ticks for animation
  LDI r25, 0xFFE
  LOAD r26, r25     ; ticks

  ; Fill with animated stripes
  LDI r4, 0         ; y
stripe_y:
  LDI r6, 0         ; x
stripe_x:
  ; Color from position + time
  MOV r1, r6
  ADD r1, r26       ; x + ticks
  LDI r25, 31
  AND r1, r25       ; (x + ticks) & 31
  LDI r25, 3
  MUL r1, r25       ; 0-93
  LDI r25, 8
  SHL r1, r25       ; shift into green channel
  MOV r3, r4
  LDI r2, 7
  AND r3, r2        ; y & 7
  LDI r25, 16
  SHL r3, r25       ; red channel
  OR r1, r3         ; combine
  MOV r3, r1

  WPIXEL r0, r6, r4, r3
  ADD r6, r7        ; x++
  LDI r25, 100
  CMP r6, r25
  BLT r0, stripe_x

  ADD r4, r7        ; y++
  LDI r25, 80
  CMP r4, r25
  BLT r0, stripe_y

  POP r2
  POP r1
  POP r0
  POP r31
  RET

; ── Draw window frame (title bar + border) on screen ──
; r0 = win_id
draw_window_frame:
  PUSH r31
  PUSH r0
  PUSH r1
  PUSH r2
  PUSH r3
  PUSH r4
  PUSH r5
  PUSH r6

  ; Get window info
  MOV r0, r0
  LDI r1, 0x9200    ; winfo addr
  LDI r6, 6         ; WINFO
  WINSYS r6
  JZ r0, frame_done  ; window not found

  LOAD r2, 0x9200   ; win_x
  LOAD r3, 0x9201   ; win_y
  LOAD r4, 0x9202   ; win_w
  LOAD r5, 0x9203   ; win_h

  ; Title bar (12px tall, purple)
  LDI r1, 0x444488
  MOV r6, r3         ; y = win_y
title_y:
  MOV r0, r2         ; x = win_x
title_x:
  PSET r0, r6, r1
  ADD r0, r7
  MOV r25, r2
  ADD r25, r4        ; x limit
  CMP r0, r25
  BLT r0, title_x
  ADD r6, r7
  LDI r25, 12
  ADD r25, r3        ; y limit (win_y + 12)
  CMP r6, r25
  BLT r0, title_y

  ; Border outline (gray)
  LDI r1, 0x888888
  ; Left + right edges
  MOV r0, r3         ; y = win_y
border_y:
  PSET r2, r0, r1    ; left edge
  MOV r25, r2
  ADD r25, r4
  SUB r25, r7        ; right edge x
  PSET r25, r0, r1
  ADD r0, r7
  MOV r25, r3
  ADD r25, r5
  CMP r0, r25
  BLT r0, border_y

  ; Bottom edge
  MOV r0, r3
  ADD r0, r5
  SUB r0, r7         ; bottom y
  MOV r6, r2         ; x = win_x
bottom_x:
  PSET r6, r0, r1
  ADD r6, r7
  MOV r25, r2
  ADD r25, r4
  CMP r6, r25
  BLT r0, bottom_x

frame_done:
  POP r6
  POP r5
  POP r4
  POP r3
  POP r2
  POP r1
  POP r0
  POP r31
  RET
