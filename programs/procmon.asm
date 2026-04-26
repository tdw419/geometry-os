; procmon.asm -- Process Monitor for Geometry OS (Phase 140)
;
; Shows: all GeOS processes with PID, State, PC, Priority, Pages
; Interactive: W/S scrolls, K kills selected process, Q quits
; Updates every frame via NPROC + PROCINFO opcodes.
;
; RAM Layout:
;   0x6000-0x60FF  String buffers
;   0x6100-0x61FF  Number conversion buffer
;   0x6200         Selected PID (u32)
;
; Uses: NPROC, PROCINFO, DRAWTEXT, RECTF, FILL, FRAME, IKEY, KILL, STRO

#define TICKS   0xFFE
#define BUF     0x6000
#define NUMBUF  0x6100
#define SEL_PID 0x6200

; Colors
#define C_BG    0x0D1B2A
#define C_TITLE 0x1B3A5C
#define C_PANEL 0x1B2838
#define C_FG    0xFFFFFF
#define C_LABEL 0x8888CC
#define C_GREEN 0x44FF44
#define C_YELLOW 0xFFFF44
#define C_RED    0xFF4444
#define C_CYAN   0x44FFFF
#define C_GRAY   0x666688
#define C_SEL    0x334466

; Init
LDI r30, 0xFD00
LDI r1, 1

; Init selected PID
LDI r15, 0
LDI r20, SEL_PID
STORE r20, r15

; =========================================
; Main loop
; =========================================
main_loop:
    ; Clear screen
    LDI r1, C_BG
    FILL r1

    ; Title bar
    LDI r1, 0
    LDI r2, 0
    LDI r3, 256
    LDI r4, 20
    LDI r5, C_TITLE
    RECTF r1, r2, r3, r4, r5

    ; Title
    LDI r20, BUF
    STRO r20, "Process Monitor"
    LDI r1, 56
    LDI r2, 4
    LDI r3, BUF
    LDI r4, C_FG
    LDI r5, C_TITLE
    DRAWTEXT r1, r2, r3, r4, r5

    ; Column headers
    LDI r20, BUF
    STRO r20, "PID State  PC    Pri Pg"
    LDI r1, 4
    LDI r2, 22
    LDI r3, BUF
    LDI r4, C_LABEL
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    ; Separator line
    LDI r1, 4
    LDI r2, 32
    LDI r3, 248
    LDI r4, 1
    LDI r5, C_LABEL
    RECTF r1, r2, r3, r4, r5

    ; Get process count
    NPROC
    MOV r10, r0          ; r10 = total count

    ; Draw process rows (up to 10 visible)
    LDI r11, 0           ; current PID
row_loop:
    CMP r11, r10
    BGE r0, rows_done

    ; Compute screen Y: 36 + PID * 18
    LDI r15, 18
    MOV r26, r11
    MUL r26, r15
    ADD r26, r15         ; r15 still 18, so add 36 total
    ; Actually: 36 + pid*18
    LDI r15, 36
    ADD r26, r15

    ; Check if off screen
    LDI r15, 230
    CMP r26, r15
    BGE r0, rows_done

    ; Check if selected
    LDI r20, SEL_PID
    LOAD r25, r20
    CMP r11, r25
    JNZ r0, not_sel

    ; Highlight row
    LDI r1, 4
    MOV r2, r26
    LDI r3, 248
    LDI r4, 16
    LDI r5, C_SEL
    RECTF r1, r2, r3, r4, r5
    JMP draw_pid

not_sel:
    ; Normal row
    LDI r1, 4
    MOV r2, r26
    LDI r3, 248
    LDI r4, 16
    LDI r5, 0x141428
    RECTF r1, r2, r3, r4, r5

draw_pid:
    ; Draw PID number
    MOV r14, r11
    LDI r20, BUF
    CALL int_to_str
    LDI r1, 8
    LDI r15, 4
    MOV r2, r26
    ADD r2, r15
    LDI r3, BUF
    LDI r4, C_CYAN
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    ; Get state (field 0)
    MOV r1, r11
    LDI r2, 0
    PROCINFO r1, r2
    MOV r14, r0

    ; Draw state name
    LDI r20, BUF
    LDI r15, 0
    CMP r14, r15
    JNZ r0, chk_s1
    STRO r20, "Ready"
    LDI r15, C_GREEN
    JMP draw_st
chk_s1:
    LDI r15, 1
    CMP r14, r15
    JNZ r0, chk_s2
    STRO r20, "Run"
    LDI r15, C_GREEN
    JMP draw_st
chk_s2:
    LDI r15, 2
    CMP r14, r15
    JNZ r0, chk_s3
    STRO r20, "Sleep"
    LDI r15, C_YELLOW
    JMP draw_st
chk_s3:
    LDI r15, 3
    CMP r14, r15
    JNZ r0, chk_s4
    STRO r20, "Block"
    LDI r15, C_YELLOW
    JMP draw_st
chk_s4:
    LDI r15, 4
    CMP r14, r15
    JNZ r0, chk_s5
    STRO r20, "Zombie"
    LDI r15, C_RED
    JMP draw_st
chk_s5:
    STRO r20, "???"
    LDI r15, C_GRAY

draw_st:
    LDI r1, 38
    LDI r16, 4
    MOV r2, r26
    ADD r2, r16
    LDI r3, BUF
    MOV r4, r15
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    ; Get PC (field 1)
    MOV r1, r11
    LDI r2, 1
    PROCINFO r1, r2
    MOV r14, r0

    ; Draw PC as decimal
    LDI r20, BUF
    CALL int_to_str
    LDI r1, 88
    LDI r16, 4
    MOV r2, r26
    ADD r2, r16
    LDI r3, BUF
    LDI r4, C_FG
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    ; Get priority (field 2)
    MOV r1, r11
    LDI r2, 2
    PROCINFO r1, r2
    MOV r14, r0
    LDI r20, BUF
    CALL int_to_str
    LDI r1, 150
    LDI r16, 4
    MOV r2, r26
    ADD r2, r16
    LDI r3, BUF
    LDI r4, C_FG
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    ; Get pages (field 4)
    MOV r1, r11
    LDI r2, 4
    PROCINFO r1, r2
    MOV r14, r0
    LDI r20, BUF
    CALL int_to_str
    LDI r1, 180
    LDI r16, 4
    MOV r2, r26
    ADD r2, r16
    LDI r3, BUF
    LDI r4, C_FG
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    ; Next PID
    LDI r15, 1
    ADD r11, r15
    JMP row_loop

rows_done:
    ; Footer bar
    LDI r1, 0
    LDI r2, 236
    LDI r3, 256
    LDI r4, 20
    LDI r5, C_TITLE
    RECTF r1, r2, r3, r4, r5

    ; Footer help
    LDI r20, BUF
    STRO r20, "W/S:Scroll K:Kill Q:Quit"
    LDI r1, 16
    LDI r2, 242
    LDI r3, BUF
    LDI r4, C_GRAY
    LDI r5, C_TITLE
    DRAWTEXT r1, r2, r3, r4, r5

    ; Process count
    MOV r14, r10
    LDI r20, BUF
    STRO r20, "Total:"
    LDI r1, 200
    LDI r2, 242
    LDI r3, BUF
    LDI r4, C_GRAY
    LDI r5, C_TITLE
    DRAWTEXT r1, r2, r3, r4, r5

    MOV r14, r10
    LDI r20, NUMBUF
    CALL int_to_str
    LDI r1, 235
    LDI r2, 242
    LDI r3, NUMBUF
    LDI r4, C_CYAN
    LDI r5, C_TITLE
    DRAWTEXT r1, r2, r3, r4, r5

    ; Handle input
    IKEY r15
    JZ r15, no_input

    ; W = 87 (scroll up / prev)
    LDI r16, 87
    CMP r15, r16
    JNZ r0, chk_s_down
    LDI r20, SEL_PID
    LOAD r16, r20
    JZ r16, no_input
    LDI r17, 1
    SUB r16, r17
    STORE r20, r16
    JMP no_input

chk_s_down:
    ; S = 83 (scroll down / next)
    LDI r16, 83
    CMP r15, r16
    JNZ r0, chk_kill
    LDI r20, SEL_PID
    LOAD r16, r20
    LDI r17, 1
    ADD r16, r17
    CMP r16, r10
    BGE r0, no_input
    STORE r20, r16
    JMP no_input

chk_kill:
    ; K = 75 (kill)
    LDI r16, 75
    CMP r15, r16
    JNZ r0, chk_quit
    LDI r20, SEL_PID
    LOAD r16, r20
    JZ r16, no_input
    MOV r1, r16
    KILL r1
    JMP no_input

chk_quit:
    ; Q = 81 (quit)
    LDI r16, 81
    CMP r15, r16
    JNZ r0, no_input
    HALT

no_input:
    FRAME
    JMP main_loop


; =========================================
; int_to_str subroutine
; Converts r14 (u32) to decimal string at r20
; Returns null-terminated. r0 = buffer start.
; Clobbers r14-r19.
; =========================================
int_to_str:
    PUSH r31
    PUSH r20
    LDI r16, 0

    JZ r14, its_zero

its_loop:
    MOV r18, r14
    LDI r17, 10
    MOD r18, r17
    LDI r19, 48
    ADD r18, r19
    PUSH r18
    LDI r17, 10
    DIV r14, r17
    LDI r19, 1
    ADD r16, r19
    JNZ r14, its_loop

its_write:
    POP r18
    STORE r20, r18
    LDI r19, 1
    ADD r20, r19
    LDI r19, 1
    SUB r16, r19
    JNZ r16, its_write

    LDI r18, 0
    STORE r20, r18
    POP r0
    POP r31
    RET

its_zero:
    LDI r18, 48
    STORE r20, r18
    LDI r19, 1
    ADD r20, r19
    LDI r18, 0
    STORE r20, r18
    POP r0
    POP r31
    RET
