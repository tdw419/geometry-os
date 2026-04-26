; host_term.asm -- Host Shell Terminal for Geometry OS (v6)
;
; Spawns bash inside a real PTY via the PTYOPEN opcode. Pipes keystrokes
; through PTYWRITE, drains PTY output through PTYREAD each frame.
;
; v6 improvements (Phase 132: ANSI Color Rendering):
;   - SGR (Set Graphics Rendition) color support
;   - Per-character color buffer parallel to text buffer
;   - ANSI 16-color palette (standard + bright)
;   - Color-run rendering for efficient display
;
; v5 improvements (Phase 133: Wider Display):
;   - SMALLTEXT opcode (3x5 font) for 80 readable columns
;   - 80x40 terminal (standard 80 cols, 40 rows) via PTYSIZE
;   - Arrow key support (ESC [ A/B/C/D sequences)
;   - Shift-aware lowercase text input
;   - ANSI escape stripping (CSI + OSC sequences)
;   - Backspace/Delete, Ctrl-C (0x03), Ctrl-D (0x04), Tab (0x09)
;
; RAM Layout:
;   0x4000-0x77FF  Text buffer (80*47 = 3760 u32 cells, row-major)
;   0x4E00         Cursor column
;   0x4E01         Cursor row
;   0x4E02         Blink counter
;   0x4E03         PTY handle
;   0x4E04         ANSI state
;   0x4E05         STATUS_CONNECTED
;   0x4E06         STATUS_CWD_LEN
;   0x4E07         OSC_LEN
;   0x4E08         SCROLL_X (horizontal scroll offset)
;   0x4E09         FG_COLOR (current foreground color, default 0xBBBBBB)
;   0x4E0A         CSI_PARAM (accumulated SGR parameter)
;   0x5000         Empty cmd string
;   0x5400         Send buffer
;   0x5800-0x5FFF  Receive buffer (2048 cells)
;   0x6000-0x60FF  Scratch buffer for SMALLTEXT rendering (128 chars)
;   0x6100-0x613F  STATUS_CWD (64 chars max)
;   0x6200-0x6250  OSC_BUF (80 chars max)
;   0x7800-0x847F  Color buffer (80*40 = 3200 u32 cells, parallel to text buffer)
;
; Registers:
;   r0  CMP/result
;   r1  constant 1
;   r28 PTY handle (live copies)
;   r30 stack pointer
;
#define COLS 80
#define ROWS 40
#define BUF 0x4000
#define BUF_END 0x77F0
#define CUR_COL 0x4E00
#define CUR_ROW 0x4E01
#define BLINK 0x4E02
#define PTY_HANDLE 0x4E03
#define ANSI_STATE 0x4E04
#define STATUS_CONNECTED 0x4E05
#define STATUS_CWD_LEN 0x4E06
#define OSC_LEN 0x4E07
#define SCROLL_X 0x4E08
#define FG_COLOR 0x4E09
#define CSI_PARAM 0x4E0A
#define STATUS_CWD 0x6100
#define CMD_BUF 0x5000
#define SEND_BUF 0x5400
#define RECV_BUF 0x5800
#define SCRATCH 0x6000
#define OSC_BUF 0x6200
#define COLOR_BUF 0x7800

; ANSI states
#define ANS_NORMAL 0
#define ANS_ESC    1
#define ANS_CSI    2
#define ANS_OSC    3
#define ANS_UTF8_2 4
#define ANS_UTF8_3A 5
#define ANS_UTF8_3B 6

; Extended key codes (from keys.rs)
#define KEY_UP    0x80
#define KEY_DOWN  0x81
#define KEY_LEFT  0x82
#define KEY_RIGHT 0x83
#define KEY_HOME  0x84
#define KEY_END   0x85

; UTF-8 decoding accumulator
#define UTF8_CP 0x4E0B
; CSI second parameter (row for H, unused for most others)
#define CSI_PARAM2 0x4E0C
; Dirty flag for status bar (1 = needs redraw)
#define DIRTY_STATUS 0x4E0D

; =========================================
; INIT
; =========================================
LDI r1, 1
LDI r30, 0xFD00

; Background fill -- dark gray
LDI r0, 0x0A0A0A
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
LDI r20, STATUS_CONNECTED
STORE r20, r0
LDI r20, STATUS_CWD_LEN
STORE r20, r0
LDI r20, OSC_LEN
STORE r20, r0
LDI r20, SCROLL_X
STORE r20, r0

; FG_COLOR init (default light gray)
LDI r20, FG_COLOR
LDI r0, 0xBBBBBB
STORE r20, r0

; CSI_PARAM init
LDI r20, CSI_PARAM
LDI r0, 0
STORE r20, r0

; CSI_PARAM2 init
LDI r20, CSI_PARAM2
LDI r0, 0
STORE r20, r0

; DIRTY_STATUS init (start dirty so first frame draws)
LDI r20, DIRTY_STATUS
LDI r0, 1
STORE r20, r0

; Clear COLOR_BUF to default FG color
LDI r20, COLOR_BUF
LDI r6, 0xBBBBBB
clr_color_init:
    STORE r20, r6
    ADD r20, r1
    CMPI r20, 0x8480
    BLT r0, clr_color_init

; Title bar background
LDI r1, 0
LDI r2, 0
LDI r3, 256
LDI r4, 10
LDI r5, 0x1A1A2E
RECTF r1, r2, r3, r4, r5

; Title text (using SMALLTEXT for compact display)
LDI r20, SCRATCH
STRO r20, "shell 80x40"
LDI r1, 2
LDI r2, 2
LDI r3, SCRATCH
LDI r4, 0x44DD44
LDI r5, 0x1A1A2E
SMALLTEXT r1, r2, r3, r4, r5

; Close button hit region
LDI r1, 230
LDI r2, 0
LDI r3, 26
LDI r4, 10
HITSET r1, r2, r3, r4, 99

; Empty cmd string for PTYOPEN
LDI r1, CMD_BUF
LDI r0, 0
STORE r1, r0

; PTYOPEN
LDI r5, CMD_BUF
PTYOPEN r5, r10

; Save handle and mark connected
LDI r20, PTY_HANDLE
STORE r20, r10
LDI r28, 0
ADD r28, r10
LDI r20, STATUS_CONNECTED
LDI r0, 1
STORE r20, r0

; PTYSIZE -- tell the PTY we want 80 cols x 40 rows
LDI r10, 40
LDI r11, 80
PTYSIZE r28, r10, r11

; r1 = 1 (restore)
LDI r1, 1

; =========================================
; STARTUP DRAIN (unified)
; =========================================
LDI r20, SEND_BUF
STRO r20, "[connecting...]"
CALL write_str_to_buf
CALL render
FRAME

; r21 = got_output flag
; r22 = frame counter
; r23 = phase (0=drain, 1=probing, 2=probe_drain)
LDI r21, 0
LDI r22, 0
LDI r23, 0

startup_drain:
    LDI r6, RECV_BUF
    LDI r7, 512
    PTYREAD r28, r6, r7
    MOV r8, r0
    CMPI r8, 0
    JZ r0, startup_check_phase
    LDI r7, 0xFFFFFFFF
    CMP r8, r7
    JZ r0, pty_closed

    LDI r9, 0
sd_append:
    CMP r9, r8
    BGE r0, sd_after
    LDI r20, RECV_BUF
    ADD r20, r9
    LOAD r5, r20
    CALL process_byte
    ADD r9, r1
    JMP sd_append
sd_after:
    LDI r21, 1

startup_check_phase:
    CALL render
    FRAME
    ADD r22, r1

    ; Phase 0: drain for 60 frames
    CMPI r23, 0
    JNZ r0, sd_phase1
    CMPI r22, 60
    BLT r22, startup_drain
    ; Check if we got output
    CMPI r21, 0
    JNZ r0, startup_done
    ; No output -- enter probe phase
    LDI r23, 1
    LDI r22, 0
    JMP sd_do_probe

sd_phase1:
    ; Phase 1/2: probe + drain for 30 frames
    CMPI r22, 30
    BLT r22, startup_drain
    ; Give up -- proceed anyway
    JMP startup_done

sd_do_probe:
    LDI r20, SEND_BUF
    STRO r20, "[probing...]"
    CALL write_str_to_buf
    CALL render
    FRAME
    LDI r20, SEND_BUF
    STRO r20, "echo ready\n"
    LDI r6, SEND_BUF
    LDI r7, 11
    PTYWRITE r28, r6, r7
    LDI r23, 2
    LDI r22, 0
    JMP startup_drain

startup_done:
    LDI r1, 1

; =========================================
; MAIN LOOP
; =========================================
main_loop:
    LDI r1, 1

    ; Drain pty into text buffer
    LDI r6, RECV_BUF
    LDI r7, 512
    PTYREAD r28, r6, r7
    MOV r8, r0
    CMPI r8, 0
    JZ r0, after_drain
    LDI r7, 0xFFFFFFFF
    CMP r8, r7
    JZ r0, pty_closed

    LDI r9, 0
append_loop:
    CMP r9, r8
    BGE r0, after_drain
    LDI r20, RECV_BUF
    ADD r20, r9
    LOAD r5, r20
    CALL process_byte
    ADD r9, r1
    JMP append_loop

pty_closed:
    LDI r20, STATUS_CONNECTED
    LDI r0, 0
    STORE r20, r0
    LDI r20, DIRTY_STATUS
    LDI r0, 1
    STORE r20, r0
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

    ; Translate key and send to PTY
    CALL translate_key
    CMPI r0, 0
    JZ r0, main_loop

    ; Send bytes from SEND_BUF
    LDI r6, SEND_BUF
    PTYWRITE r28, r6, r0
    JMP main_loop

; =========================================
; PROCESS_BYTE -- ANSI state machine
; r5 = byte from PTY
; =========================================
process_byte:
    PUSH r31
    LDI r1, 1

    LDI r20, ANSI_STATE
    LOAD r4, r20

    CMPI r4, ANS_NORMAL
    JNZ r0, pb_check_esc

    CMPI r5, 27
    JNZ r0, pb_normal_byte

    LDI r20, ANSI_STATE
    LDI r0, ANS_ESC
    STORE r20, r0
    JMP pb_ret

pb_normal_byte:
    ; Check for UTF-8 multi-byte start
    CMPI r5, 192
    BLT r0, pb_ascii
    CMPI r5, 224
    BGE r0, pb_utf8_3_start

    ; 2-byte UTF-8 (0xC0-0xDF): cp = (r5 & 0x1F) << 6
    LDI r7, 31
    AND r5, r7
    LDI r7, 6
    SHL r5, r7
    LDI r20, UTF8_CP
    STORE r20, r5
    LDI r20, ANSI_STATE
    LDI r0, ANS_UTF8_2
    STORE r20, r0
    JMP pb_ret

pb_utf8_3_start:
    CMPI r5, 240
    BGE r0, pb_ret
    ; 3-byte UTF-8 (0xE0-0xEF): cp = (r5 & 0x0F) << 12
    LDI r7, 15
    AND r5, r7
    LDI r7, 12
    SHL r5, r7
    LDI r20, UTF8_CP
    STORE r20, r5
    LDI r20, ANSI_STATE
    LDI r0, ANS_UTF8_3A
    STORE r20, r0
    JMP pb_ret

pb_ascii:
    CALL append_byte
    JMP pb_ret

pb_check_esc:
    CMPI r4, ANS_ESC
    JNZ r0, pb_check_csi

    CMPI r5, 91
    JNZ r0, pb_esc_check_osc

    LDI r20, ANSI_STATE
    LDI r0, ANS_CSI
    STORE r20, r0
    JMP pb_ret

pb_esc_check_osc:
    CMPI r5, 93
    JNZ r0, pb_esc_other

    LDI r20, ANSI_STATE
    LDI r0, ANS_OSC
    STORE r20, r0
    JMP pb_ret

pb_esc_other:
    LDI r20, ANSI_STATE
    LDI r0, ANS_NORMAL
    STORE r20, r0
    JMP pb_ret

pb_check_csi:
    CMPI r4, ANS_CSI
    JNZ r0, pb_check_osc

    ; Check if this is a final byte (>= 0x40)
    CMPI r5, 64
    BLT r0, pb_csi_param

    ; Final byte arrived -- dispatch based on letter
    LDI r20, ANSI_STATE
    LDI r0, ANS_NORMAL
    STORE r20, r0

    ; 'm' (109) = SGR color
    CMPI r5, 109
    JNZ r0, pb_csi_k
    CALL apply_sgr
    JMP pb_ret

pb_csi_k:
    ; 'K' (75) = erase in line
    CMPI r5, 75
    JNZ r0, pb_csi_j
    CALL csi_erase_line
    JMP pb_ret

pb_csi_j:
    ; 'J' (74) = erase in display
    CMPI r5, 74
    JNZ r0, pb_csi_h
    CALL csi_erase_display
    JMP pb_ret

pb_csi_h:
    ; 'H' (72) = cursor position (CSI_PARAM2=row, CSI_PARAM=col)
    CMPI r5, 72
    JNZ r0, pb_csi_a
    CALL csi_cursor_position
    JMP pb_ret

pb_csi_a:
    ; 'A' (65) = cursor up
    CMPI r5, 65
    JNZ r0, pb_csi_b
    CALL csi_cursor_up
    JMP pb_ret

pb_csi_b:
    ; 'B' (66) = cursor down
    CMPI r5, 66
    JNZ r0, pb_csi_c
    CALL csi_cursor_down
    JMP pb_ret

pb_csi_c:
    ; 'C' (67) = cursor right
    CMPI r5, 67
    JNZ r0, pb_csi_d
    CALL csi_cursor_right
    JMP pb_ret

pb_csi_d:
    ; 'D' (68) = cursor left
    CMPI r5, 68
    JNZ r0, pb_csi_unknown
    CALL csi_cursor_left
    JMP pb_ret

pb_csi_unknown:
    ; Unknown CSI final byte -- ignore
    JMP pb_ret

pb_csi_param:
    ; Check for digit (0x30-0x39)
    CMPI r5, 48
    BLT r0, pb_csi_semi
    CMPI r5, 58
    BGE r0, pb_csi_semi

    ; Accumulate digit: CSI_PARAM = CSI_PARAM * 10 + (r5 - 48)
    LDI r20, CSI_PARAM
    LOAD r0, r20
    LDI r6, 10
    MUL r0, r6
    LDI r6, 48
    SUB r5, r6
    ADD r0, r5
    STORE r20, r0
    JMP pb_ret

pb_csi_semi:
    ; Semicolon (0x3B)
    ; For SGR: apply current param (so multi-color like ESC[1;32m works)
    ; For cursor: save param for later (CSI_PARAM2 = row before ';')
    CMPI r5, 59
    JNZ r0, pb_ret

    ; Save CSI_PARAM to CSI_PARAM2 for cursor sequences
    LDI r20, CSI_PARAM
    LOAD r0, r20
    LDI r7, CSI_PARAM2
    STORE r7, r0

    ; Also apply as SGR (harmless for non-SGR sequences)
    CALL apply_sgr

    ; Reset CSI_PARAM for next digit
    LDI r20, CSI_PARAM
    LDI r0, 0
    STORE r20, r0
    JMP pb_ret

pb_check_osc:
    CMPI r4, ANS_OSC
    JNZ r0, pb_check_utf8_2

    CMPI r5, 7
    JZ r0, pb_osc_end
    CMPI r5, 27
    JNZ r0, pb_osc_continue
    LDI r20, ANSI_STATE
    LDI r0, ANS_ESC
    STORE r20, r0
    JMP pb_ret

pb_osc_continue:
    LDI r20, OSC_LEN
    LOAD r0, r20
    LDI r7, 80
    CMP r0, r7
    BGE r0, pb_ret
    LDI r7, OSC_BUF
    ADD r7, r0
    STORE r7, r5
    ADD r0, r1
    STORE r20, r0
    JMP pb_ret

pb_osc_end:
    CALL process_osc
    LDI r20, ANSI_STATE
    LDI r0, ANS_NORMAL
    STORE r20, r0
    JMP pb_ret

pb_check_utf8_2:
    CMPI r4, ANS_UTF8_2
    JNZ r0, pb_check_utf8_3a

    ; Validate continuation byte (0x80-0xBF)
    CMPI r5, 128
    BLT r0, pb_reset_state
    CMPI r5, 192
    BGE r0, pb_reset_state

    ; cp |= (r5 & 0x3F)
    LDI r7, 63
    AND r5, r7
    LDI r20, UTF8_CP
    LOAD r0, r20
    OR r0, r5
    STORE r20, r0

    ; Map codepoint to extended font index and append
    CALL map_codepoint
    LDI r20, ANSI_STATE
    LDI r0, ANS_NORMAL
    STORE r20, r0
    JMP pb_ret

pb_check_utf8_3a:
    CMPI r4, ANS_UTF8_3A
    JNZ r0, pb_check_utf8_3b

    ; Validate continuation byte
    CMPI r5, 128
    BLT r0, pb_reset_state
    CMPI r5, 192
    BGE r0, pb_reset_state

    ; cp |= (r5 & 0x3F) << 6
    LDI r7, 63
    AND r5, r7
    LDI r7, 6
    SHL r5, r7
    LDI r20, UTF8_CP
    LOAD r0, r20
    OR r0, r5
    STORE r20, r0

    LDI r20, ANSI_STATE
    LDI r0, ANS_UTF8_3B
    STORE r20, r0
    JMP pb_ret

pb_check_utf8_3b:
    CMPI r4, ANS_UTF8_3B
    JNZ r0, pb_reset_state

    ; Validate continuation byte
    CMPI r5, 128
    BLT r0, pb_reset_state
    CMPI r5, 192
    BGE r0, pb_reset_state

    ; cp |= (r5 & 0x3F)
    LDI r7, 63
    AND r5, r7
    LDI r20, UTF8_CP
    LOAD r0, r20
    OR r0, r5
    STORE r20, r0

    ; Map codepoint to extended font index and append
    CALL map_codepoint
    LDI r20, ANSI_STATE
    LDI r0, ANS_NORMAL
    STORE r20, r0
    JMP pb_ret

pb_reset_state:
    LDI r20, ANSI_STATE
    LDI r0, ANS_NORMAL
    STORE r20, r0

pb_ret:
    POP r31
    RET

; =========================================
; APPEND_BYTE -- append r5 to text buffer
; =========================================
append_byte:
    PUSH r31
    LDI r1, 1

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
    CMPI r5, 32
    BLT r0, ab_ret
    ; Accept 32-126 (ASCII printable)
    CMPI r5, 127
    BLT r0, ab_store
    ; Reject 127 (DEL)
    CMPI r5, 127
    JZ r0, ab_ret
    ; Accept 128-157 (extended box-drawing)
    CMPI r5, 158
    BGE r0, ab_ret

ab_store:
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

    ; color_buf[row*COLS + col] = FG_COLOR
    LDI r20, COLOR_BUF
    ADD r20, r2
    LDI r6, FG_COLOR
    LOAD r6, r6
    STORE r20, r6

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
; MAP_CODEPOINT -- map Unicode codepoint to extended font byte
; Reads UTF8_CP, maps to 128-157 range, calls append_byte
; Unknown codepoints render as '?' (63)
; CJK full-width (0x3000-0x9FFF) rendered as full block (148)
; =========================================
map_codepoint:
    PUSH r31
    LDI r1, 1

    LDI r20, UTF8_CP
    LOAD r5, r20

    ; Box drawing U+2500-U+257F
    CMPI r5, 9472
    BLT r0, mc_check_arrows
    CMPI r5, 9600
    BGE r0, mc_check_blocks

    ; Map U+2500-257F to 128+
    ; 0x2500 = 9472, 128 + (cp - 9472) but only for known chars
    LDI r7, 9472
    SUB r5, r7
    ; r5 = offset from U+2500

    ; U+2500 (0) = 128 (horiz)
    CMPI r5, 0
    JNZ r0, mc_2502
    LDI r5, 128
    CALL append_byte
    JMP mc_ret

mc_2502:
    CMPI r5, 2
    JNZ r0, mc_250c
    LDI r5, 129
    CALL append_byte
    JMP mc_ret

mc_250c:
    CMPI r5, 12
    JNZ r0, mc_2510
    LDI r5, 130
    CALL append_byte
    JMP mc_ret

mc_2510:
    CMPI r5, 16
    JNZ r0, mc_2514
    LDI r5, 131
    CALL append_byte
    JMP mc_ret

mc_2514:
    CMPI r5, 20
    JNZ r0, mc_2518
    LDI r5, 132
    CALL append_byte
    JMP mc_ret

mc_2518:
    CMPI r5, 24
    JNZ r0, mc_251c
    LDI r5, 133
    CALL append_byte
    JMP mc_ret

mc_251c:
    CMPI r5, 28
    JNZ r0, mc_2524
    LDI r5, 134
    CALL append_byte
    JMP mc_ret

mc_2524:
    CMPI r5, 36
    JNZ r0, mc_252c
    LDI r5, 135
    CALL append_byte
    JMP mc_ret

mc_252c:
    CMPI r5, 44
    JNZ r0, mc_2534
    LDI r5, 136
    CALL append_byte
    JMP mc_ret

mc_2534:
    CMPI r5, 52
    JNZ r0, mc_253c
    LDI r5, 137
    CALL append_byte
    JMP mc_ret

mc_253c:
    CMPI r5, 60
    JNZ r0, mc_2550
    LDI r5, 138
    CALL append_byte
    JMP mc_ret

mc_2550:
    ; U+2550 = 9552, offset = 80
    CMPI r5, 80
    JNZ r0, mc_2551
    LDI r5, 155
    CALL append_byte
    JMP mc_ret

mc_2551:
    CMPI r5, 81
    JNZ r0, mc_box_unknown
    LDI r5, 156
    CALL append_byte
    JMP mc_ret

mc_box_unknown:
    ; Unknown box-drawing char, render as '?'
    LDI r5, 63
    CALL append_byte
    JMP mc_ret

mc_check_arrows:
    ; U+2190-U+2193 (arrows)
    CMPI r5, 8592
    JNZ r0, mc_check_sym
    LDI r5, 139
    CALL append_byte
    JMP mc_ret

mc_check_sym:
    CMPI r5, 8594
    JNZ r0, mc_check_up
    LDI r5, 140
    CALL append_byte
    JMP mc_ret

mc_check_up:
    CMPI r5, 8593
    JNZ r0, mc_check_down
    LDI r5, 141
    CALL append_byte
    JMP mc_ret

mc_check_down:
    CMPI r5, 8595
    JNZ r0, mc_check_sym2
    LDI r5, 142
    CALL append_byte
    JMP mc_ret

mc_check_sym2:
    ; U+2713 (check) = 10003
    CMPI r5, 10003
    JNZ r0, mc_check_sym3
    LDI r5, 143
    CALL append_byte
    JMP mc_ret

mc_check_sym3:
    ; U+2717 (ballot X) = 10007
    CMPI r5, 10007
    JNZ r0, mc_check_bullet
    LDI r5, 144
    CALL append_byte
    JMP mc_ret

mc_check_bullet:
    ; U+2022 (bullet) = 8226
    CMPI r5, 8226
    JNZ r0, mc_check_middot
    LDI r5, 145
    CALL append_byte
    JMP mc_ret

mc_check_middot:
    ; U+00B7 (middot) = 183
    CMPI r5, 183
    JNZ r0, mc_check_ellipsis
    LDI r5, 146
    CALL append_byte
    JMP mc_ret

mc_check_ellipsis:
    ; U+2026 (ellipsis) = 8230
    CMPI r5, 8230
    JNZ r0, mc_check_raquo
    LDI r5, 147
    CALL append_byte
    JMP mc_ret

mc_check_raquo:
    ; U+00BB (right double angle) = 187
    CMPI r5, 187
    JNZ r0, mc_check_cjk
    LDI r5, 157
    CALL append_byte
    JMP mc_ret

mc_check_blocks:
    ; Block elements U+2580-U+259F
    CMPI r5, 9600
    BLT r0, mc_unknown
    CMPI r5, 9632
    BGE r0, mc_check_cjk

    LDI r7, 9600
    SUB r5, r7
    ; U+2580 (0) = 151 (upper half)
    CMPI r5, 0
    JNZ r0, mc_2584
    LDI r5, 151
    CALL append_byte
    JMP mc_ret

mc_2584:
    CMPI r5, 4
    JNZ r0, mc_2588
    LDI r5, 152
    CALL append_byte
    JMP mc_ret

mc_2588:
    CMPI r5, 8
    JNZ r0, mc_258c
    LDI r5, 148
    CALL append_byte
    JMP mc_ret

mc_258c:
    CMPI r5, 12
    JNZ r0, mc_258e
    LDI r5, 149
    CALL append_byte
    JMP mc_ret

mc_258e:
    CMPI r5, 14
    JNZ r0, mc_2590
    LDI r5, 153
    CALL append_byte
    JMP mc_ret

mc_2590:
    CMPI r5, 16
    JNZ r0, mc_258a
    LDI r5, 150
    CALL append_byte
    JMP mc_ret

mc_258a:
    CMPI r5, 10
    JNZ r0, mc_block_unknown
    LDI r5, 154
    CALL append_byte
    JMP mc_ret

mc_block_unknown:
    LDI r5, 148
    CALL append_byte
    JMP mc_ret

mc_check_cjk:
    ; CJK Unified Ideographs U+3000-U+9FFF -- render as double-width full block
    CMPI r5, 12288
    BLT r0, mc_unknown
    CMPI r5, 40960
    BGE r0, mc_unknown

    ; Full-width char: insert full block, advance col by 2
    LDI r5, 148
    CALL append_byte
    CALL append_byte
    JMP mc_ret

mc_unknown:
    ; Unknown codepoint, render as '?'
    LDI r5, 63
    CALL append_byte

mc_ret:
    POP r31
    RET

; =========================================
; TRANSLATE_KEY
; =========================================
translate_key:
    PUSH r31
    LDI r1, 1

    CMPI r5, 32
    BLT r0, tk_special
    CMPI r5, 127
    BGE r0, tk_special
    LDI r20, SEND_BUF
    STORE r20, r5
    LDI r0, 1
    JMP tk_ret

tk_special:
    CMPI r5, 13
    JNZ r0, tk_bs
    LDI r20, SEND_BUF
    LDI r0, 10
    STORE r20, r0
    LDI r0, 1
    JMP tk_ret

tk_bs:
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
    LDI r20, SEND_BUF
    ADD r20, r1
    LDI r0, 91
    STORE r20, r0
    LDI r20, SEND_BUF
    LDI r2, 2
    ADD r20, r2
    LDI r0, 65
    STORE r20, r0
    LDI r0, 3
    JMP tk_ret

tk_arrow_down:
    CMPI r5, KEY_DOWN
    JNZ r0, tk_arrow_right
    LDI r20, SEND_BUF
    LDI r0, 27
    STORE r20, r0
    LDI r20, SEND_BUF
    ADD r20, r1
    LDI r0, 91
    STORE r20, r0
    LDI r20, SEND_BUF
    LDI r2, 2
    ADD r20, r2
    LDI r0, 66
    STORE r20, r0
    LDI r0, 3
    JMP tk_ret

tk_arrow_right:
    CMPI r5, KEY_RIGHT
    JNZ r0, tk_arrow_left
    LDI r20, SEND_BUF
    LDI r0, 27
    STORE r20, r0
    LDI r20, SEND_BUF
    ADD r20, r1
    LDI r0, 91
    STORE r20, r0
    LDI r20, SEND_BUF
    LDI r2, 2
    ADD r20, r2
    LDI r0, 67
    STORE r20, r0
    LDI r0, 3
    JMP tk_ret

tk_arrow_left:
    CMPI r5, KEY_LEFT
    JNZ r0, tk_home
    LDI r20, SEND_BUF
    LDI r0, 27
    STORE r20, r0
    LDI r20, SEND_BUF
    ADD r20, r1
    LDI r0, 91
    STORE r20, r0
    LDI r20, SEND_BUF
    LDI r2, 2
    ADD r20, r2
    LDI r0, 68
    STORE r20, r0
    LDI r0, 3
    JMP tk_ret

tk_home:
    CMPI r5, KEY_HOME
    JNZ r0, tk_end
    LDI r20, SEND_BUF
    LDI r0, 27
    STORE r20, r0
    LDI r20, SEND_BUF
    ADD r20, r1
    LDI r0, 91
    STORE r20, r0
    LDI r20, SEND_BUF
    LDI r2, 2
    ADD r20, r2
    LDI r0, 72
    STORE r20, r0
    LDI r0, 3
    JMP tk_ret

tk_end:
    CMPI r5, KEY_END
    JNZ r0, tk_ignore
    LDI r20, SEND_BUF
    LDI r0, 27
    STORE r20, r0
    LDI r20, SEND_BUF
    ADD r20, r1
    LDI r0, 91
    STORE r20, r0
    LDI r20, SEND_BUF
    LDI r2, 2
    ADD r20, r2
    LDI r0, 70
    STORE r20, r0
    LDI r0, 3
    JMP tk_ret

tk_ignore:
    LDI r0, 0

tk_ret:
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
    LDI r6, 39
dn_store:
    STORE r20, r6
    POP r31
    RET

; =========================================
; SCROLL_UP -- shift rows up, clear last row
; =========================================
scroll_up:
    PUSH r31
    LDI r1, 1
    LDI r11, COLS
    LDI r10, 0
scroll_loop:
    CMPI r10, 39
    BGE r0, scroll_clear

    ; dst = BUF + row * COLS
    LDI r21, BUF
    MOV r0, r10
    LDI r11, COLS
    MUL r0, r11
    ADD r21, r0

    ; src = BUF + (row+1) * COLS
    LDI r20, BUF
    MOV r0, r10
    ADD r0, r1
    LDI r11, COLS
    MUL r0, r11
    ADD r20, r0

    ; MEMCPY dst, src, COLS
    MEMCPY r21, r20, r11

    ; Same for color buffer: dst = COLOR_BUF + row * COLS
    LDI r21, COLOR_BUF
    MOV r0, r10
    LDI r11, COLS
    MUL r0, r11
    ADD r21, r0

    ; src = COLOR_BUF + (row+1) * COLS
    LDI r20, COLOR_BUF
    MOV r0, r10
    ADD r0, r1
    LDI r11, COLS
    MUL r0, r11
    ADD r20, r0

    ; MEMCPY dst, src, COLS
    MEMCPY r21, r20, r11

    ADD r10, r1
    JMP scroll_loop

scroll_clear:
    ; Clear last row of text buffer using MEMCPY from a pre-zeroed region
    ; We use a trick: copy from a region we already cleared (row 0 after scroll is row 1's old data)
    ; Simpler: just use a STORE loop for the fill (MEMCPY can't fill)
    LDI r20, BUF
    LDI r6, 39
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

    ; Clear last row of color buffer with default FG color
    LDI r20, COLOR_BUF
    LDI r6, 39
    LDI r11, COLS
    MUL r6, r11
    ADD r20, r6
    LDI r6, FG_COLOR
    LOAD r6, r6
    LDI r22, 0
sc_color_loop:
    STORE r20, r6
    ADD r20, r1
    ADD r22, r1
    CMPI r22, COLS
    BLT r0, sc_color_loop

    POP r31
    RET

; =========================================
; WRITE_STR_TO_BUF
; =========================================
write_str_to_buf:
    PUSH r31
    PUSH r20
    PUSH r25
    MOV r25, r20          ; r25 = string pointer (r20 clobbered by append_byte)
    LDI r1, 1
wsb_loop:
    LOAD r5, r25
    JZ r5, wsb_done
    CALL append_byte
    ADD r25, r1
    JMP wsb_loop
wsb_done:
    POP r25
    POP r20
    POP r31
    RET

; =========================================
; PROCESS_OSC -- handle OSC sequence
; =========================================
process_osc:
    PUSH r31
    LDI r1, 1

    LDI r20, OSC_BUF
    LOAD r0, r20
    CMPI r0, 55
    JNZ r0, po_done

    LDI r20, OSC_BUF
    ADD r20, r1
    LOAD r0, r20
    CMPI r0, 59
    JNZ r0, po_done

    LDI r10, 0
    LDI r11, 0
po_find_path:
    LDI r20, OSC_BUF
    ADD r20, r10
    LOAD r0, r20
    JZ r0, po_done
    CMPI r0, 47
    JNZ r0, po_next
    ADD r11, r1
    CMPI r11, 3
    BGE r0, po_copy_path
po_next:
    ADD r10, r1
    LDI r7, 80
    CMP r10, r7
    BLT r0, po_find_path
    JMP po_done

po_copy_path:
    LDI r12, 0
po_cp_loop:
    LDI r20, OSC_BUF
    ADD r20, r10
    LOAD r0, r20
    JZ r0, po_cp_done
    LDI r7, STATUS_CWD
    ADD r7, r12
    STORE r7, r0
    ADD r10, r1
    ADD r12, r1
    LDI r7, 60
    CMP r12, r7
    BLT r0, po_cp_loop
po_cp_done:
    LDI r7, STATUS_CWD
    ADD r7, r12
    LDI r0, 0
    STORE r7, r0
    LDI r20, STATUS_CWD_LEN
    STORE r20, r12

po_done:
    LDI r20, OSC_LEN
    LDI r0, 0
    STORE r20, r0
    ; Mark status bar dirty (CWD may have changed)
    LDI r20, DIRTY_STATUS
    LDI r0, 1
    STORE r20, r0
    POP r31
    RET

; =========================================
; DRAW_STATUS_BAR
; =========================================
draw_status_bar:
    PUSH r31
    LDI r1, 1

    LDI r1, 0
    LDI r2, 0
    LDI r3, 256
    LDI r4, 10
    LDI r5, 0x1A1A2E
    RECTF r1, r2, r3, r4, r5

    LDI r1, 1
    LDI r20, SCRATCH
    STRO r20, "bash: "
    LDI r20, SCRATCH
    LDI r10, 0
dsb_find_end:
    LDI r7, SCRATCH
    ADD r7, r10
    LOAD r0, r7
    JZ r0, dsb_append_cwd
    ADD r10, r1
    JMP dsb_find_end

dsb_append_cwd:
    LDI r20, STATUS_CWD_LEN
    LOAD r0, r20
    JZ r0, dsb_no_cwd

    LDI r11, 0
dsb_cp:
    LDI r7, STATUS_CWD
    ADD r7, r11
    LOAD r0, r7
    JZ r0, dsb_cp_done
    LDI r7, SCRATCH
    ADD r7, r10
    STORE r7, r0
    ADD r10, r1
    ADD r11, r1
    LDI r7, 38
    CMP r10, r7
    BLT r0, dsb_cp
    JMP dsb_cp_done

dsb_no_cwd:
    LDI r7, SCRATCH
    ADD r7, r10
    LDI r0, 126
    STORE r7, r0
    ADD r10, r1

dsb_cp_done:
    LDI r7, SCRATCH
    ADD r7, r10
    LDI r0, 0
    STORE r7, r0

    LDI r1, 2
    LDI r2, 2
    LDI r3, SCRATCH
    LDI r4, 0x44DD44
    LDI r5, 0x1A1A2E
    SMALLTEXT r1, r2, r3, r4, r5

    LDI r20, STATUS_CONNECTED
    LOAD r0, r20
    JZ r0, dsb_disconnected

    LDI r20, SCRATCH
    LDI r0, 42
    STORE r20, r0
    LDI r0, 0
    ADD r20, r1
    STORE r20, r0
    LDI r1, 240
    LDI r2, 2
    LDI r3, SCRATCH
    LDI r4, 0x44FF44
    LDI r5, 0x1A1A2E
    SMALLTEXT r1, r2, r3, r4, r5
    JMP dsb_ret

dsb_disconnected:
    LDI r20, SCRATCH
    LDI r0, 88
    STORE r20, r0
    LDI r0, 0
    ADD r20, r1
    STORE r20, r0
    LDI r1, 240
    LDI r2, 2
    LDI r3, SCRATCH
    LDI r4, 0xFF4444
    LDI r5, 0x1A1A2E
    SMALLTEXT r1, r2, r3, r4, r5

dsb_ret:
    POP r31
    RET

; =========================================
; APPLY_SGR -- apply SGR code from CSI_PARAM to FG_COLOR
; Reads CSI_PARAM from RAM, updates FG_COLOR in RAM.
; Preserves all registers except r0 (CMP clobbers it).
; =========================================
apply_sgr:
    PUSH r31
    LDI r1, 1

    LDI r20, CSI_PARAM
    LOAD r6, r20       ; r6 = SGR code

    ; 0 = reset (default light gray)
    CMPI r6, 0
    JNZ r0, sgr_bold
    LDI r20, FG_COLOR
    LDI r0, 0xBBBBBB
    STORE r20, r0
    JMP sgr_ret

sgr_bold:
    ; 1 = bold (brighten -- just use bright white)
    CMPI r6, 1
    JNZ r0, sgr_black
    LDI r20, FG_COLOR
    LDI r0, 0xFFFFFF
    STORE r20, r0
    JMP sgr_ret

sgr_black:
    ; 30 = black
    CMPI r6, 30
    JNZ r0, sgr_red
    LDI r20, FG_COLOR
    LDI r0, 0x555555
    STORE r20, r0
    JMP sgr_ret

sgr_red:
    CMPI r6, 31
    JNZ r0, sgr_green
    LDI r20, FG_COLOR
    LDI r0, 0xCD0000
    STORE r20, r0
    JMP sgr_ret

sgr_green:
    CMPI r6, 32
    JNZ r0, sgr_yellow
    LDI r20, FG_COLOR
    LDI r0, 0x00CD00
    STORE r20, r0
    JMP sgr_ret

sgr_yellow:
    CMPI r6, 33
    JNZ r0, sgr_blue
    LDI r20, FG_COLOR
    LDI r0, 0xCDCD00
    STORE r20, r0
    JMP sgr_ret

sgr_blue:
    CMPI r6, 34
    JNZ r0, sgr_magenta
    LDI r20, FG_COLOR
    LDI r0, 0x0000EE
    STORE r20, r0
    JMP sgr_ret

sgr_magenta:
    CMPI r6, 35
    JNZ r0, sgr_cyan
    LDI r20, FG_COLOR
    LDI r0, 0xCD00CD
    STORE r20, r0
    JMP sgr_ret

sgr_cyan:
    CMPI r6, 36
    JNZ r0, sgr_white
    LDI r20, FG_COLOR
    LDI r0, 0x00CDCD
    STORE r20, r0
    JMP sgr_ret

sgr_white:
    CMPI r6, 37
    JNZ r0, sgr_bright_black
    LDI r20, FG_COLOR
    LDI r0, 0xE5E5E5
    STORE r20, r0
    JMP sgr_ret

sgr_bright_black:
    CMPI r6, 90
    JNZ r0, sgr_bright_red
    LDI r20, FG_COLOR
    LDI r0, 0x808080
    STORE r20, r0
    JMP sgr_ret

sgr_bright_red:
    CMPI r6, 91
    JNZ r0, sgr_bright_green
    LDI r20, FG_COLOR
    LDI r0, 0xFF0000
    STORE r20, r0
    JMP sgr_ret

sgr_bright_green:
    CMPI r6, 92
    JNZ r0, sgr_bright_yellow
    LDI r20, FG_COLOR
    LDI r0, 0x00FF00
    STORE r20, r0
    JMP sgr_ret

sgr_bright_yellow:
    CMPI r6, 93
    JNZ r0, sgr_bright_blue
    LDI r20, FG_COLOR
    LDI r0, 0xFFFF00
    STORE r20, r0
    JMP sgr_ret

sgr_bright_blue:
    CMPI r6, 94
    JNZ r0, sgr_bright_magenta
    LDI r20, FG_COLOR
    LDI r0, 0x5C5CFF
    STORE r20, r0
    JMP sgr_ret

sgr_bright_magenta:
    CMPI r6, 95
    JNZ r0, sgr_bright_cyan
    LDI r20, FG_COLOR
    LDI r0, 0xFF00FF
    STORE r20, r0
    JMP sgr_ret

sgr_bright_cyan:
    CMPI r6, 96
    JNZ r0, sgr_bright_white
    LDI r20, FG_COLOR
    LDI r0, 0x00FFFF
    STORE r20, r0
    JMP sgr_ret

sgr_bright_white:
    CMPI r6, 97
    JNZ r0, sgr_ret
    LDI r20, FG_COLOR
    LDI r0, 0xFFFFFF
    STORE r20, r0

sgr_ret:
    ; Reset CSI_PARAM for next sequence
    LDI r20, CSI_PARAM
    LDI r0, 0
    STORE r20, r0

    POP r31
    RET

; =========================================
; CSI_ERASE_LINE -- 'K' handler
; CSI_PARAM: 0=cursor to EOL (default), 1=BOL to cursor, 2=whole line
; =========================================
csi_erase_line:
    PUSH r31
    LDI r1, 1

    LDI r20, CSI_PARAM
    LOAD r6, r20       ; r6 = mode (0, 1, or 2)

    ; Get current row
    LDI r20, CUR_ROW
    LOAD r7, r20       ; r7 = row
    LDI r8, COLS
    MUL r7, r8         ; r7 = row * COLS

    ; Get current col
    LDI r20, CUR_COL
    LOAD r9, r20       ; r9 = col

    ; Determine start and end columns
    ; mode 0: col..COLS-1 (default)
    ; mode 1: 0..col
    ; mode 2: 0..COLS-1
    LDI r10, 0         ; r10 = start_col
    MOV r11, r9        ; r11 = end_col (exclusive)
    CMPI r6, 0
    JZ r0, cel_clear
    CMPI r6, 1
    JNZ r0, cel_mode2
    ; mode 1: 0..col+1
    LDI r10, 0
    ADD r11, r1        ; col+1
    JMP cel_clear
cel_mode2:
    ; mode 2: 0..COLS
    LDI r10, 0
    LDI r11, COLS

cel_clear:
    ; Clear text cells from start_col to end_col
    MOV r12, r10
cel_loop:
    CMP r12, r11
    BGE r0, cel_done
    LDI r20, BUF
    ADD r20, r7
    ADD r20, r12
    LDI r0, 32
    STORE r20, r0
    ; Clear color cell too
    LDI r20, COLOR_BUF
    LDI r0, 0
    ADD r0, r7
    ADD r0, r12
    LDI r13, FG_COLOR
    LOAD r13, r13
    ADD r20, r0
    STORE r20, r13
    ADD r12, r1
    JMP cel_loop

cel_done:
    LDI r20, DIRTY_STATUS
    LDI r0, 1
    STORE r20, r0

    POP r31
    RET

; =========================================
; CSI_ERASE_DISPLAY -- 'J' handler
; CSI_PARAM: 0=cursor to end (default), 1=start to cursor, 2=whole screen
; =========================================
csi_erase_display:
    PUSH r31
    LDI r1, 1

    LDI r20, CSI_PARAM
    LOAD r6, r20       ; r6 = mode

    CMPI r6, 2
    JNZ r0, ced_check0

    ; Mode 2: clear entire screen
    LDI r20, BUF
    LDI r6, 32
    LDI r10, 0
ced_full:
    STORE r20, r6
    ADD r20, r1
    ADD r10, r1
    LDI r7, ROWS
    LDI r8, COLS
    MUL r7, r8
    CMP r10, r7
    BLT r0, ced_full

    ; Clear color buffer
    LDI r20, COLOR_BUF
    LDI r6, FG_COLOR
    LOAD r6, r6
    LDI r10, 0
ced_full_color:
    STORE r20, r6
    ADD r20, r1
    ADD r10, r1
    LDI r7, ROWS
    LDI r8, COLS
    MUL r7, r8
    CMP r10, r7
    BLT r0, ced_full_color

    ; Reset cursor
    LDI r20, CUR_COL
    LDI r0, 0
    STORE r20, r0
    LDI r20, CUR_ROW
    LDI r0, 0
    STORE r20, r0
    JMP ced_done

ced_check0:
    ; Mode 0: clear from cursor to end of screen
    ; Mode 1: clear from start to cursor
    ; For now just implement mode 0 (most common -- used by 'clear')
    ; Clear current row from cursor to end
    LDI r20, CUR_COL
    LOAD r9, r20
    LDI r20, CUR_ROW
    LOAD r10, r20
    LDI r11, COLS
    MUL r10, r11
    ADD r10, r9        ; offset = row*COLS + col

    LDI r7, ROWS
    LDI r8, COLS
    MUL r7, r8         ; total cells
ced_loop0:
    CMP r10, r7
    BGE r0, ced_done
    LDI r20, BUF
    ADD r20, r10
    LDI r0, 32
    STORE r20, r0
    LDI r20, COLOR_BUF
    ADD r20, r10
    LDI r6, FG_COLOR
    LOAD r6, r6
    STORE r20, r6
    ADD r10, r1
    JMP ced_loop0

ced_done:
    LDI r20, DIRTY_STATUS
    LDI r0, 1
    STORE r20, r0

    POP r31
    RET

; =========================================
; CSI_CURSOR_POSITION -- 'H' handler
; CSI_PARAM2 = row (1-based, 0 or omitted = 1)
; CSI_PARAM  = col (1-based, 0 or omitted = 1)
; =========================================
csi_cursor_position:
    PUSH r31
    LDI r1, 1

    ; Row: CSI_PARAM2, default 0 means row 1
    LDI r20, CSI_PARAM2
    LOAD r6, r20
    CMPI r6, 0
    JNZ r0, ccp_row_set
    LDI r6, 1
ccp_row_set:
    SUB r6, r1         ; convert to 0-based
    CMPI r6, 0
    BGE r0, ccp_row_clamp
    LDI r6, 0
ccp_row_clamp:
    LDI r7, ROWS
    SUB r7, r1
    CMP r6, r7
    BLT r0, ccp_row_ok
    MOV r6, r7
ccp_row_ok:
    LDI r20, CUR_ROW
    STORE r20, r6

    ; Col: CSI_PARAM, default 0 means col 1
    LDI r20, CSI_PARAM
    LOAD r6, r20
    CMPI r6, 0
    JNZ r0, ccp_col_set
    LDI r6, 1
ccp_col_set:
    SUB r6, r1         ; convert to 0-based
    CMPI r6, 0
    BGE r0, ccp_col_clamp
    LDI r6, 0
ccp_col_clamp:
    LDI r7, COLS
    SUB r7, r1
    CMP r6, r7
    BLT r0, ccp_col_ok
    MOV r6, r7
ccp_col_ok:
    LDI r20, CUR_COL
    STORE r20, r6

    POP r31
    RET

; =========================================
; CSI cursor movement A/B/C/D
; CSI_PARAM = count (default 1)
; =========================================
csi_cursor_up:
    PUSH r31
    LDI r1, 1
    LDI r20, CSI_PARAM
    LOAD r6, r20
    CMPI r6, 0
    JNZ r0, ccu_move
    LDI r6, 1
ccu_move:
    LDI r20, CUR_ROW
    LOAD r7, r20
    SUB r7, r6
    CMPI r7, 0
    BGE r0, ccu_ok
    LDI r7, 0
ccu_ok:
    STORE r20, r7
    POP r31
    RET

csi_cursor_down:
    PUSH r31
    LDI r1, 1
    LDI r20, CSI_PARAM
    LOAD r6, r20
    CMPI r6, 0
    JNZ r0, ccd_move
    LDI r6, 1
ccd_move:
    LDI r20, CUR_ROW
    LOAD r7, r20
    ADD r7, r6
    LDI r8, ROWS
    SUB r8, r1
    CMP r7, r8
    BLT r0, ccd_ok
    MOV r7, r8
ccd_ok:
    STORE r20, r7
    POP r31
    RET

csi_cursor_right:
    PUSH r31
    LDI r1, 1
    LDI r20, CSI_PARAM
    LOAD r6, r20
    CMPI r6, 0
    JNZ r0, ccr_move
    LDI r6, 1
ccr_move:
    LDI r20, CUR_COL
    LOAD r7, r20
    ADD r7, r6
    LDI r8, COLS
    SUB r8, r1
    CMP r7, r8
    BLT r0, ccr_ok
    MOV r7, r8
ccr_ok:
    STORE r20, r7
    POP r31
    RET

csi_cursor_left:
    PUSH r31
    LDI r1, 1
    LDI r20, CSI_PARAM
    LOAD r6, r20
    CMPI r6, 0
    JNZ r0, ccl_move
    LDI r6, 1
ccl_move:
    LDI r20, CUR_COL
    LOAD r7, r20
    SUB r7, r6
    CMPI r7, 0
    BGE r0, ccl_ok
    LDI r7, 0
ccl_ok:
    STORE r20, r7
    POP r31
    RET

; =========================================
; RENDER -- redraw text buffer using SMALLTEXT (3x5 font)
; Terminal area starts at y=10, 6px per row (5px glyph + 1px spacing)
; 40 rows * 6px = 240px. Total: 10 + 240 = 250px (fits in 256px)
; Now uses color-run rendering from COLOR_BUF
; =========================================
render:
    PUSH r31
    LDI r1, 1

    ; Only redraw status bar when dirty
    LDI r20, DIRTY_STATUS
    LOAD r0, r20
    JZ r0, render_skip_status
    CALL draw_status_bar
    LDI r20, DIRTY_STATUS
    LDI r0, 0
    STORE r20, r0

render_skip_status:

    ; Clear content area
    LDI r1, 0
    LDI r2, 10
    LDI r3, 256
    LDI r4, 246
    LDI r5, 0x0A0A0A
    RECTF r1, r2, r3, r4, r5

    LDI r1, 1
    LDI r10, 0
    LDI r12, 10
    LDI r7, COLS

render_row:
    ; Compute row base addresses
    MOV r11, r10
    MUL r11, r7
    LDI r25, BUF
    ADD r25, r11
    LDI r26, COLOR_BUF
    ADD r26, r11

    LDI r20, 0

render_run:
    ; Check if past end of row
    CMP r20, r7
    BGE r0, end_row

    ; Read run-start color
    MOV r27, r26
    ADD r27, r20
    LOAD r22, r27

    ; Scan for end of same-color run
    MOV r21, r20

rr_scan:
    CMP r21, r7
    BGE r0, rr_emit

    MOV r27, r26
    ADD r27, r21
    LOAD r23, r27
    CMP r23, r22
    JNZ r0, rr_emit

    ADD r21, r1
    JMP rr_scan

rr_emit:
    ; Copy run chars to scratch buffer
    LDI r23, 0
    MOV r24, r20

rr_copy:
    CMP r24, r21
    BGE r0, rr_copy_done
    MOV r27, r25
    ADD r27, r24
    LOAD r6, r27
    LDI r27, SCRATCH
    ADD r27, r23
    STORE r27, r6
    ADD r23, r1
    ADD r24, r1
    JMP rr_copy

rr_copy_done:
    ; Null-terminate scratch
    LDI r27, SCRATCH
    ADD r27, r23
    LDI r6, 0
    STORE r27, r6

    ; Skip empty runs
    CMP r23, r6
    JZ r0, rr_next

    ; Compute x pixel pos (col * 3)
    MOV r14, r20
    LDI r6, 3
    MUL r14, r6
    MOV r13, r12
    LDI r3, SCRATCH
    MOV r4, r22
    LDI r5, 0
    SMALLTEXT r14, r13, r3, r4, r5

rr_next:
    MOV r20, r21
    JMP render_run

end_row:
    LDI r6, 6
    ADD r12, r6

    ADD r10, r1
    CMPI r10, ROWS
    BLT r0, render_row

    ; Cursor blink
    LDI r20, BLINK
    LOAD r0, r20
    LDI r7, 8
    AND r0, r7
    CMPI r0, 4
    BGE r0, cursor_done

draw_cursor:
    LDI r20, CUR_COL
    LOAD r0, r20
    LDI r7, 3
    MUL r0, r7
    LDI r20, CUR_ROW
    LOAD r2, r20
    LDI r7, 6
    MUL r2, r7
    LDI r3, 10
    ADD r2, r3
    LDI r3, 2
    LDI r4, 5
    LDI r5, 0x44FF44
    RECTF r0, r2, r3, r4, r5

cursor_done:
    POP r31
    RET
