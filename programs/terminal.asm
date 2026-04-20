; terminal.asm -- Interactive Terminal for Geometry OS (v3 with command dispatch)
;
; Self-contained GUI app: draw, input, render loop.
; Supports 4 builtin commands: clear, help, ver, hi
;
; RAM Layout:
;   0x4000-0x44EB  Text buffer (42*30 = 1260 u32 cells, row-major)
;   0x4800         Cursor column
;   0x4801         Cursor row
;   0x4802         Blink counter
;   0x5000-0x502A  Scratch line buffer (42 chars + null)

#define COLS 42
#define ROWS 30
#define BUF 0x4000
#define BUF_END 0x44EC
#define CUR_COL 0x4800
#define CUR_ROW 0x4801
#define BLINK 0x4802
#define SCRATCH 0x5000

; =========================================
; INIT
; =========================================
LDI r1, 1
LDI r30, 0xFD00   ; Initialize stack pointer (r30=SP) to high RAM

; Clear screen
LDI r0, 0x0C0C0C
FILL r0

; Clear text buffer to spaces
LDI r20, BUF
LDI r0, 32
clear_buf:
    STORE r20, r0
    ADD r20, r1
    CMPI r20, BUF_END
    BLT r20, clear_buf

; Init cursor and blink to 0
LDI r20, CUR_COL
LDI r0, 0
STORE r20, r0
LDI r20, CUR_ROW
STORE r20, r0
LDI r20, BLINK
STORE r20, r0

; Title bar
LDI r1, 0
LDI r2, 0
LDI r3, 256
LDI r4, 16
LDI r5, 0x333355
RECTF r1, r2, r3, r4, r5

; Title text "GeoTerm" -- use STRO + TEXT
LDI r20, SCRATCH
STRO r20, "GeoTerm"
LDI r1, 4
LDI r2, 4
LDI r3, SCRATCH
TEXT r1, r2, r3

; Close button hit region
LDI r1, 220
LDI r2, 0
LDI r3, 36
LDI r4, 16
HITSET r1, r2, r3, r4, 99

; Restore r1 = 1 before writing prompt!
LDI r1, 1

; Write prompt "$ " at buffer row 0
LDI r20, BUF
LDI r0, 36           ; '$'
STORE r20, r0
ADD r20, r1
LDI r0, 32           ; ' '
STORE r20, r0

; Set cursor to col 2
LDI r20, CUR_COL
LDI r0, 2
STORE r20, r0

; =========================================
; MAIN LOOP
; =========================================
main_loop:
    LDI r1, 1

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
    LDI r5, 0x0C0C0C
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

    ; TEXT x=0, y=r12, addr=SCRATCH
    LDI r1, 0
    LDI r13, SCRATCH
    TEXT r1, r12, r13

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
; DO_ENTER -- command dispatch
; =========================================
do_enter:
    ; 1. Extract command text from current row into SCRATCH
    CALL extract_cmd

    ; 2. Advance to next row for output
    CALL do_newline

    ; 3. Try matching commands (dispatch_cmd writes output rows)
    CALL dispatch_cmd

    ; 4. Write prompt "$ " on the row after any output
    LDI r1, 1
    LDI r20, CUR_ROW
    LOAD r2, r20
    LDI r3, COLS
    MUL r2, r3
    LDI r20, BUF
    ADD r20, r2
    LDI r0, 36           ; '$'
    STORE r20, r0
    ADD r20, r1
    LDI r0, 32           ; ' '
    STORE r20, r0
    LDI r20, CUR_COL
    LDI r0, 2
    STORE r20, r0
    JMP hk_ret

; =========================================
; EXTRACT_CMD
; Copy chars from BUF[row*COLS+2 .. row*COLS+col-1] into SCRATCH
; Null terminate. Skips leading spaces.
; =========================================
extract_cmd:
    PUSH r31
    LDI r1, 1

    ; Compute base = row * COLS
    LDI r20, CUR_ROW
    LOAD r6, r20          ; r6 = row
    LDI r7, COLS
    MUL r6, r7            ; r6 = row * COLS

    ; Source starts at col 2 (skip "$ ")
    LDI r20, BUF
    ADD r20, r6           ; r20 = BUF + row*COLS
    ADD r20, r1
    ADD r20, r1           ; r20 = BUF + row*COLS + 2

    ; Destination
    LDI r21, SCRATCH

    ; Get end position (cursor col)
    LDI r20, CUR_COL
    LOAD r7, r20          ; r7 = cursor col

    ; Recompute source pointer
    LDI r20, BUF
    ADD r20, r6           ; r20 = BUF + row*COLS
    ADD r20, r1
    ADD r20, r1           ; r20 = BUF + row*COLS + 2

    ; Copy loop: copy chars from col 2 to cursor col
    LDI r22, 2            ; current column index
ec_loop:
    ; If col_index >= cursor_col, done
    CMP r22, r7
    BGE r0, ec_done

    ; Load char from source
    LOAD r0, r20
    ; Store to scratch
    STORE r21, r0

    ADD r20, r1           ; advance source
    ADD r21, r1           ; advance dest
    ADD r22, r1           ; col++
    JMP ec_loop

ec_done:
    ; Null terminate
    LDI r0, 0
    STORE r21, r0

    POP r31
    RET

; =========================================
; DISPATCH_CMD
; Match SCRATCH against builtin commands.
; If match found, write output to current row and newline.
; If no match and input not empty, write "?" and newline.
; =========================================
dispatch_cmd:
    PUSH r31
    LDI r1, 1

    ; --- Try "clear" ---
    LDI r20, SCRATCH
    LOAD r22, r20
    CMPI r22, 99         ; 'c'
    JNZ r0, try_help
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 108        ; 'l'
    JNZ r0, try_help
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 101        ; 'e'
    JNZ r0, try_help
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 97         ; 'a'
    JNZ r0, try_help
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 114        ; 'r'
    JNZ r0, try_help
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null terminator
    JNZ r0, try_help
    ; MATCH: clear
    JMP cmd_clear

try_help:
    ; --- Try "help" ---
    LDI r20, SCRATCH
    LOAD r22, r20
    CMPI r22, 104        ; 'h'
    JNZ r0, try_ver
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 101        ; 'e'
    JNZ r0, try_ver
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 108        ; 'l'
    JNZ r0, try_ver
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 112        ; 'p'
    JNZ r0, try_ver
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null
    JNZ r0, try_ver
    ; MATCH: help
    JMP cmd_help

try_ver:
    ; --- Try "ver" ---
    LDI r20, SCRATCH
    LOAD r22, r20
    CMPI r22, 118        ; 'v'
    JNZ r0, try_hi
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 101        ; 'e'
    JNZ r0, try_hi
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 114        ; 'r'
    JNZ r0, try_hi
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null
    JNZ r0, try_hi
    ; MATCH: ver
    JMP cmd_ver

try_hi:
    ; --- Try "hi" ---
    LDI r20, SCRATCH
    LOAD r22, r20
    CMPI r22, 104        ; 'h'
    JNZ r0, try_unknown
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 105        ; 'i'
    JNZ r0, try_unknown
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null
    JNZ r0, try_unknown
    ; MATCH: hi
    JMP cmd_hi

try_unknown:
    ; Check if input is empty (first char is null)
    LDI r20, SCRATCH
    LOAD r22, r20
    JZ r22, dc_ret       ; empty input, no output

    ; Unknown command: write "? <cmd>"
    CALL write_unknown
    JMP dc_ret

; =========================================
; COMMANDS
; =========================================

cmd_clear:
    ; Clear the text buffer to spaces, reset cursor to row 0, col 2
    LDI r1, 1
    LDI r20, BUF
    LDI r0, 32
cc_clear:
    STORE r20, r0
    ADD r20, r1
    CMPI r20, BUF_END
    BLT r20, cc_clear

    ; Reset cursor to row 0, col 2
    LDI r20, CUR_ROW
    LDI r0, 0
    STORE r20, r0
    LDI r20, CUR_COL
    LDI r0, 2
    STORE r20, r0

    ; Write prompt on row 0
    LDI r20, BUF
    LDI r0, 36           ; '$'
    STORE r20, r0
    ADD r20, r1
    LDI r0, 32           ; ' '
    STORE r20, r0

    JMP dc_ret

cmd_help:
    ; Write "cmds: clear help ver hi" to current row
    LDI r1, 1
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7
    LDI r20, BUF
    ADD r20, r6
    STRO r20, "cmds: clear help ver hi"
    CALL do_newline
    JMP dc_ret

cmd_ver:
    ; Write "GeoTerm v1.0" to current row
    LDI r1, 1
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7
    LDI r20, BUF
    ADD r20, r6
    STRO r20, "GeoTerm v1.0"
    CALL do_newline
    JMP dc_ret

cmd_hi:
    ; Write "hello!" to current row
    LDI r1, 1
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7
    LDI r20, BUF
    ADD r20, r6
    STRO r20, "hello!"
    CALL do_newline
    JMP dc_ret

write_unknown:
    ; Write "? " followed by the command text from SCRATCH to current row
    PUSH r31
    LDI r1, 1
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7
    LDI r20, BUF
    ADD r20, r6           ; r20 = BUF + row*COLS (destination)

    ; Write "? "
    LDI r0, 63            ; '?'
    STORE r20, r0
    ADD r20, r1
    LDI r0, 32            ; ' '
    STORE r20, r0
    ADD r20, r1

    ; Copy SCRATCH to rest of row
    LDI r21, SCRATCH
wu_loop:
    LOAD r0, r21
    JZ r0, wu_done
    STORE r20, r0
    ADD r20, r1
    ADD r21, r1
    JMP wu_loop

wu_done:
    CALL do_newline
    POP r31
    RET

dc_ret:
    POP r31
    RET

; =========================================
; DO_BACKSPACE
; =========================================
do_backspace:
    LDI r20, CUR_COL
    LOAD r0, r20
    JZ r0, hk_ret
    SUB r0, r1
    STORE r20, r0
    ; Clear char
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

do_newline:
    LDI r1, 1
    LDI r20, CUR_COL
    LDI r0, 0
    STORE r20, r0
    LDI r20, CUR_ROW
    LOAD r0, r20
    ADD r0, r1
    STORE r20, r0
    RET

hk_ret:
    POP r31
    RET
