; ai_interact.asm -- AI Agent Input Demo (Phase 89)
; Demonstrates programmatic input injection via AI_INJECT opcode.
; The program injects keystrokes into itself, reads them back, and
; displays the result on screen -- proving AI can drive the GUI.

  ; Set up constants
  LDI r7, 1             ; increment constant

  ; === Phase 1: Inject a key and read it back ===
  LDI r1, 0             ; op=0 (inject key)
  LDI r2, 65            ; keycode = 65 ('A')
  LDI r3, 0             ; shift = 0
  AI_INJECT r1           ; inject 'A' into key buffer

  ; Read the injected key back
  IKEY r4               ; r4 = 65 ('A') from the key buffer

  ; Verify: draw green if correct, red if wrong
  LDI r10, 0xFF00       ; green
  LDI r11, 0xFF0000     ; red
  LDI r12, 65           ; expected value
  CMP r4, r12
  JZ r0, key_ok

  ; Wrong key -- draw red border
  LDI r20, 0
  LDI r21, 0
  LDI r22, 256
  LDI r23, 4
  RECTF r20, r21, r22, r23, r11
  JMP do_mouse

key_ok:
  ; Correct key -- draw green border
  LDI r20, 0
  LDI r21, 0
  LDI r22, 256
  LDI r23, 4
  RECTF r20, r21, r22, r23, r10

do_mouse:
  ; === Phase 2: Inject mouse move ===
  LDI r1, 1             ; op=1 (inject mouse move)
  LDI r2, 128           ; x = 128
  LDI r3, 128           ; y = 128
  AI_INJECT r1           ; inject mouse move

  ; === Phase 3: Inject mouse click ===
  LDI r1, 2             ; op=2 (inject mouse click)
  LDI r2, 100           ; x = 100
  LDI r3, 200           ; y = 200
  LDI r4, 2             ; button = 2 (click)
  AI_INJECT r1           ; inject mouse click

  ; === Phase 4: Inject text string ===
  ; Write "HI" to RAM at 0x2000
  LDI r10, 0x2000
  LDI r11, 72           ; 'H'
  STORE r10, r11
  MOV r12, r10
  ADD r12, r7           ; r12 = 0x2001
  LDI r11, 73           ; 'I'
  STORE r12, r11
  MOV r12, r10
  ADD r12, r7
  ADD r12, r7           ; r12 = 0x2002
  LDI r11, 0
  STORE r12, r11        ; null terminator

  ; Inject the text string
  LDI r1, 3             ; op=3 (inject text string)
  LDI r2, 0x2000        ; addr of null-terminated string
  AI_INJECT r1           ; inject "HI" -- r0 = count (2)

  ; Draw colored bar for each injected char
  LDI r14, 0x0000FF     ; blue
  LDI r10, 10           ; start x
  LDI r11, 10           ; y position
  LDI r12, 10           ; width
  LDI r13, 10           ; height
  LDI r15, 0            ; counter

draw_loop:
  CMP r15, r0            ; compare counter with injected count
  BGE r0, done
  RECTF r10, r11, r12, r13, r14
  LDI r16, 12
  ADD r10, r16           ; move x right
  ADD r15, r7            ; counter++
  JMP draw_loop

done:
  HALT
