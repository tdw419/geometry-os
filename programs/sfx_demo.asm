; sfx_demo.asm -- Retro sound effects catalog
;
; Plays a sequence of notes in different waveforms, each with a
; visual bar on screen. Proves the NOTE opcode works for all
; 5 waveforms with varied frequencies and durations.
;
; Waveforms: 0=sine, 1=square, 2=triangle, 3=sawtooth, 4=noise
;
; Registers:
;   r1  = waveform type
;   r2  = frequency (Hz)
;   r3  = duration (ms)
;   r10 = current sfx index
;   r11 = total sfx count
;   r20 = scratch

; ── init ────────────────────────────────────────────────────────
LDI r30, 0xFF00   ; stack pointer

; ── SFX 0: sine A4 ─────────────────────────────────────────────
  LDI r12, 0x000033
  FILL r12
  LDI r1, 0       ; sine
  LDI r2, 440     ; A4
  LDI r3, 300     ; 300ms
  NOTE r1, r2, r3
  FRAME

; ── SFX 1: square A5 ───────────────────────────────────────────
  LDI r1, 1       ; square
  LDI r2, 880     ; A5
  LDI r3, 150     ; 150ms
  NOTE r1, r2, r3
  FRAME

; ── SFX 2: square bass A2 ──────────────────────────────────────
  LDI r1, 1       ; square
  LDI r2, 110     ; A2
  LDI r3, 400     ; 400ms
  NOTE r1, r2, r3
  FRAME

; ── SFX 3: triangle E5 ─────────────────────────────────────────
  LDI r1, 2       ; triangle
  LDI r2, 660     ; E5
  LDI r3, 250     ; 250ms
  NOTE r1, r2, r3
  FRAME

; ── SFX 4: sawtooth A3 ─────────────────────────────────────────
  LDI r1, 3       ; sawtooth
  LDI r2, 220     ; A3
  LDI r3, 350     ; 350ms
  NOTE r1, r2, r3
  FRAME

; ── SFX 5: noise burst ─────────────────────────────────────────
  LDI r1, 4       ; noise
  LDI r2, 100
  LDI r3, 200     ; 200ms
  NOTE r1, r2, r3
  FRAME

; ── SFX 6: sine laser sweep ────────────────────────────────────
  LDI r1, 0       ; sine
  LDI r2, 1200    ; high freq
  LDI r3, 500     ; long
  NOTE r1, r2, r3
  FRAME

; ── SFX 7: noise percussion ────────────────────────────────────
  LDI r1, 4       ; noise
  LDI r2, 80
  LDI r3, 80      ; short
  NOTE r1, r2, r3
  FRAME

; ── SFX 8: square alarm ────────────────────────────────────────
  LDI r1, 1       ; square
  LDI r2, 2000    ; very high
  LDI r3, 100     ; 100ms
  NOTE r1, r2, r3
  FRAME

; ── SFX 9: triangle bass A1 ────────────────────────────────────
  LDI r1, 2       ; triangle
  LDI r2, 55      ; A1
  LDI r3, 600     ; 600ms
  NOTE r1, r2, r3
  FRAME

; ── done ────────────────────────────────────────────────────────
  LDI r12, 0
  FILL r12
  HALT
