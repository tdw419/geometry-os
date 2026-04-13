; lib/math.asm -- Math Library for Geometry OS
;
; Calling convention: r0 = return, r1-r5 = args, r6-r9 = temps
;
; LINKING:
;   .include "lib/math.asm"
;   .include "math.asm"       (if lib/ is in assembler lib_dir)
;
; Note: CMP sets r0 = -1/0/1. BLT/BGE check r0 from the LAST CMP.
;       Do NOT modify r0 between CMP and BLT/BGE.

; =====================================================================
; INTEGER SQUARE ROOT (Newton's method)
; =====================================================================

; ── sqrt ──────────────────────────────────────────────────────────
; Integer square root.
; Args: r1 = value (unsigned)
; Returns: r0 = floor(sqrt(value))
sqrt:
    PUSH r6
    PUSH r7
    ; Handle 0 and 1
    LDI r6, 1
    CMP r1, r6           ; r0 = compare value vs 1
    BGE r0, sqrt_real    ; if value >= 2, do real sqrt
    MOV r0, r1           ; sqrt(0)=0, sqrt(1)=1
    JMP sqrt_ret
sqrt_real:
    ; Initial guess = value / 2
    MOV r6, r1
    LDI r7, 2
    DIV r6, r7
    JZ r6, sqrt_g1
    JMP sqrt_loop
sqrt_g1:
    LDI r6, 1
sqrt_loop:
    ; new = (guess + value/guess) / 2
    MOV r7, r1
    DIV r7, r6           ; r7 = value/guess
    ADD r7, r6           ; r7 = guess + value/guess
    LDI r0, 2
    DIV r7, r0           ; r7 = (guess + value/guess) / 2
    ; Check convergence: new >= old?
    CMP r7, r6           ; r0 = compare(new, old)
    BGE r0, sqrt_done    ; if new >= old, converged -> old is answer
    MOV r6, r7           ; update guess
    JMP sqrt_loop
sqrt_done:
    MOV r0, r6
sqrt_ret:
    POP r7
    POP r6
    RET
