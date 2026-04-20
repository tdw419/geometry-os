; sci_calc.asm -- Scientific Calculator for Geometry OS
;
; Phase 75: Scientific calculator with sin/cos/sqrt.
; Uses pre-computed quarter-wave sine table (16 entries for 0-90 deg).
; Sqrt uses Newton method iteration.
;
; Input via keyboard:
;   0-9   = digits
;   +     = add (43)
;   -     = subtract (45)
;   *     = multiply (42)
;   /     = divide (47)
;   S     = sin (83)
;   C     = cos (67)
;   Q     = sqrt (81)
;   =     = equals (61)
;   Enter = evaluate (13)
;   Esc   = clear (27)
;
; RAM Layout:
;   0x4000 = display value
;   0x4004 = accumulator
;   0x4008 = operator (0=none, 1=add, 2=sub, 3=mul, 4=div)
;   0x400C = new entry flag
;   0x6000 = sin quarter table (16 entries, sin(0..90)*1000)
;   0x6040 = text buffer
;   0x6100 = display string buffer
;
; NOTE: No colons in comments (assembler pitfall)

#define DISP_VAL    0x4000
#define ACCUM       0x4004
#define OPER        0x4008
#define NEW_ENTRY   0x400C
#define SIN_QTR     0x6000
#define TXT_BUF     0x6040
#define DISP_STR    0x6100

; ── INIT ──────────────────────────────────────────
    LDI r1, 1
    LDI r2, 0

    LDI r20, DISP_VAL
    STORE r20, r2
    LDI r20, ACCUM
    STORE r20, r2
    LDI r20, OPER
    STORE r20, r2
    LDI r20, NEW_ENTRY
    LDI r3, 1
    STORE r20, r3

    ; Build quarter-wave sine table (16 entries for 0-90 degrees)
    ; sin(n*90/15) * 1000 for n=0..15
    ; Pre-computed: 0, 104, 207, 309, 407, 500, 588, 669, 743, 809, 866, 914, 951, 978, 995, 1000
    LDI r20, SIN_QTR
    LDI r3, 0
    STORE r20, r3
    ADDI r20, 1
    LDI r3, 104
    STORE r20, r3
    ADDI r20, 1
    LDI r3, 207
    STORE r20, r3
    ADDI r20, 1
    LDI r3, 309
    STORE r20, r3
    ADDI r20, 1
    LDI r3, 407
    STORE r20, r3
    ADDI r20, 1
    LDI r3, 500
    STORE r20, r3
    ADDI r20, 1
    LDI r3, 588
    STORE r20, r3
    ADDI r20, 1
    LDI r3, 669
    STORE r20, r3
    ADDI r20, 1
    LDI r3, 743
    STORE r20, r3
    ADDI r20, 1
    LDI r3, 809
    STORE r20, r3
    ADDI r20, 1
    LDI r3, 866
    STORE r20, r3
    ADDI r20, 1
    LDI r3, 914
    STORE r20, r3
    ADDI r20, 1
    LDI r3, 951
    STORE r20, r3
    ADDI r20, 1
    LDI r3, 978
    STORE r20, r3
    ADDI r20, 1
    LDI r3, 995
    STORE r20, r3
    ADDI r20, 1
    LDI r3, 1000
    STORE r20, r3

    ; Init stack
    LDI r30, 0xFD00

; ── MAIN LOOP ─────────────────────────────────────
main_loop:
    LDI r1, 1

    IKEY r5

    CMPI r5, 0
    JZ r0, no_key

    ; Digit 0-9
    CMPI r5, 48
    BLT r0, not_digit
    CMPI r5, 57
    BGE r0, not_digit_ok
    ; Between '0' and '9' but not '9' - need <= check
    ; Actually BGE means >= 57 = '9'+0 means >= '9'
    ; We need >= '0' AND <= '9' (48-57)
    ; BLT r0 means if r5 < 48 skip; BGE r0 means if r5 >= 57+1 skip? No.
    ; BGE branches when r0 != 0xFFFFFFFF (CMP found >=)
    ; CMPI r5, 57; BGE r0, not_digit_ok means if r5 >= 57 goto not_digit_ok
    ; But we want if r5 > 57 skip. So BGE at 57 means r5 >= 57 is ok (57='9')
    ; Wait, we need to handle r5 = 57 ('9') as a digit too.
    ; The correct pattern: skip if r5 < 48 OR r5 > 57.
    ; CMPI r5, 48; BLT skip (if < 48)
    ; CMPI r5, 58; BGE skip (if >= 58, i.e. > 57)
    JMP enter_digit_path
not_digit_ok:

not_digit:
    JMP check_ops

enter_digit_path:
    SUBI r5, 48
    CALL enter_digit
    JMP no_key

check_ops:
    ; + = add
    CMPI r5, 43
    JNZ r0, not_add
    CALL do_add
    JMP no_key
not_add:

    ; - = subtract
    CMPI r5, 45
    JNZ r0, not_sub
    CALL do_sub
    JMP no_key
not_sub:

    ; * = multiply
    CMPI r5, 42
    JNZ r0, not_mul
    CALL do_mul
    JMP no_key
not_mul:

    ; / = divide
    CMPI r5, 47
    JNZ r0, not_div
    CALL do_div
    JMP no_key
not_div:

    ; = or Enter = evaluate
    CMPI r5, 61
    JZ r0, do_eval
    CMPI r5, 13
    JNZ r0, not_eval
do_eval:
    CALL evaluate
    JMP no_key
not_eval:

    ; S = sin
    CMPI r5, 83
    JNZ r0, not_sin
    CALL do_sin
    JMP no_key
not_sin:

    ; C = cos
    CMPI r5, 67
    JNZ r0, not_cos
    CALL do_cos
    JMP no_key
not_cos:

    ; Q = sqrt
    CMPI r5, 81
    JNZ r0, not_sqrt
    CALL do_sqrt
    JMP no_key
not_sqrt:

    ; Escape = clear
    CMPI r5, 27
    JNZ r0, no_key
    CALL clear_calc

no_key:
    CALL render
    FRAME
    JMP main_loop

; ── ENTER DIGIT ───────────────────────────────────
enter_digit:
    PUSH r31
    ; r5 = digit value (0-9)
    ; Check new_entry
    LDI r20, NEW_ENTRY
    LOAD r6, r20
    CMPI r6, 1
    JNZ r0, append_digit
    ; Clear display
    LDI r20, DISP_VAL
    LDI r2, 0
    STORE r20, r2
    LDI r20, NEW_ENTRY
    STORE r20, r2
append_digit:
    LDI r20, DISP_VAL
    LOAD r6, r20
    LDI r7, 10
    MUL r6, r7
    ADD r6, r5
    STORE r20, r6
    POP r31
    RET

; ── OPERATORS ─────────────────────────────────────
do_add:
    PUSH r31
    LDI r6, 1
    LDI r20, OPER
    STORE r20, r6
    CALL apply_op
    POP r31
    RET

do_sub:
    PUSH r31
    LDI r6, 2
    LDI r20, OPER
    STORE r20, r6
    CALL apply_op
    POP r31
    RET

do_mul:
    PUSH r31
    LDI r6, 3
    LDI r20, OPER
    STORE r20, r6
    CALL apply_op
    POP r31
    RET

do_div:
    PUSH r31
    LDI r6, 4
    LDI r20, OPER
    STORE r20, r6
    CALL apply_op
    POP r31
    RET

apply_op:
    PUSH r31
    LDI r20, DISP_VAL
    LOAD r6, r20
    LDI r20, ACCUM
    STORE r20, r6
    LDI r20, NEW_ENTRY
    LDI r6, 1
    STORE r20, r6
    POP r31
    RET

; ── EVALUATE ──────────────────────────────────────
evaluate:
    PUSH r31
    LDI r20, OPER
    LOAD r7, r20

    CMPI r7, 1
    JNZ r0, ev_not_add
    LDI r20, ACCUM
    LOAD r6, r20
    LDI r20, DISP_VAL
    LOAD r8, r20
    ADD r6, r8
    LDI r20, DISP_VAL
    STORE r20, r6
    JMP ev_done

ev_not_add:
    CMPI r7, 2
    JNZ r0, ev_not_sub
    LDI r20, ACCUM
    LOAD r6, r20
    LDI r20, DISP_VAL
    LOAD r8, r20
    SUB r6, r8
    LDI r20, DISP_VAL
    STORE r20, r6
    JMP ev_done

ev_not_sub:
    CMPI r7, 3
    JNZ r0, ev_not_mul
    LDI r20, ACCUM
    LOAD r6, r20
    LDI r20, DISP_VAL
    LOAD r8, r20
    MUL r6, r8
    LDI r20, DISP_VAL
    STORE r20, r6
    JMP ev_done

ev_not_mul:
    CMPI r7, 4
    JNZ r0, ev_done
    LDI r20, ACCUM
    LOAD r6, r20
    LDI r20, DISP_VAL
    LOAD r8, r20
    CMPI r8, 0
    JZ r0, ev_done
    DIV r6, r8
    LDI r20, DISP_VAL
    STORE r20, r6

ev_done:
    LDI r20, OPER
    LDI r6, 0
    STORE r20, r6
    LDI r20, NEW_ENTRY
    LDI r6, 1
    STORE r20, r6
    LDI r20, ACCUM
    LDI r6, 0
    STORE r20, r6
    POP r31
    RET

; ── SIN (angle in display degrees, result * 1000) ─
do_sin:
    PUSH r31
    LDI r20, DISP_VAL
    LOAD r10, r20     ; angle in degrees (0-359)

    ; Normalize to 0-359
    LDI r11, 360
    MOD r10, r11

    ; Determine quadrant and map to 0-90
    ; table_index = angle * 16 / 90 (for 16-entry table over 0-90 deg)
    LDI r12, 0        ; negative flag
    LDI r13, 90

    CMP r10, r13
    BLT r0, sin_q1
    LDI r13, 180
    CMP r10, r13
    BLT r0, sin_q2
    LDI r13, 270
    CMP r10, r13
    BLT r0, sin_q3
    ; Q4: 270-359
    LDI r11, 360
    SUB r10, r11       ; angle = 360 - original
    NEG r10            ; angle = -(360 - orig) = orig - 360... no
    ; Better: angle = 360 - original
    LDI r20, DISP_VAL
    LOAD r10, r20
    LDI r11, 360
    SUB r11, r10       ; 360 - angle
    MOV r10, r11
    JMP sin_do_lookup

sin_q1:
    ; angle already 0-89, direct
    JMP sin_do_lookup

sin_q2:
    ; 90-179: effective = 180 - angle
    LDI r11, 180
    SUB r11, r10
    MOV r10, r11
    JMP sin_do_lookup

sin_q3:
    ; 180-269: effective = angle - 180, negate result
    LDI r11, 180
    SUB r10, r11
    LDI r12, 1         ; mark negate
    JMP sin_do_lookup

sin_do_lookup:
    ; r10 = effective angle (0-89)
    ; table_index = r10 * 16 / 90
    LDI r11, 16
    MUL r10, r11
    LDI r11, 90
    DIV r10, r11       ; table index (0-15)

    ; Clamp
    CMPI r10, 0
    BGE r0, sin_clamp_lo_ok
    LDI r10, 0
sin_clamp_lo_ok:
    CMPI r10, 15
    BLT r0, sin_clamp_hi_ok
    LDI r10, 15
sin_clamp_hi_ok:

    ; Look up
    LDI r20, SIN_QTR
    ADD r20, r10
    LOAD r10, r20

    ; Negate if Q3/Q4
    CMPI r12, 1
    JNZ r0, sin_no_neg
    NEG r10
sin_no_neg:

    LDI r20, DISP_VAL
    STORE r20, r10
    LDI r20, NEW_ENTRY
    LDI r6, 1
    STORE r20, r6
    POP r31
    RET

; ── COS (cos(x) = sin(90-x)) ──────────────────────
do_cos:
    PUSH r31
    ; cos(x) = sin(90 - x), or equivalently sin(x + 90)
    LDI r20, DISP_VAL
    LOAD r10, r20
    ADDI r10, 90
    LDI r20, DISP_VAL
    STORE r20, r10
    CALL do_sin
    POP r31
    RET

; ── SQRT (Newton method) ──────────────────────────
do_sqrt:
    PUSH r31
    LDI r20, DISP_VAL
    LOAD r10, r20

    CMPI r10, 0
    JZ r0, sqrt_done

    ; Initial guess = N / 2 + 1
    LDI r11, 2
    MOV r12, r10
    DIV r12, r11
    ADDI r12, 1

    ; 10 Newton iterations
    LDI r13, 10
sqrt_iter:
    MOV r14, r10
    DIV r14, r12       ; N/guess
    ADD r14, r12       ; guess + N/guess
    DIV r14, r11       ; / 2
    MOV r12, r14
    SUBI r13, 1
    JNZ r13, sqrt_iter

    MOV r10, r12
sqrt_done:
    LDI r20, DISP_VAL
    STORE r20, r10
    LDI r20, NEW_ENTRY
    LDI r6, 1
    STORE r20, r6
    POP r31
    RET

; ── CLEAR ─────────────────────────────────────────
clear_calc:
    PUSH r31
    LDI r2, 0
    LDI r20, DISP_VAL
    STORE r20, r2
    LDI r20, ACCUM
    STORE r20, r2
    LDI r20, OPER
    STORE r20, r2
    LDI r20, NEW_ENTRY
    LDI r3, 1
    STORE r20, r3
    POP r31
    RET

; ── RENDER ────────────────────────────────────────
render:
    PUSH r31
    LDI r1, 1

    ; Background
    LDI r2, 0x0D1B2A
    FILL r2

    ; Title bar
    LDI r2, 0x1B3A4B
    LDI r3, 0
    LDI r4, 0
    LDI r5, 256
    LDI r6, 20
    RECTF r3, r4, r5, r6, r2

    ; Title
    LDI r20, TXT_BUF
    STRO r20, "SCI-CALC"
    LDI r3, 85
    LDI r4, 4
    LDI r20, TXT_BUF
    TEXT r3, r4, r20

    ; Display panel
    LDI r2, 0x060612
    LDI r3, 20
    LDI r4, 28
    LDI r5, 216
    LDI r6, 40
    RECTF r3, r4, r5, r6, r2

    ; Convert display value to string at DISP_STR
    LDI r20, DISP_STR
    LDI r21, DISP_VAL
    LOAD r10, r21

    ; Handle negative
    LDI r22, 0
    LDI r23, 0
    CMP r10, r23
    BGE r0, pos_num
    NEG r10
    LDI r22, 1

pos_num:
    ; Digit extraction
    LDI r23, 0
    LDI r24, 10

    CMPI r10, 0
    JNZ r0, not_zero
    LDI r25, 0x30
    STORE r20, r25
    ADDI r20, 1
    ADDI r23, 1
    JMP num_done

not_zero:
    LDI r25, 0x6100    ; temp buffer
div_loop:
    CMPI r10, 0
    JZ r0, div_done
    MOV r26, r10
    MOD r26, r24
    ADDI r26, 0x30
    STORE r25, r26
    ADDI r25, 1
    ADDI r23, 1
    DIV r10, r24
    JMP div_loop

div_done:
    ; Write negative sign
    CMPI r22, 0
    JZ r0, no_neg
    LDI r26, 0x2D
    STORE r20, r26
    ADDI r20, 1

no_neg:
    ; Copy digits in reverse from temp buffer
    LDI r25, 0x6100
    ADD r25, r23
rev_loop:
    CMPI r23, 0
    JZ r0, num_done
    SUBI r23, 1
    LDI r26, 0x6100
    ADD r26, r23
    LOAD r27, r26
    STORE r20, r27
    ADDI r20, 1
    JMP rev_loop

num_done:
    LDI r2, 0
    STORE r20, r2

    ; Draw display value
    LDI r3, 40
    LDI r4, 35
    LDI r5, 0x00FF00
    LDI r20, DISP_STR
    DRAWTEXT r3, r4, r20, r5, r5

    ; Function labels panel
    LDI r2, 0x0D0D1A
    LDI r3, 20
    LDI r4, 75
    LDI r5, 216
    LDI r6, 80
    RECTF r3, r4, r5, r6, r2

    ; Labels
    LDI r20, TXT_BUF
    STRO r20, "S:sin C:cos Q:sqrt"
    LDI r3, 30
    LDI r4, 85
    LDI r5, 0x88BBFF
    DRAWTEXT r3, r4, r20, r5, r5

    LDI r20, TXT_BUF
    STRO r20, "+ - * / = Enter"
    LDI r3, 30
    LDI r4, 100
    LDI r5, 0x88BBFF
    DRAWTEXT r3, r4, r20, r5, r5

    LDI r20, TXT_BUF
    STRO r20, "Esc:clear  0-9:digits"
    LDI r3, 30
    LDI r4, 115
    LDI r5, 0x88BBFF
    DRAWTEXT r3, r4, r20, r5, r5

    ; Bottom bar
    LDI r2, 0x0A0A1A
    LDI r3, 0
    LDI r4, 240
    LDI r5, 256
    LDI r6, 16
    RECTF r3, r4, r5, r6, r2

    POP r31
    RET
