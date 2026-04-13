; multiproc.asm -- SPATIAL_SPAWN demo
;
; Primary process: draws a bouncing white dot in the LEFT half (x: 0..127)
; Spawned process: draws a bouncing red  dot in the RIGHT half (x: 128..255)
; Both share the same 256x256 screen and run concurrently via SPAWN.
;
; Register layout (primary):
;   r1 = x position
;   r2 = y position
;   r3 = vx (velocity x)
;   r4 = vy (velocity y)
;   r5 = color (white)
;   r6 = scratch
;   r7 = spawn address

; ── init primary ──────────────────────────────────────────────────────────────
  LDI r1, 32          ; start x = 32 (left half)
  LDI r2, 128         ; start y = 128
  LDI r3, 1           ; vx = +1
  LDI r4, 1           ; vy = +1
  LDI r5, 0xFFFFFF    ; white

  ; spawn the red-dot process
  LDI r7, red_proc
  SPAWN r7

; ── primary game loop ─────────────────────────────────────────────────────────
loop:
  ; erase old pixel
  LDI r6, 0
  PIXEL r1, r2, r6

  ; move
  ADD r1, r3
  ADD r2, r4

  ; bounce left wall (x = 0)
  LDI r6, 0
  CMP r1, r6
  JNZ r6, chk_rw
  LDI r3, 1
chk_rw:
  ; bounce right wall of left half (x = 127)
  LDI r6, 127
  CMP r1, r6
  JNZ r6, chk_tw
  LDI r3, -1
chk_tw:
  ; bounce top wall (y = 0)
  LDI r6, 0
  CMP r2, r6
  JNZ r6, chk_bw
  LDI r4, 1
chk_bw:
  ; bounce bottom wall (y = 255)
  LDI r6, 255
  CMP r2, r6
  JNZ r6, draw
  LDI r4, -1

draw:
  PIXEL r1, r2, r5
  FRAME
  JMP loop

; ── spawned process: red dot in right half ─────────────────────────────────────
; Spawned processes start with zeroed registers, so we initialise everything here.
red_proc:
  LDI r1, 196         ; start x = 196 (right half)
  LDI r2, 64          ; start y = 64
  LDI r3, -1          ; vx = -1
  LDI r4, 1           ; vy = +1
  LDI r5, 0xFF2020    ; red

red_loop:
  ; erase
  LDI r6, 0
  PIXEL r1, r2, r6

  ; move
  ADD r1, r3
  ADD r2, r4

  ; bounce left wall of right half (x = 128)
  LDI r6, 128
  CMP r1, r6
  JNZ r6, red_chk_rw
  LDI r3, 1
red_chk_rw:
  ; bounce right wall (x = 255)
  LDI r6, 255
  CMP r1, r6
  JNZ r6, red_chk_tw
  LDI r3, -1
red_chk_tw:
  ; bounce top (y = 0)
  LDI r6, 0
  CMP r2, r6
  JNZ r6, red_chk_bw
  LDI r4, 1
red_chk_bw:
  ; bounce bottom (y = 255)
  LDI r6, 255
  CMP r2, r6
  JNZ r6, red_draw
  LDI r4, -1
red_draw:
  PIXEL r1, r2, r5
  ; No FRAME here — the primary process drives frame sync
  JMP red_loop
