; host_term.asm -- Host Shell Terminal for Geometry OS
;
; Spawns bash inside a real PTY via the PTYOPEN opcode, pipes keystrokes
; through PTYWRITE, and drains pty output through PTYREAD each frame into
; the on-screen text buffer.
;
; Persistent state: cd, env vars, shell history all behave like a normal
; shell session because there's a real bash process running on the host.
;
; Limitations (Phase A): no ANSI parsing -- escape sequences are filtered
; out instead of interpreted. Colors, cursor moves, and screen clears
; from the host will look like missing characters. Plain output works.
;
; RAM Layout:
;   0x4000-0x44EB  Text buffer (42*30 = 1260 u32 cells, row-major)
;   0x4800         Cursor column
;   0x4801         Cursor row
;   0x4802         Blink counter
;   0x4803         PTY handle
;   0x5000         Empty cmd string (single null word -> default $SHELL)
;   0x5400         Send buffer (one byte per PTYWRITE)
;   0x5800-0x5FFF  Receive buffer (2048 cells)
;
; Registers reserved by convention:
;   r0  CMP/result
;   r1  constant 1 (reload after any op that may clobber)
;   r30 stack pointer
;
; Opcode map used here:
;   PTYOPEN  cmd_addr_reg, handle_reg
;   PTYWRITE handle_reg, buf_reg, len_reg
;   PTYREAD  handle_reg, buf_reg, max_len_reg   ; r0 = bytes drained
;   PTYCLOSE handle_reg

#define COLS 42
#define ROWS 30
#define BUF 0x4000
#define BUF_END 0x44EC
#define CUR_COL 0x4800
#define CUR_ROW 0x4801
#define BLINK 0x4802
#define PTY_HANDLE 0x4803
#define CMD_BUF 0x5000
#define SEND_BUF 0x5400
#define RECV_BUF 0x5800

; =========================================
; INIT
; =========================================
LDI r1, 1
LDI r30, 0xFD00

; Background fill
LDI r0, 0x080812
FILL r0

; Clear text buffer to spaces
LDI r20, BUF
LDI r6, 32
clear_buf_init:
    STORE r20, r6
    ADD r20, r1
    CMPI r20, BUF_END
    BLT r0, clear_buf_init

; Cursor + blink + handle init
LDI r20, CUR_COL
LDI r0, 0
STORE r20, r0
LDI r20, CUR_ROW
STORE r20, r0
LDI r20, BLINK
STORE r20, r0
LDI r20, PTY_HANDLE
LDI r0, 0xFFFF
STORE r20, r0

; Title bar
LDI r1, 0
LDI r2, 0
LDI r3, 256
LDI r4, 16
LDI r5, 0x002211
RECTF r1, r2, r3, r4, r5

; Title text
LDI r20, SEND_BUF
STRO r20, "Host Shell"
LDI r1, 4
LDI r2, 4
LDI r3, SEND_BUF
LDI r4, 0x44FF44
LDI r5, 0x002211
DRAWTEXT r1, r2, r3, r4, r5

; Close button hit region
LDI r1, 220
LDI r2, 0
LDI r3, 36
LDI r4, 16
HITSET r1, r2, r3, r4, 99

; Empty cmd string (PTYOPEN reads null-terminated string; null at addr -> default shell)
LDI r1, CMD_BUF
LDI r0, 0
STORE r1, r0

; PTYOPEN cmd_addr=CMD_BUF, handle_reg=r10
LDI r5, CMD_BUF
PTYOPEN r5, r10

; Save handle (also keep in r10 for the loop)
LDI r20, PTY_HANDLE
STORE r20, r10

; Stash a working copy of the handle in r28 so the main loop can pass it
; to PTYREAD/PTYWRITE without reloading every iteration.
LDI r28, 0
ADD r28, r10

LDI r1, 1

; =========================================
; MAIN LOOP
; =========================================
main_loop:
    LDI r1, 1

    ; Drain pty into text buffer
    LDI r6, RECV_BUF
    LDI r7, 256
    PTYREAD r28, r6, r7
    ; r0 = bytes drained (or 0xFFFFFFFF on close)
    CMPI r0, 0
    JZ r0, after_drain
    ; Negative-as-unsigned (closed): skip the byte loop
    LDI r8, 0xFFFFFFFF
    CMP r0, r8
    JZ r0, after_drain

    ; Append r0 bytes from RECV_BUF to text buffer
    LDI r9, 0           ; index
append_loop:
    CMP r9, r0
    BGE r0, after_drain
    LDI r20, RECV_BUF
    ADD r20, r9
    LOAD r5, r20
    CALL append_byte
    ADD r9, r1
    JMP append_loop

after_drain:
    LDI r1, 1

    ; Blink counter
    LDI r20, BLINK
    LOAD r0, r20
    ADD r0, r1
    STORE r20, r0

    ; Render
    CALL render
    FRAME

    ; Read keystroke; pipe to pty if any
    IKEY r5
    JZ r5, main_loop
    LDI r20, SEND_BUF
    STORE r20, r5
    LDI r6, SEND_BUF
    LDI r7, 1
    PTYWRITE r28, r6, r7
    JMP main_loop

; =========================================
; APPEND_BYTE  -- append r5 to the text buffer.
; \n (10) -> newline; \r (13) -> col=0; printable -> insert; else skip.
; =========================================
append_byte:
    PUSH r31
    LDI r1, 1

    ; Newline
    CMPI r5, 10
    JNZ r0, ab_check_cr
    CALL do_newline
    JMP ab_ret

ab_check_cr:
    CMPI r5, 13
    JNZ r0, ab_check_print
    LDI r20, CUR_COL
    LDI r0, 0
    STORE r20, r0
    JMP ab_ret

ab_check_print:
    ; printable range 32..126 inclusive
    CMPI r5, 32
    BLT r0, ab_ret
    CMPI r5, 127
    BGE r0, ab_ret

    ; Compute buf[row*COLS + col] = r5
    LDI r20, CUR_ROW
    LOAD r2, r20
    LDI r3, COLS
    MUL r2, r3
    LDI r20, CUR_COL
    LOAD r0, r20
    ADD r2, r0
    LDI r20, BUF
    ADD r20, r2
    STORE r20, r5

    ; col++
    LDI r20, CUR_COL
    LOAD r0, r20
    ADD r0, r1
    STORE r20, r0
    CMPI r0, COLS
    JNZ r0, ab_ret
    CALL do_newline

ab_ret:
    POP r31
    RET

; =========================================
; DO_NEWLINE -- col=0, row++ or scroll
; =========================================
do_newline:
    PUSH r31
    LDI r1, 1
    LDI r20, CUR_COL
    LDI r0, 0
    STORE r20, r0
    LDI r20, CUR_ROW
    LOAD r6, r20
    ADD r6, r1
    CMPI r6, ROWS
    BLT r0, dn_store
    CALL scroll_up
    LDI r20, CUR_ROW
    LDI r6, 29
dn_store:
    STORE r20, r6
    POP r31
    RET

; =========================================
; SCROLL_UP -- shift rows 1..29 up to 0..28, clear row 29
; =========================================
scroll_up:
    PUSH r31
    LDI r1, 1
    LDI r10, 0
scroll_loop:
    CMPI r10, 29
    BGE r0, scroll_clear

    LDI r20, BUF
    LDI r0, 0
    ADD r0, r10
    ADD r0, r1
    LDI r11, COLS
    MUL r0, r11
    ADD r20, r0          ; src = BUF + (row+1)*COLS

    LDI r21, BUF
    LDI r0, 0
    ADD r0, r10
    LDI r11, COLS
    MUL r0, r11
    ADD r21, r0          ; dst = BUF + row*COLS

    LDI r22, 0
scroll_copy:
    LOAD r0, r20
    STORE r21, r0
    ADD r20, r1
    ADD r21, r1
    ADD r22, r1
    CMPI r22, COLS
    BLT r22, scroll_copy

    ADD r10, r1
    JMP scroll_loop

scroll_clear:
    LDI r20, BUF
    LDI r6, 29
    LDI r11, COLS
    MUL r6, r11
    ADD r20, r6
    LDI r6, 32
    LDI r22, 0
sc_loop:
    STORE r20, r6
    ADD r20, r1
    ADD r22, r1
    CMPI r22, COLS
    BLT r0, sc_loop
    POP r31
    RET

; =========================================
; RENDER -- redraw text buffer + cursor
; =========================================
render:
    PUSH r31
    LDI r1, 1

    ; Clear content area
    LDI r1, 0
    LDI r2, 16
    LDI r3, 256
    LDI r4, 240
    LDI r5, 0x080812
    RECTF r1, r2, r3, r4, r5

    LDI r1, 1
    LDI r8, 8
    LDI r9, 6
    LDI r10, 0
    LDI r11, BUF
    LDI r12, 16
render_row:
    LDI r16, 0x6000
    LDI r17, 0
copy_col:
    LOAD r6, r11
    STORE r16, r6
    ADD r11, r1
    ADD r16, r1
    ADD r17, r1
    CMPI r17, COLS
    BLT r17, copy_col
    LDI r0, 0
    STORE r16, r0

    LDI r1, 0
    LDI r13, 0x6000
    LDI r14, 0xCCCCCC
    LDI r15, 0
    DRAWTEXT r1, r12, r13, r14, r15

    LDI r1, 1
    ADD r12, r8
    ADD r10, r1
    CMPI r10, ROWS
    BLT r10, render_row

    ; Cursor (blink at half-rate)
    LDI r20, BLINK
    LOAD r0, r20
    LDI r7, 8
    AND r0, r7
    CMPI r0, 4
    BLT r0, draw_cursor
    JMP cursor_done
draw_cursor:
    LDI r20, CUR_COL
    LOAD r0, r20
    MUL r0, r9
    LDI r20, CUR_ROW
    LOAD r2, r20
    MUL r2, r8
    LDI r3, 16
    ADD r2, r3
    LDI r3, 6
    LDI r4, 8
    LDI r5, 0x44FF44
    RECTF r0, r2, r3, r4, r5
cursor_done:
    POP r31
    RET
