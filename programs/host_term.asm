; host_term.asm -- Host Shell Terminal for Geometry OS (v2)
;
; Spawns bash inside a real PTY via the PTYOPEN opcode. Pipes keystrokes
; through PTYWRITE, drains PTY output through PTYREAD each frame.
;
; v2 improvements:
;   - ANSI escape stripping (ESC [ ... letter sequences silently skipped)
;   - OSC sequence stripping (ESC ] ... BEL/ST sequences skipped)
;   - Backspace/Delete support
;   - Ctrl-C sends 0x03, Ctrl-D sends 0x04
;   - Tab sends 0x09
;   - Scrolling text buffer (42x30)
;
; RAM Layout:
;   0x4000-0x44EB  Text buffer (42*30 = 1260 u32 cells, row-major)
;   0x4800         Cursor column
;   0x4801         Cursor row
;   0x4802         Blink counter
;   0x4803         PTY handle
;   0x4804         ANSI state (0=normal, 1=saw ESC, 2=in CSI, 3=in OSC)
;   0x5000         Empty cmd string (null -> default $SHELL)
;   0x5400         Send buffer (one byte per PTYWRITE)
;   0x5800-0x5FFF  Receive buffer (2048 cells)
;
; Registers:
;   r0  CMP/result
;   r1  constant 1
;   r28 PTY handle (live copy)
;   r30 stack pointer

#define COLS 42
#define ROWS 30
#define BUF 0x4000
#define BUF_END 0x44EC
#define CUR_COL 0x4800
#define CUR_ROW 0x4801
#define BLINK 0x4802
#define PTY_HANDLE 0x4803
#define ANSI_STATE 0x4804
#define CMD_BUF 0x5000
#define SEND_BUF 0x5400
#define RECV_BUF 0x5800

; ANSI states
#define ANS_NORMAL 0
#define ANS_ESC    1
#define ANS_CSI    2
#define ANS_OSC    3

; =========================================
; INIT
; =========================================
LDI r1, 1
LDI r30, 0xFD00

; Background fill
LDI r0, 0x0C0C0C
FILL r0

; Clear text buffer to spaces
LDI r20, BUF
LDI r6, 32
clear_buf_init:
    STORE r20, r6
    ADD r20, r1
    CMPI r20, BUF_END
    BLT r0, clear_buf_init

; Cursor + blink + handle + ansi_state init
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
LDI r20, ANSI_STATE
LDI r0, 0
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
STRO r20, "Host Shell v2"
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

; Empty cmd string for PTYOPEN
LDI r1, CMD_BUF
LDI r0, 0
STORE r1, r0

; PTYOPEN
LDI r5, CMD_BUF
PTYOPEN r5, r10

; Save handle
LDI r20, PTY_HANDLE
STORE r20, r10
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
    ; r0 = bytes drained (0 = nothing, 0xFFFFFFFF = closed)
    CMPI r0, 0
    JZ r0, after_drain
    LDI r8, 0xFFFFFFFF
    CMP r0, r8
    JZ r0, pty_closed

    ; Process each byte through ANSI filter -> text buffer
    LDI r9, 0
append_loop:
    CMP r9, r0
    BGE r0, after_drain
    LDI r20, RECV_BUF
    ADD r20, r9
    LOAD r5, r20
    CALL process_byte
    ADD r9, r1
    JMP append_loop

pty_closed:
    ; Show message and stop
    LDI r20, SEND_BUF
    STRO r20, "[pty closed]"
    CALL write_str_to_buf
    JMP after_drain

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

    ; Read keystroke
    IKEY r5
    JZ r5, main_loop

    ; Translate special keys
    CALL translate_key
    ; If r0 == 0 after translate, don't send
    CMPI r0, 0
    JZ r0, main_loop

    ; Send translated byte
    LDI r20, SEND_BUF
    STORE r20, r0
    LDI r6, SEND_BUF
    LDI r7, 1
    PTYWRITE r28, r6, r7
    JMP main_loop

; =========================================
; PROCESS_BYTE -- ANSI state machine + text buffer append
; r5 = byte from PTY
; Uses ANSI_STATE at 0x4804
; =========================================
process_byte:
    PUSH r31
    LDI r1, 1

    ; Load ANSI state
    LDI r20, ANSI_STATE
    LOAD r4, r20

    ; State: NORMAL
    CMPI r4, ANS_NORMAL
    JNZ r0, pb_check_esc

    ; Check for ESC (0x1B)
    CMPI r5, 27
    JNZ r0, pb_normal_byte

    ; Saw ESC -> transition to ESC state
    LDI r20, ANSI_STATE
    LDI r0, ANS_ESC
    STORE r20, r0
    JMP pb_ret

pb_normal_byte:
    ; Pass through to append_byte
    CALL append_byte
    JMP pb_ret

pb_check_esc:
    ; State: ESC (just saw 0x1B)
    CMPI r4, ANS_ESC
    JNZ r0, pb_check_csi

    ; Check for [ -> CSI
    CMPI r5, 91   ; '['
    JNZ r0, pb_esc_check_osc

    LDI r20, ANSI_STATE
    LDI r0, ANS_CSI
    STORE r20, r0
    JMP pb_ret

pb_esc_check_osc:
    ; Check for ] -> OSC
    CMPI r5, 93   ; ']'
    JNZ r0, pb_esc_other

    LDI r20, ANSI_STATE
    LDI r0, ANS_OSC
    STORE r20, r0
    JMP pb_ret

pb_esc_other:
    ; Any other char after ESC: not a recognized sequence.
    ; Some two-char ESC sequences (like ESC M, ESC 7, ESC 8) -- just skip.
    ; Reset to normal, don't display.
    LDI r20, ANSI_STATE
    LDI r0, ANS_NORMAL
    STORE r20, r0
    JMP pb_ret

pb_check_csi:
    ; State: CSI (saw ESC [)
    CMPI r4, ANS_CSI
    JNZ r0, pb_check_osc

    ; CSI sequences end with a byte in 0x40-0x7E (letter or @)
    ; Also accept digits and semicolons as intermediate params
    CMPI r5, 64    ; '@' -- first terminator
    BLT r0, pb_csi_continue

    ; Byte >= 0x40 is a terminator -> sequence done
    LDI r20, ANSI_STATE
    LDI r0, ANS_NORMAL
    STORE r20, r0
    JMP pb_ret

pb_csi_continue:
    ; Still collecting CSI params (digits, semicolons, etc)
    ; Stay in CSI state
    JMP pb_ret

pb_check_osc:
    ; State: OSC (saw ESC ])
    CMPI r4, ANS_OSC
    JNZ r0, pb_reset_state

    ; OSC ends with BEL (0x07) or ST (ESC \)
    CMPI r5, 7     ; BEL
    JZ r0, pb_osc_end
    ; Check for ESC (start of ST)
    CMPI r5, 27
    JNZ r0, pb_osc_continue
    ; This ESC might be start of ST (ESC \)
    ; Just transition to ESC state and let it resolve
    LDI r20, ANSI_STATE
    LDI r0, ANS_ESC
    STORE r20, r0
    JMP pb_ret

pb_osc_continue:
    ; Keep consuming OSC
    JMP pb_ret

pb_osc_end:
    LDI r20, ANSI_STATE
    LDI r0, ANS_NORMAL
    STORE r20, r0
    JMP pb_ret

pb_reset_state:
    ; Unknown state, reset
    LDI r20, ANSI_STATE
    LDI r0, ANS_NORMAL
    STORE r20, r0

pb_ret:
    POP r31
    RET

; =========================================
; APPEND_BYTE -- append r5 to text buffer (visible chars only)
; \n (10) -> newline; \r (13) -> col=0; printable -> insert
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

    ; buf[row*COLS + col] = r5
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
; TRANSLATE_KEY -- translate IKEY code to terminal byte in r0
; Returns 0 if key should be ignored
; r5 = raw IKEY value (preserved)
; =========================================
translate_key:
    ; Printable ASCII (32-126): pass through
    CMPI r5, 32
    BLT r0, tk_special
    CMPI r5, 127
    BGE r0, tk_special
    ; Return the char itself
    JMP tk_ret

tk_special:
    ; Enter -> \n (10)
    CMPI r5, 13
    JNZ r0, tk_bs
    LDI r0, 10
    JMP tk_ret

tk_bs:
    ; Backspace (8) -> 0x7F (DEL, what terminals actually send)
    CMPI r5, 8
    JNZ r0, tk_del
    LDI r0, 127
    JMP tk_ret

tk_del:
    ; Delete (127) -> 0x1B [ 3 ~ (just send DEL for now)
    CMPI r5, 127
    JNZ r0, tk_ctrl_c
    LDI r0, 127
    JMP tk_ret

tk_ctrl_c:
    ; Ctrl-C: IKEY sends 3 when Ctrl is held with C
    CMPI r5, 3
    JNZ r0, tk_ctrl_d
    LDI r0, 3
    JMP tk_ret

tk_ctrl_d:
    CMPI r5, 4
    JNZ r0, tk_tab
    LDI r0, 4
    JMP tk_ret

tk_tab:
    CMPI r5, 9
    JNZ r0, tk_escape
    LDI r0, 9
    JMP tk_ret

tk_escape:
    CMPI r5, 27
    JNZ r0, tk_ignore
    LDI r0, 27
    JMP tk_ret

tk_ignore:
    ; Unknown key -- don't send anything
    LDI r0, 0

tk_ret:
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
; WRITE_STR_TO_BUF -- write null-term string at [r20] to text buffer
; Used for status messages
; =========================================
write_str_to_buf:
    PUSH r31
    PUSH r20
    LDI r1, 1
wsb_loop:
    LOAD r5, r20
    JZ r5, wsb_done
    CALL append_byte
    ADD r20, r1
    JMP wsb_loop
wsb_done:
    POP r20
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
    LDI r5, 0x0C0C0C
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

    ; Use light gray text for terminal output
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

    ; Cursor blink
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
