; BLINK: Toggle a pixel on/off using keyboard input and CMP
;
; Demonstrates the CMP opcode (0x50) by toggling a center pixel
; between green (0x00FF00) and black. Reads keyboard port at
; RAM[0xFFFF] and uses CMP against zero to detect keypresses.
;
; Register allocation:
;   r10 = green color (0x00FF00)
;   r11 = black color (0x000000)
;   r12 = keyboard port address (0xFFFF)
;   r13 = center X (128)
;   r14 = center Y (128)
;   r15 = zero constant (0)
;   r16 = one constant (1)
;   r17 = max toggles (3)
;   r1  = toggle state (0=off, 1=on)
;   r2  = toggle counter
;   r3  = temp (loaded values)

; ── Setup ────────────────────────────────────────────────────
    LDI r10, 0x00FF00
    LDI r11, 0
    LDI r12, 0xFFFF
    LDI r13, 128
    LDI r14, 128
    LDI r15, 0
    LDI r16, 1
    LDI r17, 3
    LDI r1, 1
    LDI r2, 0

; ── Store "BLINK" signature at RAM[0x200..0x204] ────────────
    LDI r20, 0x0200
    LDI r3, 66
    STORE r20, r3
    ADD r20, r16
    LDI r3, 76
    STORE r20, r3
    ADD r20, r16
    LDI r3, 73
    STORE r20, r3
    ADD r20, r16
    LDI r3, 78
    STORE r20, r3
    ADD r20, r16
    LDI r3, 75
    STORE r20, r3

; ── Initial state: pixel ON (green) ─────────────────────────
    PSET r13, r14, r10

; ── Main loop: poll keyboard, toggle on keypress ─────────────
poll:
    LOAD r3, r12
    CMP r3, r15
    JZ r0, poll

    ; Key pressed: clear the port
    LDI r3, 0
    STORE r12, r3

    ; Check current toggle state using CMP
    CMP r1, r15
    JZ r0, turn_on

    ; Currently ON (r1 > 0): turn OFF
    PSET r13, r14, r11
    LDI r1, 0
    JMP increment

turn_on:
    PSET r13, r14, r10
    LDI r1, 1

increment:
    ADD r2, r16
    CMP r2, r17
    JNZ r0, poll

    HALT
