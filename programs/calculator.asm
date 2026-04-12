; CALCULATOR: Add/subtract calculator with text display
;
; Controls:
;   0-9: enter digits
;   +: add operator
;   -: subtract operator
;   =: compute and display result
;   Q: quit (halt)
;
; After pressing =, displays "X+Y=Z" or "X-Y=Z" and halts.
;
; Register allocation:
;   r0  = CMP result (clobbered by CMP)
;   r1  = state (0=first, 1=op entered, 2=second)
;   r2  = first number accumulator
;   r3  = operator (43='+', 45='-')
;   r4  = second number accumulator
;   r5  = result / number to convert
;   r6  = 0xFFFF (keyboard port)
;   r7  = key value
;   r8  = 48 (ASCII '0') -- constant
;   r9  = 10 -- constant
;   r10 = 0 -- constant
;   r11 = 1 -- constant
;   r12-r14 = temps
;   r15 = text buffer pointer
;   r31 = return address (CALL/RET)
;
; RAM layout:
;   0x0300-0x031F = display text buffer
;   0x0350-0x035F = reversed digit temp buffer
;   0x0360        = digit count
;   0x0362        = quotient temp
;   0x0364        = buffer pointer temp
;   0x0370        = key save temp
;   0x0380        = result save

; ── Constants ──────────────────────────────────────────────────
    LDI r8, 48
    LDI r9, 10
    LDI r10, 0
    LDI r11, 1
    LDI r6, 0xFFFF

; ── Initialize state ──────────────────────────────────────────
    LDI r1, 0
    LDI r2, 0
    LDI r3, 0
    LDI r4, 0

; ── Main keyboard poll loop ──────────────────────────────────
poll:
    LOAD r7, r6
    CMP r7, r10
    JZ r0, poll
    LDI r12, 0x0370
    STORE r12, r7
    LDI r7, 0
    STORE r6, r7
    LOAD r7, r12

    ; Check Q (quit)
    LDI r12, 81
    CMP r7, r12
    JNZ r0, chk_eq
    HALT

chk_eq:
    ; Check = (compute)
    LDI r12, 61
    CMP r7, r12
    JNZ r0, chk_dig
    JMP compute

chk_dig:
    ; Check if key is digit (48-57)
    LDI r12, 48
    CMP r7, r12
    ADD r0, r11
    JZ r0, chk_op
    LDI r12, 57
    CMP r7, r12
    LDI r13, 1
    CMP r0, r13
    JZ r0, chk_op
    ; Is a digit: r7 = digit value
    SUB r7, r8
    ; Branch on state
    CMP r1, r10
    JZ r0, dig_first
    ; State 1 or 2: accumulate in r4
    MUL r4, r9
    ADD r4, r7
    LDI r1, 2
    JMP poll

dig_first:
    MUL r2, r9
    ADD r2, r7
    JMP poll

chk_op:
    ; Check + (43)
    LDI r12, 43
    CMP r7, r12
    JNZ r0, chk_min
    CMP r1, r10
    JNZ r0, poll
    LDI r3, 43
    LDI r1, 1
    JMP poll

chk_min:
    ; Check - (45)
    LDI r12, 45
    CMP r7, r12
    JNZ r0, poll
    CMP r1, r10
    JNZ r0, poll
    LDI r3, 45
    LDI r1, 1
    JMP poll

; ── Compute result ────────────────────────────────────────────
compute:
    XOR r5, r5
    ADD r5, r2
    LDI r12, 43
    CMP r3, r12
    JZ r0, do_add
    SUB r5, r4
    JMP build

do_add:
    ADD r5, r4

; ── Build display string and render ──────────────────────────
build:
    ; Save result to RAM
    LDI r12, 0x0380
    STORE r12, r5
    ; Convert first number to string at 0x0300
    LDI r15, 0x0300
    XOR r5, r5
    ADD r5, r2
    CALL nts
    ; Append operator character
    STORE r15, r3
    ADD r15, r11
    ; Convert second number to string
    XOR r5, r5
    ADD r5, r4
    CALL nts
    ; Append '='
    LDI r12, 61
    STORE r15, r12
    ADD r15, r11
    ; Convert result to string
    LDI r12, 0x0380
    LOAD r5, r12
    CALL nts
    ; Null-terminate
    LDI r12, 0
    STORE r15, r12
    ; Render text at screen (0, 0)
    LDI r12, 0x0300
    TEXT r10, r10, r12
    HALT

; ── num_to_str subroutine ────────────────────────────────────
; Input:  r5 = number to convert, r15 = buffer address
; Output: digits written to buffer, r15 advanced past string
; Clobbers: r0, r12, r13, r14
; Uses RAM: 0x0350 (rev buf), 0x0360 (count), 0x0362 (quotient), 0x0364 (ptr)
nts:
    CMP r5, r10
    JNZ r0, nts_l1
    ; Handle zero
    LDI r12, 48
    STORE r15, r12
    ADD r15, r11
    RET

nts_l1:
    ; Init reversed buffer and counter
    LDI r14, 0x0350
    LDI r12, 0x0364
    STORE r12, r14
    LDI r13, 0
    LDI r12, 0x0360
    STORE r12, r13

nts_l2:
    ; Loop: extract digits from right
    CMP r5, r10
    JZ r0, nts_r1
    ; quotient = number / 10
    XOR r13, r13
    ADD r13, r5
    DIV r13, r9
    ; Save quotient
    LDI r12, 0x0362
    STORE r12, r13
    ; remainder = number - quotient * 10
    XOR r12, r12
    ADD r12, r13
    MUL r12, r9
    SUB r5, r12
    ; Convert to ASCII
    ADD r5, r8
    ; Store in reversed buffer
    LDI r12, 0x0364
    LOAD r14, r12
    STORE r14, r5
    ADD r14, r11
    LDI r12, 0x0364
    STORE r12, r14
    ; Increment digit count
    LDI r12, 0x0360
    LOAD r13, r12
    ADD r13, r11
    STORE r12, r13
    ; number = quotient
    LDI r12, 0x0362
    LOAD r5, r12
    JMP nts_l2

nts_r1:
    ; Reverse: copy digits from buffer to output in correct order
    LDI r12, 0x0360
    LOAD r14, r12
    LDI r13, 0x0350

nts_r2:
    CMP r14, r10
    JZ r0, nts_end
    SUB r14, r11
    XOR r12, r12
    ADD r12, r13
    ADD r12, r14
    LOAD r12, r12
    STORE r15, r12
    ADD r15, r11
    JMP nts_r2

nts_end:
    RET
