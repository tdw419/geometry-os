; gui_calc.asm -- GUI Calculator App for Geometry OS
;
; Proves: HITSET/HITQ mouse buttons, RECTF rendering, TEXT display,
;         ADD/SUB/MUL/DIV arithmetic, FRAME loop, number conversion.
;
; Layout (256x256 canvas):
;   Display area: x=20, y=20, w=216, h=40 (shows current number)
;   Button grid:  4 columns x 5 rows, starting at (20, 70)
;   Each button:  48x38 pixels, 4px gap
;   Row 0: C  /  *  -
;   Row 1: 7  8  9  +
;   Row 2: 4  5  6  =
;   Row 3: 1  2  3  (blank)
;   Row 4: 0 (span 2 cols)  .  (blank)
;
; RAM Layout:
;   0x400 = display value (current number shown)
;   0x404 = accumulator (stored from first operand)
;   0x408 = operator (0=none, 1=add, 2=sub, 3=mul, 4=div)
;   0x40C = state (0=entering first, 1=operator pressed, 2=entering second)
;   0x410 = new entry flag (1=next digit starts fresh)
;   0x414 = tick counter (for animation)
;   0x500 = text buffer for display (20 chars)
;   0x600 = digit conversion temp buffer
;   0x620 = digit count temp
;   0x624 = quotient temp
;   0x628 = buffer pointer temp
;
; Register allocation:
;   r1  = constant 1 (reloaded as needed)
;   r2  = scratch
;   r3  = scratch
;   r4  = scratch
;   r5  = scratch (colors, values)
;   r6  = scratch
;   r10 = mouse X from MOUSEQ
;   r11 = mouse Y from MOUSEQ
;   r12 = hit query result
;   r20 = RAM pointer
;   r21 = digit conversion scratch
;   r22 = temp
;
; Button IDs:
;   1=C, 2=/, 3=*, 4=-, 5=7, 6=8, 7=9, 8=+,
;   9=4, 10=5, 11=6, 12==, 13=1, 14=2, 15=3,
;   16=0, 17=., 99=nothing
;
; NOTE: Avoid colons in comments (assembler pitfall)
; NOTE: SLEEP takes a register, not an immediate
; NOTE: STORE takes registers, not immediates

#define DISP_VAL   0x400
#define ACCUM      0x404
#define OPER       0x408
#define STATE      0x40C
#define NEW_ENTRY  0x410
#define TICK       0x414
#define TXT_BUF    0x500
#define DIG_BUF    0x600
#define DIG_COUNT  0x620
#define DIG_QUOT   0x624
#define DIG_PTR    0x628

; ── INIT ──────────────────────────────────────────
    LDI r1, 1

; Display value = 0
    LDI r20, DISP_VAL
    LDI r2, 0
    STORE r20, r2

; Accumulator = 0
    LDI r20, ACCUM
    STORE r20, r2

; Operator = none (0)
    LDI r20, OPER
    STORE r20, r2

; State = entering first number (0)
    LDI r20, STATE
    STORE r20, r2

; New entry = no (0)
    LDI r20, NEW_ENTRY
    STORE r20, r2

; Tick = 0
    LDI r20, TICK
    STORE r20, r2

; Fill screen dark
    LDI r2, 0x1A1A2E
    FILL r2

; ── Register all button hit regions ──
; Button grid: 4 cols x 5 rows
; Each button: 48x38, gap=4, total cell=52
; Start: x=20, y=70

; Row 0: C  /  *  -
    LDI r1, 20
    LDI r2, 70
    LDI r3, 48
    LDI r4, 38
    HITSET r1, r2, r3, r4, 1

    LDI r1, 72
    HITSET r1, r2, r3, r4, 2

    LDI r1, 124
    HITSET r1, r2, r3, r4, 3

    LDI r1, 176
    HITSET r1, r2, r3, r4, 4

; Row 1: 7  8  9  +
    LDI r1, 20
    LDI r2, 112
    HITSET r1, r2, r3, r4, 5

    LDI r1, 72
    HITSET r1, r2, r3, r4, 6

    LDI r1, 124
    HITSET r1, r2, r3, r4, 7

    LDI r1, 176
    HITSET r1, r2, r3, r4, 8

; Row 2: 4  5  6  =
    LDI r1, 20
    LDI r2, 154
    HITSET r1, r2, r3, r4, 9

    LDI r1, 72
    HITSET r1, r2, r3, r4, 10

    LDI r1, 124
    HITSET r1, r2, r3, r4, 11

    LDI r1, 176
    HITSET r1, r2, r3, r4, 12

; Row 3: 1  2  3  (empty)
    LDI r1, 20
    LDI r2, 196
    HITSET r1, r2, r3, r4, 13

    LDI r1, 72
    HITSET r1, r2, r3, r4, 14

    LDI r1, 124
    HITSET r1, r2, r3, r4, 15

; Row 4: 0 (wide)  .  (empty)
; Button 0 spans 2 cols: 100x38
    LDI r1, 20
    LDI r2, 238
    LDI r3, 100
    LDI r4, 16
    HITSET r1, r2, r3, r4, 16

    LDI r1, 124
    LDI r3, 48
    LDI r4, 16
    HITSET r1, r2, r3, r4, 17

; ── MAIN LOOP ─────────────────────────────────────
main_loop:
    LDI r1, 1

; Increment tick
    LDI r20, TICK
    LOAD r2, r20
    ADD r2, r1
    STORE r20, r2

; Read mouse
    MOUSEQ r10

; Query hit regions
    HITQ r12
    JZ r12, draw_ui

; Handle button clicks
    CMPI r12, 1
    JZ r0, btn_clear

    CMPI r12, 2
    JZ r0, btn_div

    CMPI r12, 3
    JZ r0, btn_mul

    CMPI r12, 4
    JZ r0, btn_sub

    CMPI r12, 5
    JZ r0, btn_7

    CMPI r12, 6
    JZ r0, btn_8

    CMPI r12, 7
    JZ r0, btn_9

    CMPI r12, 8
    JZ r0, btn_add

    CMPI r12, 9
    JZ r0, btn_4

    CMPI r12, 10
    JZ r0, btn_5

    CMPI r12, 11
    JZ r0, btn_6

    CMPI r12, 12
    JZ r0, btn_eq

    CMPI r12, 13
    JZ r0, btn_1

    CMPI r12, 14
    JZ r0, btn_2

    CMPI r12, 15
    JZ r0, btn_3

    CMPI r12, 16
    JZ r0, btn_0

    CMPI r12, 17
    JZ r0, btn_dot

    JMP draw_ui

; ── Digit buttons ──────────────────────────────────
; Each digit: if new_entry, clear display first, then append digit
btn_0:
    LDI r1, 1
    LDI r20, NEW_ENTRY
    LOAD r2, r20
    CMPI r2, 1
    JZ r0, do_0_clear
    JMP do_0_append
do_0_clear:
    LDI r20, DISP_VAL
    LDI r2, 0
    STORE r20, r2
    LDI r20, NEW_ENTRY
    STORE r20, r2
do_0_append:
    LDI r20, DISP_VAL
    LOAD r2, r20
    LDI r3, 10
    MUL r2, r3
    STORE r20, r2
    JMP draw_ui

btn_1:
    LDI r1, 1
    CALL enter_digit_start
    LDI r2, 1
    JMP enter_digit_finish

btn_2:
    LDI r1, 1
    CALL enter_digit_start
    LDI r2, 2
    JMP enter_digit_finish

btn_3:
    LDI r1, 1
    CALL enter_digit_start
    LDI r2, 3
    JMP enter_digit_finish

btn_4:
    LDI r1, 1
    CALL enter_digit_start
    LDI r2, 4
    JMP enter_digit_finish

btn_5:
    LDI r1, 1
    CALL enter_digit_start
    LDI r2, 5
    JMP enter_digit_finish

btn_6:
    LDI r1, 1
    CALL enter_digit_start
    LDI r2, 6
    JMP enter_digit_finish

btn_7:
    LDI r1, 1
    CALL enter_digit_start
    LDI r2, 7
    JMP enter_digit_finish

btn_8:
    LDI r1, 1
    CALL enter_digit_start
    LDI r2, 8
    JMP enter_digit_finish

btn_9:
    LDI r1, 1
    CALL enter_digit_start
    LDI r2, 9
    JMP enter_digit_finish

btn_dot:
    ; For simplicity, just append a visual indicator -- skip real decimal
    ; In a full implementation we would track decimal position
    JMP draw_ui

; enter_digit_start: if new_entry, clear display value
enter_digit_start:
    LDI r1, 1
    LDI r20, NEW_ENTRY
    LOAD r2, r20
    CMPI r2, 1
    JZ r0, eds_clear
    RET
eds_clear:
    LDI r20, DISP_VAL
    LDI r2, 0
    STORE r20, r2
    LDI r20, NEW_ENTRY
    STORE r20, r2
    LDI r1, 1
    RET

; enter_digit_finish: r2=digit value, multiply display by 10 and add digit
enter_digit_finish:
    LDI r1, 1
    LDI r20, DISP_VAL
    LOAD r3, r20
    LDI r4, 10
    MUL r3, r4
    ADD r3, r2
    STORE r20, r3
    JMP draw_ui

; ── Operator buttons ───────────────────────────────
btn_add:
    LDI r1, 1
    LDI r20, OPER
    LDI r2, 1
    STORE r20, r2
    CALL save_accum_and_set_state
    JMP draw_ui

btn_sub:
    LDI r1, 1
    LDI r20, OPER
    LDI r2, 2
    STORE r20, r2
    CALL save_accum_and_set_state
    JMP draw_ui

btn_mul:
    LDI r1, 1
    LDI r20, OPER
    LDI r2, 3
    STORE r20, r2
    CALL save_accum_and_set_state
    JMP draw_ui

btn_div:
    LDI r1, 1
    LDI r20, OPER
    LDI r2, 4
    STORE r20, r2
    CALL save_accum_and_set_state
    JMP draw_ui

; save_accum_and_set_state: save display to accumulator, set state=2, new_entry=1
save_accum_and_set_state:
    LDI r1, 1
    ; If state is 2 (entering second), compute pending first
    LDI r20, STATE
    LOAD r2, r20
    CMPI r2, 2
    JNZ r0, sas_set
    ; Compute pending operation
    CALL do_compute
sas_set:
    ; Save display to accumulator
    LDI r20, DISP_VAL
    LOAD r2, r20
    LDI r20, ACCUM
    STORE r20, r2
    ; Set state = 2 (entering second number)
    LDI r20, STATE
    LDI r2, 2
    STORE r20, r2
    ; Set new_entry = 1
    LDI r20, NEW_ENTRY
    LDI r2, 1
    STORE r20, r2
    LDI r1, 1
    RET

; ── Equals button ──────────────────────────────────
btn_eq:
    LDI r1, 1
    CALL do_compute
    JMP draw_ui

; do_compute: perform pending operation
; accumulator OP display_val -> display_val
; Then set state=0, new_entry=1
do_compute:
    LDI r1, 1
    ; Check if there is an operator
    LDI r20, OPER
    LOAD r2, r20
    CMPI r2, 0
    JZ r0, dc_done
    ; Load accumulator and display
    LDI r20, ACCUM
    LOAD r3, r20
    LDI r20, DISP_VAL
    LOAD r4, r20
    ; Branch on operator
    CMPI r2, 1
    JZ r0, dc_add
    CMPI r2, 2
    JZ r0, dc_sub
    CMPI r2, 3
    JZ r0, dc_mul
    CMPI r2, 4
    JZ r0, dc_div
    JMP dc_done

dc_add:
    ADD r3, r4
    JMP dc_store

dc_sub:
    SUB r3, r4
    JMP dc_store

dc_mul:
    MUL r3, r4
    JMP dc_store

dc_div:
    CMPI r4, 0
    JZ r0, dc_done
    DIV r3, r4

dc_store:
    LDI r20, DISP_VAL
    STORE r20, r3
    ; Clear operator
    LDI r20, OPER
    LDI r2, 0
    STORE r20, r2

dc_done:
    ; Set state=0, new_entry=1
    LDI r20, STATE
    LDI r2, 0
    STORE r20, r2
    LDI r20, NEW_ENTRY
    LDI r2, 1
    STORE r20, r2
    LDI r1, 1
    RET

; ── Clear button ───────────────────────────────────
btn_clear:
    LDI r1, 1
    LDI r20, DISP_VAL
    LDI r2, 0
    STORE r20, r2
    LDI r20, ACCUM
    STORE r20, r2
    LDI r20, OPER
    STORE r20, r2
    LDI r20, STATE
    STORE r20, r2
    LDI r20, NEW_ENTRY
    STORE r20, r2
    JMP draw_ui

; ── DRAW UI ────────────────────────────────────────
draw_ui:
    LDI r1, 1

    ; Redraw background
    LDI r2, 0x1A1A2E
    FILL r2

    ; ── Display area ──
    LDI r1, 20
    LDI r2, 20
    LDI r3, 216
    LDI r4, 40
    LDI r5, 0x0F3460
    RECTF r1, r2, r3, r4, r5

    ; Convert display value to string
    LDI r20, DISP_VAL
    LOAD r5, r20
    LDI r20, TXT_BUF
    CALL num_to_str

    ; Render display text (right-aligned at x=220, y=32)
    LDI r1, 220
    LDI r2, 32
    LDI r3, TXT_BUF
    TEXT r1, r2, r3

    ; ── Draw buttons ──
    ; Row 0: C(1) /(2) *(3) -(4)
    LDI r1, 20
    LDI r2, 70
    LDI r3, 48
    LDI r4, 38
    LDI r5, 0xE74C3C
    RECTF r1, r2, r3, r4, r5
    ; "C" label
    LDI r20, TXT_BUF
    LDI r2, 67
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 40
    LDI r2, 82
    LDI r3, TXT_BUF
    TEXT r1, r2, r3

    ; / button (orange)
    LDI r1, 72
    LDI r2, 70
    LDI r5, 0xFF8800
    RECTF r1, r2, r3, r4, r5
    LDI r20, TXT_BUF
    LDI r2, 47
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 92
    LDI r2, 82
    LDI r3, TXT_BUF
    TEXT r1, r2, r3

    ; * button (orange)
    LDI r1, 124
    LDI r2, 70
    RECTF r1, r2, r3, r4, r5
    LDI r20, TXT_BUF
    LDI r2, 42
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 144
    LDI r2, 82
    LDI r3, TXT_BUF
    TEXT r1, r2, r3

    ; - button (orange)
    LDI r1, 176
    LDI r2, 70
    RECTF r1, r2, r3, r4, r5
    LDI r20, TXT_BUF
    LDI r2, 45
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 196
    LDI r2, 82
    LDI r3, TXT_BUF
    TEXT r1, r2, r3

    ; Row 1: 7(5) 8(6) 9(7) +(8)
    LDI r1, 20
    LDI r2, 112
    LDI r3, 48
    LDI r4, 38
    LDI r5, 0x2C3E50
    RECTF r1, r2, r3, r4, r5
    ; "7"
    LDI r20, TXT_BUF
    LDI r2, 55
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 40
    LDI r2, 124
    LDI r3, TXT_BUF
    TEXT r1, r2, r3

    LDI r1, 72
    LDI r2, 112
    RECTF r1, r2, r3, r4, r5
    LDI r20, TXT_BUF
    LDI r2, 56
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 92
    LDI r2, 124
    LDI r3, TXT_BUF
    TEXT r1, r2, r3

    LDI r1, 124
    LDI r2, 112
    RECTF r1, r2, r3, r4, r5
    LDI r20, TXT_BUF
    LDI r2, 57
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 144
    LDI r2, 124
    LDI r3, TXT_BUF
    TEXT r1, r2, r3

    ; + button (green)
    LDI r1, 176
    LDI r2, 112
    LDI r5, 0x27AE60
    RECTF r1, r2, r3, r4, r5
    LDI r20, TXT_BUF
    LDI r2, 43
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 196
    LDI r2, 124
    LDI r3, TXT_BUF
    TEXT r1, r2, r3

    ; Row 2: 4(9) 5(10) 6(11) =(12)
    LDI r1, 20
    LDI r2, 154
    LDI r3, 48
    LDI r4, 38
    LDI r5, 0x2C3E50
    RECTF r1, r2, r3, r4, r5
    LDI r20, TXT_BUF
    LDI r2, 52
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 40
    LDI r2, 166
    LDI r3, TXT_BUF
    TEXT r1, r2, r3

    LDI r1, 72
    LDI r2, 154
    RECTF r1, r2, r3, r4, r5
    LDI r20, TXT_BUF
    LDI r2, 53
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 92
    LDI r2, 166
    LDI r3, TXT_BUF
    TEXT r1, r2, r3

    LDI r1, 124
    LDI r2, 154
    RECTF r1, r2, r3, r4, r5
    LDI r20, TXT_BUF
    LDI r2, 54
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 144
    LDI r2, 166
    LDI r3, TXT_BUF
    TEXT r1, r2, r3

    ; = button (green)
    LDI r1, 176
    LDI r2, 154
    LDI r5, 0x27AE60
    RECTF r1, r2, r3, r4, r5
    LDI r20, TXT_BUF
    LDI r2, 61
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 196
    LDI r2, 166
    LDI r3, TXT_BUF
    TEXT r1, r2, r3

    ; Row 3: 1(13) 2(14) 3(15) (empty)
    LDI r1, 20
    LDI r2, 196
    LDI r3, 48
    LDI r4, 38
    LDI r5, 0x2C3E50
    RECTF r1, r2, r3, r4, r5
    LDI r20, TXT_BUF
    LDI r2, 49
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 40
    LDI r2, 208
    LDI r3, TXT_BUF
    TEXT r1, r2, r3

    LDI r1, 72
    LDI r2, 196
    RECTF r1, r2, r3, r4, r5
    LDI r20, TXT_BUF
    LDI r2, 50
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 92
    LDI r2, 208
    LDI r3, TXT_BUF
    TEXT r1, r2, r3

    LDI r1, 124
    LDI r2, 196
    RECTF r1, r2, r3, r4, r5
    LDI r20, TXT_BUF
    LDI r2, 51
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 144
    LDI r2, 208
    LDI r3, TXT_BUF
    TEXT r1, r2, r3

    ; Row 4: 0(16, wide) .(17) (empty)
    LDI r1, 20
    LDI r2, 238
    LDI r3, 100
    LDI r4, 16
    LDI r5, 0x2C3E50
    RECTF r1, r2, r3, r4, r5
    LDI r20, TXT_BUF
    LDI r2, 48
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 60
    LDI r2, 242
    LDI r3, TXT_BUF
    TEXT r1, r2, r3

    LDI r1, 124
    LDI r2, 238
    LDI r3, 48
    LDI r4, 16
    LDI r5, 0x2C3E50
    RECTF r1, r2, r3, r4, r5
    LDI r20, TXT_BUF
    LDI r2, 46
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 144
    LDI r2, 242
    LDI r3, TXT_BUF
    TEXT r1, r2, r3

    ; ── Title ──
    LDI r20, TXT_BUF
    LDI r2, 71
    STORE r20, r2
    LDI r2, 85
    ADD r20, r1
    STORE r20, r2
    LDI r2, 73
    ADD r20, r1
    STORE r20, r2
    LDI r2, 32
    ADD r20, r1
    STORE r20, r2
    LDI r2, 67
    ADD r20, r1
    STORE r20, r2
    LDI r2, 65
    ADD r20, r1
    STORE r20, r2
    LDI r2, 76
    ADD r20, r1
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2
    LDI r1, 80
    LDI r2, 5
    LDI r3, TXT_BUF
    TEXT r1, r2, r3

do_frame:
    FRAME
    JMP main_loop

; ── num_to_str subroutine ──────────────────────────
; Input:  r5 = number, r20 = buffer address
; Output: null-terminated string at r20, r20 advanced
; Clobbers: r0, r2, r3, r4, r21, r22
; Uses RAM: DIG_BUF(0x600), DIG_COUNT(0x620), DIG_QUOT(0x624), DIG_PTR(0x628)
num_to_str:
    LDI r1, 1
    CMPI r5, 0
    JNZ r0, nts_loop1

    ; Handle zero
    LDI r2, 48
    STORE r20, r2
    ADD r20, r1
    LDI r2, 0
    STORE r20, r2
    RET

nts_loop1:
    ; Init reversed buffer
    LDI r22, DIG_BUF
    LDI r2, DIG_PTR
    STORE r2, r22
    LDI r21, 0
    LDI r2, DIG_COUNT
    STORE r2, r21

nts_loop2:
    CMPI r5, 0
    JZ r0, nts_rev1

    ; r3 = r5 / 10
    LDI r3, 0
    ADD r3, r5
    LDI r4, 10
    DIV r3, r4
    LDI r2, DIG_QUOT
    STORE r2, r3

    ; r5 = r5 - r3*10 (remainder)
    LDI r2, 0
    ADD r2, r3
    MUL r2, r4
    SUB r5, r2
    ; Convert to ASCII
    LDI r2, 48
    ADD r5, r2

    ; Store in reversed buffer
    LDI r2, DIG_PTR
    LOAD r22, r2
    STORE r22, r5
    ADD r22, r1
    LDI r2, DIG_PTR
    STORE r2, r22

    ; Increment count
    LDI r2, DIG_COUNT
    LOAD r21, r2
    ADD r21, r1
    LDI r2, DIG_COUNT
    STORE r2, r21

    ; r5 = quotient
    LDI r2, DIG_QUOT
    LOAD r5, r2
    JMP nts_loop2

nts_rev1:
    ; Reverse digits into output buffer
    LDI r2, DIG_COUNT
    LOAD r21, r2
    LDI r22, DIG_BUF

nts_rev2:
    CMPI r21, 0
    JZ r0, nts_end
    SUB r21, r1
    ; Load digit from reversed buffer
    LDI r2, 0
    ADD r2, r22
    ADD r2, r21
    LOAD r2, r2
    ; Store to output buffer
    STORE r20, r2
    ADD r20, r1
    JMP nts_rev2

nts_end:
    LDI r2, 0
    STORE r20, r2
    LDI r1, 1
    RET
