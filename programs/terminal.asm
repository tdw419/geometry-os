; terminal.asm -- Interactive Terminal for Geometry OS (v5 with VFS cat + 13 commands)
;
; Self-contained GUI app: draw, input, render loop.
; Commands: clear, cls, help, ver, hi, echo, ls, date, cat, sys,
;           colors, whoami, uname, uptime
; Auto-scrolls when content exceeds 30 rows
;
; RAM Layout:
;   0x4000-0x44EB  Text buffer (42*30 = 1260 u32 cells, row-major)
;   0x4800         Cursor column
;   0x4801         Cursor row
;   0x4802         Blink counter
;   0x5000-0x502A  Scratch line buffer (42 chars + null)
;   0x5200-0x52C8  Cat file buffer (200 u32s for cat command)

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
LDI r6, 32
clear_buf:
    STORE r20, r6
    ADD r20, r1
    CMPI r20, BUF_END
    BLT r0, clear_buf

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

; Title text "GeoTerm" -- use STRO + DRAWTEXT (green on dark title bar)
LDI r20, SCRATCH
STRO r20, "GeoTerm"
LDI r1, 4
LDI r2, 4
LDI r3, SCRATCH
LDI r4, 0x00FF00  ; fg = green
LDI r5, 0x333355  ; bg = match title bar
DRAWTEXT r1, r2, r3, r4, r5

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

    ; DRAWTEXT x=0, y=r12, addr=SCRATCH, fg=white, bg=0 (transparent)
    LDI r1, 0
    LDI r13, SCRATCH
    LDI r14, 0xCCCCCC  ; fg = light gray
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
    JNZ r0, try_echo
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 105        ; 'i'
    JNZ r0, try_echo
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null
    JNZ r0, try_echo
    ; MATCH: hi
    JMP cmd_hi

try_echo:
    ; --- Try "echo " (5 chars: e,c,h,o,space) ---
    LDI r20, SCRATCH
    LOAD r22, r20
    CMPI r22, 101        ; 'e'
    JNZ r0, try_ls
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 99         ; 'c'
    JNZ r0, try_ls
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 104        ; 'h'
    JNZ r0, try_ls
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 111        ; 'o'
    JNZ r0, try_ls
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 32         ; ' ' (space after echo)
    JNZ r0, try_ls
    JMP cmd_echo

try_ls:
    ; --- Try "ls" ---
    LDI r20, SCRATCH
    LOAD r22, r20
    CMPI r22, 108        ; 'l'
    JNZ r0, try_date
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 115        ; 's'
    JNZ r0, try_date
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null
    JNZ r0, try_date
    JMP cmd_ls

try_date:
    ; --- Try "date" ---
    LDI r20, SCRATCH
    LOAD r22, r20
    CMPI r22, 100        ; 'd'
    JNZ r0, try_cls
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 97         ; 'a'
    JNZ r0, try_cls
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 116        ; 't'
    JNZ r0, try_cls
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 101        ; 'e'
    JNZ r0, try_cls
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null
    JNZ r0, try_cls
    JMP cmd_date

try_cls:
    ; --- Try "cls" (alias for clear) ---
    LDI r20, SCRATCH
    LOAD r22, r20
    CMPI r22, 99         ; 'c'
    JNZ r0, try_unknown
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 108        ; 'l'
    JNZ r0, try_unknown
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 115        ; 's'
    JNZ r0, try_unknown
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null
    JNZ r0, try_unknown
    JMP cmd_clear

try_cat:
    ; --- Try "cat " (4 chars: c,a,t,space) ---
    LDI r20, SCRATCH
    LOAD r22, r20
    CMPI r22, 99         ; 'c'
    JNZ r0, try_sys
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 97         ; 'a'
    JNZ r0, try_sys
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 116        ; 't'
    JNZ r0, try_sys
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 32         ; ' ' (space after cat)
    JNZ r0, try_sys
    JMP cmd_cat

try_sys:
    ; --- Try "sys" ---
    LDI r20, SCRATCH
    LOAD r22, r20
    CMPI r22, 115        ; 's'
    JNZ r0, try_colors
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 121        ; 'y'
    JNZ r0, try_colors
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 115        ; 's'
    JNZ r0, try_colors
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null
    JNZ r0, try_colors
    JMP cmd_sys

try_colors:
    ; --- Try "colors" ---
    LDI r20, SCRATCH
    LOAD r22, r20
    CMPI r22, 99         ; 'c'
    JNZ r0, try_whoami
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 111        ; 'o'
    JNZ r0, try_whoami
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 108        ; 'l'
    JNZ r0, try_whoami
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 111        ; 'o'
    JNZ r0, try_whoami
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 114        ; 'r'
    JNZ r0, try_whoami
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 115        ; 's'
    JNZ r0, try_whoami
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null
    JNZ r0, try_whoami
    JMP cmd_colors

try_whoami:
    ; --- Try "whoami" ---
    LDI r20, SCRATCH
    LOAD r22, r20
    CMPI r22, 119        ; 'w'
    JNZ r0, try_uname
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 104        ; 'h'
    JNZ r0, try_uname
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 111        ; 'o'
    JNZ r0, try_uname
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 97         ; 'a'
    JNZ r0, try_uname
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 109        ; 'm'
    JNZ r0, try_uname
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 105        ; 'i'
    JNZ r0, try_uname
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null
    JNZ r0, try_uname
    JMP cmd_whoami

try_uname:
    ; --- Try "uname" ---
    LDI r20, SCRATCH
    LOAD r22, r20
    CMPI r22, 117        ; 'u'
    JNZ r0, try_uptime
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 110        ; 'n'
    JNZ r0, try_uptime
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 97         ; 'a'
    JNZ r0, try_uptime
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 109        ; 'm'
    JNZ r0, try_uptime
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 101        ; 'e'
    JNZ r0, try_uptime
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null
    JNZ r0, try_uptime
    JMP cmd_uname

try_uptime:
    ; --- Try "uptime" ---
    LDI r20, SCRATCH
    LOAD r22, r20
    CMPI r22, 117        ; 'u'
    JNZ r0, try_unknown
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 112        ; 'p'
    JNZ r0, try_unknown
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 116        ; 't'
    JNZ r0, try_unknown
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 105        ; 'i'
    JNZ r0, try_unknown
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 109        ; 'm'
    JNZ r0, try_unknown
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 101        ; 'e'
    JNZ r0, try_unknown
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null
    JNZ r0, try_unknown
    JMP cmd_uptime

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
    LDI r6, 32
cc_clear:
    STORE r20, r6
    ADD r20, r1
    CMPI r20, BUF_END
    BLT r0, cc_clear

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
    STRO r20, "cmds: clear cls help ver hi echo ls date cat"
    CALL do_newline
    LDI r1, 1
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7
    LDI r20, BUF
    ADD r20, r6
    STRO r20, "      sys colors whoami uname uptime"
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
    STRO r20, "GeoTerm v1.1"
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

cmd_echo:
    ; Write args (SCRATCH+5 onward, after "echo ") to current row
    PUSH r31
    LDI r1, 1
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7
    LDI r20, BUF
    ADD r20, r6           ; r20 = BUF + row*COLS

    ; Source is SCRATCH+5 (skip "echo ")
    LDI r21, SCRATCH
    ADD r21, r1           ; +1
    ADD r21, r1           ; +2
    ADD r21, r1           ; +3
    ADD r21, r1           ; +4
    ADD r21, r1           ; +5

echo_loop:
    LOAD r0, r21
    JZ r0, echo_done
    STORE r20, r0
    ADD r20, r1
    ADD r21, r1
    JMP echo_loop

echo_done:
    CALL do_newline
    POP r31
    RET

cmd_ls:
    ; List VFS directory using LS opcode
    PUSH r31
    LDI r1, 1
    LDI r20, 0x5100       ; LS output buffer
    LS r20                ; list files, count in r0
    CMPI r0, 0
    JZ r0, ls_empty

    ; Read entries from buffer, write one per row
    LDI r21, 0x5100
ls_next:
    LOAD r0, r21
    JZ r0, ls_done        ; end of entries

    ; Write entry to current row
    LDI r22, CUR_ROW
    LOAD r6, r22
    LDI r7, COLS
    MUL r6, r7
    LDI r23, BUF
    ADD r23, r6

    ; Copy string from r21 to r23
ls_copy:
    LOAD r0, r21
    JZ r0, ls_copied
    STORE r23, r0
    ADD r21, r1
    ADD r23, r1
    JMP ls_copy

ls_copied:
    ADD r21, r1           ; skip null terminator
    CALL do_newline
    JMP ls_next

ls_empty:
    LDI r1, 1
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7
    LDI r20, BUF
    ADD r20, r6
    STRO r20, "(empty)"
    CALL do_newline
    POP r31
    RET

ls_done:
    POP r31
    RET

cmd_date:
    ; Display date string
    LDI r1, 1
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7
    LDI r20, BUF
    ADD r20, r6
    STRO r20, "2026-04-20"
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

; =========================================
; CAT command -- read VFS file and display
; Uses OPEN/READ/CLOSE opcodes
; File path is in SCRATCH+4 (after "cat ")
; =========================================
cmd_cat:
    PUSH r31
    LDI r1, 1

    ; Build filename address: SCRATCH+4 points to filename
    LDI r21, SCRATCH
    ADD r21, r1
    ADD r21, r1
    ADD r21, r1
    ADD r21, r1           ; r21 = SCRATCH+4 (filename)

    ; OPEN filename (r21), flags=0 (read), mode=0
    ; OPEN takes (path_reg, flags_reg, mode_reg)
    LDI r20, 0
    OPEN r21, r20, r20
    ; fd returned in r0
    MOV r18, r0           ; r18 = fd
    CMPI r0, 0xFFFFFFFF
    JNZ r0, cat_read      ; if not -1, file opened OK

    ; Error: file not found
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7
    LDI r20, BUF
    ADD r20, r6
    STRO r20, "(file not found)"
    CALL do_newline
    POP r31
    RET

cat_read:
    ; Read file content into buffer at 0x5200
    ; READ fd(r18), buf(r22=0x5200), len(r23=200)
    LDI r22, 0x5200       ; CAT_BUF
    LDI r23, 200          ; max words to read
    READ r18, r22, r23
    ; r0 = bytes read (0 = EOF, 0xFFFFFFFF = error)
    CMPI r0, 0
    JZ r0, cat_close      ; EOF or empty
    CMPI r0, 0xFFFFFFFF
    JNZ r0, cat_close     ; error

    ; Write chars from buffer until null or 200 chars
    LDI r22, 0x5200
    LDI r24, 0            ; word counter
cat_write_loop:
    CMPI r24, 200
    BGE r0, cat_close
    LOAD r0, r22
    JZ r0, cat_close      ; null terminator
    ; Write char to current terminal row
    CALL cat_write_char
    ADD r22, r1
    ADD r24, r1
    JMP cat_write_loop

cat_close:
    CLOSE r18
    POP r31
    RET

cat_write_char:
    ; Write r0 (char) at current cursor position in buffer
    ; Advance cursor, handle line wrapping via do_newline
    PUSH r31
    LDI r1, 1
    LDI r20, CUR_ROW
    LOAD r2, r20
    LDI r7, COLS
    MUL r2, r7
    LDI r20, CUR_COL
    LOAD r3, r20
    ADD r2, r3
    LDI r20, BUF
    ADD r20, r2
    STORE r20, r0

    ; Advance cursor col
    LDI r20, CUR_COL
    LOAD r3, r20
    ADD r3, r1
    CMPI r3, COLS
    JNZ r0, cwc_done      ; branch if not equal (r0 = CMPI result)
    CALL do_newline
    JMP cwc_ret
cwc_done:
    LDI r20, CUR_COL
    STORE r20, r3
cwc_ret:
    POP r31
    RET

; =========================================
; SYS command -- show system info
; =========================================
cmd_sys:
    LDI r1, 1
    ; Line 1: "Geometry OS v2.0"
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7
    LDI r20, BUF
    ADD r20, r6
    STRO r20, "Geometry OS v2.0"
    CALL do_newline
    ; Line 2: "Opcodes: 113"
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7
    LDI r20, BUF
    ADD r20, r6
    STRO r20, "Opcodes: 113  Programs: 72"
    CALL do_newline
    ; Line 3: "Tests: 1795"
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7
    LDI r20, BUF
    ADD r20, r6
    STRO r20, "Tests: 1795  Canvas: 256x256"
    CALL do_newline
    JMP dc_ret

; =========================================
; COLORS command -- display color swatches
; Draws colored rectangles on the canvas
; =========================================
cmd_colors:
    LDI r1, 1
    ; Row 1 label
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7
    LDI r20, BUF
    ADD r20, r6
    STRO r20, "Color swatches drawn:"
    CALL do_newline
    ; Draw 6 color swatches across the screen
    ; Red
    LDI r1, 0
    LDI r2, 0
    LDI r3, 42
    LDI r4, 42
    LDI r5, 0xFF0000
    RECTF r1, r2, r3, r4, r5
    ; Green
    LDI r1, 43
    LDI r5, 0x00FF00
    RECTF r1, r2, r3, r4, r5
    ; Blue
    LDI r1, 86
    LDI r5, 0x0000FF
    RECTF r1, r2, r3, r4, r5
    ; Yellow
    LDI r1, 129
    LDI r5, 0xFFFF00
    RECTF r1, r2, r3, r4, r5
    ; Cyan
    LDI r1, 172
    LDI r5, 0x00FFFF
    RECTF r1, r2, r3, r4, r5
    ; Magenta
    LDI r1, 215
    LDI r3, 41
    LDI r5, 0xFF00FF
    RECTF r1, r2, r3, r4, r5
    JMP dc_ret

; =========================================
; WHOAMI command
; =========================================
cmd_whoami:
    LDI r1, 1
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7
    LDI r20, BUF
    ADD r20, r6
    STRO r20, "GeoOS User"
    CALL do_newline
    JMP dc_ret

; =========================================
; UNAME command
; =========================================
cmd_uname:
    LDI r1, 1
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7
    LDI r20, BUF
    ADD r20, r6
    STRO r20, "GeometryOS 2.0 rv32imac"
    CALL do_newline
    JMP dc_ret

; =========================================
; UPTIME command -- show frame count as uptime
; =========================================
cmd_uptime:
    LDI r1, 1
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7
    LDI r20, BUF
    ADD r20, r6
    STRO r20, "uptime: "
    ; Blink counter doubles as a rough tick counter
    ; Show the value at BLINK address as a simple number
    LDI r20, BLINK
    LOAD r0, r20
    ; Write a few digits (just show raw value is too complex)
    ; Instead just show "running" indicator
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1           ; advance past "uptime: "
    STRO r20, "running"
    CALL do_newline
    JMP dc_ret

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
    LOAD r6, r20          ; r6 = current row
    ADD r6, r1            ; r6 = new row
    CMPI r6, ROWS
    BLT r0, dn_store      ; branch if new row < ROWS (r0 = CMPI result)
    PUSH r31              ; save return address across scroll call
    CALL scroll_up
    POP r31               ; restore return address
    LDI r20, CUR_ROW      ; reload after scroll clobbers r20
    LDI r6, 29            ; clamp to last row
dn_store:
    STORE r20, r6
    RET

; =========================================
; SCROLL_UP
; Shift all text rows up by 1, clear last row
; =========================================
scroll_up:
    PUSH r31
    LDI r1, 1
    LDI r10, 0            ; row counter
scroll_loop:
    CMPI r10, 29
    BGE r0, scroll_clear

    ; Source ptr: BUF + (row+1)*COLS
    LDI r20, BUF
    LDI r0, 0
    ADD r0, r10
    ADD r0, r1
    LDI r11, COLS
    MUL r0, r11
    ADD r20, r0

    ; Dest ptr: BUF + row*COLS
    LDI r21, BUF
    LDI r0, 0
    ADD r0, r10
    LDI r11, COLS
    MUL r0, r11
    ADD r21, r0

    ; Copy COLS words
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
    ; Clear last row (row 29) to spaces
    LDI r20, BUF
    LDI r6, 29
    LDI r11, COLS
    MUL r6, r11
    ADD r20, r6           ; r20 = BUF + 29*42
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

hk_ret:
    POP r31
    RET
