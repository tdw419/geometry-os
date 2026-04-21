; clipboard_demo.asm -- Phase 96: System Clipboard Protocol Demo
;
; Demonstrates the Geometry OS System Clipboard Convention:
;   RAM 0xF10 = ownership flag (0=free, 1=writing, PID=owned)
;   RAM 0xF11 = data length (0-14 words)
;   RAM 0xF12-0xF1F = data (up to 14 u32 words)
;
; Writer claims clipboard, writes 5 color values, releases.
; Then reader picks them up and draws colored rectangles on screen.

; --- Constants ---
LDI r7, 1
LDI r10, 0xF10       ; clipboard ownership flag
LDI r11, 0xF11       ; clipboard data length
LDI r12, 0xF12       ; clipboard data start

; --- Step 1: Claim clipboard (spin until free) ---
claim:
  LOAD r15, r10       ; r15 = ownership flag
  JZ r15, do_write    ; if 0 (free), proceed to write
  JMP claim           ; spin until free

; --- Step 2: Write 5 colors to clipboard ---
do_write:
  LDI r15, 1
  STORE r10, r15      ; set ownership = 1 (writing)

  ; Write color data to clipboard slots 0-4
  LDI r15, 0xFF0000   ; red
  STORE r12, r15

  LDI r15, 0x00FF00   ; green
  MOV r16, r12
  ADD r16, r7
  STORE r16, r15

  LDI r15, 0x0000FF   ; blue
  MOV r16, r12
  ADD r16, r7
  ADD r16, r7
  STORE r16, r15

  LDI r15, 0xFFFF00   ; yellow
  MOV r16, r12
  ADD r16, r7
  ADD r16, r7
  ADD r16, r7
  STORE r16, r15

  LDI r15, 0xFF00FF   ; magenta
  MOV r16, r12
  ADD r16, r7
  ADD r16, r7
  ADD r16, r7
  ADD r16, r7
  STORE r16, r15

  ; Write length
  LDI r15, 5
  STORE r11, r15      ; length = 5

  ; Release clipboard
  LDI r15, 0
  STORE r10, r15      ; ownership = 0 (free)

; --- Step 3: Read back and draw colored rectangles ---
draw_colors:
  ; Load colors from clipboard and draw rectangles
  ; Color 0: red rectangle at x=10, y=50
  LOAD r1, r12
  LDI r2, 10
  LDI r3, 50
  LDI r4, 40
  LDI r5, 30
  RECTF r2, r3, r4, r5, r1

  ; Color 1: green rectangle at x=60, y=50
  MOV r16, r12
  ADD r16, r7
  LOAD r1, r16
  LDI r2, 60
  LDI r3, 50
  LDI r4, 40
  LDI r5, 30
  RECTF r2, r3, r4, r5, r1

  ; Color 2: blue rectangle at x=110, y=50
  MOV r16, r12
  ADD r16, r7
  ADD r16, r7
  LOAD r1, r16
  LDI r2, 110
  LDI r3, 50
  LDI r4, 40
  LDI r5, 30
  RECTF r2, r3, r4, r5, r1

  ; Color 3: yellow rectangle at x=160, y=50
  MOV r16, r12
  ADD r16, r7
  ADD r16, r7
  ADD r16, r7
  LOAD r1, r16
  LDI r2, 160
  LDI r3, 50
  LDI r4, 40
  LDI r5, 30
  RECTF r2, r3, r4, r5, r1

  ; Color 4: magenta rectangle at x=210, y=50
  MOV r16, r12
  ADD r16, r7
  ADD r16, r7
  ADD r16, r7
  ADD r16, r7
  LOAD r1, r16
  LDI r2, 210
  LDI r3, 50
  LDI r4, 40
  LDI r5, 30
  RECTF r2, r3, r4, r5, r1

  HALT                ; done -- 5 colored rectangles on screen
