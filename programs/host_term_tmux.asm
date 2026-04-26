; host_term_tmux.asm -- Multi-tab Terminal (tmux-like) for Geometry OS
;
; Spawns bash inside real PTYs via PTYOPEN. Up to 4 tabs, each with its own
; bash process. Ctrl+1-4 or Alt+1-4 switches tabs. Ctrl+Shift+T opens new
; tab. Ctrl+Shift+W closes current tab. Background tabs buffer output.
;
; RAM Layout:
;   0x4000-0x47FF  RECV_BUF (2048 cells, shared)
;   0x4800-0x48FF  SEND_BUF (256 cells, shared)
;   0x4900-0x49FF  SCRATCH (256 cells, shared)
;   0x4A00         CMD_BUF (empty string for PTYOPEN)
;   0x4B00         ACTIVE_TAB (0-3)
;   0x4C00+tab*32  Tab metadata (16 words per tab):
;       +0: PTY handle (0xFFFF = unused)
;       +1: CUR_COL
;       +2: CUR_ROW
;       +3: ANSI_STATE
;       +4: CONNECTED
;       +5: BLINK
;   0x9000+tab*0x600  Tab text buffers (42*26 = 1092 cells each)
;
; Registers:
;   r0  CMP/result
;   r1  constant 1
;   r28 current tab index (live copy)
;   r30 stack pointer

#define COLS 42
#define ROWS 26
#define TAB_BUF_CELLS 1092

#define RECV_BUF 0x4000
#define SEND_BUF 0x4800
#define SCRATCH  0x4900
#define CMD_BUF  0x4A00
#define ACTIVE_TAB 0x4B00
#define TAB_META 0x4C00

; Tab buffer base addresses (each 0x600 apart)
#define TAB0_BUF 0x9000
#define TAB1_BUF 0x9600
#define TAB2_BUF 0x9C00
#define TAB3_BUF 0xA200

; Tab metadata offsets
#define T_PTY   0
#define T_COL   1
#define T_ROW   2
#define T_ANSI  3
#define T_CONN  4
#define T_BLINK 5

; ANSI states
#define ANS_NORMAL 0
#define ANS_ESC    1
#define ANS_CSI    2
#define ANS_OSC    3

; Extended key codes
#define KEY_UP    0x80
#define KEY_DOWN  0x81
#define KEY_LEFT  0x82
#define KEY_RIGHT 0x83
#define KEY_HOME  0x84
#define KEY_END   0x85

; Tab switch codes (Ctrl+1-4)
#define CTRL_1 0x92
#define CTRL_2 0x93
#define CTRL_3 0x94
#define CTRL_4 0x95
; Tab switch codes (Alt+1-4)
#define ALT_1 0xA0
#define ALT_2 0xA1
#define ALT_3 0xA2
#define ALT_4 0xA3
; Ctrl+Shift+T/W
#define CS_T 0x90
#define CS_W 0x91

; =========================================
; INIT
; =========================================
LDI r1, 1
LDI r30, 0xFD00

; Dark background
LDI r0, 0x0A0A0A
FILL r0

; Initialize all 4 tab metadata: PTY=0xFFFF (unused), rest=0
LDI r10, 0
init_tabs_loop:
    ; PTY handle = 0xFFFF (unused)
    LDI r20, TAB_META
    MOV r21, r10
    LDI r7, 32
    MUL r21, r7
    ADD r20, r21
    LDI r0, 0xFFFF
    STORE r20, r0

    ; CUR_COL = 0
    MOV r22, r20
    ADD r22, r1
    LDI r0, 0
    STORE r22, r0

    ; CUR_ROW = 0
    MOV r22, r20
    LDI r7, 2
    ADD r22, r7
    LDI r0, 0
    STORE r22, r0

    ; ANSI_STATE = 0
    MOV r22, r20
    LDI r7, 3
    ADD r22, r7
    LDI r0, 0
    STORE r22, r0

    ; CONNECTED = 0
    MOV r22, r20
    LDI r7, 4
    ADD r22, r7
    LDI r0, 0
    STORE r22, r0

    ; BLINK = 0
    MOV r22, r20
    LDI r7, 5
    ADD r22, r7
    LDI r0, 0
    STORE r22, r0

    ADD r10, r1
    CMPI r10, 4
    BLT r0, init_tabs_loop

; Clear all 4 tab text buffers to spaces
LDI r10, 0
clear_tabs_buf:
    ; Get buffer base for tab r10
    CALL get_tab_buf_base
    ; r20 = buffer base
    LDI r6, 32
    LDI r15, TAB_BUF_CELLS
    MOV r21, r20
    LDI r7, TAB_BUF_CELLS
    ADD r21, r7
ctb_loop:
    STORE r20, r6
    ADD r20, r1
    CMP r20, r21
    BLT r0, ctb_loop

    ADD r10, r1
    CMPI r10, 4
    BLT r0, clear_tabs_buf

; Empty cmd string for PTYOPEN
LDI r20, CMD_BUF
LDI r0, 0
STORE r20, r0

; Set active tab = 0
LDI r20, ACTIVE_TAB
LDI r0, 0
STORE r20, r0

; =========================================
; SPAWN INITIAL TAB (tab 0)
; =========================================
LDI r10, 0
CALL spawn_tab

; =========================================
; STARTUP DRAIN for tab 0
; =========================================
LDI r20, ACTIVE_TAB
LDI r0, 0
STORE r20, r0
LDI r28, 0

LDI r22, 0          ; frame counter
LDI r23, 40         ; max startup frames

startup_drain:
    LDI r10, 0
    CALL drain_tab
    CALL render_active_tab
    CALL draw_tab_bar
    FRAME
    ADD r22, r1
    CMPI r22, 40
    BLT r0, startup_drain

; =========================================
; MAIN LOOP
; =========================================
main_loop:
    LDI r1, 1

    ; Drain ALL tabs (background buffering)
    LDI r10, 0
drain_all_loop:
    CALL drain_tab
    ADD r10, r1
    CMPI r10, 4
    BLT r0, drain_all_loop

    ; Load active tab
    LDI r20, ACTIVE_TAB
    LOAD r28, r20

    ; Render active tab
    CALL render_active_tab

    ; Draw tab bar
    CALL draw_tab_bar

    ; Frame
    FRAME

    ; Read keystroke
    IKEY r5
    JZ r5, main_loop

    ; Check tab switch keys
    CALL check_tab_switch
    ; r0 = 1 if tab was switched (consumed the key)
    CMPI r0, 1
    JZ r0, main_loop

    ; Check Ctrl+Shift+T (new tab)
    CMPI r5, CS_T
    JNZ r0, check_close
    CALL open_new_tab
    JMP main_loop

check_close:
    ; Check Ctrl+Shift+W (close tab)
    CMPI r5, CS_W
    JNZ r0, send_to_pty
    CALL close_active_tab
    JMP main_loop

send_to_pty:
    ; Send key to active tab's PTY
    CALL translate_key
    CMPI r0, 0
    JZ r0, main_loop

    ; Get active tab's PTY handle
    LDI r20, ACTIVE_TAB
    LOAD r28, r20
    CALL get_tab_pty
    ; r20 = PTY handle
    ; Check if handle is valid
    LDI r7, 0xFFFF
    CMP r20, r7
    JZ r0, main_loop

    ; PTYWRITE handle, send_buf, len
    MOV r6, r20       ; handle
    LDI r7, SEND_BUF
    ; r0 = byte count from translate_key
    PTYWRITE r6, r7, r0
    JMP main_loop

; =========================================
; GET_TAB_BUF_BASE -- return buffer address for tab r10
; Returns: r20 = buffer base address
; Clobbers: r7
; =========================================
get_tab_buf_base:
    CMPI r10, 0
    JNZ r0, gtbb_t1
    LDI r20, TAB0_BUF
    RET
gtbb_t1:
    CMPI r10, 1
    JNZ r0, gtbb_t2
    LDI r20, TAB1_BUF
    RET
gtbb_t2:
    CMPI r10, 2
    JNZ r0, gtbb_t3
    LDI r20, TAB2_BUF
    RET
gtbb_t3:
    LDI r20, TAB3_BUF
    RET

; =========================================
; GET_TAB_META -- return metadata base for tab r10
; Returns: r20 = metadata base address
; Clobbers: r7
; =========================================
get_tab_meta:
    LDI r20, TAB_META
    MOV r7, r10
    LDI r21, 32
    MUL r7, r21
    ADD r20, r7
    RET

; =========================================
; GET_TAB_PTY -- get PTY handle for tab r10
; Returns: r20 = PTY handle (0xFFFF if unused)
; =========================================
get_tab_pty:
    CALL get_tab_meta
    LOAD r20, r20
    RET

; =========================================
; SPAWN_TAB -- spawn bash in PTY for tab r10
; Uses: CMD_BUF, r5, r10
; =========================================
spawn_tab:
    PUSH r31
    LDI r1, 1

    ; PTYOPEN cmd_addr, handle_reg
    LDI r5, CMD_BUF
    LDI r7, 10
    PTYOPEN r5, r7

    ; Check success
    CMPI r0, 0
    JNZ r0, st_fail

    ; Store handle in tab metadata
    CALL get_tab_meta
    ; r20 = meta base
    STORE r20, r7    ; store handle (r7 from PTYOPEN result)

    ; Mark connected
    MOV r22, r20
    LDI r7, 4
    ADD r22, r7
    LDI r0, 1
    STORE r22, r0

    JMP st_done
st_fail:
    ; PTY open failed, leave handle as 0xFFFF
st_done:
    POP r31
    RET

; =========================================
; DRAIN_TAB -- read PTY output for tab r10, process through ANSI
; =========================================
drain_tab:
    PUSH r31
    PUSH r10
    LDI r1, 1

    ; Get PTY handle
    CALL get_tab_pty
    ; r20 = PTY handle

    ; Check if slot is in use
    LDI r7, 0xFFFF
    CMP r20, r7
    JZ r0, dt_skip

    ; PTYREAD handle, recv_buf, max_len
    MOV r6, r20
    LDI r7, RECV_BUF
    LDI r8, 512
    PTYREAD r6, r7, r8
    MOV r8, r0

    ; Check for closed
    LDI r7, 0xFFFFFFFF
    CMP r8, r7
    JNZ r0, dt_process

    ; Mark disconnected
    CALL get_tab_meta
    MOV r22, r20
    LDI r7, 4
    ADD r22, r7
    LDI r0, 0
    STORE r22, r0
    JMP dt_skip

dt_process:
    ; Process each byte through ANSI state machine for this tab
    CMPI r8, 0
    JZ r0, dt_skip

    LDI r9, 0
dt_byte_loop:
    CMP r9, r8
    BGE r0, dt_skip

    LDI r20, RECV_BUF
    ADD r20, r9
    LOAD r5, r20
    CALL process_byte_tab
    ADD r9, r1
    JMP dt_byte_loop

dt_skip:
    POP r10
    POP r31
    RET

; =========================================
; PROCESS_BYTE_TAB -- process byte r5 for tab r10
; Uses tab-specific ANSI state and cursor
; =========================================
process_byte_tab:
    PUSH r31
    LDI r1, 1

    ; Load ANSI state for this tab
    CALL get_tab_meta
    MOV r22, r20
    LDI r7, 3
    ADD r22, r7
    LOAD r4, r22

    ; State: NORMAL
    CMPI r4, ANS_NORMAL
    JNZ r0, pbt_check_esc

    CMPI r5, 27
    JNZ r0, pbt_normal_byte

    ; Saw ESC
    CALL get_tab_meta
    MOV r22, r20
    LDI r7, 3
    ADD r22, r7
    LDI r0, ANS_ESC
    STORE r22, r0
    JMP pbt_ret

pbt_normal_byte:
    CALL append_byte_tab
    JMP pbt_ret

pbt_check_esc:
    CMPI r4, ANS_ESC
    JNZ r0, pbt_check_csi

    CMPI r5, 91
    JNZ r0, pbt_esc_other

    CALL get_tab_meta
    MOV r22, r20
    LDI r7, 3
    ADD r22, r7
    LDI r0, ANS_CSI
    STORE r22, r0
    JMP pbt_ret

pbt_esc_other:
    CALL get_tab_meta
    MOV r22, r20
    LDI r7, 3
    ADD r22, r7
    LDI r0, ANS_NORMAL
    STORE r22, r0
    JMP pbt_ret

pbt_check_csi:
    CMPI r4, ANS_CSI
    JNZ r0, pbt_reset

    ; CSI terminator
    CMPI r5, 64
    BLT r0, pbt_ret

    CALL get_tab_meta
    MOV r22, r20
    LDI r7, 3
    ADD r22, r7
    LDI r0, ANS_NORMAL
    STORE r22, r0
    JMP pbt_ret

pbt_reset:
    CALL get_tab_meta
    MOV r22, r20
    LDI r7, 3
    ADD r22, r7
    LDI r0, ANS_NORMAL
    STORE r22, r0

pbt_ret:
    POP r31
    RET

; =========================================
; APPEND_BYTE_TAB -- append r5 to tab r10's text buffer
; =========================================
append_byte_tab:
    PUSH r31
    LDI r1, 1

    ; Newline
    CMPI r5, 10
    JNZ r0, abt_check_cr
    CALL do_newline_tab
    JMP abt_ret

abt_check_cr:
    CMPI r5, 13
    JNZ r0, abt_check_print
    ; col = 0
    CALL get_tab_meta
    MOV r22, r20
    LDI r7, 1
    ADD r22, r7
    LDI r0, 0
    STORE r22, r0
    JMP abt_ret

abt_check_print:
    CMPI r5, 32
    BLT r0, abt_ret
    CMPI r5, 127
    BGE r0, abt_ret

    ; buf[row*COLS + col] = r5
    CALL get_tab_meta
    ; r20 = meta base
    MOV r21, r20
    LDI r7, 2
    ADD r21, r7
    LOAD r2, r21     ; r2 = CUR_ROW
    LDI r3, COLS
    MUL r2, r3
    MOV r21, r20
    LDI r7, 1
    ADD r21, r7
    LOAD r0, r21     ; r0 = CUR_COL
    ADD r2, r0

    CALL get_tab_buf_base
    ADD r20, r2
    STORE r20, r5

    ; col++
    CALL get_tab_meta
    MOV r22, r20
    LDI r7, 1
    ADD r22, r7
    LOAD r0, r22
    ADD r0, r1
    STORE r22, r0
    CMPI r0, COLS
    JNZ r0, abt_ret
    CALL do_newline_tab

abt_ret:
    POP r31
    RET

; =========================================
; DO_NEWLINE_TAB -- newline for tab r10
; =========================================
do_newline_tab:
    PUSH r31
    LDI r1, 1

    ; col = 0
    CALL get_tab_meta
    MOV r22, r20
    LDI r7, 1
    ADD r22, r7
    LDI r0, 0
    STORE r22, r0

    ; row++
    MOV r22, r20
    LDI r7, 2
    ADD r22, r7
    LOAD r6, r22
    ADD r6, r1
    CMPI r6, ROWS
    BLT r0, dnt_store
    CALL scroll_up_tab
    LDI r6, 25       ; ROWS-1
dnt_store:
    ; Re-get meta (scroll_up_tab may have changed r20)
    CALL get_tab_meta
    MOV r22, r20
    LDI r7, 2
    ADD r22, r7
    STORE r22, r6

    POP r31
    RET

; =========================================
; SCROLL_UP_TAB -- scroll text buffer for tab r10
; =========================================
scroll_up_tab:
    PUSH r31
    LDI r1, 1

    ; Shift rows 1..(ROWS-1) up to 0..(ROWS-2), clear last row
    PUSH r10
    LDI r12, 0
sut_loop:
    CMPI r12, 25     ; ROWS-1
    BGE r0, sut_clear

    ; src = BUF + (row+1)*COLS
    CALL get_tab_buf_base
    MOV r21, r12
    ADD r21, r1
    LDI r7, COLS
    MUL r21, r7
    ADD r20, r21     ; r20 = src

    ; dst = BUF + row*COLS
    CALL get_tab_buf_base
    MOV r22, r12
    LDI r7, COLS
    MUL r22, r7
    ADD r21, r22     ; r21 = dst

    ; Copy COLS cells
    LDI r13, 0
sut_copy:
    LOAD r0, r20
    STORE r21, r0
    ADD r20, r1
    ADD r21, r1
    ADD r13, r1
    CMPI r13, COLS
    BLT r0, sut_copy

    ADD r12, r1
    JMP sut_loop

sut_clear:
    ; Clear last row
    CALL get_tab_buf_base
    LDI r7, 25
    LDI r22, COLS
    MUL r7, r22
    ADD r20, r7
    LDI r6, 32
    LDI r14, 0
scut_loop:
    STORE r20, r6
    ADD r20, r1
    ADD r14, r1
    CMPI r14, COLS
    BLT r0, scut_loop

    POP r10
    POP r31
    RET

; =========================================
; CHECK_TAB_SWITCH -- check if r5 is a tab switch key
; Returns: r0 = 1 if handled, 0 if not
; =========================================
check_tab_switch:
    PUSH r31

    ; Ctrl+1 or Alt+1 -> tab 0
    CMPI r5, CTRL_1
    JZ r0, cts_tab0
    CMPI r5, ALT_1
    JZ r0, cts_tab0

    ; Ctrl+2 or Alt+2 -> tab 1
    CMPI r5, CTRL_2
    JZ r0, cts_tab1
    CMPI r5, ALT_2
    JZ r0, cts_tab1

    ; Ctrl+3 or Alt+3 -> tab 2
    CMPI r5, CTRL_3
    JZ r0, cts_tab2
    CMPI r5, ALT_3
    JZ r0, cts_tab2

    ; Ctrl+4 or Alt+4 -> tab 3
    CMPI r5, CTRL_4
    JZ r0, cts_tab3
    CMPI r5, ALT_4
    JZ r0, cts_tab3

    ; Not a tab switch
    LDI r0, 0
    JMP cts_ret

cts_tab0:
    LDI r0, 0
    CALL switch_to_tab
    LDI r0, 1
    JMP cts_ret
cts_tab1:
    LDI r0, 1
    CALL switch_to_tab
    LDI r0, 1
    JMP cts_ret
cts_tab2:
    LDI r0, 2
    CALL switch_to_tab
    LDI r0, 1
    JMP cts_ret
cts_tab3:
    LDI r0, 3
    CALL switch_to_tab
    LDI r0, 1

cts_ret:
    POP r31
    RET

; =========================================
; SWITCH_TO_TAB -- switch to tab r0
; Only switches if that tab has a PTY
; =========================================
switch_to_tab:
    PUSH r31
    LDI r1, 1

    ; Check if tab has a PTY
    MOV r10, r0
    CALL get_tab_pty
    ; r20 = PTY handle
    LDI r7, 0xFFFF
    CMP r20, r7
    JZ r0, st_skip

    ; Switch
    LDI r20, ACTIVE_TAB
    STORE r20, r10

st_skip:
    POP r31
    RET

; =========================================
; OPEN_NEW_TAB -- find first free slot, spawn bash
; =========================================
open_new_tab:
    PUSH r31
    LDI r1, 1

    ; Find first unused tab
    LDI r10, 0
ont_loop:
    CALL get_tab_pty
    LDI r7, 0xFFFF
    CMP r20, r7
    JZ r0, ont_found
    ADD r10, r1
    CMPI r10, 4
    BLT r0, ont_loop
    ; All tabs full, skip
    JMP ont_done

ont_found:
    CALL spawn_tab
    ; Switch to new tab
    LDI r20, ACTIVE_TAB
    STORE r20, r10

ont_done:
    POP r31
    RET

; =========================================
; CLOSE_ACTIVE_TAB -- close current tab's PTY, switch to next
; =========================================
close_active_tab:
    PUSH r31
    LDI r1, 1

    LDI r20, ACTIVE_TAB
    LOAD r10, r20

    ; Get PTY handle
    CALL get_tab_pty
    ; r20 = handle

    ; PTYCLOSE handle_reg
    ; We need the handle in a register for PTYCLOSE
    MOV r7, r20
    PTYCLOSE r7

    ; Clear tab metadata
    CALL get_tab_meta
    LDI r0, 0xFFFF
    STORE r20, r0      ; PTY = unused
    MOV r22, r20
    LDI r7, 4
    ADD r22, r7
    LDI r0, 0
    STORE r22, r0      ; CONNECTED = 0

    ; Find another active tab to switch to
    LDI r10, 0
cat_find:
    CMPI r10, 4
    BGE r0, cat_done
    CALL get_tab_pty
    LDI r7, 0xFFFF
    CMP r20, r7
    JNZ r0, cat_switch
    ADD r10, r1
    JMP cat_find

cat_switch:
    LDI r20, ACTIVE_TAB
    STORE r20, r10

cat_done:
    POP r31
    RET

; =========================================
; TRANSLATE_KEY -- translate IKEY code to PTY byte(s)
; Fills SEND_BUF, returns byte count in r0
; r5 = raw IKEY value
; =========================================
translate_key:
    PUSH r31
    LDI r1, 1

    ; Printable ASCII (32-126)
    CMPI r5, 32
    BLT r0, tk_special
    CMPI r5, 127
    BGE r0, tk_special
    LDI r20, SEND_BUF
    STORE r20, r5
    LDI r0, 1
    JMP tk_ret

tk_special:
    ; Enter (0x0D) -> \n (10)
    CMPI r5, 13
    JNZ r0, tk_bs
    LDI r20, SEND_BUF
    LDI r0, 10
    STORE r20, r0
    LDI r0, 1
    JMP tk_ret

tk_bs:
    ; Backspace (0x08) -> DEL (0x7F)
    CMPI r5, 8
    JNZ r0, tk_del
    LDI r20, SEND_BUF
    LDI r0, 127
    STORE r20, r0
    LDI r0, 1
    JMP tk_ret

tk_del:
    CMPI r5, 127
    JNZ r0, tk_tab
    LDI r20, SEND_BUF
    LDI r0, 127
    STORE r20, r0
    LDI r0, 1
    JMP tk_ret

tk_tab:
    CMPI r5, 9
    JNZ r0, tk_ctrl_c
    LDI r20, SEND_BUF
    LDI r0, 9
    STORE r20, r0
    LDI r0, 1
    JMP tk_ret

tk_ctrl_c:
    CMPI r5, 3
    JNZ r0, tk_ctrl_d
    LDI r20, SEND_BUF
    LDI r0, 3
    STORE r20, r0
    LDI r0, 1
    JMP tk_ret

tk_ctrl_d:
    CMPI r5, 4
    JNZ r0, tk_escape
    LDI r20, SEND_BUF
    LDI r0, 4
    STORE r20, r0
    LDI r0, 1
    JMP tk_ret

tk_escape:
    CMPI r5, 27
    JNZ r0, tk_arrow_up
    LDI r20, SEND_BUF
    LDI r0, 27
    STORE r20, r0
    LDI r0, 1
    JMP tk_ret

tk_arrow_up:
    CMPI r5, KEY_UP
    JNZ r0, tk_arrow_down
    LDI r20, SEND_BUF
    LDI r0, 27
    STORE r20, r0
    MOV r22, r20
    ADD r22, r1
    LDI r0, 91
    STORE r22, r0
    MOV r22, r20
    LDI r7, 2
    ADD r22, r7
    LDI r0, 65
    STORE r22, r0
    LDI r0, 3
    JMP tk_ret

tk_arrow_down:
    CMPI r5, KEY_DOWN
    JNZ r0, tk_arrow_right
    LDI r20, SEND_BUF
    LDI r0, 27
    STORE r20, r0
    MOV r22, r20
    ADD r22, r1
    LDI r0, 91
    STORE r22, r0
    MOV r22, r20
    LDI r7, 2
    ADD r22, r7
    LDI r0, 66
    STORE r22, r0
    LDI r0, 3
    JMP tk_ret

tk_arrow_right:
    CMPI r5, KEY_RIGHT
    JNZ r0, tk_arrow_left
    LDI r20, SEND_BUF
    LDI r0, 27
    STORE r20, r0
    MOV r22, r20
    ADD r22, r1
    LDI r0, 91
    STORE r22, r0
    MOV r22, r20
    LDI r7, 2
    ADD r22, r7
    LDI r0, 67
    STORE r22, r0
    LDI r0, 3
    JMP tk_ret

tk_arrow_left:
    CMPI r5, KEY_LEFT
    JNZ r0, tk_home
    LDI r20, SEND_BUF
    LDI r0, 27
    STORE r20, r0
    MOV r22, r20
    ADD r22, r1
    LDI r0, 91
    STORE r22, r0
    MOV r22, r20
    LDI r7, 2
    ADD r22, r7
    LDI r0, 68
    STORE r22, r0
    LDI r0, 3
    JMP tk_ret

tk_home:
    CMPI r5, KEY_HOME
    JNZ r0, tk_end
    LDI r20, SEND_BUF
    LDI r0, 27
    STORE r20, r0
    MOV r22, r20
    ADD r22, r1
    LDI r0, 91
    STORE r22, r0
    MOV r22, r20
    LDI r7, 2
    ADD r22, r7
    LDI r0, 72
    STORE r22, r0
    LDI r0, 3
    JMP tk_ret

tk_end:
    CMPI r5, KEY_END
    JNZ r0, tk_ignore
    LDI r20, SEND_BUF
    LDI r0, 27
    STORE r20, r0
    MOV r22, r20
    ADD r22, r1
    LDI r0, 91
    STORE r22, r0
    MOV r22, r20
    LDI r7, 2
    ADD r22, r7
    LDI r0, 70
    STORE r22, r0
    LDI r0, 3
    JMP tk_ret

tk_ignore:
    LDI r0, 0

tk_ret:
    POP r31
    RET

; =========================================
; DRAW_TAB_BAR -- render tab bar at bottom of screen
; Shows: [1:bash] [2:bash] [3:---] [4:---]
; Active tab is highlighted
; =========================================
draw_tab_bar:
    PUSH r31
    LDI r1, 1

    ; Tab bar background (10px tall at bottom)
    LDI r1, 0
    LDI r2, 246
    LDI r3, 256
    LDI r4, 10
    LDI r5, 0x1A1A2E
    RECTF r1, r2, r3, r4, r5

    ; Load active tab
    LDI r20, ACTIVE_TAB
    LOAD r28, r20

    ; Draw each tab
    LDI r10, 0
    LDI r12, 0       ; x position for tab label

dtb_loop:
    ; Check if tab is active
    MOV r11, r10
    LDI r20, ACTIVE_TAB
    LOAD r0, r20
    CMP r11, r0
    JZ r0, dtb_active

    ; Inactive tab colors
    LDI r14, 0x888888
    LDI r15, 0x1A1A2E
    JMP dtb_draw

dtb_active:
    LDI r14, 0xFFFFFF
    LDI r15, 0x333355

dtb_draw:
    ; Draw tab background (highlight box)
    ; x = r12, y = 246, w = 60, h = 8
    MOV r1, r12
    LDI r2, 247
    LDI r3, 58
    LDI r4, 8
    RECTF r1, r2, r3, r4, r15

    ; Build label in SCRATCH: "N:label"
    LDI r20, SCRATCH
    ; Tab number character: 49='1', 50='2', etc.
    LDI r0, 49
    ADD r0, r10
    STORE r20, r0
    ADD r20, r1
    LDI r0, 58     ; ':'
    STORE r20, r0
    ADD r20, r1

    ; Check if tab has PTY
    PUSH r10
    CALL get_tab_pty
    ; r20 = handle (but we need to re-derive meta...)
    ; Actually r20 is PTY handle from get_tab_pty
    LDI r7, 0xFFFF
    CMP r20, r7
    POP r10
    JZ r0, dtb_empty

    ; Check connected
    PUSH r10
    CALL get_tab_meta
    MOV r22, r20
    LDI r7, 4
    ADD r22, r7
    LOAD r0, r22
    POP r10
    JZ r0, dtb_dead

    ; Label = "bash"
    LDI r20, SCRATCH
    LDI r7, 2
    ADD r20, r7
    STRO r20, "bash"
    JMP dtb_render

dtb_dead:
    LDI r20, SCRATCH
    LDI r7, 2
    ADD r20, r7
    STRO r20, "dead"
    JMP dtb_render

dtb_empty:
    LDI r20, SCRATCH
    LDI r7, 2
    ADD r20, r7
    STRO r20, "---"

dtb_render:
    ; Draw label with SMALLTEXT
    MOV r1, r12
    ADD r1, r1        ; x + 2
    LDI r2, 248
    LDI r3, SCRATCH
    SMALLTEXT r1, r2, r3, r14, r15

    ; Advance x position
    LDI r7, 62
    ADD r12, r7

    ADD r10, r1
    CMPI r10, 4
    BLT r0, dtb_loop

    ; Restore r1
    LDI r1, 1

    POP r31
    RET

; =========================================
; RENDER_ACTIVE_TAB -- render the active tab's text buffer
; =========================================
render_active_tab:
    PUSH r31
    LDI r1, 1

    ; Clear content area (y=0 to y=246)
    LDI r1, 0
    LDI r2, 0
    LDI r3, 256
    LDI r4, 246
    LDI r5, 0x0A0A0A
    RECTF r1, r2, r3, r4, r5

    ; Title bar background
    LDI r1, 0
    LDI r2, 0
    LDI r3, 256
    LDI r4, 10
    LDI r5, 0x1A1A2E
    RECTF r1, r2, r3, r4, r5

    ; Title: "tmux [tab N]"
    LDI r20, SCRATCH
    STRO r20, "tmux [tab "
    ; Find end
    LDI r10, 0
rat_find_end:
    LDI r7, SCRATCH
    ADD r7, r10
    LOAD r0, r7
    JZ r0, rat_append_num
    ADD r10, r1
    JMP rat_find_end

rat_append_num:
    LDI r20, ACTIVE_TAB
    LOAD r0, r20
    LDI r7, 49
    ADD r0, r7
    LDI r20, SCRATCH
    ADD r20, r10
    STORE r20, r0
    ADD r10, r1
    LDI r20, SCRATCH
    ADD r20, r10
    LDI r0, 93     ; ']'
    STORE r20, r0
    ADD r10, r1
    LDI r20, SCRATCH
    ADD r20, r10
    LDI r0, 0
    STORE r20, r0

    ; Draw title
    LDI r1, 2
    LDI r2, 1
    LDI r3, SCRATCH
    LDI r4, 0x44DD44
    LDI r5, 0x1A1A2E
    DRAWTEXT r1, r2, r3, r4, r5

    ; Render text buffer
    LDI r20, ACTIVE_TAB
    LOAD r10, r20

    CALL get_tab_buf_base
    MOV r11, r20      ; r11 = buf pointer
    LDI r12, 10       ; y position (start after title bar)
    LDI r1, 1
    LDI r13, 0  ; row counter

rat_row:
    ; Copy COLS chars to scratch for rendering
    LDI r16, SCRATCH
    LDI r17, 0
rat_copy_col:
    LOAD r6, r11
    STORE r16, r6
    ADD r11, r1
    ADD r16, r1
    ADD r17, r1
    CMPI r17, COLS
    BLT r0, rat_copy_col
    LDI r0, 0
    STORE r16, r0      ; null-terminate

    ; Render with SMALLTEXT
    LDI r1, 0
    LDI r13, SCRATCH
    LDI r14, 0xBBBBBB
    LDI r15, 0
    SMALLTEXT r1, r12, r13, r14, r15

    LDI r1, 1
    ; Advance y by 6
    ADD r12, r1
    ADD r12, r1
    ADD r12, r1
    ADD r12, r1
    ADD r12, r1
    ADD r12, r1

    ADD r13, r1
    CMPI r13, ROWS
    BLT r13, rat_row

    ; Cursor blink for active tab
    LDI r20, ACTIVE_TAB
    LOAD r10, r20
    CALL get_tab_meta
    ; Blink counter
    MOV r22, r20
    LDI r7, 5
    ADD r22, r7
    LOAD r0, r22
    ADD r0, r1
    STORE r22, r0
    LDI r7, 8
    AND r0, r7
    CMPI r0, 4
    BGE r0, cursor_skip

    ; Draw cursor
    CALL get_tab_meta
    MOV r22, r20
    LDI r7, 1
    ADD r22, r7
    LOAD r0, r22     ; CUR_COL
    LDI r7, 3
    MUL r0, r7
    MOV r22, r20
    LDI r7, 2
    ADD r22, r7
    LOAD r2, r22     ; CUR_ROW
    LDI r7, 6
    MUL r2, r7
    LDI r7, 10
    ADD r2, r7       ; + title bar offset
    LDI r3, 2
    LDI r4, 5
    LDI r5, 0x44FF44
    RECTF r0, r2, r3, r4, r5

cursor_skip:
    LDI r1, 1
    POP r31
    RET
