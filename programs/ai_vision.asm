; ai_vision.asm -- Phase 88: AI Vision Bridge demo
;
; Demonstrates the AI_AGENT opcode:
;   op=0: screenshot to VFS file
;   op=1: canvas checksum
;   op=3: vision API call (with mock response)
;
; The program draws red bars, takes a checksum,
; modifies the screen, takes another checksum, and calls the vision API.

; ── Draw red bars ──
LDI r7, 1
LDI r3, 0xFF0000   ; red
LDI r1, 0          ; y = 0
LDI r5, 256        ; screen width
LDI r6, 32         ; bar spacing

draw_loop:
  LDI r2, 0        ; x = 0
  draw_row:
    PSET r2, r1, r3
    ADD r2, r7     ; x++
    CMP r2, r5
    BLT r0, draw_row
  ADD r1, r7       ; y++
  CMP r1, r6
  BLT r0, draw_loop

; ── Get checksum before (op=1) ──
LDI r10, 1
AI_AGENT r10       ; r0 = checksum
MOV r12, r0        ; save in r12

; ── Modify screen -- blue square ──
LDI r3, 0x0000FF   ; blue
LDI r1, 100
LDI r6, 156

fill_y:
  LDI r2, 100
  fill_x:
    PSET r2, r1, r3
    ADD r2, r7
    CMP r2, r6
    BLT r0, fill_x
  ADD r1, r7
  CMP r1, r6
  BLT r0, fill_y

; ── Get checksum after (op=1) ──
LDI r10, 1
AI_AGENT r10       ; r0 = new checksum
MOV r13, r0        ; save in r13

; ── Vision API call with mock (op=3) ──
; Write prompt string "desc" to RAM at 0x5000
LDI r14, 0x5000
LDI r10, 0x64      ; 'd'
STORE r14, r10
LDI r15, 0x65      ; 'e'
MOV r10, r14
ADD r10, r7
STORE r10, r15
LDI r15, 0x73      ; 's'
MOV r10, r14
ADD r10, r7
ADD r10, r7
STORE r10, r15
LDI r15, 0x63      ; 'c'
MOV r10, r14
ADD r10, r7
ADD r10, r7
ADD r10, r7
STORE r10, r15
MOV r10, r14
ADD r10, r7
ADD r10, r7
ADD r10, r7
ADD r10, r7
LDI r15, 0
STORE r10, r15

; Set up registers for vision API
LDI r10, 3         ; op=3 (vision API)
MOV r11, r14       ; prompt addr
LDI r12, 0x6000    ; response addr
LDI r13, 256       ; max response length
AI_AGENT r10       ; r0 = response length

HALT
