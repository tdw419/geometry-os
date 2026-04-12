; scroll_demo.asm -- draw a bright bar at y=240 then scroll it upward
; CMP always writes result into r0, so keep x in r1, use r0 only for CMP

LDI r1, 0       ; x counter
LDI r2, 240     ; y
LDI r3, 0x00FF88 ; green bar color
LDI r9, 256     ; x limit (loop until x == 256)

bar_loop:
  PSET r1, r2, r3
  LDI r8, 1
  ADD r1, r8
  CMP r1, r9       ; r0 = 0 if r1==256, else non-zero
  JZ r0, scroll_phase
  JMP bar_loop

scroll_phase:
  LDI r4, 240     ; scroll 240 times = bar reaches top
  LDI r5, 1       ; scroll 1 pixel per step

scroll_loop:
  SCROLL r5
  LDI r6, 1
  SUB r4, r6
  JNZ r4, scroll_loop

HALT
