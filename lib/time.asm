; lib/time.asm -- Standard Library: time and delay utilities
;
; Calling convention:
;   Arguments: r1-r5 (r0 = return value)
;   Caller-saved: r1-r9
;   Callee-saved: r10-r25
;
; Hardware tick counter: RAM[0xFFE] = ticks (incremented each scheduler tick)

; ═══════════════════════════════════════════════════════════════
; get_ticks -- read current tick counter
;   returns r0 = current tick count
; ═══════════════════════════════════════════════════════════════
get_ticks:
    LDI r9, 0xFFE
    LOAD r0, r9
    RET

; ═══════════════════════════════════════════════════════════════
; delay_ticks -- busy-wait for N ticks
;   r1 = number of ticks to wait
; ═══════════════════════════════════════════════════════════════
delay_ticks:
    LDI r9, 0xFFE
    LOAD r10, r9           ; r10 = start ticks
    ADD r10, r1            ; r10 = target tick count
delay_loop:
    LOAD r0, r9            ; r0 = current ticks
    CMP r0, r10            ; compare current with target
    LDI r1, 1
    CMP r0, r1             ; if current < target, CMP(target,current) would be...
    ; Wait, CMP r0, r10 means CMP(current, target)
    ; If current >= target, CMP >= 0 (either 0 or 1)
    ; If current < target, CMP == 0xFFFFFFFF
    LDI r1, 0xFFFFFFFF
    CMP r0, r1             ; compare CMP result with -1
    JZ r0, delay_loop      ; still waiting (CMP == -1 means current < target)
    RET

; ═══════════════════════════════════════════════════════════════
; delay_frames -- busy-wait for N frames (using SLEEP)
;   r1 = number of frames to wait
; ═══════════════════════════════════════════════════════════════
delay_frames:
    SLEEP r1
    RET
