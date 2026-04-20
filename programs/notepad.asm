; notepad.asm -- Multi-line Text Editor for Geometry OS
;
; Proves: TEXT rendering, IKEY keyboard input, cursor management,
;         RECTF UI elements, RAM-based state, FRAME loop, HITSET.
;
; Features:
;   - Type text characters (printable ASCII 32-126)
;   - Backspace to delete characters
;   - Enter to insert new lines
;   - Arrow keys to navigate cursor (left/right/up/down)
;   - Line numbers on the left margin
;   - Title bar with "GeoPad" header
;   - Blinking cursor indicator
;   - Character and line count display
;
; RAM Layout:
;   0x4000-0x5AFF  Text buffer (48*32 = 1536 u32 cells, row-major)
;                   48 cols x 32 rows of editable text
;   0x6000         Cursor column (0-47)
;   0x6001         Cursor row (0-31)
;   0x6002         Total lines used (count of non-empty lines)
;   0x6003         Blink counter (for cursor blink)
;   0x6100-0x6130  Scratch buffer for rendering one line (48 chars + null)
;   0x6200-0x6230  Scratch buffer for line number text (4 chars + null)
;
; Key codes:
;   8  = Backspace
;   13 = Enter
;   37 = Left arrow
;   38 = Up arrow
;   39 = Right arrow
;   40 = Down arrow
;   32-126 = Printable ASCII
;
; Screen layout (256x256):
;   Row 0-15:    Title bar (dark blue background)
;   Row 16-255:  Text editing area (dark gray background)
;   Col 0-39:    Line numbers margin (darker gray)
;   Col 42-255:  Text area

#define COLS    36         ; editable columns per line
#define ROWS    30         ; visible text rows
#define BUF     0x4000     ; text buffer start
#define CUR_COL 0x6000     ; cursor column
#define CUR_ROW 0x6001     ; cursor row
#define LINES   0x6002     ; total lines count
#define BLINK   0x6003     ; blink counter
#define SCRATCH 0x6100     ; line render scratch
#define LNSCR   0x6200     ; line number scratch
#define CHAR_H  8          ; character height in pixels
#define CHAR_W  6          ; character width in pixels
#define TITLE_H 16         ; title bar height
#define MARGIN_W 42        ; line number margin width

; =========================================
; INIT
; =========================================
LDI r1, 1
LDI r30, 0xFD00

; Clear text buffer to spaces (32)
LDI r20, BUF
LDI r6, 32
LDI r7, COLS
LDI r8, ROWS
clear_buf_row:
    ; clear one row (COLS spaces)
    LDI r21, 0
clear_buf_col:
    STORE r20, r6
    ADD r20, r1
    ADD r21, r1
    CMP r21, r7
    BLT r0, clear_buf_col
    ; next row
    SUB r8, r1
    JZ r8, clear_buf_done
    JMP clear_buf_row
clear_buf_done:

; Init cursor position
LDI r20, CUR_COL
LDI r2, 0
STORE r20, r2
LDI r20, CUR_ROW
STORE r20, r2

; Init lines count = 1 (start with one empty line)
LDI r20, LINES
LDI r2, 1
STORE r20, r2

; Init blink counter
LDI r20, BLINK
STORE r20, r2

; =========================================
; MAIN LOOP
; =========================================
main_loop:
    LDI r1, 1

    ; Draw everything
    CALL render

    ; Signal frame
    FRAME

    ; Read keyboard
    IKEY r5
    JZ r5, main_loop

    ; Handle special keys
    CMPI r5, 8
    JZ r0, do_backspace
    CMPI r5, 13
    JZ r0, do_enter
    CMPI r5, 37
    JZ r0, do_left
    CMPI r5, 38
    JZ r0, do_up
    CMPI r5, 39
    JZ r0, do_right
    CMPI r5, 40
    JZ r0, do_down

    ; Printable character (32-126)?
    CMPI r5, 32
    BLT r0, main_loop
    CMPI r5, 127
    BGE r0, main_loop

    ; Insert character at cursor
    CALL insert_char
    JMP main_loop

; =========================================
; INSERT CHARACTER
; =========================================
insert_char:
    PUSH r31
    LDI r1, 1

    ; Compute buffer address: BUF + cur_row * COLS + cur_col
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7
    LDI r20, CUR_COL
    LOAD r8, r20
    ADD r6, r8
    LDI r20, BUF
    ADD r20, r6

    ; Store the character
    STORE r20, r5

    ; Advance cursor column
    ADD r8, r1
    CMPI r8, COLS
    BLT r0, no_wrap
    ; Wrap to next line
    LDI r8, 0
    LDI r20, CUR_COL
    STORE r20, r8
    ; Advance row
    LDI r20, CUR_ROW
    LOAD r6, r20
    ADD r6, r1
    CMPI r6, ROWS
    BLT r0, no_wrap_row_limit
    LDI r6, ROWS
    SUB r6, r1
no_wrap_row_limit:
    STORE r20, r6
    ; Update lines count if needed
    CALL update_lines
    POP r31
    RET
no_wrap:
    LDI r20, CUR_COL
    STORE r20, r8
    POP r31
    RET

; =========================================
; DO BACKSPACE
; =========================================
do_backspace:
    PUSH r31
    LDI r1, 1

    ; Load cursor position
    LDI r20, CUR_COL
    LOAD r6, r20
    LDI r20, CUR_ROW
    LOAD r7, r20

    ; If col > 0, just move back and clear
    CMPI r6, 0
    JZ r6, bs_at_line_start

    ; Move cursor left
    SUB r6, r1
    LDI r20, CUR_COL
    STORE r20, r6

    ; Clear character at new position
    CALL clear_cursor_pos
    POP r31
    RET

bs_at_line_start:
    ; At start of line -- if row > 0, join with previous line
    CMPI r7, 0
    JZ r7, bs_done

    ; Move to end of previous line
    SUB r7, r1
    LDI r20, CUR_ROW
    STORE r20, r7

    ; Find last non-space character on previous line
    LDI r8, COLS
    SUB r8, r1           ; start from last col
find_end:
    ; Compute buf addr for (row, col)
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r9, COLS
    MUL r6, r9
    ADD r6, r8
    LDI r20, BUF
    ADD r20, r6
    LOAD r10, r20        ; load char
    CMPI r10, 32
    JNZ r0, found_end   ; non-space = end of content
    CMPI r8, 0
    JZ r8, found_end     ; at col 0 = empty line
    SUB r8, r1
    JMP find_end

found_end:
    ADD r8, r1           ; cursor goes one past last char
    LDI r20, CUR_COL
    STORE r20, r8
bs_done:
    POP r31
    RET

; =========================================
; DO ENTER
; =========================================
do_enter:
    PUSH r31
    LDI r1, 1

    ; Load cursor position
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r20, CUR_COL
    LOAD r7, r20

    ; If at last row, don't advance
    LDI r8, ROWS
    SUB r8, r1
    CMP r6, r8
    BGE r0, enter_done

    ; Move to start of next row
    ADD r6, r1
    LDI r20, CUR_ROW
    STORE r20, r6

    ; Clear cursor column
    LDI r7, 0
    LDI r20, CUR_COL
    STORE r20, r7

    ; Clear the new line to spaces
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r9, COLS
    MUL r6, r9
    LDI r20, BUF
    ADD r20, r6
    LDI r21, 0
clear_new_line:
    LDI r10, 32
    STORE r20, r10
    ADD r20, r1
    ADD r21, r1
    CMPI r21, COLS
    BLT r0, clear_new_line

    ; Update lines count
    CALL update_lines
enter_done:
    POP r31
    RET

; =========================================
; DO LEFT
; =========================================
do_left:
    PUSH r31
    LDI r1, 1
    LDI r20, CUR_COL
    LOAD r6, r20
    CMPI r6, 0
    JZ r6, left_done
    SUB r6, r1
    STORE r20, r6
left_done:
    POP r31
    RET

; =========================================
; DO RIGHT
; =========================================
do_right:
    PUSH r31
    LDI r1, 1
    LDI r20, CUR_COL
    LOAD r6, r20
    LDI r8, COLS
    SUB r8, r1
    CMP r6, r8
    BGE r0, right_done
    ADD r6, r1
    STORE r20, r6
right_done:
    POP r31
    RET

; =========================================
; DO UP
; =========================================
do_up:
    PUSH r31
    LDI r1, 1
    LDI r20, CUR_ROW
    LOAD r6, r20
    CMPI r6, 0
    JZ r6, up_done
    SUB r6, r1
    STORE r20, r6
up_done:
    POP r31
    RET

; =========================================
; DO DOWN
; =========================================
do_down:
    PUSH r31
    LDI r1, 1
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r8, ROWS
    SUB r8, r1
    CMP r6, r8
    BGE r0, down_done
    ADD r6, r1
    STORE r20, r6
down_done:
    POP r31
    RET

; =========================================
; CLEAR CHARACTER AT CURSOR POSITION
; =========================================
clear_cursor_pos:
    ; Expects r1 = 1
    ; Computes buffer addr from CUR_ROW and CUR_COL, writes space (32)
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7
    LDI r20, CUR_COL
    LOAD r8, r20
    ADD r6, r8
    LDI r20, BUF
    ADD r20, r6
    LDI r10, 32
    STORE r20, r10
    RET

; =========================================
; UPDATE LINES COUNT
; =========================================
update_lines:
    ; Count non-empty lines from bottom up
    ; Expects r1 = 1
    LDI r6, 0            ; count = 0
    LDI r7, ROWS
    SUB r7, r1           ; start from last row
count_lines:
    ; Compute buf addr for row r7
    LDI r20, 0
    ADD r20, r7
    LDI r8, COLS
    MUL r20, r8
    LDI r21, BUF
    ADD r21, r20

    ; Check if any non-space char in this row
    LDI r22, 0
check_line:
    LOAD r23, r21
    CMPI r23, 32
    JNZ r0, line_has_content
    ADD r21, r1
    ADD r22, r1
    CMPI r22, COLS
    BLT r0, check_line
    ; All spaces -- empty line, check if we already found content
    CMPI r6, 0
    JNZ r6, count_done   ; already found content above, stop
    ; No content yet, keep looking up
    SUB r7, r1
    CMPI r7, 0
    BGE r0, count_lines
    JMP count_done

line_has_content:
    ADD r6, r1
    SUB r7, r1
    CMPI r7, 0
    BGE r0, count_lines

count_done:
    CMPI r6, 0
    JNZ r6, has_lines
    LDI r6, 1            ; at least 1 line
has_lines:
    LDI r20, LINES
    STORE r20, r6
    RET

; =========================================
; RENDER
; =========================================
render:
    PUSH r31
    LDI r1, 1

    ; ── Title bar ──
    LDI r2, 0x16213E
    FILL r2

    ; Title bar background
    LDI r1, 0
    LDI r2, 0
    LDI r3, 256
    LDI r4, TITLE_H
    LDI r5, 0x16213E
    RECTF r1, r2, r3, r4, r5

    ; Title text: "GeoPad"
    LDI r20, SCRATCH
    STRO r20, "GeoPad v1.0"
    LDI r2, 0
    STORE r20, r2
    LDI r1, 8
    LDI r2, 4
    LDI r3, SCRATCH
    TEXT r1, r2, r3

    ; ── Text area background ──
    LDI r1, 0
    LDI r2, TITLE_H
    LDI r3, 256
    LDI r4, 240
    LDI r5, 0x1A1A2E
    RECTF r1, r2, r3, r4, r5

    ; ── Line number margin ──
    LDI r1, 0
    LDI r2, TITLE_H
    LDI r3, MARGIN_W
    LDI r4, 240
    LDI r5, 0x141428
    RECTF r1, r2, r3, r4, r5

    ; ── Render text lines ──
    LDI r1, 1             ; restore r1 (RECTF clobbers it to 0)
    LDI r10, 0           ; row counter
    LDI r12, TITLE_H     ; y position

render_row:
    ; Draw line number -- use r14,r15 for intermediate math to preserve r12 (y-pos)
    LDI r20, LNSCR
    ; Convert row number (r10) to 2-char decimal string
    LDI r6, 10
    LDI r11, 0
    ADD r11, r10
    ADD r11, r1           ; r11 = row+1 (1-indexed)
    DIV r11, r6           ; r11 = (row+1) / 10 (tens digit)
    LDI r2, 48
    ADD r2, r11
    STORE r20, r2         ; LNSCR[0] = '0' + tens
    ADD r20, r1
    LDI r14, 0
    ADD r14, r11          ; r14 = tens digit
    LDI r2, 10
    MUL r14, r2           ; r14 = tens * 10
    LDI r13, 0
    ADD r13, r10
    ADD r13, r1           ; r13 = row+1
    SUB r13, r14          ; r13 = (row+1) % 10 (ones digit)
    LDI r2, 48
    ADD r2, r13
    STORE r20, r2         ; LNSCR[1] = '0' + ones
    ADD r20, r1
    LDI r2, 32            ; space separator
    STORE r20, r2         ; LNSCR[2] = ' '
    ADD r20, r1
    LDI r2, 0
    STORE r20, r2         ; LNSCR[3] = null

    ; Render line number at x=2, y=r12
    LDI r1, 2
    LDI r3, LNSCR
    TEXT r1, r12, r3

    ; Render text line from buffer
    ; Copy COLS chars from buffer to scratch
    LDI r1, 1             ; restore r1 (TEXT x-pos set it to 2)
    LDI r20, SCRATCH
    LDI r21, 0
    ; Compute buffer address for this row
    LDI r22, BUF
    LDI r23, COLS
    LDI r24, 0
    ADD r24, r10
    MUL r24, r23
    ADD r22, r24

copy_line:
    LOAD r6, r22
    STORE r20, r6
    ADD r22, r1
    ADD r20, r1
    ADD r21, r1
    CMPI r21, COLS
    BLT r0, copy_line

    ; Null terminate
    LDI r2, 0
    STORE r20, r2         ; null terminate scratch

    ; Render text at x=MARGIN_W, y=r12
    LDI r1, MARGIN_W
    LDI r3, SCRATCH
    TEXT r1, r12, r3

    ; Advance y
    LDI r1, 1
    LDI r2, CHAR_H
    ADD r12, r2

    ; Next row
    ADD r10, r1
    CMPI r10, ROWS
    BLT r0, render_row

    ; ── Draw cursor ──
    CALL draw_cursor

    ; ── Status bar ──
    CALL draw_status

    POP r31
    RET

; =========================================
; DRAW CURSOR (blinking underline)
; =========================================
draw_cursor:
    ; Expects r1 = 1
    ; Read blink counter and toggle
    LDI r20, BLINK
    LOAD r6, r20
    ADD r6, r1
    STORE r20, r6

    ; Cursor visible on even counts (blink every 30 frames)
    LDI r7, 30
    LDI r8, 0
    ADD r8, r6
    DIV r8, r7
    MOD r8, r7
    CMPI r8, 0
    JNZ r8, cursor_done

    ; Load cursor position
    LDI r20, CUR_COL
    LOAD r6, r20
    LDI r20, CUR_ROW
    LOAD r7, r20

    ; Compute screen position
    ; x = MARGIN_W + cur_col * CHAR_W
    LDI r1, 1
    LDI r2, CHAR_W
    MUL r6, r2
    LDI r3, MARGIN_W
    ADD r6, r3           ; r6 = x

    ; y = TITLE_H + cur_row * CHAR_H + (CHAR_H - 2)
    LDI r2, CHAR_H
    MUL r7, r2
    LDI r3, TITLE_H
    ADD r7, r3           ; r7 = base y
    LDI r3, CHAR_H
    SUB r3, r1
    SUB r3, r1           ; CHAR_H - 2
    ADD r7, r3           ; r7 = y (near bottom of character cell)

    ; Draw cursor as white underline (width=CHAR_W, height=2)
    LDI r4, CHAR_W
    LDI r5, 2
    LDI r8, 0xFFFFFF
    RECTF r6, r7, r4, r5, r8

cursor_done:
    RET

; =========================================
; DRAW STATUS BAR
; =========================================
draw_status:
    ; Expects r1 = 1
    PUSH r31
    ; Status bar at bottom of screen
    LDI r1, 0
    LDI r2, 248
    LDI r3, 256
    LDI r4, 8
    LDI r5, 0x0D0D1A
    RECTF r1, r2, r3, r4, r5
    LDI r1, 1             ; restore r1 (RECTF clobbered it to 0)

    ; Show "Lines: N  Row: R  Col: C"
    LDI r20, SCRATCH
    STRO r20, "Ln:"
    ADD r20, r1

    ; Lines count (2-digit)
    LDI r25, LINES
    LOAD r6, r25
    CALL two_digit
    ADD r20, r1
    ADD r20, r1

    STRO r20, " R:"
    ADD r20, r1
    ADD r20, r1

    ; Current row (2-digit)
    LDI r25, CUR_ROW
    LOAD r6, r25
    CALL two_digit
    ADD r20, r1
    ADD r20, r1

    STRO r20, " C:"
    ADD r20, r1
    ADD r20, r1

    ; Current col (2-digit)
    LDI r25, CUR_COL
    LOAD r6, r25
    CALL two_digit
    ADD r20, r1
    ADD r20, r1

    LDI r2, 0
    STORE r20, r2

    ; Render at x=4, y=250
    LDI r1, 4
    LDI r2, 250
    LDI r3, SCRATCH
    TEXT r1, r2, r3

    POP r31
    RET

; =========================================
; TWO_DIGIT: Convert r6 (0-99) to 2 ASCII digits at SCRATCH
; =========================================
two_digit:
    ; r6 = value
    ; Writes 2 chars starting at address in r20, does NOT null-terminate
    ; Expects r1 = 1
    LDI r21, 10
    LDI r22, 0
    ADD r22, r6
    DIV r22, r21          ; tens
    LDI r2, 48
    ADD r2, r22
    STORE r20, r2
    ADD r20, r1

    LDI r23, 0
    ADD r23, r6
    MOD r23, r21          ; ones
    LDI r2, 48
    ADD r2, r23
    STORE r20, r2
    RET
