; replay_demo.asm -- Draw frames, then replay them backward
;
; Phase 38c demo: Uses SNAP_TRACE to record, FRAME to checkpoint,
; then REPLAY to display frames in reverse.
;
; Draws a vertical color bar that moves across the screen for 16 frames,
; then replays those frames backward using the REPLAY opcode.

  LDI r1, 1         ; mode = start recording
  SNAP_TRACE r1      ; begin trace recording

  LDI r10, 0         ; frame counter
  LDI r11, 16        ; total frames to draw
  LDI r12, 0         ; bar x position

draw_loop:
  ; Clear screen to black
  LDI r9, 0
  FILL r9

  ; Draw a vertical bar at position r12
  LDI r2, 0          ; y = 0
  LDI r3, 256        ; y limit
  LDI r5, 0x00FF00   ; green bar color
  LDI r6, 4          ; bar width

bar_y:
  ; Draw 4 pixels wide at (r12, r2)
  LDI r4, 0
bar_x:
  LDI r7, 0
  ADD r7, r12
  ADD r7, r4         ; x = bar_pos + offset
  SCREENP r7, r2, r5    ; set pixel
  LDI r8, 1
  ADD r4, r8
  CMP r4, r6
  BLT r0, bar_x

  LDI r8, 1
  ADD r2, r8
  CMP r2, r3
  BLT r0, bar_y

  ; Advance bar position
  LDI r8, 16
  ADD r12, r8

  ; Show frame (also triggers checkpoint since trace recording is on)
  FRAME

  LDI r8, 1
  ADD r10, r8
  CMP r10, r11
  BLT r0, draw_loop

  ; Stop recording
  LDI r1, 0
  SNAP_TRACE r1

  ; Now replay backward: show frames from newest to oldest
  LDI r13, 0         ; replay index (0 = newest)

replay_loop:
  LDI r1, 0
  ADD r1, r13        ; frame index to replay
  REPLAY r1

  ; Small delay between replay frames
  LDI r14, 0
delay:
  LDI r15, 1
  ADD r14, r15
  CMP r14, r11       ; short delay
  BLT r0, delay

  LDI r8, 1
  ADD r13, r8
  CMP r13, r11       ; replay all 16 frames
  BLT r0, replay_loop

  HALT
