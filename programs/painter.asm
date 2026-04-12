; PAINTER: Read keyboard port, draw colored pixels where cursor is
;
; Controls:
;   W = move cursor up (by 4 pixels)
;   A = move cursor left (by 4 pixels)
;   S = move cursor down (by 4 pixels)
;   D = move cursor right (by 4 pixels)
;   Space = paint colored pixel at cursor position
;   Q = quit
;
; After 5 paint operations, the program halts.
;
; Register allocation:
;   r10 = cursor X (starts at 128)
;   r11 = cursor Y (starts at 128)
;   r12 = key port address (0xFFFF)
;   r13 = color register
;   r14 = zero constant (0)
;   r15 = one constant (1)
;   r16 = four constant (4)
;   r17 = saved key address (0x0300)
;   r18 = max paints (5)
;   r1  = paint counter
;   r2  = temp
;   r3  = temp
;   r4  = temp
;   r5  = color cycling offset

; ── Constants ──────────────────────────────────────────────────
    LDI r10, 128         ; cursor X
    LDI r11, 128         ; cursor Y
    LDI r12, 0xFFFF      ; key port address
    LDI r13, 0x00FFFF    ; cursor color (cyan)
    LDI r14, 0           ; zero
    LDI r15, 1           ; one
    LDI r16, 4           ; step size
    LDI r17, 0x0300      ; saved key RAM address
    LDI r18, 5           ; max paints before halt
    LDI r1, 0            ; paint counter
    LDI r5, 0            ; color cycling offset

; ── Store PAINTER signature at RAM[0x0200..0x0207] ─────────
    LDI r2, 0x0200
    LDI r3, 80           ; P
    STORE r2, r3
    ADD r2, r15
    LDI r3, 65           ; A
    STORE r2, r3
    ADD r2, r15
    LDI r3, 73           ; I
    STORE r2, r3
    ADD r2, r15
    LDI r3, 78           ; N
    STORE r2, r3
    ADD r2, r15
    LDI r3, 84           ; T
    STORE r2, r3
    ADD r2, r15
    LDI r3, 69           ; E
    STORE r2, r3
    ADD r2, r15
    LDI r3, 82           ; R
    STORE r2, r3

; ── Draw initial cursor pixel ────────────────────────────────
    PSET r10, r11, r13

; ── Main poll loop ────────────────────────────────────────────
poll:
    LOAD r2, r12         ; r2 = key from port
    CMP r2, r14          ; r0 = 0 if key == 0
    JZ r0, poll          ; no key -> go back to poll

    ; Key pressed -- save key to RAM, then clear port
    STORE r17, r2        ; save key at RAM[0x0300]
    LDI r2, 0
    STORE r12, r2        ; clear keyboard port

    ; Reload saved key
    LOAD r2, r17

; ── Dispatch on key value ────────────────────────────────────

    ; Check for W (87 = up)
    LDI r3, 87
    CMP r2, r3
    JNZ r0, check_s
    SUB r11, r16         ; Y -= 4
    ; Clamp Y >= 0 (if Y underflowed to large value, fix it)
    LDI r3, 252
    CMP r11, r3
    JNZ r0, move_done
    LDI r11, 0
    JMP move_done

check_s:
    ; Check for S (83 = down)
    LDI r3, 83
    CMP r2, r3
    JNZ r0, check_a
    ADD r11, r16         ; Y += 4
    ; Clamp Y <= 252
    LDI r3, 253
    CMP r11, r3
    JNZ r0, move_done
    LDI r11, 252
    JMP move_done

check_a:
    ; Check for A (65 = left)
    LDI r3, 65
    CMP r2, r3
    JNZ r0, check_d
    SUB r10, r16         ; X -= 4
    ; Clamp X >= 0
    LDI r3, 252
    CMP r10, r3
    JNZ r0, move_done
    LDI r10, 0
    JMP move_done

check_d:
    ; Check for D (68 = right)
    LDI r3, 68
    CMP r2, r3
    JNZ r0, check_space
    ADD r10, r16         ; X += 4
    ; Clamp X <= 252
    LDI r3, 253
    CMP r10, r3
    JNZ r0, move_done
    LDI r10, 252
    JMP move_done

check_space:
    ; Check for Space (32 = paint)
    LDI r3, 32
    CMP r2, r3
    JNZ r0, check_q

    ; Compute paint color
    ; Use paint_counter * 50000 + 0xFF
    ; This cycles through colors as counter grows
    LDI r3, 50000
    MUL r3, r1           ; r3 = paint_counter * 50000
    LDI r4, 0xFF
    OR r3, r4            ; ensure some blue channel

    PSET r10, r11, r3    ; paint pixel

    ; Increment paint counter
    ADD r1, r15
    ADD r5, r15

    ; Check if done
    CMP r1, r18
    JNZ r0, paint_done
    HALT                 ; reached max paints

paint_done:
    JMP poll

check_q:
    ; Check for Q (81 = quit)
    LDI r3, 81
    CMP r2, r3
    JNZ r0, unknown_key
    HALT                 ; quit on Q

unknown_key:
    JMP poll             ; ignore unknown keys

move_done:
    PSET r10, r11, r13   ; draw cursor at new position
    JMP poll
