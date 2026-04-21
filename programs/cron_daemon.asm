; cron_daemon.asm -- Phase 101: Cron Daemon
;
; A frame-based cron daemon that reads a schedule from VFS and spawns
; programs at regular intervals. Like Unix cron but using frame counts.
;
; Crontab format: "interval program_name\n" per line (text)
; Example: "100 hello\n" means run hello.asm every 100 frames
;
; Memory layout:
;   0x1000: crontab filename string ("crontab")
;   0x2000: read buffer for crontab file contents
;   0x4000-0x43FF: schedule table (8 entries, 64 words each)
;     Entry: [interval(1), last_run(1), name_len(1), padding(1), name(60)]
;   0x5000: program name buffer for EXEC
;   0xFA0: reload flag
;
; Uses: LDI, STORE, LOAD, MOV, ADD, SUB, MUL, AND, CMP, BLT, BGE,
;   JZ, JNZ, JMP, CALL, RET, PUSH, POP, OPEN, READ, CLOSE, FRAME, EXEC

.org 0x000

; ═══════════════════════════════════════════════════════════════
; Phase 1: Initialize
    ; Clear screen
    LDI r0, 0
    FILL r0

    ; Initialize stack pointer
    LDI r30, 0xFF00

    ; Write "crontab" filename to 0x1000
    LDI r10, 0x1000
    LDI r11, 99
    STORE r10, r11
    LDI r10, 0x1001
    LDI r11, 114
    STORE r10, r11
    LDI r10, 0x1002
    LDI r11, 111
    STORE r10, r11
    LDI r10, 0x1003
    LDI r11, 110
    STORE r10, r11
    LDI r10, 0x1004
    LDI r11, 116
    STORE r10, r11
    LDI r10, 0x1005
    LDI r11, 97
    STORE r10, r11
    LDI r10, 0x1006
    LDI r11, 98
    STORE r10, r11
    LDI r10, 0x1007
    LDI r11, 0
    STORE r10, r11

    ; Clear schedule table (512 words at 0x4000)
    LDI r10, 0x4000
    LDI r12, 0
    LDI r15, 0x4200
clear_table:
    STORE r10, r12
    LDI r13, 1
    ADD r10, r13
    CMP r10, r15
    BLT r0, clear_table

    ; ═══════════════════════════════════════════════════════════════
    ; Phase 2: Read and parse crontab
    ; ═══════════════════════════════════════════════════════════════
read_crontab:
    LDI r1, 0x1000
    LDI r2, 0
    OPEN r1, r2
    MOV r5, r0

    ; Check error
    LDI r10, 0xFFFFFFFF
    CMP r5, r10
    JZ r0, no_crontab

    ; Read into 0x2000
    LDI r3, 0x2000
    LDI r4, 512
    READ r5, r3, r4

    CLOSE r5

    ; Parse entries
    CALL parse_crontab

no_crontab:
    ; ═══════════════════════════════════════════════════════════════
    ; Phase 3: Main loop -- check schedule, spawn jobs
    ; ═══════════════════════════════════════════════════════════════
main_loop:
    ; Get current TICKS
    LDI r10, 0xFFE
    LOAD r10, r10

    ; Check each entry (max 8)
    LDI r11, 0
check_entry:
    ; Entry base = 0x4000 + index * 64
    MOV r12, r11
    LDI r13, 64
    MUL r12, r13       ; r12 = index * 64
    LDI r13, 0x4000
    ADD r12, r13       ; r12 = 0x4000 + index * 64

    ; interval = entry[0]
    LOAD r14, r12
    JZ r14, next_entry

    ; last_run = entry[1]
    MOV r15, r12
    LDI r16, 1
    ADD r15, r16
    LOAD r16, r15

    ; elapsed = ticks - last_run
    MOV r17, r10
    SUB r17, r16
    CMP r17, r14
    BLT r0, next_entry

    ; Time to spawn! Copy name to 0x5000
    PUSH r31
    CALL copy_name
    POP r31

    ; EXEC
    LDI r1, 0x5000
    EXEC r1

    ; Update last_run
    STORE r15, r10

next_entry:
    LDI r13, 1
    ADD r11, r13
    LDI r13, 8
    CMP r11, r13
    BLT r0, check_entry

    ; Check reload flag
    LDI r10, 0xFA0
    LOAD r10, r10
    LDI r11, 0x43524F
    CMP r10, r11
    JNZ r0, skip_reload
    LDI r10, 0xFA0
    LDI r11, 0
    STORE r10, r11
    JMP read_crontab

skip_reload:
    FRAME
    JMP main_loop

; ═══════════════════════════════════════════════════════════════
; parse_crontab: Parse buffer at 0x2000 into table at 0x4000
; ═══════════════════════════════════════════════════════════════
parse_crontab:
    PUSH r31

    LDI r10, 0x2000    ; source ptr
    LDI r11, 0         ; entry index (0..7)

parse_line:
    ; Skip whitespace/newlines
skip_ws:
    LOAD r12, r10
    JZ r12, parse_done ; null = end
    LDI r13, 10
    CMP r12, r13
    JZ r0, advance_skip
    LDI r13, 13
    CMP r12, r13
    JZ r0, advance_skip
    LDI r13, 32
    CMP r12, r13
    JNZ r0, try_digit
advance_skip:
    LDI r13, 1
    ADD r10, r13
    JMP skip_ws

try_digit:
    ; Check if it's a digit (48-57)
    LDI r13, 48
    CMP r12, r13
    BLT r0, skip_line  ; < '0' -> skip line
    LDI r13, 58
    CMP r12, r13
    BGE r0, skip_line  ; > '9' -> skip line

    ; Parse decimal number into r14
    LDI r14, 0
    LDI r15, 10
parse_digit:
    ; r12 already loaded
    ; Check digit range
    LDI r16, 48
    CMP r12, r16
    BLT r0, got_interval
    LDI r16, 58
    CMP r12, r16
    BGE r0, got_interval

    ; r14 = r14 * 10 + (digit - 48)
    MUL r14, r15
    LDI r16, 48
    SUB r12, r16
    ADD r14, r12

    LDI r13, 1
    ADD r10, r13
    LOAD r12, r10
    JMP parse_digit

got_interval:
    ; Skip space after interval
    LDI r13, 32
    CMP r12, r13
    JNZ r0, skip_line  ; no space after interval = malformed
    LDI r13, 1
    ADD r10, r13       ; skip space

    ; Compute entry base
    LDI r15, 64
    MOV r16, r11
    MUL r16, r15
    LDI r17, 0x4000
    ADD r16, r17       ; r16 = entry base

    ; Store interval
    STORE r16, r14

    ; Store last_run = 0
    MOV r17, r16
    LDI r18, 1
    ADD r17, r18
    LDI r18, 0
    STORE r17, r18

    ; Copy name at offset 4
    MOV r17, r16
    LDI r18, 4
    ADD r17, r18       ; r17 = dest for name
    LDI r19, 0         ; name len

copy_name_char:
    LOAD r12, r10
    JZ r12, end_copy_name
    LDI r13, 10
    CMP r12, r13
    JZ r0, end_copy_name
    LDI r13, 13
    CMP r12, r13
    JZ r0, end_copy_name

    STORE r17, r12
    LDI r13, 1
    ADD r10, r13
    ADD r17, r13
    ADD r19, r13
    LDI r20, 56
    CMP r19, r20
    BGE r0, end_copy_name
    JMP copy_name_char

end_copy_name:
    ; Store name_len at offset 2
    MOV r17, r16
    LDI r18, 2
    ADD r17, r18
    STORE r17, r19

    ; Next entry
    LDI r13, 1
    ADD r11, r13
    LDI r13, 8
    CMP r11, r13
    BGE r0, parse_done

    ; Fall through to skip rest of line

skip_line:
    ; Skip to next newline or null
    LOAD r12, r10
    JZ r12, parse_done
    LDI r13, 10
    CMP r12, r13
    JZ r0, skip_newline
    LDI r13, 1
    ADD r10, r13
    JMP skip_line

skip_newline:
    LDI r13, 1
    ADD r10, r13
    JMP parse_line

parse_done:
    POP r31
    RET

; ═══════════════════════════════════════════════════════════════
; copy_name: Copy entry name to 0x5000 for EXEC
; Input: r12 = entry base, r16 clobbered
; ═══════════════════════════════════════════════════════════════
copy_name:
    PUSH r31

    ; Get name_len at offset 2
    MOV r16, r12
    LDI r17, 2
    ADD r16, r17
    LOAD r17, r16      ; r17 = name length

    ; Copy name from offset 4
    MOV r16, r12
    LDI r18, 4
    ADD r16, r18
    LDI r18, 0x5000
    LDI r19, 0

cpn_loop:
    CMP r19, r17
    BGE r0, cpn_done
    LOAD r20, r16
    STORE r18, r20
    LDI r20, 1
    ADD r16, r20
    ADD r18, r20
    ADD r19, r20
    JMP cpn_loop

cpn_done:
    LDI r20, 0
    STORE r18, r20

    POP r31
    RET
