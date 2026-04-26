; nano_editor.asm -- Nano-like Text Editor for Geometry OS
; Phase 139 -- Daily Driver Text Editor App
;
; Opens a host file, displays with scrolling, edits with cursor keys,
; and saves via FSWRITE. Search and goto for navigation.
;
; Controls:
;   Arrow keys (bitmask 0xFFB)  -- Move cursor
;   Printable ASCII (32-126)    -- Insert character
;   Enter (10)                  -- Insert newline
;   Backspace (8)               -- Delete char before cursor
;   Ctrl+S (19)                 -- Save file
;   Ctrl+Q (17)                 -- Quit editor
;   Ctrl+F (6)                  -- Search forward
;   F3 (next match in search)   -- Find next
;   Ctrl+G (7)                  -- Goto line number
;   Escape (27)                 -- Cancel prompt
;
; Screen: 256x256, DRAWTEXT 8x8 font
;   Title bar: y=0..11 (filename, status)
;   Content:   y=14..235 (28 lines at 8px each)
;   Hint bar:  y=240..255 (key hints)
;
; RAM Layout:
;   0x5000-0x51FF  Line starts table (offsets, max 512 lines)
;   0x5400-0x73FF  File content buffer (8192 chars)
;   0x7400         Line count (u32)
;   0x7401         Modified flag (0=clean, 1=dirty)
;   0x7402         Cursor line (u32)
;   0x7403         Cursor col (u32)
;   0x7404         Scroll offset (first visible line)
;   0x7405         File handle
;   0x7406         Buffer size (chars in buffer)
;   0x7420-0x744F  Filename (null-terminated, max 48 chars)
;   0x7460-0x748A  Scratch buffer (43 cells for one line + null)
;   0x7490-0x74BF  Search string buffer (48 chars)
;   0x74C0         Prompt mode (0=none, 1=search, 2=goto)
;   0x74C1         Search string length
;   0x74C2         Search match line (-1 if none)
;   0x74C3         Search match col
;   0x74C4         Goto number accumulator

; === Constants ===
#define COLS     42
#define VIS      28
#define LH       8

; Colors (packed RGB)
#define C_BG     0x0D0D0D
#define C_BAR    0x1A1A2E
#define C_FG     0xDDDDDD
#define C_TITLE  0x8888CC
#define C_GREEN  0x44DD44
#define C_AMBER  0xFFAA00
#define C_HINT   0x555577
#define C_CURLN  0x151520
#define C_SEL    0x335577
#define C_MATCH  0x664400
#define C_PROMPT 0x333355

; RAM addresses
#define LS       0x5000
#define FB       0x5400
#define FB_MAX   8192

#define R_NL     0x7400
#define R_DIRTY  0x7401
#define R_CL     0x7402
#define R_CC     0x7403
#define R_SC     0x7404
#define R_FH     0x7405
#define R_BS     0x7406
#define R_FN     0x7420
#define R_SCR    0x7460
#define R_SEARCH 0x7490
#define R_PM     0x74C0
#define R_SLEN   0x74C1
#define R_SML    0x74C2
#define R_SMC    0x74C3
#define R_GNUM   0x74C4

; Multi-buffer state
#define R_BACT   0xB500    ; active buffer index (0 or 1)
#define R_BCNT   0xB501    ; buffer count (1 or 2)
#define R_B0FN   0xB510    ; buffer 0 filename (48 chars)
#define R_B1FN   0xB540    ; buffer 1 filename (48 chars)
#define R_B0CL   0xB570    ; buffer 0 saved cursor_line
#define R_B0CC   0xB571    ; buffer 0 saved cursor_col
#define R_B0SC   0xB572    ; buffer 0 saved scroll_offset
#define R_B1CL   0xB580    ; buffer 1 saved cursor_line
#define R_B1CC   0xB581    ; buffer 1 saved cursor_col
#define R_B1SC   0xB582    ; buffer 1 saved scroll_offset

; =========================================
; INIT
; =========================================
    LDI r30, 0xFE00
    LDI r1, 1

    ; Clear metadata
    LDI r10, R_NL
    LDI r11, 0
    STORE r10, r11
    LDI r10, R_DIRTY
    STORE r10, r11
    LDI r10, R_CL
    STORE r10, r11
    LDI r10, R_CC
    STORE r10, r11
    LDI r10, R_SC
    STORE r10, r11
    LDI r10, R_BS
    STORE r10, r11

    ; Set filename
    LDI r10, R_FN
    STRO r10, "~/.geos_notes.txt"

    ; Init prompt state
    LDI r10, R_PM
    LDI r11, 0
    STORE r10, r11
    LDI r10, R_SLEN
    STORE r10, r11
    LDI r10, R_SML
    LDI r11, 0xFFFFFFFF
    STORE r10, r11
    LDI r10, R_SMC
    LDI r11, 0
    STORE r10, r11
    LDI r10, R_GNUM
    STORE r10, r11

    ; Init multi-buffer state
    LDI r10, R_BACT
    LDI r11, 0
    STORE r10, r11              ; active_buf = 0
    LDI r10, R_BCNT
    LDI r11, 1
    STORE r10, r11              ; buf_count = 1

    ; Copy default filename to buffer 0 slot
    LDI r15, R_B0FN
    LDI r16, R_FN
ib_copy_fn:
    LOAD r17, r16
    STORE r15, r17
    LDI r17, 0
    CMP r17, r16
    ; Check if we just copied a null
    LOAD r17, r16
    LDI r18, 0
    CMP r17, r18
    JZ r0, ib_fn_done
    LDI r17, 1
    ADD r15, r17
    ADD r16, r17
    JMP ib_copy_fn
ib_fn_done:

    ; Try to load file
    CALL load_file

    ; Build line table
    CALL build_lines

    ; If line_count == 0, ensure at least 1 line
    LDI r10, R_NL
    LOAD r10, r10
    LDI r11, 0
    CMP r10, r11
    JNZ r0, main_loop

    ; Force 1 empty line
    LDI r10, R_NL
    LDI r11, 1
    STORE r10, r11
    LDI r10, LS
    LDI r11, 0
    STORE r10, r11

; =========================================
; MAIN LOOP
; =========================================
main_loop:
    ; Clear screen
    LDI r2, C_BG
    FILL r2

    ; Read keyboard
    IKEY r5
    LDI r6, 0xFFB
    LOAD r6, r6

    ; Handle input
    CALL handle_input

    ; Render
    CALL render_title
    CALL render_content
    CALL render_cursor
    CALL render_hints

    FRAME
    JMP main_loop

; =========================================
; LOAD FILE -- open and read into buffer
; =========================================
load_file:
    PUSH r31

    ; FSOPEN path, mode=0 (read)
    LDI r10, R_FN
    LDI r11, 0
    FSOPEN r10, r11

    ; Check error (r0 >= 0x80000000)
    LDI r10, 0x80000000
    CMP r0, r10
    BGE r0, lf_done

    ; Save handle
    MOV r20, r0

    ; FSREAD handle, FB, 8192
    LDI r11, FB
    LDI r12, FB_MAX
    FSREAD r20, r11, r12
    MOV r21, r0

    ; FSCLOSE
    FSCLOSE r20

    ; Check read result
    LDI r10, 0x80000000
    CMP r21, r10
    BGE r0, lf_done

    ; Save buffer size
    LDI r10, R_BS
    STORE r10, r21

lf_done:
    POP r31
    RET

; =========================================
; SAVE FILE -- write buffer to host file
; =========================================
save_file:
    PUSH r31

    ; FSOPEN path, mode=1 (write/create)
    LDI r10, R_FN
    LDI r11, 1
    FSOPEN r10, r11

    LDI r10, 0x80000000
    CMP r0, r10
    BGE r0, sf_done

    MOV r20, r0

    ; FSWRITE handle, FB, buf_size
    LDI r11, FB
    LDI r12, R_BS
    LOAD r12, r12
    FSWRITE r20, r11, r12

    ; FSCLOSE
    FSCLOSE r20

    ; Clear dirty flag
    LDI r10, R_DIRTY
    LDI r11, 0
    STORE r10, r11

sf_done:
    POP r31
    RET

; =========================================
; BUILD LINES -- scan buffer for newlines
; line_starts[0] = 0, line_starts[N] = offset after Nth newline
; =========================================
build_lines:
    PUSH r31
    PUSH r1
    LDI r1, 1

    ; line_starts[0] = 0
    LDI r10, LS
    LDI r11, 0
    STORE r10, r11

    ; line_count = 1
    LDI r10, R_NL
    LDI r11, 1
    STORE r10, r11

    ; buf_size
    LDI r10, R_BS
    LOAD r10, r10

    ; If buf_size == 0, done
    LDI r11, 0
    CMP r10, r11
    JZ r0, bl_done

    ; Scan with offset counter
    LDI r12, 0

bl_scan:
    CMP r12, r10
    BGE r0, bl_done

    ; Load byte at FB + offset
    LDI r13, FB
    ADD r13, r12
    LOAD r13, r13
    LDI r14, 10
    CMP r13, r14
    JNZ r0, bl_next

    ; Found newline -- next line starts at offset + 1
    LDI r14, LS
    LDI r15, R_NL
    LOAD r15, r15
    ADD r14, r15
    LDI r16, 1
    ADD r16, r12
    STORE r14, r16

    ; line_count++
    ADD r15, r1
    LDI r14, R_NL
    STORE r14, r15

bl_next:
    ADD r12, r1
    JMP bl_scan

bl_done:
    POP r1
    POP r31
    RET

; =========================================
; HANDLE INPUT
; r5 = IKEY key, r6 = arrow bitmask
; =========================================
handle_input:
    PUSH r31
    PUSH r5
    PUSH r6

    ; Check if in prompt mode
    LDI r10, R_PM
    LOAD r10, r10
    LDI r11, 0
    CMP r10, r11
    JNZ r0, hi_prompt

    ; Check arrow bitmask first
    MOV r10, r6

    ; bit 0 = up
    LDI r11, 1
    MOV r12, r10
    AND r12, r11
    JNZ r12, hi_up

    ; bit 1 = down
    LDI r11, 2
    MOV r12, r10
    AND r12, r11
    JNZ r12, hi_down

    ; bit 2 = left
    LDI r11, 4
    MOV r12, r10
    AND r12, r11
    JNZ r12, hi_left

    ; bit 3 = right
    LDI r11, 8
    MOV r12, r10
    AND r12, r11
    JNZ r12, hi_right

    ; Check IKEY key
    MOV r11, r5
    LDI r12, 0
    CMP r11, r12
    JZ r0, hi_done

    ; Ctrl+Q (17)?
    LDI r12, 17
    CMP r11, r12
    JZ r0, hi_quit

    ; Ctrl+S (19)?
    LDI r12, 19
    CMP r11, r12
    JZ r0, hi_save

    ; Ctrl+F (6)?
    LDI r12, 6
    CMP r11, r12
    JZ r0, hi_search

    ; Ctrl+G (7)?
    LDI r12, 7
    CMP r11, r12
    JZ r0, hi_goto

    ; Ctrl+B (2)? -- switch buffer
    LDI r12, 2
    CMP r11, r12
    JZ r0, hi_buf_switch

    ; Ctrl+N (14)? -- new buffer
    LDI r12, 14
    CMP r11, r12
    JZ r0, hi_buf_new

    ; Escape (27) -- clear search match
    LDI r12, 27
    CMP r11, r12
    JZ r0, hi_clear_match

    ; Backspace (8)?
    LDI r12, 8
    CMP r11, r12
    JZ r0, hi_bksp

    ; Enter (10)?
    LDI r12, 10
    CMP r11, r12
    JZ r0, hi_enter

    ; Printable (32-126)?
    LDI r12, 32
    CMP r11, r12
    BLT r0, hi_done
    LDI r12, 127
    CMP r11, r12
    BGE r0, hi_done

    ; Insert printable char
    CALL insert_char
    JMP hi_done

hi_up:
    CALL cursor_up
    JMP hi_done
hi_down:
    CALL cursor_down
    JMP hi_done
hi_left:
    CALL cursor_left
    JMP hi_done
hi_right:
    CALL cursor_right
    JMP hi_done
hi_quit:
    HALT
hi_save:
    CALL save_file
    JMP hi_done
hi_bksp:
    CALL do_backspace
    JMP hi_done
hi_enter:
    CALL insert_newline
    JMP hi_done
hi_search:
    CALL enter_search
    JMP hi_done
hi_goto:
    CALL enter_goto
    JMP hi_done
hi_buf_switch:
    CALL switch_buf
    JMP hi_done
hi_buf_new:
    CALL new_buf
    JMP hi_done
hi_clear_match:
    LDI r10, R_SML
    LDI r11, 0xFFFFFFFF
    STORE r10, r11
    JMP hi_done

; =========================================
; PROMPT MODE HANDLER
; =========================================
hi_prompt:
    MOV r11, r5
    LDI r12, 0
    CMP r11, r12
    JZ r0, hi_done

    ; Escape (27) -- cancel prompt
    LDI r12, 27
    CMP r11, r12
    JZ r0, hp_cancel

    ; Enter (10) -- execute prompt
    LDI r12, 10
    CMP r11, r12
    JZ r0, hp_execute

    ; Backspace (8) -- delete last char
    LDI r12, 8
    CMP r11, r12
    JZ r0, hp_bksp

    ; Printable (32-126)?
    LDI r12, 32
    CMP r11, r12
    BLT r0, hi_done
    LDI r12, 127
    CMP r11, r12
    BGE r0, hi_done

    ; Add char to prompt
    LDI r10, R_PM
    LOAD r10, r10              ; prompt mode
    LDI r12, 1
    CMP r10, r12
    JZ r0, hp_add_search

    ; Goto mode -- accumulate digit
    LDI r11, 48                ; '0'
    SUB r5, r11                ; digit value (assumes 0-9 input)
    LDI r12, 0
    CMP r5, r12
    BLT r0, hi_done
    LDI r12, 9
    CMP r5, r12
    BGE r0, hi_done            ; not a digit

    ; gnum = gnum * 10 + digit
    LDI r10, R_GNUM
    LOAD r10, r10
    LDI r12, 10
    MUL r10, r12
    ADD r10, r5
    LDI r12, R_GNUM
    STORE r12, r10
    JMP hi_done

hp_add_search:
    ; Add char to search string
    LDI r1, 1                  ; increment constant
    LDI r10, R_SLEN
    LOAD r10, r10
    LDI r12, 40                ; max 40 chars
    CMP r10, r12
    BGE r0, hi_done

    LDI r12, R_SEARCH
    ADD r12, r10
    STORE r12, r5              ; search_buf[slen] = char
    ADD r10, r1
    LDI r12, R_SLEN
    STORE r12, r10             ; slen++
    JMP hi_done

hp_bksp:
    LDI r10, R_PM
    LOAD r10, r10
    LDI r12, 1
    CMP r10, r12
    JZ r0, hp_bksp_search

    ; Goto backspace
    LDI r10, R_GNUM
    LOAD r10, r10
    LDI r12, 10
    DIV r10, r12
    LDI r12, R_GNUM
    STORE r12, r10
    JMP hi_done

hp_bksp_search:
    LDI r1, 1                  ; decrement constant
    LDI r10, R_SLEN
    LOAD r10, r10
    LDI r12, 0
    CMP r10, r12
    JZ r0, hi_done
    SUB r10, r1
    LDI r12, R_SLEN
    STORE r12, r10             ; slen--
    LDI r12, R_SEARCH
    ADD r12, r10
    LDI r13, 0
    STORE r12, r13             ; null terminate
    JMP hi_done

hp_execute:
    LDI r10, R_PM
    LOAD r10, r10
    LDI r12, 1
    CMP r10, r12
    JZ r0, hp_do_search

    ; Execute goto
    CALL do_goto
    JMP hp_cancel

hp_do_search:
    CALL do_search
    ; Fall through to cancel (exit prompt mode)

hp_cancel:
    LDI r10, R_PM
    LDI r11, 0
    STORE r10, r11
    JMP hi_done

hi_done:
    POP r6
    POP r5
    POP r31
    RET

; =========================================
; CURSOR UP
; =========================================
cursor_up:
    PUSH r31
    PUSH r1
    LDI r1, 1

    LDI r10, R_CL
    LOAD r10, r10
    LDI r11, 0
    CMP r10, r11
    JZ r0, cu_done

    SUB r10, r1
    LDI r12, R_CL
    STORE r12, r10

    CALL clamp_col
    CALL scroll_adj

cu_done:
    POP r1
    POP r31
    RET

; =========================================
; CURSOR DOWN
; =========================================
cursor_down:
    PUSH r31
    PUSH r1
    LDI r1, 1

    LDI r10, R_CL
    LOAD r10, r10
    LDI r11, R_NL
    LOAD r11, r11
    SUB r11, r1
    CMP r10, r11
    BGE r0, cd_done

    ADD r10, r1
    LDI r12, R_CL
    STORE r12, r10

    CALL clamp_col
    CALL scroll_adj

cd_done:
    POP r1
    POP r31
    RET

; =========================================
; CURSOR LEFT
; =========================================
cursor_left:
    PUSH r31
    PUSH r1
    LDI r1, 1

    LDI r10, R_CC
    LOAD r10, r10
    LDI r11, 0
    CMP r10, r11
    JNZ r0, cl_dec

    ; At col 0 -- move to end of previous line
    LDI r10, R_CL
    LOAD r10, r10
    LDI r11, 0
    CMP r10, r11
    JZ r0, cl_done

    SUB r10, r1
    LDI r12, R_CL
    STORE r12, r10
    CALL clamp_end
    CALL scroll_adj
    JMP cl_done

cl_dec:
    SUB r10, r1
    LDI r12, R_CC
    STORE r12, r10

cl_done:
    POP r1
    POP r31
    RET

; =========================================
; CURSOR RIGHT
; =========================================
cursor_right:
    PUSH r31
    PUSH r1
    LDI r1, 1

    ; Get current line length
    CALL get_llen
    MOV r10, r0

    LDI r11, R_CC
    LOAD r11, r11
    CMP r11, r10
    BLT r0, cr_inc

    ; At end of line -- move to start of next line
    LDI r10, R_CL
    LOAD r10, r10
    LDI r11, R_NL
    LOAD r11, r11
    SUB r11, r1
    CMP r10, r11
    BGE r0, cr_done

    ADD r10, r1
    LDI r12, R_CL
    STORE r12, r10
    LDI r10, R_CC
    LDI r11, 0
    STORE r10, r11
    CALL scroll_adj
    JMP cr_done

cr_inc:
    ADD r11, r1
    LDI r12, R_CC
    STORE r12, r11

cr_done:
    POP r1
    POP r31
    RET

; =========================================
; CLAMP COL -- set cur_col to min(cur_col, line_length)
; =========================================
clamp_col:
    PUSH r31
    PUSH r1
    LDI r1, 1

    CALL get_llen
    MOV r10, r0

    LDI r11, R_CC
    LOAD r11, r11
    CMP r11, r10
    BLT r0, cc_ok

    ; cur_col >= line_len, clamp
    LDI r12, R_CC
    STORE r12, r10
    ; If line_len == 0, set to 0
    LDI r11, 0
    CMP r10, r11
    JNZ r0, cc_ok
    LDI r12, R_CC
    STORE r12, r11

cc_ok:
    POP r1
    POP r31
    RET

; =========================================
; CLAMP END -- set cur_col to line_length (for going to end of line)
; =========================================
clamp_end:
    PUSH r31
    PUSH r1
    LDI r1, 1

    CALL get_llen
    MOV r10, r0

    LDI r11, R_CC
    STORE r11, r10
    ; If line_len is 0, that's fine (col = 0)

    POP r1
    POP r31
    RET

; =========================================
; GET LINE LENGTH -- returns in r0
; Length excludes trailing newline
; =========================================
get_llen:
    PUSH r1
    PUSH r10
    PUSH r11
    PUSH r12
    LDI r1, 1

    ; Get cur_line
    LDI r10, R_CL
    LOAD r10, r10

    ; Get line_starts[cur_line]
    LDI r11, LS
    ADD r11, r10
    LOAD r11, r11

    ; Is this the last line?
    LDI r12, R_NL
    LOAD r12, r12
    MOV r13, r10
    ADD r13, r1
    CMP r13, r12
    BGE r0, gll_last

    ; Not last: len = line_starts[next] - line_starts[cur] - 1
    LDI r12, LS
    ADD r12, r10
    ADD r12, r1
    LOAD r12, r12
    SUB r12, r11
    SUB r12, r1
    MOV r0, r12
    JMP gll_done

gll_last:
    ; Last: len = buf_size - line_starts[cur]
    LDI r12, R_BS
    LOAD r12, r12
    SUB r12, r11
    MOV r0, r12

gll_done:
    POP r12
    POP r11
    POP r10
    POP r1
    RET

; =========================================
; SCROLL ADJ -- ensure cursor is visible
; =========================================
scroll_adj:
    PUSH r31
    PUSH r1
    PUSH r10
    PUSH r11
    LDI r1, 1

    LDI r10, R_CL
    LOAD r10, r10
    LDI r11, R_SC
    LOAD r11, r11

    ; cur_line < scroll_off? scroll up
    CMP r10, r11
    BLT r0, sa_up

    ; cur_line >= scroll_off + VIS? scroll down
    LDI r12, VIS
    ADD r12, r11
    CMP r10, r12
    BLT r0, sa_done

    ; scroll_off = cur_line - VIS + 1
    MOV r12, r10
    LDI r13, VIS
    SUB r12, r13
    ADD r12, r1
    LDI r13, R_SC
    STORE r13, r12
    JMP sa_done

sa_up:
    LDI r12, R_SC
    STORE r12, r10

sa_done:
    POP r11
    POP r10
    POP r1
    POP r31
    RET

; =========================================
; INSERT CHAR
; Insert the key (from r5) at cursor position
; =========================================
insert_char:
    PUSH r31
    PUSH r1
    PUSH r5
    PUSH r20
    LDI r1, 1

    ; Check buffer space
    LDI r10, R_BS
    LOAD r10, r10
    LDI r11, FB_MAX
    CMP r10, r11
    BGE r0, ic_full

    ; Get cursor position (offset in buffer)
    LDI r10, R_CL
    LOAD r10, r10
    LDI r11, LS
    ADD r11, r10
    LOAD r11, r11
    LDI r12, R_CC
    LOAD r12, r12
    ADD r11, r12            ; r11 = cursor_pos (offset)

    ; Shift buffer right from buf_size down to cursor_pos
    LDI r13, R_BS
    LOAD r13, r13           ; r13 = buf_size (shift counter)

ic_shift:
    CMP r13, r11
    BLT r0, ic_write
    JZ r0, ic_write

    ; Copy buf[offset-1] to buf[offset]  -- shift right
    LDI r14, FB
    ADD r14, r13
    SUB r14, r1             ; source = FB + offset - 1
    LOAD r15, r14
    LDI r14, FB
    ADD r14, r13            ; dest = FB + offset
    STORE r14, r15

    SUB r13, r1
    JMP ic_shift

ic_write:
    ; Write char at cursor_pos
    LDI r14, FB
    ADD r14, r11
    STORE r14, r5           ; buf[cursor_pos] = key char

    ; buf_size++
    LDI r10, R_BS
    LOAD r10, r10
    ADD r10, r1
    LDI r11, R_BS
    STORE r11, r10

    ; cur_col++
    LDI r10, R_CC
    LOAD r10, r10
    ADD r10, r1
    LDI r11, R_CC
    STORE r11, r10

    ; Set dirty
    LDI r10, R_DIRTY
    LDI r11, 1
    STORE r10, r11

    ; Rebuild lines
    CALL build_lines

ic_full:
    POP r20
    POP r5
    POP r1
    POP r31
    RET

; =========================================
; DO BACKSPACE
; Delete char before cursor, shift buffer left
; =========================================
do_backspace:
    PUSH r31
    PUSH r1
    PUSH r20
    LDI r1, 1

    ; Get cursor position
    LDI r10, R_CL
    LOAD r10, r10
    LDI r11, LS
    ADD r11, r10
    LOAD r11, r11
    LDI r12, R_CC
    LOAD r12, r12
    ADD r11, r12            ; r11 = cursor_pos

    ; If cursor_pos == 0, nothing to delete
    LDI r13, 0
    CMP r11, r13
    JZ r0, db_done

    ; Delete at cursor_pos - 1
    SUB r11, r1             ; r11 = delete_pos

    ; Shift left: copy [delete_pos+1 .. buf_size-1] to [delete_pos .. buf_size-2]
    LDI r13, R_BS
    LOAD r13, r13
    MOV r14, r11            ; r14 = current position

db_shift:
    MOV r15, r14
    ADD r15, r1             ; r15 = r14 + 1
    CMP r15, r13
    BGE r0, db_shift_done

    ; Copy buf[r14+1] to buf[r14]
    LDI r16, FB
    ADD r16, r15
    LOAD r17, r16
    LDI r16, FB
    ADD r16, r14
    STORE r16, r17

    ADD r14, r1
    JMP db_shift

db_shift_done:
    ; buf_size--
    LDI r10, R_BS
    LOAD r10, r10
    SUB r10, r1
    LDI r15, R_BS
    STORE r15, r10

    ; Adjust cursor
    LDI r10, R_CC
    LOAD r10, r10
    LDI r11, 0
    CMP r10, r11
    JNZ r0, db_col_dec

    ; cur_col was 0 -- deleted a newline, join with prev line
    LDI r10, R_CL
    LOAD r10, r10
    SUB r10, r1
    LDI r11, R_CL
    STORE r11, r10
    CALL build_lines
    CALL clamp_end
    JMP db_dirty

db_col_dec:
    SUB r10, r1
    LDI r11, R_CC
    STORE r11, r10
    CALL build_lines

db_dirty:
    LDI r10, R_DIRTY
    LDI r11, 1
    STORE r10, r11

db_done:
    POP r20
    POP r1
    POP r31
    RET

; =========================================
; INSERT NEWLINE
; Insert 0x0A at cursor position
; =========================================
insert_newline:
    PUSH r31
    PUSH r1
    PUSH r20
    LDI r1, 1

    ; Check buffer space
    LDI r10, R_BS
    LOAD r10, r10
    LDI r11, FB_MAX
    CMP r10, r11
    BGE r0, inl_done

    ; Get cursor position
    LDI r10, R_CL
    LOAD r10, r10
    LDI r11, LS
    ADD r11, r10
    LOAD r11, r11
    LDI r12, R_CC
    LOAD r12, r12
    ADD r11, r12            ; r11 = cursor_pos

    ; Shift buffer right (same as insert_char)
    LDI r13, R_BS
    LOAD r13, r13

inl_shift:
    CMP r13, r11
    BLT r0, inl_write
    JZ r0, inl_write

    LDI r14, FB
    ADD r14, r13
    SUB r14, r1
    LOAD r15, r14
    LDI r14, FB
    ADD r14, r13
    STORE r14, r15

    SUB r13, r1
    JMP inl_shift

inl_write:
    ; Write newline char
    LDI r14, FB
    ADD r14, r11
    LDI r15, 10
    STORE r14, r15

    ; buf_size++
    LDI r10, R_BS
    LOAD r10, r10
    ADD r10, r1
    LDI r11, R_BS
    STORE r11, r10

    ; cur_line++, cur_col = 0
    LDI r10, R_CL
    LOAD r10, r10
    ADD r10, r1
    LDI r11, R_CL
    STORE r11, r10

    LDI r10, R_CC
    LDI r11, 0
    STORE r10, r11

    ; Set dirty
    LDI r10, R_DIRTY
    LDI r11, 1
    STORE r10, r11

    CALL build_lines
    CALL scroll_adj

inl_done:
    POP r20
    POP r1
    POP r31
    RET

; =========================================
; RENDER TITLE BAR
; =========================================
render_title:
    PUSH r31

    ; Bar background
    LDI r1, 0
    LDI r2, 0
    LDI r3, 256
    LDI r4, 12
    LDI r5, C_BAR
    RECTF r1, r2, r3, r4, r5

    ; Filename
    LDI r10, 4
    LDI r11, 2
    LDI r12, R_FN
    LDI r13, C_TITLE
    LDI r14, C_BAR
    DRAWTEXT r10, r11, r12, r13, r14

    ; Buffer indicator (show active buffer number)
    LDI r10, R_BCNT
    LOAD r10, r10
    LDI r11, 1
    CMP r10, r11
    JZ r0, rt_nomod             ; only 1 buffer, skip indicator

    ; Show "[1]" or "[2]" in cyan
    LDI r10, R_BACT
    LOAD r10, r10
    LDI r11, 48                  ; '0'
    ADD r10, r11                 ; ascii digit
    LDI r11, 1
    ADD r10, r11                 ; 1-based
    LDI r15, R_SCR
    LDI r16, 91                  ; '['
    STORE r15, r16
    LDI r16, 1
    ADD r15, r16
    STORE r15, r10
    LDI r16, 1
    ADD r15, r16
    LDI r16, 93                  ; ']'
    STORE r15, r16
    LDI r16, 1
    ADD r15, r16
    LDI r16, 0
    STORE r15, r16
    LDI r10, 180
    LDI r11, 2
    LDI r12, R_SCR
    LDI r13, C_GREEN
    LDI r14, C_BAR
    DRAWTEXT r10, r11, r12, r13, r14

    ; Modified indicator
    LDI r10, R_DIRTY
    LOAD r10, r10
    LDI r11, 0
    CMP r10, r11
    JZ r0, rt_nomod

    LDI r10, R_SCR
    STRO r10, " *"
    LDI r10, 200
    LDI r11, 2
    LDI r12, R_SCR
    LDI r13, C_AMBER
    LDI r14, C_BAR
    DRAWTEXT r10, r11, r12, r13, r14

rt_nomod:
    POP r31
    RET

; =========================================
; RENDER CONTENT -- draw visible lines
; =========================================
render_content:
    PUSH r31
    PUSH r1
    LDI r1, 1

    ; scroll_off
    LDI r10, R_SC
    LOAD r10, r10

    ; Loop i = 0..VIS-1
    LDI r11, 0

rc_loop:
    LDI r12, VIS
    CMP r11, r12
    BGE r0, rc_done

    ; line_num = scroll_off + i
    MOV r12, r10
    ADD r12, r11

    ; y position = 14 + i * 8
    MOV r13, r11
    LDI r14, LH
    MUL r13, r14
    LDI r14, 14
    ADD r13, r14            ; r13 = y

    ; Check line_num < line_count
    LDI r14, R_NL
    LOAD r14, r14
    CMP r12, r14
    BGE r0, rc_next

    ; Highlight current line
    LDI r14, R_CL
    LOAD r14, r14
    CMP r12, r14
    JNZ r0, rc_no_hl

    LDI r14, 0
    LDI r15, 256
    LDI r16, LH
    LDI r17, C_CURLN
    RECTF r14, r13, r15, r16, r17

rc_no_hl:
    ; Get line_start offset
    LDI r14, LS
    ADD r14, r12
    LOAD r14, r14            ; r14 = line_start (offset)

    ; Copy line to scratch (up to COLS chars, stop at newline)
    LDI r15, R_SCR           ; scratch dest
    LDI r16, 0               ; col counter

rc_copy:
    LDI r17, COLS
    CMP r16, r17
    BGE r0, rc_copy_end

    ; Check buffer bounds
    MOV r17, r14
    ADD r17, r16             ; r17 = line_start + col
    LDI r18, R_BS
    LOAD r18, r18
    CMP r17, r18
    BGE r0, rc_copy_end

    ; Load char
    LDI r18, FB
    ADD r18, r17
    LOAD r18, r18

    ; Check newline
    LDI r19, 10
    CMP r18, r19
    JZ r0, rc_copy_end

    ; Store in scratch
    STORE r15, r18
    ADD r15, r1
    ADD r16, r1
    JMP rc_copy

rc_copy_end:
    ; Null terminate
    LDI r17, 0
    STORE r15, r17

    ; Draw the line
    LDI r17, 0
    LDI r18, R_SCR
    LDI r19, C_FG
    LDI r20, 0
    DRAWTEXT r17, r13, r18, r19, r20

rc_next:
    ADD r11, r1
    JMP rc_loop

rc_done:
    POP r1
    POP r31
    RET

; =========================================
; RENDER CURSOR -- draw cursor block
; =========================================
render_cursor:
    PUSH r31
    PUSH r1
    LDI r1, 1

    ; Check if cursor line is visible
    LDI r10, R_CL
    LOAD r10, r10
    LDI r11, R_SC
    LOAD r11, r11
    SUB r10, r11             ; r10 = cur_line - scroll_off

    LDI r11, 0
    CMP r10, r11
    BLT r0, rcur_done
    LDI r11, VIS
    CMP r10, r11
    BGE r0, rcur_done

    ; y = 14 + (cur_line - scroll_off) * 8
    LDI r11, LH
    MUL r10, r11
    LDI r11, 14
    ADD r10, r11             ; r10 = y

    ; x = cur_col * 6
    LDI r11, R_CC
    LOAD r11, r11
    LDI r12, 6
    MUL r11, r12             ; r11 = x

    ; Draw cursor block
    LDI r12, 6
    LDI r13, LH
    LDI r14, C_SEL
    RECTF r11, r10, r12, r13, r14

rcur_done:
    POP r1
    POP r31
    RET

; =========================================
; RENDER HINTS -- bottom bar with key hints
; =========================================
render_hints:
    PUSH r31

    ; Bar background
    LDI r1, 0
    LDI r2, 240
    LDI r3, 256
    LDI r4, 16
    LDI r5, C_BAR
    RECTF r1, r2, r3, r4, r5

    ; Check if in prompt mode
    LDI r10, R_PM
    LOAD r10, r10
    LDI r11, 0
    CMP r10, r11
    JNZ r0, rh_prompt

    ; Normal hints
    LDI r10, R_SCR
    STRO r10, "^S:Save ^Q:Quit ^F:Find ^G:Goto ^B:Buf ^N:New"
    LDI r10, 4
    LDI r11, 242
    LDI r12, R_SCR
    LDI r13, C_HINT
    LDI r14, C_BAR
    DRAWTEXT r10, r11, r12, r13, r14
    JMP rh_ln

rh_prompt:
    ; Search prompt
    LDI r11, 1
    CMP r10, r11
    JNZ r0, rh_goto_prompt

    LDI r10, R_SCR
    STRO r10, "Search: "
    LDI r10, 4
    LDI r11, 242
    LDI r12, R_SCR
    LDI r13, C_AMBER
    LDI r14, C_BAR
    DRAWTEXT r10, r11, r12, r13, r14

    ; Show search text after "Search: " (8 chars)
    LDI r10, R_SLEN
    LOAD r10, r10
    LDI r11, 0
    CMP r10, r11
    JZ r0, rh_ln

    ; Copy search string to scratch
    LDI r10, R_SCR
    STRO r10, "                                        "
    LDI r15, R_SCR
    LDI r16, 0

rh_scopy:
    LDI r1, 1
    LDI r17, R_SLEN
    LOAD r17, r17
    CMP r16, r17
    BGE r0, rh_scopy_done
    LDI r17, R_SEARCH
    ADD r17, r16
    LOAD r17, r17
    STORE r15, r17
    ADD r15, r1
    ADD r16, r1
    JMP rh_scopy

rh_scopy_done:
    LDI r17, 0
    STORE r15, r17
    LDI r10, 52             ; x = 8 chars * 6px + 4
    LDI r11, 242
    LDI r12, R_SCR
    LDI r13, C_FG
    LDI r14, C_BAR
    DRAWTEXT r10, r11, r12, r13, r14
    JMP rh_ln

rh_goto_prompt:
    ; Goto prompt
    LDI r10, R_SCR
    STRO r10, "Goto line: "
    LDI r10, 4
    LDI r11, 242
    LDI r12, R_SCR
    LDI r13, C_AMBER
    LDI r14, C_BAR
    DRAWTEXT r10, r11, r12, r13, r14

    ; Show number
    LDI r10, R_GNUM
    LOAD r10, r10
    LDI r11, 0
    CMP r10, r11
    JZ r0, rh_show_zero

    ; Convert number to decimal string
    LDI r1, 1                  ; increment constant
    LDI r10, R_GNUM
    LOAD r10, r10
    LDI r11, R_SCR
    LDI r12, 0
    STRO r11, "                                        "
    LDI r15, R_SCR
    ADD r15, r12            ; points to end of number string
    LDI r16, 10

rh_ndiv:
    LDI r11, 0
    CMP r10, r11
    JZ r0, rh_ndone
    MOV r17, r10            ; save value before MOD
    LDI r11, 10
    MOD r10, r11            ; r10 = value % 10 (digit)
    LDI r12, 48
    ADD r10, r12            ; digit char
    STORE r15, r10
    ADD r15, r1
    LDI r11, 10
    DIV r17, r11            ; r17 = value / 10 (quotient)
    MOV r10, r17            ; r10 = quotient for next iteration
    JMP rh_ndiv

rh_ndone:
    ; Reverse the string in place
    ; r15 points past last digit, r12 = R_SCR was start
    ; Actually we need to reverse from R_SCR to r15-1
    LDI r10, R_SCR
    MOV r11, r15
    SUB r11, r1             ; r11 = last digit position
    LDI r12, 0
    CMP r10, r11
    BGE r0, rh_nshow        ; 0 or 1 digits, no reversal needed

rh_rev:
    CMP r10, r11
    BGE r0, rh_nshow
    LOAD r12, r10
    LOAD r13, r11
    STORE r10, r13
    STORE r11, r12
    ADD r10, r1
    SUB r11, r1
    JMP rh_rev

rh_nshow:
    ; Null terminate
    LDI r10, 0
    STORE r15, r10
    LDI r10, 64             ; x = 10 chars * 6px + 4
    LDI r11, 242
    LDI r12, R_SCR
    LDI r13, C_FG
    LDI r14, C_BAR
    DRAWTEXT r10, r11, r12, r13, r14
    JMP rh_ln

rh_show_zero:
    LDI r10, R_SCR
    LDI r11, 48             ; '0'
    STORE r10, r11
    LDI r11, 0
    ADD r10, r1
    STORE r10, r11
    LDI r10, 64
    LDI r11, 242
    LDI r12, R_SCR
    LDI r13, C_FG
    LDI r14, C_BAR
    DRAWTEXT r10, r11, r12, r13, r14
    JMP rh_ln

rh_ln:
    ; Line/col info
    LDI r10, R_SCR
    STRO r10, "Ln:"
    LDI r10, 4
    LDI r11, 250
    LDI r12, R_SCR
    LDI r13, C_GREEN
    LDI r14, C_BAR
    DRAWTEXT r10, r11, r12, r13, r14

    POP r31
    RET

; =========================================
; ENTER SEARCH -- start search prompt mode
; =========================================
enter_search:
    PUSH r31

    ; If already have search string, do find-next instead
    LDI r10, R_SLEN
    LOAD r10, r10
    LDI r11, 0
    CMP r10, r11
    JNZ r0, es_next

    ; Enter prompt mode
    LDI r10, R_PM
    LDI r11, 1
    STORE r10, r11

    ; Clear search string
    LDI r10, R_SLEN
    LDI r11, 0
    STORE r10, r11
    LDI r10, R_SEARCH
    STORE r10, r11

    POP r31
    RET

es_next:
    ; Find next from current position
    CALL do_search
    POP r31
    RET

; =========================================
; ENTER GOTO -- start goto prompt mode
; =========================================
enter_goto:
    PUSH r31

    LDI r10, R_PM
    LDI r11, 2
    STORE r10, r11

    ; Clear goto number
    LDI r10, R_GNUM
    LDI r11, 0
    STORE r10, r11

    POP r31
    RET

; =========================================
; DO SEARCH -- find search string in buffer
; Scans forward from cursor position (or from match+1 if searching again)
; =========================================
do_search:
    PUSH r31
    PUSH r1
    PUSH r5
    PUSH r10
    PUSH r11
    PUSH r12
    PUSH r13
    PUSH r14
    PUSH r15
    PUSH r16
    PUSH r17
    PUSH r18
    LDI r1, 1

    ; Get search length
    LDI r10, R_SLEN
    LOAD r10, r10
    LDI r11, 0
    CMP r10, r11
    JZ r0, ds_done          ; empty string, skip

    ; Start position: cursor_pos + 1 (search forward from after cursor)
    LDI r10, R_CL
    LOAD r10, r10
    LDI r11, LS
    ADD r11, r10
    LOAD r11, r11
    LDI r12, R_CC
    LOAD r12, r12
    ADD r11, r12            ; r11 = cursor offset
    ADD r11, r1             ; start one past cursor

    ; Check bounds
    LDI r13, R_BS
    LOAD r13, r13
    LDI r10, R_SLEN
    LOAD r10, r10

ds_outer:
    ; Check if remaining buffer is long enough
    MOV r14, r11
    ADD r14, r10             ; end of potential match
    CMP r14, r13
    BGE r0, ds_not_found

    ; Try matching at position r11
    LDI r15, 0               ; match offset

ds_inner:
    CMP r15, r10
    BGE r0, ds_found

    ; Load buffer char
    LDI r16, FB
    ADD r16, r11
    ADD r16, r15
    LOAD r16, r16

    ; Load search char
    LDI r17, R_SEARCH
    ADD r17, r15
    LOAD r17, r17

    CMP r16, r17
    JNZ r0, ds_no_match

    ADD r15, r1
    JMP ds_inner

ds_no_match:
    ADD r11, r1
    JMP ds_outer

ds_found:
    ; Found at position r11
    ; Convert offset to line,col
    ; Find which line this offset belongs to
    LDI r12, R_NL
    LOAD r12, r12            ; line count
    LDI r14, 0               ; line index

ds_find_line:
    CMP r14, r12
    BGE r0, ds_not_found

    LDI r15, LS
    ADD r15, r14
    LOAD r15, r15            ; line_starts[line]

    ; Check next line start
    MOV r16, r14
    ADD r16, r1
    CMP r16, r12
    BGE r0, ds_last_line

    LDI r17, LS
    ADD r17, r16
    LOAD r17, r17            ; line_starts[line+1]

    ; r11 >= line_starts[line] AND r11 < line_starts[line+1]
    CMP r11, r15
    BLT r0, ds_fl_next
    CMP r11, r17
    BGE r0, ds_fl_next

    ; Found the line
    ; col = r11 - line_starts[line]
    SUB r11, r15             ; col = offset - line_start
    MOV r10, r14             ; line
    JMP ds_set_match

ds_fl_next:
    ADD r14, r1
    JMP ds_find_line

ds_last_line:
    CMP r11, r15
    BLT r0, ds_not_found
    ; It is on the last line
    SUB r11, r15
    MOV r10, r14
    JMP ds_set_match

ds_set_match:
    ; Set cursor to match position
    LDI r15, R_CL
    STORE r15, r10           ; cursor line
    LDI r15, R_CC
    STORE r15, r11           ; cursor col
    LDI r15, R_SML
    STORE r15, r10           ; match line
    LDI r15, R_SMC
    STORE r15, r11           ; match col

    ; Adjust scroll
    CALL scroll_adj
    JMP ds_done

ds_not_found:
    ; Clear match indicator
    LDI r10, R_SML
    LDI r11, 0xFFFFFFFF
    STORE r10, r11

ds_done:
    POP r18
    POP r17
    POP r16
    POP r15
    POP r14
    POP r13
    POP r12
    POP r11
    POP r10
    POP r5
    POP r1
    POP r31
    RET

; =========================================
; DO GOTO -- jump to line number in R_GNUM
; =========================================
do_goto:
    PUSH r31
    PUSH r1
    PUSH r10
    PUSH r11
    PUSH r12
    LDI r1, 1

    ; Get target line (user enters 1-based, we use 0-based)
    LDI r10, R_GNUM
    LOAD r10, r10
    LDI r11, 0
    CMP r10, r11
    JZ r0, dg_done          ; goto line 0 = noop

    ; Convert to 0-based
    SUB r10, r1
    LDI r11, 0
    CMP r10, r11
    BLT r0, dg_done         ; negative, invalid

    ; Clamp to last line
    LDI r12, R_NL
    LOAD r12, r12
    SUB r12, r1             ; last valid line index
    CMP r10, r12
    BLT r0, dg_ok
    MOV r10, r12            ; clamp

dg_ok:
    ; Set cursor line
    LDI r11, R_CL
    STORE r11, r10
    LDI r11, R_CC
    LDI r12, 0
    STORE r11, r12          ; col = 0

    ; Adjust scroll
    CALL scroll_adj

dg_done:
    ; Reset goto number
    LDI r10, R_GNUM
    LDI r11, 0
    STORE r10, r11

    POP r12
    POP r11
    POP r10
    POP r1
    POP r31
    RET

; =========================================
; SWITCH_BUF -- toggle between buffer 0 and 1
; Saves current cursor state, auto-saves if dirty,
; loads the other buffer's file
; =========================================
switch_buf:
    PUSH r31
    PUSH r1
    PUSH r10
    PUSH r11
    PUSH r12
    PUSH r13
    PUSH r14
    PUSH r15
    PUSH r16
    LDI r1, 1

    ; Check buf_count > 1
    LDI r10, R_BCNT
    LOAD r10, r10
    LDI r11, 1
    CMP r10, r11
    JZ r0, sb_done              ; only 1 buffer, no switch

    ; Get current active buffer
    LDI r10, R_BACT
    LOAD r10, r10               ; 0 or 1

    ; Save current cursor state to active buffer's slot
    LDI r11, 0
    CMP r10, r11
    JNZ r0, sb_save1

sb_save0:
    ; Save buf 0 cursor state
    LDI r11, R_CL
    LOAD r11, r11
    LDI r12, R_B0CL
    STORE r12, r11

    LDI r11, R_CC
    LOAD r11, r11
    LDI r12, R_B0CC
    STORE r12, r11

    LDI r11, R_SC
    LOAD r11, r11
    LDI r12, R_B0SC
    STORE r12, r11
    JMP sb_autosave

sb_save1:
    ; Save buf 1 cursor state
    LDI r11, R_CL
    LOAD r11, r11
    LDI r12, R_B1CL
    STORE r12, r11

    LDI r11, R_CC
    LOAD r11, r11
    LDI r12, R_B1CC
    STORE r12, r11

    LDI r11, R_SC
    LOAD r11, r11
    LDI r12, R_B1SC
    STORE r12, r11

sb_autosave:
    ; Auto-save if dirty
    LDI r11, R_DIRTY
    LOAD r11, r11
    LDI r12, 0
    CMP r11, r12
    JZ r0, sb_no_save
    CALL save_file

sb_no_save:
    ; Toggle active buffer
    LDI r10, R_BACT
    LOAD r10, r10
    LDI r11, 1
    XOR r10, r11                ; toggle 0 <-> 1
    LDI r11, R_BACT
    STORE r11, r10              ; save new active

    ; Copy new buffer's filename to R_FN
    LDI r11, 0
    CMP r10, r11
    JNZ r0, sb_fn1

sb_fn0:
    ; Copy buf 0 filename to R_FN
    LDI r15, R_B0FN
    JMP sb_fn_copy

sb_fn1:
    ; Copy buf 1 filename to R_FN
    LDI r15, R_B1FN

sb_fn_copy:
    LDI r16, R_FN
sb_fn_loop:
    LOAD r17, r15
    STORE r16, r17
    LDI r17, 0
    CMP r17, r15
    LOAD r17, r15
    LDI r18, 0
    CMP r17, r18
    JZ r0, sb_fn_done
    ADD r15, r1
    ADD r16, r1
    JMP sb_fn_loop

sb_fn_done:
    ; Load new file
    CALL load_file

    ; Build lines
    CALL build_lines

    ; If line_count == 0, ensure at least 1 line
    LDI r10, R_NL
    LOAD r10, r10
    LDI r11, 0
    CMP r10, r11
    JNZ r0, sb_restore

    LDI r10, R_NL
    LDI r11, 1
    STORE r10, r11
    LDI r10, LS
    LDI r11, 0
    STORE r10, r11

sb_restore:
    ; Restore cursor state from new buffer's slot
    LDI r10, R_BACT
    LOAD r10, r10
    LDI r11, 0
    CMP r10, r11
    JNZ r0, sb_rest1

sb_rest0:
    LDI r11, R_B0CL
    LOAD r11, r11
    LDI r12, R_CL
    STORE r12, r11

    LDI r11, R_B0CC
    LOAD r11, r11
    LDI r12, R_CC
    STORE r12, r11

    LDI r11, R_B0SC
    LOAD r11, r11
    LDI r12, R_SC
    STORE r12, r11
    JMP sb_done

sb_rest1:
    LDI r11, R_B1CL
    LOAD r11, r11
    LDI r12, R_CL
    STORE r12, r11

    LDI r11, R_B1CC
    LOAD r11, r11
    LDI r12, R_CC
    STORE r12, r11

    LDI r11, R_B1SC
    LOAD r11, r11
    LDI r12, R_SC
    STORE r12, r11

sb_done:
    ; Clear dirty flag
    LDI r10, R_DIRTY
    LDI r11, 0
    STORE r10, r11

    POP r16
    POP r15
    POP r14
    POP r13
    POP r12
    POP r11
    POP r10
    POP r1
    POP r31
    RET

; =========================================
; NEW_BUF -- open a second buffer with default scratch file
; =========================================
new_buf:
    PUSH r31
    PUSH r1
    PUSH r10
    PUSH r11
    PUSH r15
    PUSH r16
    PUSH r17
    PUSH r18
    LDI r1, 1

    ; Check if already have 2 buffers
    LDI r10, R_BCNT
    LOAD r10, r10
    LDI r11, 2
    CMP r10, r11
    BGE r0, nb_switch           ; already 2 buffers, just switch

    ; Save current cursor state to buffer 0 slot
    LDI r11, R_CL
    LOAD r11, r11
    LDI r12, R_B0CL
    STORE r12, r11

    LDI r11, R_CC
    LOAD r11, r11
    LDI r12, R_B0CC
    STORE r12, r11

    LDI r11, R_SC
    LOAD r11, r11
    LDI r12, R_B0SC
    STORE r12, r11

    ; Auto-save current buffer if dirty
    LDI r11, R_DIRTY
    LOAD r11, r11
    LDI r12, 0
    CMP r11, r12
    JZ r0, nb_no_save
    CALL save_file

nb_no_save:
    ; Set buffer 1 filename to scratch file
    LDI r15, R_B1FN
    STRO r15, "~/.geos_scratch.txt"

    ; Set buf_count = 2
    LDI r10, R_BCNT
    LDI r11, 2
    STORE r10, r11

    ; Set active_buf = 1
    LDI r10, R_BACT
    LDI r11, 1
    STORE r10, r11

    ; Copy new filename to R_FN
    LDI r15, R_B1FN
    LDI r16, R_FN
nb_fn_copy:
    LOAD r17, r15
    STORE r16, r17
    LDI r17, 0
    CMP r17, r15
    LOAD r17, r15
    LDI r18, 0
    CMP r17, r18
    JZ r0, nb_fn_done
    ADD r15, r1
    ADD r16, r1
    JMP nb_fn_copy

nb_fn_done:
    ; Clear buffer
    LDI r10, R_BS
    LDI r11, 0
    STORE r10, r11

    ; Load file (may not exist -- that is OK)
    CALL load_file

    ; Build lines
    CALL build_lines

    ; Ensure at least 1 line
    LDI r10, R_NL
    LOAD r10, r10
    LDI r11, 0
    CMP r10, r11
    JNZ r0, nb_reset_cursor

    LDI r10, R_NL
    LDI r11, 1
    STORE r10, r11
    LDI r10, LS
    LDI r11, 0
    STORE r10, r11

nb_reset_cursor:
    ; Reset cursor for new buffer
    LDI r10, R_CL
    LDI r11, 0
    STORE r10, r11
    LDI r10, R_CC
    STORE r10, r11
    LDI r10, R_SC
    STORE r10, r11

    ; Save initial cursor for buffer 1
    LDI r10, R_B1CL
    LDI r11, 0
    STORE r10, r11
    LDI r10, R_B1CC
    STORE r10, r11
    LDI r10, R_B1SC
    STORE r10, r11

    ; Clear dirty flag
    LDI r10, R_DIRTY
    LDI r11, 0
    STORE r10, r11

    JMP nb_done

nb_switch:
    ; Already 2 buffers -- just switch
    CALL switch_buf

nb_done:
    POP r18
    POP r17
    POP r16
    POP r15
    POP r11
    POP r10
    POP r1
    POP r31
    RET
