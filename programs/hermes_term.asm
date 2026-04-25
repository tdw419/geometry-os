; hermes_term.asm -- Hermes Agent Terminal for Geometry OS
;
; Connects to hermes_bridge (localhost:9123) via TCP, providing a full
; Hermes chat experience inside the Geometry OS desktop.
;
; Protocol:
;   Client sends:  "SEND <text>\n"
;   Server sends:  "LINE <text>\n"   -- one line of hermes output
;   Server sends:  "DONE\n"           -- hermes finished responding
;   Server sends:  "READY\n"          -- initial handshake
;   Server sends:  "ERR <msg>\n"      -- error
;
; Commands: /clear /help /reconnect
;
; RAM Layout:
;   0x4000-0x44EB  Text buffer (42*30 = 1260 u32 cells, row-major)
;   0x4800         Cursor column
;   0x4801         Cursor row
;   0x4802         Blink counter
;   0x5000-0x51FF  Scratch line buffer (512 chars + null)
;   0x5400-0x55FF  TCP send buffer (512 chars + null)
;   0x5800-0x5FFF  TCP recv buffer (2048 chars)
;   0x7800         TCP connection fd (0xFFFF = not connected)
;   0x7801         State: 0=idle, 1=receiving
;
; Registers:
;   r0: CMP result (reserved)
;   r1: Constant 1 (must reload after ops that clobber)
;   r5: Key input
;   r30: Stack pointer (SP)

#define COLS 42
#define ROWS 30
#define BUF 0x4000
#define BUF_END 0x44EC
#define CUR_COL 0x4800
#define CUR_ROW 0x4801
#define BLINK 0x4802
#define SCRATCH 0x5000
#define SEND_BUF 0x5400
#define RECV_BUF 0x5800
#define TCP_FD 0x7800
#define RECV_STATE 0x7801

; =========================================
; INIT
; =========================================
LDI r1, 1
LDI r30, 0xFD00   ; Stack pointer

; Clear screen
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

; Init cursor and blink
LDI r20, CUR_COL
LDI r0, 0
STORE r20, r0
LDI r20, CUR_ROW
STORE r20, r0
LDI r20, BLINK
STORE r20, r0

; Mark as not connected
LDI r20, TCP_FD
LDI r0, 0xFFFF
STORE r20, r0
LDI r20, RECV_STATE
LDI r0, 0
STORE r20, r0

; Title bar
LDI r1, 0
LDI r2, 0
LDI r3, 256
LDI r4, 16
LDI r5, 0x1A0033
RECTF r1, r2, r3, r4, r5

; Title text "Hermes Terminal"
LDI r20, SCRATCH
STRO r20, "Hermes Terminal"
LDI r1, 4
LDI r2, 4
LDI r3, SCRATCH
LDI r4, 0x00FF00  ; green
LDI r5, 0x1A0033  ; match title bar
DRAWTEXT r1, r2, r3, r4, r5

; Close button hit region
LDI r1, 220
LDI r2, 0
LDI r3, 36
LDI r4, 16
HITSET r1, r2, r3, r4, 99

LDI r1, 1

; Welcome messages
LDI r20, SCRATCH
STRO r20, "Hermes Terminal v1.0"
CALL write_line_to_buf
LDI r1, 1
LDI r20, SCRATCH
STRO r20, "Connecting to hermes_bridge..."
CALL write_line_to_buf
LDI r1, 1

; --- Connect to hermes_bridge ---
; Store IP "127.0.0.1" in SEND_BUF temporarily
; (STRO null-terminates automatically)
LDI r20, SEND_BUF
STRO r20, "127.0.0.1"

; CONNECT addr_reg=r3, port_reg=r4, fd_reg=r5
LDI r3, SEND_BUF    ; address of IP string
LDI r4, 9123        ; port
LDI r5, 0           ; fd will be stored here
CONNECT r3, r4, r5

; Check r0 for success (0 = OK)
LDI r1, 1
CMPI r0, 0
JNZ r0, connect_failed

; Store the fd
LDI r20, TCP_FD
STORE r20, r5

LDI r20, SCRATCH
STRO r20, "Connected! Type a message."
CALL write_line_to_buf
LDI r1, 1
CALL write_prompt
JMP main_loop

connect_failed:
    LDI r20, SCRATCH
    STRO r20, "Connection failed."
    CALL write_line_to_buf
    LDI r1, 1
    LDI r20, SCRATCH
    STRO r20, "Start bridge: tools/hermes_bridge/target/release/hermes_bridge"
    CALL write_line_to_buf
    LDI r1, 1
    LDI r20, SCRATCH
    STRO r20, "/reconnect to retry"
    CALL write_line_to_buf
    LDI r1, 1
    CALL write_prompt

; =========================================
; MAIN LOOP
; =========================================
main_loop:
    LDI r1, 1

    ; Only poll for TCP data when actively receiving
    LDI r20, RECV_STATE
    LOAD r0, r20
    CMPI r0, 1
    JNZ r0, skip_poll
    CALL poll_recv
    LDI r1, 1
skip_poll:

    ; Blink counter
    LDI r20, BLINK
    LOAD r0, r20
    ADD r0, r1
    STORE r20, r0

    ; Render
    CALL render

    FRAME

    ; Read key
    IKEY r5
    JZ r5, main_loop

    ; Handle key
    CALL handle_key
    JMP main_loop

; =========================================
; POLL_RECV -- check for data from bridge
; =========================================
poll_recv:
    PUSH r31
    LDI r1, 1

    ; Check if connected
    LDI r20, TCP_FD
    LOAD r2, r20
    LDI r3, 0xFFFF
    CMP r2, r3
    JZ r0, poll_ret

    ; Try to receive up to 256 bytes
    LDI r20, TCP_FD
    LOAD r6, r20          ; r6 = fd value

    ; SOCKRECV takes register indices for fd, buf, max_len, recv_count
    MOV r14, r6           ; regs[14] = fd value
    LDI r15, RECV_BUF
    LDI r16, 256
    SOCKRECV r14, r15, r16, r17

    ; r0 = error code, r17 = bytes received
    ; If r0 != 0 or r17 == 0, nothing to do
    CMPI r0, 0
    JNZ r0, poll_ret
    CMPI r17, 0
    JZ r0, poll_ret

    ; Process received data -- it's line-based protocol
    ; Parse lines from RECV_BUF and handle LINE/DONE/ERR/READY
    CALL process_recv_buf

poll_ret:
    POP r31
    RET

; =========================================
; PROCESS_RECV_BUF
; Parse the received buffer for complete lines
; and handle LINE/DONE/ERR messages
; =========================================
process_recv_buf:
    PUSH r31
    LDI r1, 1

    ; We'll scan RECV_BUF for newlines and process each line
    LDI r20, RECV_BUF
    LDI r21, SCRATCH      ; build current line into SCRATCH
    LDI r22, 0            ; line char count

prb_loop:
    LOAD r0, r20
    JZ r0, prb_done       ; null terminator

    ; Check for newline
    CMPI r0, 10
    JNZ r0, prb_char

    ; Got a complete line in SCRATCH -- process it
    LDI r0, 0
    STORE r21, r0         ; null terminate
    CALL handle_bridge_line
    LDI r1, 1

    ; Reset for next line
    LDI r21, SCRATCH
    LDI r22, 0
    ADD r20, r1
    JMP prb_loop

prb_char:
    ; Store char in scratch (max 40 chars per display line)
    CMPI r22, 40
    BGE r0, prb_skip
    STORE r21, r0
    ADD r21, r1
    ADDI r22, 1

prb_skip:
    ADD r20, r1
    JMP prb_loop

prb_done:
    POP r31
    RET

; =========================================
; HANDLE_BRIDGE_LINE
; Process one line from the bridge (in SCRATCH, null-terminated)
; =========================================
handle_bridge_line:
    PUSH r31
    LDI r1, 1

    ; Check for "LINE " prefix (76='L', 73='I', 78='N', 69='E', 32=' ')
    LDI r20, SCRATCH
    LOAD r0, r20
    CMPI r0, 76           ; 'L'
    JNZ r0, hbl_check_done
    ADD r20, r1
    LOAD r0, r20
    CMPI r0, 73           ; 'I'
    JNZ r0, hbl_check_done
    ADD r20, r1
    LOAD r0, r20
    CMPI r0, 78           ; 'N'
    JNZ r0, hbl_check_done
    ADD r20, r1
    LOAD r0, r20
    CMPI r0, 69           ; 'E'
    JNZ r0, hbl_check_done
    ADD r20, r1
    LOAD r0, r20
    CMPI r0, 32           ; ' '
    JNZ r0, hbl_check_done

    ; It's a LINE message -- skip the "LINE " prefix, copy rest to SCRATCH
    ADD r20, r1           ; skip the space
    LDI r21, SCRATCH
hbl_copy:
    LOAD r0, r20
    JZ r0, hbl_copy_done
    STORE r21, r0
    ADD r20, r1
    ADD r21, r1
    JMP hbl_copy
hbl_copy_done:
    LDI r0, 0
    STORE r21, r0

    ; Write the line to the text buffer (in cyan)
    CALL write_line_to_buf
    LDI r1, 1
    JMP hbl_ret

hbl_check_done:
    ; Check for "DONE"
    LDI r20, SCRATCH
    LOAD r0, r20
    CMPI r0, 68           ; 'D'
    JNZ r0, hbl_check_err
    ADD r20, r1
    LOAD r0, r20
    CMPI r0, 79           ; 'O'
    JNZ r0, hbl_check_err
    ADD r20, r1
    LOAD r0, r20
    CMPI r0, 78           ; 'N'
    JNZ r0, hbl_check_err
    ADD r20, r1
    LOAD r0, r20
    CMPI r0, 69           ; 'E'
    JNZ r0, hbl_check_err

    ; DONE -- hermes finished, write new prompt
    ; Set receiving state to idle
    LDI r20, RECV_STATE
    LDI r0, 0
    STORE r20, r0
    CALL write_prompt
    JMP hbl_ret

hbl_check_err:
    ; Check for "ERR "
    LDI r20, SCRATCH
    LOAD r0, r20
    CMPI r0, 69           ; 'E'
    JNZ r0, hbl_check_ready
    ADD r20, r1
    LOAD r0, r20
    CMPI r0, 82           ; 'R'
    JNZ r0, hbl_check_ready
    ADD r20, r1
    LOAD r0, r20
    CMPI r0, 82           ; 'R'
    JNZ r0, hbl_check_ready

    ; ERR message -- display in red (write to scratch with prefix)
    LDI r21, SCRATCH
    ADD r20, r1           ; skip "ERR"
    LOAD r0, r20
    CMPI r0, 32           ; space after ERR
    JNZ r0, hbl_show_err
    ADD r20, r1           ; skip space
hbl_show_err:
    LDI r23, SCRATCH
    ; Copy error text
hbl_err_copy:
    LOAD r0, r20
    JZ r0, hbl_err_done
    STORE r23, r0
    ADD r20, r1
    ADD r23, r1
    JMP hbl_err_copy
hbl_err_done:
    LDI r0, 0
    STORE r23, r0
    CALL write_line_to_buf
    LDI r1, 1
    JMP hbl_ret

hbl_check_ready:
    ; Check for "READY"
    LDI r20, SCRATCH
    LOAD r0, r20
    CMPI r0, 82           ; 'R'
    JNZ r0, hbl_ret
    ADD r20, r1
    LOAD r0, r20
    CMPI r0, 69           ; 'E'
    JNZ r0, hbl_ret
    ADD r20, r1
    LOAD r0, r20
    CMPI r0, 65           ; 'A'
    JNZ r0, hbl_ret
    ; READY -- do nothing, we already showed connected message

hbl_ret:
    POP r31
    RET

; =========================================
; RENDER
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

    ; Row loop
    LDI r1, 1
    LDI r8, 8            ; CHAR_H
    LDI r9, 6            ; CHAR_W
    LDI r10, 0           ; row counter
    LDI r11, BUF         ; buffer pointer
    LDI r12, 16          ; y = TITLE_H

render_row:
    ; Copy COLS chars from buffer to scratch
    LDI r16, SCRATCH
    LDI r17, 0
copy_col:
    LOAD r6, r11
    STORE r16, r6
    ADD r11, r1
    ADD r16, r1
    ADD r17, r1
    CMPI r17, COLS
    BLT r17, copy_col

    ; Null terminate
    LDI r0, 0
    STORE r16, r0

    ; Color based on content
    LDI r14, 0xCCCCCC  ; default light gray

    ; Check if row starts with '>' (user prompt) -> green
    LDI r16, SCRATCH
    LOAD r6, r16
    CMPI r6, 62         ; '>'
    JNZ r0, render_text_default
    LDI r14, 0x44FF44  ; green for user input
    JMP render_text

render_text_default:
    LDI r14, 0x00FFFF  ; cyan for AI/bridge responses

render_text:
    ; DRAWTEXT x=0, y=r12, addr=SCRATCH, fg=r14, bg=0 (transparent)
    LDI r1, 0
    LDI r13, SCRATCH
    LDI r15, 0         ; bg = transparent
    DRAWTEXT r1, r12, r13, r14, r15

    LDI r1, 1

    ADD r12, r8          ; y += 8
    ADD r10, r1          ; row++
    CMPI r10, ROWS
    BLT r10, render_row

    ; Cursor (blink)
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
    MUL r0, r9           ; x = col * 6
    LDI r20, CUR_ROW
    LOAD r2, r20
    MUL r2, r8           ; row * 8
    LDI r3, 16
    ADD r2, r3           ; y = 16 + row*8
    LDI r3, 6
    LDI r4, 8
    LDI r5, 0x44FF44
    RECTF r0, r2, r3, r4, r5

cursor_done:
    POP r31
    RET

; =========================================
; HANDLE_KEY
; r5 = key
; =========================================
handle_key:
    PUSH r31
    LDI r1, 1

    CMPI r5, 13
    JNZ r0, check_bs
    JMP do_enter

check_bs:
    CMPI r5, 8
    JNZ r0, check_del
    JMP do_backspace

check_del:
    CMPI r5, 127
    JNZ r0, do_char
    JMP do_backspace

do_char:
    ; buf[row*COLS + col] = key
    LDI r20, CUR_ROW
    LOAD r2, r20
    LDI r3, COLS
    MUL r2, r3           ; r2 = row * COLS
    LDI r20, CUR_COL
    LOAD r0, r20
    ADD r2, r0           ; r2 = row*COLS + col
    LDI r20, BUF
    ADD r20, r2          ; r20 = BUF + offset
    STORE r20, r5        ; write char

    ; col++
    LDI r20, CUR_COL
    LOAD r0, r20
    ADD r0, r1
    STORE r20, r0

    ; If col >= COLS, wrap
    CMPI r0, COLS
    JNZ r0, hk_ret
    CALL do_newline
    JMP hk_ret

; =========================================
; DO_ENTER -- send to bridge or handle command
; =========================================
do_enter:
    ; 1. Extract command text from current row into SCRATCH
    CALL extract_cmd

    ; 2. Advance to next row
    CALL do_newline

    ; 3. Check for / commands
    LDI r1, 1
    LDI r20, SCRATCH
    LOAD r22, r20
    CMPI r22, 47         ; '/'
    JNZ r0, send_to_bridge  ; not a command, send to bridge

    ; Check /help
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 104        ; 'h'
    JNZ r0, try_clear_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 101        ; 'e'
    JNZ r0, try_clear_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 108        ; 'l'
    JNZ r0, try_clear_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 112        ; 'p'
    JNZ r0, try_clear_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null
    JNZ r0, send_to_bridge
    JMP cmd_help

try_clear_cmd:
    ; Check /clear
    LDI r20, SCRATCH
    ADD r20, r1           ; skip '/'
    LOAD r22, r20
    CMPI r22, 99         ; 'c'
    JNZ r0, try_reconnect_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 108        ; 'l'
    JNZ r0, try_reconnect_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 101        ; 'e'
    JNZ r0, try_reconnect_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 97         ; 'a'
    JNZ r0, try_reconnect_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 114        ; 'r'
    JNZ r0, try_reconnect_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null
    JNZ r0, send_to_bridge
    JMP cmd_clear

try_reconnect_cmd:
    ; Check /reconnect
    LDI r20, SCRATCH
    ADD r20, r1           ; skip '/'
    LOAD r22, r20
    CMPI r22, 114        ; 'r'
    JNZ r0, send_to_bridge
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 101        ; 'e'
    JNZ r0, send_to_bridge
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 99         ; 'c'
    JNZ r0, send_to_bridge
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 111        ; 'o'
    JNZ r0, send_to_bridge
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 110        ; 'n'
    JNZ r0, send_to_bridge
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 110        ; 'n'
    JNZ r0, send_to_bridge
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 101        ; 'e'
    JNZ r0, send_to_bridge
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 99         ; 'c'
    JNZ r0, send_to_bridge
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 116        ; 't'
    JNZ r0, send_to_bridge
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null
    JNZ r0, send_to_bridge
    JMP cmd_reconnect

    ; If not a known command, just send it to bridge as-is
    JMP send_to_bridge

; =========================================
; COMMANDS
; =========================================
cmd_help:
    LDI r1, 1
    LDI r20, SCRATCH
    STRO r20, "/help /clear /reconnect"
    CALL write_line_to_buf
    LDI r1, 1
    CALL write_prompt
    JMP hk_ret

cmd_clear:
    LDI r1, 1
    ; Clear text buffer to spaces
    LDI r20, BUF
    LDI r6, 32
clear_buf_cmd:
    STORE r20, r6
    ADD r20, r1
    CMPI r20, BUF_END
    BLT r0, clear_buf_cmd
    ; Reset cursor
    LDI r20, CUR_COL
    LDI r0, 0
    STORE r20, r0
    LDI r20, CUR_ROW
    STORE r20, r0
    CALL write_prompt
    JMP hk_ret

cmd_reconnect:
    LDI r1, 1
    ; Disconnect old if any
    LDI r20, TCP_FD
    LOAD r2, r20
    LDI r3, 0xFFFF
    CMP r2, r3
    JZ r0, rc_connect
    ; Disconnect existing
    DISCONNECT r2
    LDI r20, TCP_FD
    LDI r0, 0xFFFF
    STORE r20, r0

rc_connect:
    LDI r20, SCRATCH
    STRO r20, "Reconnecting..."
    CALL write_line_to_buf
    LDI r1, 1

    ; Store IP string (STRO null-terminates)
    LDI r20, SEND_BUF
    STRO r20, "127.0.0.1"

    LDI r3, SEND_BUF
    LDI r4, 9123
    LDI r5, 0
    CONNECT r3, r4, r5

    LDI r1, 1
    CMPI r0, 0
    JNZ r0, rc_failed

    LDI r20, TCP_FD
    STORE r20, r5
    LDI r20, SCRATCH
    STRO r20, "Connected!"
    CALL write_line_to_buf
    LDI r1, 1
    CALL write_prompt
    JMP hk_ret

rc_failed:
    LDI r20, SCRATCH
    STRO r20, "Connection failed."
    CALL write_line_to_buf
    LDI r1, 1
    CALL write_prompt
    JMP hk_ret

; =========================================
; SEND_TO_BRIDGE
; Send SCRATCH content to hermes via TCP
; =========================================
send_to_bridge:
    LDI r1, 1

    ; Check if connected
    LDI r20, TCP_FD
    LOAD r2, r20
    LDI r3, 0xFFFF
    CMP r2, r3
    JZ r0, stb_not_connected

    ; Build "SEND " + message + "\n" in SEND_BUF
    LDI r19, SEND_BUF
    STRO r19, "SEND "
    CALL advance_to_null_tmp

    ; Copy SCRATCH content
    LDI r18, SCRATCH
    CALL copy_until_null_tmp

    ; Append newline (10)
    LDI r0, 10
    STORE r19, r0
    ADDI r19, 1
    ; Null terminate (for length calc)
    LDI r0, 0
    STORE r19, r0

    ; Calculate length
    LDI r20, SEND_BUF
    LDI r22, 0
stb_len:
    LOAD r0, r20
    JZ r0, stb_send
    ADD r20, r1
    ADDI r22, 1
    JMP stb_len

stb_send:
    ; SOCKSEND fd_reg, buf_reg, len_reg, sent_reg
    MOV r14, r2           ; fd value
    LDI r15, SEND_BUF
    MOV r16, r22
    SOCKSEND r14, r15, r16, r17

    ; Show "thinking" indicator
    LDI r20, SCRATCH
    STRO r20, "..."
    CALL write_line_to_buf
    LDI r1, 1

    ; Set receiving state
    LDI r20, RECV_STATE
    LDI r0, 1
    STORE r20, r0

    JMP hk_ret

stb_not_connected:
    LDI r20, SCRATCH
    STRO r20, "Not connected. /reconnect"
    CALL write_line_to_buf
    LDI r1, 1
    CALL write_prompt
    JMP hk_ret

; =========================================
; WRITE_PROMPT
; =========================================
write_prompt:
    PUSH r31
    LDI r1, 1
    LDI r20, CUR_ROW
    LOAD r2, r20
    LDI r3, COLS
    MUL r2, r3
    LDI r20, BUF
    ADD r20, r2
    LDI r0, 62           ; '>'
    STORE r20, r0
    ADD r20, r1
    LDI r0, 32           ; ' '
    STORE r20, r0
    LDI r20, CUR_COL
    LDI r0, 2
    STORE r20, r0
    POP r31
    RET

; =========================================
; EXTRACT_CMD
; =========================================
extract_cmd:
    PUSH r31
    LDI r1, 1

    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7

    LDI r20, CUR_COL
    LOAD r7, r20

    LDI r20, BUF
    ADD r20, r6
    ADD r20, r1
    ADD r20, r1           ; skip "> "

    LDI r21, SCRATCH
    LDI r22, 2
ec_loop:
    CMP r22, r7
    BGE r0, ec_done
    LOAD r0, r20
    STORE r21, r0
    ADD r20, r1
    ADD r21, r1
    ADD r22, r1
    JMP ec_loop

ec_done:
    LDI r0, 0
    STORE r21, r0
    POP r31
    RET

; =========================================
; WRITE_LINE_TO_BUF
; =========================================
write_line_to_buf:
    PUSH r31
    LDI r1, 1

    LDI r20, CUR_ROW
    LOAD r2, r20
    LDI r3, COLS
    MUL r2, r3
    LDI r20, BUF
    ADD r20, r2

    LDI r21, SCRATCH

wlb_loop:
    LOAD r0, r21
    JZ r0, wlb_done
    STORE r20, r0
    ADD r20, r1
    ADD r21, r1
    JMP wlb_loop

wlb_done:
    CALL do_newline
    POP r31
    RET

; =========================================
; DO_BACKSPACE
; =========================================
do_backspace:
    LDI r1, 1
    LDI r20, CUR_COL
    LOAD r0, r20
    CMPI r0, 2
    JZ r0, hk_ret

    SUBI r0, 1
    STORE r20, r0

    LDI r20, CUR_ROW
    LOAD r2, r20
    LDI r3, COLS
    MUL r2, r3
    LDI r20, CUR_COL
    LOAD r0, r20
    ADD r2, r0
    LDI r20, BUF
    ADD r20, r2
    LDI r0, 32
    STORE r20, r0
    JMP hk_ret

; =========================================
; DO_NEWLINE
; =========================================
do_newline:
    LDI r1, 1
    LDI r20, CUR_COL
    LDI r0, 0
    STORE r20, r0
    LDI r20, CUR_ROW
    LOAD r6, r20
    ADD r6, r1
    CMPI r6, ROWS
    BLT r0, dn_store
    PUSH r31
    CALL scroll_up
    POP r31
    LDI r20, CUR_ROW
    LDI r6, 29
dn_store:
    STORE r20, r6
    RET

; =========================================
; SCROLL_UP
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
    ADD r20, r0

    LDI r21, BUF
    LDI r0, 0
    ADD r0, r10
    LDI r11, COLS
    MUL r0, r11
    ADD r21, r0

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
scroll_clr_loop:
    STORE r20, r6
    ADD r20, r1
    ADD r22, r1
    CMPI r22, COLS
    BLT r0, scroll_clr_loop

    POP r31
    RET

; =========================================
; UTILITY: advance_to_null_tmp (r19)
; Used for building SEND_BUF strings
; =========================================
advance_to_null_tmp:
    PUSH r31
adv_loop2:
    LOAD r12, r19
    LDI r13, 0
    CMP r12, r13
    JZ r0, adv_done2
    ADDI r19, 1
    JMP adv_loop2
adv_done2:
    POP r31
    RET

; =========================================
; UTILITY: copy_until_null_tmp (r18 -> r19)
; =========================================
copy_until_null_tmp:
    PUSH r31
cun_loop2:
    LOAD r12, r18
    LDI r13, 0
    CMP r12, r13
    JZ r0, cun_done2
    STORE r19, r12
    ADDI r18, 1
    ADDI r19, 1
    JMP cun_loop2
cun_done2:
    POP r31
    RET

; =========================================
; HK_RET -- common return point for handle_key
; =========================================
hk_ret:
    POP r31
    RET
