; file_browser.asm -- File Browser GUI App for Geometry OS
;
; Proves: LS (list files), OPEN, READ, CLOSE (VFS file operations),
;         TEXT rendering, HITSET/HITQ (mouse interaction), FRAME loop.
;
; Two views:
;   1. File List -- shows VFS files, click to open
;   2. File Content -- shows file contents, click BACK to return
;
; RAM Layout (all above bytecode at 0x32A, using 0x400+):
;   0x400-0x41F  filename address table (16 entries, 1 per visible row)
;   0x500       mode (0=list, 1=content)
;   0x504       file count
;   0x508       content lines (lines in opened file)
;   0x50C       temp fd storage
;   0x600-0x9FF  file list buffer (from LS)
;   0xA00-0xDFF  file content buffer (from READ)
;   0x1000-0x10FF string buffer for title etc.
;
; NOTE: No colons in comments (assembler pitfall)

#define FNAME_TABLE  0x400
#define MODE         0x500
#define FILE_COUNT   0x504
#define CONTENT_LINES 0x508
#define TEMP_FD      0x50C
#define FILE_BUF     0x600
#define CONTENT_BUF  0xA00
#define STR_BUF      0x1000
#define TITLE_Y      6
#define ROW_BASE_Y   30
#define ROW_H        16
#define ROW_X        10
#define ROW_W        236
#define MAX_ROWS     12
#define BACK_Y       240

; ── INIT ──────────────────────────────────────
LDI r1, 1

; Fill screen dark blue
LDI r2, 0x1a1a2e
FILL r2

; Mode = list view
LDI r20, MODE
LDI r2, 0
STORE r20, r2

; Content lines = 0
LDI r20, CONTENT_LINES
LDI r2, 0
STORE r20, r2

; ── List files with LS ──
LDI r20, FILE_BUF
LS r20
; r0 = file count
LDI r21, FILE_COUNT
STORE r21, r0

; ── Parse LS buffer: build filename address table ──
; Scan through null-terminated filenames, store each start address
LDI r20, FILE_BUF     ; current position in buffer
LDI r22, 0            ; row index (0..MAX_ROWS-1)
LDI r23, FNAME_TABLE  ; address table pointer

scan_loop:
    CMPI r22, MAX_ROWS
    BGE r0, scan_done

    ; Check for end of list (double null or empty entry)
    LOAD r2, r20
    CMPI r2, 0
    JZ r0, scan_done

    ; Store filename start address in table
    STORE r23, r20

    ; Advance past filename to next entry
    ADDI r23, 1       ; next table slot
skip_chars:
    LOAD r2, r20
    CMPI r2, 0
    JZ r0, next_entry
    ADDI r20, 1
    JMP skip_chars

next_entry:
    ADDI r20, 1       ; skip null terminator
    ADDI r22, 1
    JMP scan_loop

scan_done:

; ── Register HITSET regions for file rows ──
; 12 rows at y = ROW_BASE_Y + i*ROW_H
; HITSET x, y, w, h, id  (id is immediate, not register)

LDI r1, ROW_X
LDI r3, ROW_W
LDI r4, ROW_H

LDI r2, 30
HITSET r1, r2, r3, r4, 1

LDI r2, 46
HITSET r1, r2, r3, r4, 2

LDI r2, 62
HITSET r1, r2, r3, r4, 3

LDI r2, 78
HITSET r1, r2, r3, r4, 4

LDI r2, 94
HITSET r1, r2, r3, r4, 5

LDI r2, 110
HITSET r1, r2, r3, r4, 6

LDI r2, 126
HITSET r1, r2, r3, r4, 7

LDI r2, 142
HITSET r1, r2, r3, r4, 8

LDI r2, 158
HITSET r1, r2, r3, r4, 9

LDI r2, 174
HITSET r1, r2, r3, r4, 10

LDI r2, 190
HITSET r1, r2, r3, r4, 11

LDI r2, 206
HITSET r1, r2, r3, r4, 12

; BACK button at bottom
LDI r1, 10
LDI r2, BACK_Y
LDI r3, 60
LDI r4, 16
HITSET r1, r2, r3, r4, 99

; ── Store title string at STR_BUF ──
; "FILES" = F(70) I(73) L(76) E(69) S(83) 0
LDI r20, STR_BUF
LDI r2, 70
STORE r20, r2
LDI r2, 73
ADDI r20, 1
STORE r20, r2
LDI r2, 76
ADDI r20, 1
STORE r20, r2
LDI r2, 69
ADDI r20, 1
STORE r20, r2
LDI r2, 83
ADDI r20, 1
STORE r20, r2
LDI r2, 0
ADDI r20, 1
STORE r20, r2

; "BACK" = B(66) A(65) C(67) K(75) 0
LDI r20, STR_BUF
ADDI r20, 16
LDI r2, 66
STORE r20, r2
LDI r2, 65
ADDI r20, 1
STORE r20, r2
LDI r2, 67
ADDI r20, 1
STORE r20, r2
LDI r2, 75
ADDI r20, 1
STORE r20, r2
LDI r2, 0
ADDI r20, 1
STORE r20, r2

; "VIEW" = V(86) I(73) E(69) W(87) 0
LDI r20, STR_BUF
ADDI r20, 32
LDI r2, 86
STORE r20, r2
LDI r2, 73
ADDI r20, 1
STORE r20, r2
LDI r2, 69
ADDI r20, 1
STORE r20, r2
LDI r2, 87
ADDI r20, 1
STORE r20, r2
LDI r2, 0
ADDI r20, 1
STORE r20, r2

; ── MAIN LOOP ─────────────────────────────────
main_loop:
    LDI r1, 1

    ; Check mode
    LDI r20, MODE
    LOAD r6, r20
    CMPI r6, 0
    JZ r0, draw_list_view

    ; ── CONTENT VIEW ──
    JMP draw_content_view

draw_list_view:
    ; Fill background
    LDI r2, 0x1a1a2e
    FILL r2

    ; Draw title bar background
    LDI r1, 0
    LDI r2, 0
    LDI r3, 256
    LDI r4, 24
    LDI r5, 0x2a2a4a
    RECTF r1, r2, r3, r4, r5

    ; Draw title text
    LDI r1, 10
    LDI r2, TITLE_Y
    LDI r3, STR_BUF
    TEXT r1, r2, r3

    ; Draw file count
    LDI r20, FILE_COUNT
    LOAD r2, r20
    ; Convert count to string at STR_BUF+48
    LDI r20, STR_BUF
    ADDI r20, 48
    LDI r6, 48
    ADD r6, r2
    STORE r20, r6
    LDI r2, 0
    ADDI r20, 1
    STORE r20, r2
    ; Show count
    LDI r1, 100
    LDI r2, TITLE_Y
    LDI r3, STR_BUF
    ADDI r3, 48
    TEXT r1, r2, r3

    ; Draw file rows
    LDI r22, 0           ; row index
    LDI r23, FNAME_TABLE ; address table pointer
    LDI r24, ROW_BASE_Y  ; current y

draw_rows:
    CMPI r22, MAX_ROWS
    BGE r0, rows_done

    ; Load filename address from table
    LOAD r20, r23
    CMPI r20, 0
    JZ r0, rows_done     ; no more files

    ; Draw row background (alternating colors)
    LDI r6, 2
    MOD r22, r6
    CMPI r6, 0
    JZ r0, row_even
    ; Odd row
    LDI r1, ROW_X
    MOV r2, r24
    LDI r3, ROW_W
    LDI r4, ROW_H
    LDI r5, 0x222244
    RECTF r1, r2, r3, r4, r5
    JMP draw_filename
row_even:
    LDI r1, ROW_X
    MOV r2, r24
    LDI r3, ROW_W
    LDI r4, ROW_H
    LDI r5, 0x1e1e3a
    RECTF r1, r2, r3, r4, r5

draw_filename:
    LDI r1, ROW_X
    ADDI r1, 4
    MOV r2, r24
    ADDI r2, 4
    TEXT r1, r2, r20

    ; Advance to next row
    ADDI r22, 1
    ADDI r23, 1
    LDI r6, ROW_H
    ADD r24, r6
    JMP draw_rows

rows_done:
    JMP do_hitq

draw_content_view:
    ; Fill background
    LDI r2, 0x1a1a2e
    FILL r2

    ; Draw title bar
    LDI r1, 0
    LDI r2, 0
    LDI r3, 256
    LDI r4, 24
    LDI r5, 0x2a2a4a
    RECTF r1, r2, r3, r4, r5

    ; Draw "VIEW" title
    LDI r1, 10
    LDI r2, TITLE_Y
    LDI r3, STR_BUF
    ADDI r3, 32
    TEXT r1, r2, r3

    ; Draw BACK button
    LDI r1, 10
    LDI r2, BACK_Y
    LDI r3, 60
    LDI r4, 16
    LDI r5, 0x555555
    RECTF r1, r2, r3, r4, r5
    ; BACK label
    LDI r1, 22
    LDI r2, BACK_Y
    ADDI r2, 4
    LDI r3, STR_BUF
    ADDI r3, 16
    TEXT r1, r2, r3

    ; Draw file content from CONTENT_BUF
    LDI r1, 10
    LDI r2, 30
    LDI r3, CONTENT_BUF
    TEXT r1, r2, r3

do_hitq:
    FRAME

    ; Query mouse
    LDI r12, 0
    HITQ r12
    CMPI r12, 0
    JZ r0, main_loop     ; no click, loop

    ; Check mode for handling clicks
    LDI r20, MODE
    LOAD r6, r20
    CMPI r6, 0
    JZ r0, handle_list_click

    ; Content view: only BACK button matters
    CMPI r12, 99
    JNZ r0, main_loop
    ; Switch to list view
    LDI r20, MODE
    LDI r2, 0
    STORE r20, r2
    JMP main_loop

handle_list_click:
    ; Check if BACK was clicked (ignore in list view)
    CMPI r12, 99
    JZ r0, main_loop

    ; Check if row click is valid (1-12)
    CMPI r12, 1
    BLT r0, main_loop
    CMPI r12, 12
    BGE r0, main_loop

    ; Get filename address from table (r12 is 1-based, table is 0-based)
    SUBI r12, 1
    LDI r23, FNAME_TABLE
    ADD r23, r12
    LOAD r20, r23
    CMPI r20, 0
    JZ r0, main_loop     ; no file at this slot

    ; ── Open file ──
    LDI r6, 0            ; mode = read
    OPEN r20, r6
    ; r0 = fd -- save immediately before CMPI clobbers it
    MOV r19, r0          ; r19 = fd (saved)
    CMPI r0, 0xFFFFFFFF
    JZ r0, main_loop     ; open failed (r0 was set to 0 by CMPI if fd != 0xFFFFFFFF)

    ; Save fd for later close
    LDI r21, TEMP_FD
    STORE r21, r19

    ; ── Read file content ──
    LDI r6, CONTENT_BUF  ; buffer address
    LDI r7, 2048         ; max bytes to read
    READ r19, r6, r7
    ; r0 = bytes read

    ; Null-terminate the content
    LDI r20, CONTENT_BUF
    ADD r20, r0
    LDI r2, 0
    STORE r20, r2

    ; ── Close file ──
    CLOSE r19

    ; Switch to content view
    LDI r20, MODE
    LDI r2, 1
    STORE r20, r2

    JMP main_loop
