; sysmon.asm -- System Dashboard for Geometry OS (Phase 140)
;
; Shows: host memory stats from /proc/meminfo, GeOS process count,
;        frame count (uptime), and VM status.
;
; RAM Layout:
;   0x6000-0x60FF  String buffers for display
;   0x6100-0x61FF  Number conversion buffer
;   0x6200-0x6AFF  /proc/meminfo read buffer (host file I/O)
;   0x6B00         Host file handle
;   0x6B04         Bytes read
;
; Uses: FSOPEN, FSREAD, FSCLOSE, NPROC, DRAWTEXT, RECTF, FILL, FRAME, STRO

#define TICKS   0xFFE
#define BUF     0x6000
#define NUMBUF  0x6100
#define PROCBUF 0x6200
#define HANDLE  0x6B00
#define BYTESRD 0x6B04
#define BUFSIZE 0x0200    ; 512 bytes for /proc read

#define COL_BG     0x0D1B2A
#define COL_DARK   0x141428
#define COL_PANEL  0x1B2838
#define COL_TITLE  0x1B3A5C
#define COL_FG     0xFFFFFF
#define COL_LABEL  0x8888CC
#define COL_GREEN  0x44FF44
#define COL_YELLOW 0xFFFF44
#define COL_CYAN   0x44FFFF
#define COL_GRAY   0x666688
#define COL_AMBER  0xFFAA44
#define COL_BAR_BG 0x222244
#define COL_BAR_FG 0x44AA44

; Init
LDI r30, 0xFD00
LDI r1, 1

; =========================================
; Main loop
; =========================================
main_loop:
    LDI r1, COL_BG
    FILL r1

    ; Title bar
    LDI r1, 0
    LDI r2, 0
    LDI r3, 256
    LDI r4, 22
    LDI r5, COL_TITLE
    RECTF r1, r2, r3, r4, r5

    LDI r20, BUF
    STRO r20, "System Dashboard"
    LDI r1, 60
    LDI r2, 5
    LDI r3, BUF
    LDI r4, COL_FG
    LDI r5, COL_TITLE
    DRAWTEXT r1, r2, r3, r4, r5

    ; ---- GeOS Status Panel (left side) ----
    LDI r1, 4
    LDI r2, 28
    LDI r3, 124
    LDI r4, 100
    LDI r5, COL_PANEL
    RECTF r1, r2, r3, r4, r5

    ; Panel title
    LDI r20, BUF
    STRO r20, "GeOS Status"
    LDI r1, 8
    LDI r2, 30
    LDI r3, BUF
    LDI r4, COL_CYAN
    LDI r5, COL_PANEL
    DRAWTEXT r1, r2, r3, r4, r5

    ; Separator
    LDI r1, 8
    LDI r2, 42
    LDI r3, 116
    LDI r4, 1
    LDI r5, COL_GRAY
    RECTF r1, r2, r3, r4, r5

    ; Process count
    LDI r20, BUF
    STRO r20, "Processes"
    LDI r1, 8
    LDI r2, 48
    LDI r3, BUF
    LDI r4, COL_LABEL
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    NPROC
    MOV r14, r0
    LDI r20, NUMBUF
    CALL int_to_str
    LDI r1, 90
    LDI r2, 48
    LDI r3, NUMBUF
    LDI r4, COL_GREEN
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    ; Frame count (uptime)
    LDI r20, BUF
    STRO r20, "Frames"
    LDI r1, 8
    LDI r2, 62
    LDI r3, BUF
    LDI r4, COL_LABEL
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    LDI r20, TICKS
    LOAD r14, r20
    LDI r20, NUMBUF
    CALL int_to_str
    LDI r1, 90
    LDI r2, 62
    LDI r3, NUMBUF
    LDI r4, COL_GREEN
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    ; RAM Size
    LDI r20, BUF
    STRO r20, "RAM"
    LDI r1, 8
    LDI r2, 76
    LDI r3, BUF
    LDI r4, COL_LABEL
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    LDI r20, BUF
    STRO r20, "64K x u32"
    LDI r1, 60
    LDI r2, 76
    LDI r3, BUF
    LDI r4, COL_FG
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    ; Screen
    LDI r20, BUF
    STRO r20, "Screen"
    LDI r1, 8
    LDI r2, 90
    LDI r3, BUF
    LDI r4, COL_LABEL
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    LDI r20, BUF
    STRO r20, "256x256"
    LDI r1, 60
    LDI r2, 90
    LDI r3, BUF
    LDI r4, COL_FG
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    ; Opcodes
    LDI r20, BUF
    STRO r20, "Opcodes"
    LDI r1, 8
    LDI r2, 104
    LDI r3, BUF
    LDI r4, COL_LABEL
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    LDI r20, BUF
    STRO r20, "150+"
    LDI r1, 60
    LDI r2, 104
    LDI r3, BUF
    LDI r4, COL_AMBER
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    ; ---- Host Memory Panel (right side) ----
    LDI r1, 132
    LDI r2, 28
    LDI r3, 120
    LDI r4, 100
    LDI r5, COL_PANEL
    RECTF r1, r2, r3, r4, r5

    LDI r20, BUF
    STRO r20, "Host Memory"
    LDI r1, 136
    LDI r2, 30
    LDI r3, BUF
    LDI r4, COL_CYAN
    LDI r5, COL_PANEL
    DRAWTEXT r1, r2, r3, r4, r5

    ; Separator
    LDI r1, 136
    LDI r2, 42
    LDI r3, 112
    LDI r4, 1
    LDI r5, COL_GRAY
    RECTF r1, r2, r3, r4, r5

    ; Try to read /proc/meminfo
    ; First write the path "/proc/meminfo" into RAM
    LDI r20, BUF
    STRO r20, "/proc/meminfo"

    ; FSOPEN path_reg, mode_reg (0=read)
    LDI r1, BUF
    LDI r2, 0           ; mode = read
    FSOPEN r1, r2
    ; r0 = handle or 0xFFFFFFFF on error
    LDI r15, 0xFFFFFFFF
    CMP r0, r15
    JZ r0, meminfo_error

    ; Store handle
    LDI r20, HANDLE
    STORE r20, r0

    ; FSREAD handle_reg, buf_reg, len_reg
    LDI r20, HANDLE
    LOAD r1, r20         ; handle
    LDI r2, PROCBUF      ; dest buffer
    LDI r3, BUFSIZE      ; max bytes
    FSREAD r1, r2, r3
    ; r0 = bytes read

    LDI r15, 0xFFFFFFFF
    CMP r0, r15
    JZ r0, meminfo_close

    ; Store bytes read
    LDI r20, BYTESRD
    STORE r20, r0

    ; Close file
    LDI r20, HANDLE
    LOAD r1, r20
    FSCLOSE r1

    ; Parse MemTotal from the buffer
    ; We'll scan for "MemTotal:" and extract the number
    LDI r10, PROCBUF     ; buffer start
    LDI r11, 0           ; position
    LDI r12, 0           ; found flag

    ; Scan for "MemTotal:"
scan_memtotal:
    ; Check if we've read enough
    LDI r20, BYTESRD
    LOAD r15, r20
    LDI r16, 9           ; "MemTotal:" is 9 chars
    ADD r16, r11
    CMP r16, r15
    BGE r0, scan_done_mt

    ; Check if current position starts with "MemTotal:"
    MOV r20, r10
    ADD r20, r11
    LOAD r14, r20        ; char at position
    LDI r15, 77          ; 'M'
    CMP r14, r15
    JNZ r0, scan_next_mt

    ; Check next char 'e'
    LDI r15, 1
    ADD r20, r15
    LOAD r14, r20
    LDI r15, 101         ; 'e'
    CMP r14, r15
    JNZ r0, scan_next_mt

    ; Check 'm'
    LDI r15, 2
    MOV r16, r10
    ADD r16, r11
    ADD r16, r15
    LOAD r14, r16
    LDI r15, 109         ; 'm'
    CMP r14, r15
    JNZ r0, scan_next_mt

    ; Found "Mem" - assume MemTotal. Skip to number after ":"
    MOV r20, r10
    ADD r20, r11
    LDI r15, 9           ; skip "MemTotal:"
    ADD r20, r15
    ; Skip spaces
skip_spaces_mt:
    LOAD r14, r20
    LDI r15, 32          ; space
    CMP r14, r15
    JNZ r0, parse_num_mt
    LDI r15, 1
    ADD r20, r15
    JMP skip_spaces_mt

parse_num_mt:
    ; Parse decimal number at r20 into r14
    LDI r14, 0           ; accumulated number
parse_digit_mt:
    PUSH r20
    LOAD r15, r20        ; char
    LDI r16, 48          ; '0'
    CMP r15, r16
    BLT r0, parse_done_mt
    LDI r16, 57          ; '9'
    CMP r15, r16
    BGE r0, parse_next_chk_mt
    JMP parse_done_mt
parse_next_chk_mt:
    ; digit = char - '0'
    LDI r16, 48
    SUB r15, r16
    ; result = result * 10 + digit
    LDI r16, 10
    MUL r14, r16
    ADD r14, r15
    POP r20
    LDI r15, 1
    ADD r20, r15
    JMP parse_digit_mt

parse_done_mt:
    POP r20
    LDI r12, 1           ; found flag
    JMP scan_done_mt

scan_next_mt:
    LDI r15, 1
    ADD r11, r15
    JMP scan_memtotal

scan_done_mt:
    ; Display MemTotal if found
    CMP r12, r0          ; r0 is 0 from CMP
    LDI r15, 0
    CMP r12, r15
    JZ r0, show_meminfo_err

    ; r14 has MemTotal in kB. Display as MB: divide by 1024
    LDI r15, 1024
    DIV r14, r15

    LDI r20, BUF
    STRO r20, "Total RAM"
    LDI r1, 136
    LDI r2, 48
    LDI r3, BUF
    LDI r4, COL_LABEL
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    LDI r20, NUMBUF
    CALL int_to_str
    LDI r1, 200
    LDI r2, 48
    LDI r3, NUMBUF
    LDI r4, COL_GREEN
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    LDI r20, BUF
    STRO r20, "MB"
    LDI r1, 232
    LDI r2, 48
    LDI r3, BUF
    LDI r4, COL_GREEN
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    ; Memory bar (visual indicator)
    LDI r1, 136
    LDI r2, 62
    LDI r3, 112
    LDI r4, 10
    LDI r5, COL_BAR_BG
    RECTF r1, r2, r3, r4, r5

    ; Fill bar proportional (assume r14 is MB, max ~16GB=16384MB)
    ; bar_width = r14 * 112 / 16384
    LDI r15, 112
    MUL r14, r15
    LDI r15, 16384
    DIV r14, r15
    ; Cap at 112
    LDI r15, 112
    CMP r14, r15
    BLT r0, draw_bar
    MOV r14, r15

draw_bar:
    LDI r1, 136
    LDI r2, 62
    MOV r3, r14
    LDI r4, 10
    LDI r5, COL_BAR_FG
    RECTF r1, r2, r3, r4, r5

    JMP host_panel_done

show_meminfo_err:
meminfo_error:
    LDI r20, BUF
    STRO r20, "Cannot read"
    LDI r1, 136
    LDI r2, 48
    LDI r3, BUF
    LDI r4, COL_YELLOW
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    LDI r20, BUF
    STRO r20, "/proc/meminfo"
    LDI r1, 136
    LDI r2, 62
    LDI r3, BUF
    LDI r4, COL_YELLOW
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

meminfo_close:
    ; Try to close if we have a handle
    LDI r20, HANDLE
    LOAD r15, r20
    LDI r16, 0xFFFFFFFF
    CMP r15, r16
    JZ r0, host_panel_done
    MOV r1, r15
    FSCLOSE r1

host_panel_done:
    ; ---- Bottom panel: VM internals ----
    LDI r1, 4
    LDI r2, 134
    LDI r3, 248
    LDI r4, 50
    LDI r5, COL_DARK
    RECTF r1, r2, r3, r4, r5

    LDI r20, BUF
    STRO r20, "VM Registers"
    LDI r1, 8
    LDI r2, 136
    LDI r3, BUF
    LDI r4, COL_CYAN
    LDI r5, COL_DARK
    DRAWTEXT r1, r2, r3, r4, r5

    ; Show current PC
    LDI r20, BUF
    STRO r20, "PC"
    LDI r1, 8
    LDI r2, 152
    LDI r3, BUF
    LDI r4, COL_LABEL
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    ; Display via PROCINFO (main process, field=1=PC)
    LDI r1, 0            ; PID 0
    LDI r2, 1            ; field = PC
    PROCINFO r1, r2
    MOV r14, r0
    LDI r20, NUMBUF
    CALL hex_to_str
    LDI r1, 40
    LDI r2, 152
    LDI r3, NUMBUF
    LDI r4, COL_GREEN
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    ; Show opcode histogram top entry
    LDI r20, BUF
    STRO r20, "VM Status: Running"
    LDI r1, 80
    LDI r2, 152
    LDI r3, BUF
    LDI r4, COL_GREEN
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    ; ---- Footer ----
    LDI r1, 0
    LDI r2, 236
    LDI r3, 256
    LDI r4, 20
    LDI r5, COL_TITLE
    RECTF r1, r2, r3, r4, r5

    LDI r20, BUF
    STRO r20, "Geometry OS v1.0  |  Q:Quit"
    LDI r1, 24
    LDI r2, 242
    LDI r3, BUF
    LDI r4, COL_GRAY
    LDI r5, COL_TITLE
    DRAWTEXT r1, r2, r3, r4, r5

    ; Handle input
    IKEY r15
    JZ r15, no_input
    LDI r16, 81          ; 'Q'
    CMP r15, r16
    JNZ r0, no_input
    HALT

no_input:
    FRAME
    JMP main_loop


; =========================================
; int_to_str subroutine
; Converts r14 (u32) to decimal string at r20
; Returns null-terminated string. r0 = start address.
; =========================================
int_to_str:
    PUSH r31
    PUSH r20
    LDI r16, 0

    JZ r14, its_zero

its_loop:
    MOV r18, r14
    LDI r17, 10
    MOD r18, r17
    LDI r19, 48
    ADD r18, r19
    PUSH r18
    LDI r17, 10
    DIV r14, r17
    LDI r19, 1
    ADD r16, r19
    JNZ r14, its_loop

its_write:
    POP r18
    STORE r20, r18
    LDI r19, 1
    ADD r20, r19
    LDI r19, 1
    SUB r16, r19
    JNZ r16, its_write

    LDI r18, 0
    STORE r20, r18
    POP r0
    POP r31
    RET

its_zero:
    LDI r18, 48
    STORE r20, r18
    LDI r19, 1
    ADD r20, r19
    LDI r18, 0
    STORE r20, r18
    POP r0
    POP r31
    RET

; =========================================
; hex_to_str subroutine
; Converts r14 (u32) to hex string "0x" prefix at r20
; Returns null-terminated. r0 = start address.
; =========================================
hex_to_str:
    PUSH r31
    PUSH r20

    LDI r18, 48
    STORE r20, r18
    LDI r19, 1
    ADD r20, r19
    LDI r18, 120
    STORE r20, r18
    ADD r20, r19

    ; 8 hardcoded nibble extractions
    MOV r18, r14
    LDI r19, 28
    SHR r18, r19
    LDI r19, 0xF
    AND r18, r19
    CALL hex_nibble_s

    MOV r18, r14
    LDI r19, 24
    SHR r18, r19
    LDI r19, 0xF
    AND r18, r19
    CALL hex_nibble_s

    MOV r18, r14
    LDI r19, 20
    SHR r18, r19
    LDI r19, 0xF
    AND r18, r19
    CALL hex_nibble_s

    MOV r18, r14
    LDI r19, 16
    SHR r18, r19
    LDI r19, 0xF
    AND r18, r19
    CALL hex_nibble_s

    MOV r18, r14
    LDI r19, 12
    SHR r18, r19
    LDI r19, 0xF
    AND r18, r19
    CALL hex_nibble_s

    MOV r18, r14
    LDI r19, 8
    SHR r18, r19
    LDI r19, 0xF
    AND r18, r19
    CALL hex_nibble_s

    MOV r18, r14
    LDI r19, 4
    SHR r18, r19
    LDI r19, 0xF
    AND r18, r19
    CALL hex_nibble_s

    MOV r18, r14
    LDI r19, 0xF
    AND r18, r19
    CALL hex_nibble_s

    LDI r18, 0
    STORE r20, r18
    POP r0
    POP r31
    RET

hex_nibble_s:
    PUSH r31
    LDI r19, 10
    CMP r18, r19
    BGE r0, hex_alpha_s
    LDI r19, 48
    ADD r18, r19
    JMP hex_store_s
hex_alpha_s:
    LDI r19, 55
    ADD r18, r19
hex_store_s:
    STORE r20, r18
    LDI r19, 1
    ADD r20, r19
    POP r31
    RET
