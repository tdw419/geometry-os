; ai_terminal.asm -- AI Chat Terminal for Geometry OS
;
; Connects to cloud AI (ZAI) via LLM opcode (0x9C).
; Type a message, press Enter to send. Response appears below.
; Conversation history is maintained for context.
; /run assembles the last AI response. /yes confirms and executes it.
; AI-written bytecode has full VM privileges, so the /run + /yes two-step
; is intentional -- any other input between the two cancels the pending run.
; Commands: /clear /help /sys /run /yes
;
; RAM Layout:
;   0x4000-0x44EB  Text buffer (42*30 = 1260 u32 cells, row-major)
;   0x4800         Cursor column
;   0x4801         Cursor row
;   0x4802         Blink counter
;   0x5000-0x50FF  Scratch line buffer (256 chars + null)
;   0x5400-0x63FF  LLM prompt buffer (4K)
;   0x6400-0x73FF  LLM response buffer (4K)
;   0x7400-0x74FF  Conversation history (last response only, for context)
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
#define PROMPT_BUF 0x5400
#define RESP_BUF 0x6400
#define HISTORY 0x7400
#define ASM_STATUS 0xFFD
#define RUN_PENDING 0x7830   ; Nonzero = compiled bytecode waiting for /yes

; =========================================
; INIT
; =========================================
LDI r1, 1
LDI r30, 0xFD00   ; Stack pointer

; Request asm_dev system prompt for every LLM call from this app.
; RAM[0x7820] = 1 tells build_llm_system_prompt() to send the GeoOS
; assembly-programmer prompt instead of the Oracle world-guide prompt.
LDI r20, 0x7820
LDI r0, 1
STORE r20, r0

; Clear screen
LDI r0, 0x0A0A14
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

; Clear prompt/response/history buffers
LDI r12, PROMPT_BUF
LDI r16, 4096
CALL clear_buf
LDI r12, RESP_BUF
LDI r16, 4096
CALL clear_buf
LDI r12, HISTORY
LDI r16, 256
CALL clear_buf

; Title bar
LDI r1, 0
LDI r2, 0
LDI r3, 256
LDI r4, 16
LDI r5, 0x1A0033
RECTF r1, r2, r3, r4, r5

; Title text "AI Terminal - ZAI"
LDI r20, SCRATCH
STRO r20, "AI Terminal - ZAI"
LDI r1, 4
LDI r2, 4
LDI r3, SCRATCH
LDI r4, 0x00FF00  ; green
LDI r5, 0x1A0033  ; match title bar
DRAWTEXT r1, r2, r3, r4, r5

; Status indicator dot (green = connected)
LDI r1, 230
LDI r2, 4
LDI r3, 8
LDI r4, 8
LDI r5, 0x00FF00
RECTF r1, r2, r3, r4, r5

; Close button hit region
LDI r1, 220
LDI r2, 0
LDI r3, 36
LDI r4, 16
HITSET r1, r2, r3, r4, 99

LDI r1, 1

; Write prompt "> " at buffer row 0
LDI r20, BUF
LDI r0, 62           ; '>'
STORE r20, r0
ADD r20, r1
LDI r0, 32           ; ' '
STORE r20, r0

; Set cursor to col 2
LDI r20, CUR_COL
LDI r0, 2
STORE r20, r0

; Welcome message
LDI r20, SCRATCH
STRO r20, "AI Terminal v1.0"
CALL write_line_to_buf
LDI r1, 1
LDI r20, SCRATCH
STRO r20, "Type a message, Enter to send"
CALL write_line_to_buf
LDI r1, 1
LDI r20, SCRATCH
STRO r20, "/help for commands"
CALL write_line_to_buf
LDI r1, 1

; Write prompt on new line
CALL write_prompt

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
    LDI r5, 0x0A0A14
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
    JNZ r0, check_ai_prefix
    LDI r14, 0x44FF44  ; green for user input
    JMP render_text

check_ai_prefix:
    ; Check if row starts with 'A' + 'I' + ':' (AI response) -> cyan
    LDI r16, SCRATCH
    LOAD r6, r16
    CMPI r6, 65         ; 'A'
    JNZ r0, render_text_default
    ADD r16, r1
    LOAD r6, r16
    CMPI r6, 73         ; 'I'
    JNZ r0, render_text_default
    ADD r16, r1
    LOAD r6, r16
    CMPI r6, 58         ; ':'
    JNZ r0, render_text_default
    LDI r14, 0x00FFFF  ; cyan for AI response
    JMP render_text

render_text_default:
    ; Check for '/' commands -> yellow
    LDI r16, SCRATCH
    LOAD r6, r16
    CMPI r6, 47         ; '/'
    JNZ r0, render_text
    LDI r14, 0xFFFF44  ; yellow for commands

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
; DO_ENTER -- send to LLM or handle command
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
    JNZ r0, send_to_llm  ; not a command, send to LLM

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
    JNZ r0, send_to_llm
    JMP cmd_help

try_clear_cmd:
    ; Check /clear
    LDI r20, SCRATCH
    ADD r20, r1           ; skip '/'
    LOAD r22, r20
    CMPI r22, 99         ; 'c'
    JNZ r0, try_sys_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 108        ; 'l'
    JNZ r0, try_sys_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 101        ; 'e'
    JNZ r0, try_sys_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 97         ; 'a'
    JNZ r0, try_sys_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 114        ; 'r'
    JNZ r0, try_sys_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null
    JNZ r0, send_to_llm
    JMP cmd_clear

try_sys_cmd:
    ; Check /sys
    LDI r20, SCRATCH
    ADD r20, r1           ; skip '/'
    LOAD r22, r20
    CMPI r22, 115        ; 's'
    JNZ r0, try_run_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 121        ; 'y'
    JNZ r0, try_run_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 115        ; 's'
    JNZ r0, try_run_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null
    JNZ r0, try_run_cmd
    JMP cmd_sys

try_run_cmd:
    ; Check /run
    LDI r20, SCRATCH
    ADD r20, r1           ; skip '/'
    LOAD r22, r20
    CMPI r22, 114        ; 'r'
    JNZ r0, try_yes_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 117        ; 'u'
    JNZ r0, try_yes_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 110        ; 'n'
    JNZ r0, try_yes_cmd
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null
    JNZ r0, try_yes_cmd
    JMP cmd_run

try_yes_cmd:
    ; Check /yes -- confirms a pending /run
    LDI r20, SCRATCH
    ADD r20, r1           ; skip '/'
    LOAD r22, r20
    CMPI r22, 121        ; 'y'
    JNZ r0, send_to_llm
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 101        ; 'e'
    JNZ r0, send_to_llm
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 115        ; 's'
    JNZ r0, send_to_llm
    ADD r20, r1
    LOAD r22, r20
    CMPI r22, 0          ; null
    JNZ r0, send_to_llm
    JMP cmd_yes

; =========================================
; SEND_TO_LLM
; Build prompt from user input + history, call LLM, display response
; =========================================
send_to_llm:
    LDI r1, 1
    ; Cancel any pending /run -- a new chat turn supersedes prior output.
    LDI r20, RUN_PENDING
    LDI r0, 0
    STORE r20, r0

    ; Show "Thinking..." status
    LDI r20, SCRATCH
    STRO r20, "Thinking..."
    CALL write_line_to_buf

    ; Build prompt at PROMPT_BUF:
    ; "You are a helpful AI inside Geometry OS, a pixel-based OS.\n\n"
    ; If history exists, append it as context
    ; Then append "User: <input>\nAI:"

    LDI r19, PROMPT_BUF

    ; System context
    STRO r19, "You are an AI assistant inside Geometry OS. Be concise. You help build programs and answer questions about the OS."
    CALL advance_to_null

    ; Append newline
    LDI r12, 10
    STORE r19, r12
    ADDI r19, 1
    STORE r19, r12
    ADDI r19, 1

    ; Check if we have conversation history
    LDI r12, HISTORY
    LOAD r13, r12
    LDI r14, 0
    CMP r13, r14
    JZ r0, skip_history

    ; Append history context
    STRO r19, "Previous: "
    CALL advance_to_null
    LDI r18, HISTORY
    CALL copy_until_null

    ; Append newline
    LDI r12, 10
    STORE r19, r12
    ADDI r19, 1

skip_history:
    ; Append "User: " + input from SCRATCH
    STRO r19, "User: "
    CALL advance_to_null
    LDI r18, SCRATCH
    CALL copy_until_null

    ; Append "\nAnswer briefly."
    LDI r12, 10
    STORE r19, r12
    ADDI r19, 1
    STRO r19, "Answer briefly."
    CALL advance_to_null
    LDI r12, 0
    STORE r19, r12

    ; Call LLM: prompt at PROMPT_BUF, response at RESP_BUF, max 3840 chars
    LDI r3, PROMPT_BUF
    LDI r4, RESP_BUF
    LDI r5, 3840
    LLM r3, r4, r5

    ; r0 = response length
    LDI r1, 1

    ; Write "AI: " prefix then response lines
    LDI r20, SCRATCH
    STRO r20, "AI: "
    ; Append first part of response
    LDI r21, RESP_BUF
    LDI r22, 0
    LDI r23, 38          ; max chars to append to first line (42 - 4)
append_first:
    LOAD r0, r21
    LDI r6, 0
    CMP r0, r6
    JZ r0, first_done
    LDI r6, 10
    CMP r0, r6
    JZ r0, first_done
    ADD r20, r1
    ADDI r22, 1
    STORE r20, r0
    ADD r21, r1
    CMPI r22, 38
    JNZ r0, append_first

first_done:
    LDI r0, 0
    ADD r20, r1
    STORE r20, r0

    CALL write_line_to_buf
    LDI r1, 1

    ; Write remaining response lines (word-wrap at COLS-2)
    LDI r25, 0           ; offset into response
    ; Skip chars already written in first line
    LDI r26, 0
skip_written:
    CMPI r26, 38
    BGE r0, write_remaining
    LOAD r0, r21
    LDI r6, 0
    CMP r0, r6
    JZ r0, llm_done
    ADD r21, r1
    ADDI r26, 1
    JMP skip_written

write_remaining:
    ; Check if there are more chars
    LOAD r0, r21
    LDI r6, 0
    CMP r0, r6
    JZ r0, llm_done

    ; Write COLS chars per line
    LDI r20, SCRATCH
    LDI r22, 0
wrap_loop:
    LOAD r0, r21
    LDI r6, 0
    CMP r0, r6
    JZ r0, wrap_line_done
    LDI r6, 10
    CMP r0, r6
    JNZ r0, wrap_no_newline
    ; Skip newline char, finish this line
    ADD r21, r1
    JMP wrap_line_done
wrap_no_newline:
    STORE r20, r0
    ADD r20, r1
    ADD r21, r1
    ADDI r22, 1
    CMPI r22, COLS
    JNZ r0, wrap_loop

wrap_line_done:
    LDI r0, 0
    STORE r20, r0
    CALL write_line_to_buf
    LDI r1, 1
    JMP write_remaining

llm_done:
    ; Save last response to HISTORY (first 200 chars)
    LDI r20, HISTORY
    LDI r21, RESP_BUF
    LDI r22, 0
save_history:
    CMPI r22, 200
    BGE r0, history_done
    LOAD r0, r21
    STORE r20, r0
    LDI r6, 0
    CMP r0, r6
    JZ r0, history_done
    ADD r20, r1
    ADD r21, r1
    ADDI r22, 1
    JMP save_history

history_done:
    ; Write prompt on new line
    CALL write_prompt
    JMP hk_ret

; =========================================
; COMMANDS
; =========================================

cmd_help:
    LDI r1, 1
    ; Cancel any pending /run -- asking for help is not confirmation.
    LDI r20, RUN_PENDING
    LDI r0, 0
    STORE r20, r0
    LDI r20, SCRATCH
    STRO r20, "Commands: /help /clear /sys /run /yes"
    CALL write_line_to_buf
    LDI r1, 1
    LDI r20, SCRATCH
    STRO r20, "Type to chat. /run compiles AI code,"
    CALL write_line_to_buf
    LDI r1, 1
    LDI r20, SCRATCH
    STRO r20, "/yes confirms and executes it."
    CALL write_line_to_buf
    LDI r1, 1
    CALL write_prompt
    JMP hk_ret

cmd_clear:
    LDI r1, 1
    ; Cancel any pending /run.
    LDI r20, RUN_PENDING
    LDI r0, 0
    STORE r20, r0
    ; Clear text buffer to spaces
    LDI r20, BUF
    LDI r6, 32
cc_clear:
    STORE r20, r6
    ADD r20, r1
    CMPI r20, BUF_END
    BLT r0, cc_clear

    ; Reset cursor
    LDI r20, CUR_ROW
    LDI r0, 0
    STORE r20, r0
    LDI r20, CUR_COL
    LDI r0, 0
    STORE r20, r0

    ; Clear history
    LDI r12, HISTORY
    LDI r16, 256
    CALL clear_buf

    LDI r1, 1
    LDI r20, SCRATCH
    STRO r20, "Cleared."
    CALL write_line_to_buf
    LDI r1, 1
    CALL write_prompt
    JMP hk_ret

cmd_sys:
    LDI r1, 1
    ; Cancel any pending /run.
    LDI r20, RUN_PENDING
    LDI r0, 0
    STORE r20, r0
    LDI r20, SCRATCH
    STRO r20, "Geometry OS v2.0"
    CALL write_line_to_buf
    LDI r1, 1
    LDI r20, SCRATCH
    STRO r20, "AI Terminal via ZAI"
    CALL write_line_to_buf
    LDI r1, 1
    LDI r20, SCRATCH
    STRO r20, "LLM opcode 0x9C"
    CALL write_line_to_buf
    LDI r1, 1
    CALL write_prompt
    JMP hk_ret

cmd_run:
    LDI r1, 1

    ; Check if there's a response to run
    LDI r20, RESP_BUF
    LOAD r0, r20
    LDI r6, 0
    CMP r0, r6
    JNZ r0, run_has_resp

    ; No response -- show error
    LDI r20, SCRATCH
    STRO r20, "No AI response to run"
    CALL write_line_to_buf
    LDI r1, 1
    CALL write_prompt
    JMP hk_ret

run_has_resp:
    ; Show status
    LDI r20, SCRATCH
    STRO r20, "Assembling response..."
    CALL write_line_to_buf
    LDI r1, 1

    ; ASM_RAM uses RESP_BUF address -- the opcode strips ``` fences automatically
    LDI r10, RESP_BUF
    ASM_RAM r10

    ; Check status at RAM[0xFFD]
    LDI r20, ASM_STATUS
    LOAD r0, r20
    LDI r6, 0
    LDI r7, 0xFFFFFFFF
    CMP r0, r7
    JNZ r0, run_check_zero

    ; Assembly failed
    LDI r20, SCRATCH
    STRO r20, "Assembly failed!"
    CALL write_line_to_buf
    LDI r1, 1
    LDI r20, SCRATCH
    STRO r20, "(Check AI output for errors)"
    CALL write_line_to_buf
    LDI r1, 1
    CALL write_prompt
    JMP hk_ret

run_check_zero:
    CMP r0, r6
    JZ r0, run_failed_empty

    ; Success -- r0 = word count. Park it in RUN_PENDING and wait for /yes.
    ; AI-generated code has full VM privileges (files, network, shutdown),
    ; so we require an explicit second keystroke before RUNNEXT.
    LDI r20, RUN_PENDING
    STORE r20, r0
    LDI r1, 1

    LDI r20, SCRATCH
    STRO r20, "Compiled OK. /yes to run."
    CALL write_line_to_buf
    LDI r1, 1
    LDI r20, SCRATCH
    STRO r20, "Any other input cancels."
    CALL write_line_to_buf
    LDI r1, 1
    CALL write_prompt
    JMP hk_ret

run_failed_empty:
    ; Treat zero-word output as a failure -- nothing to run.
    LDI r20, RUN_PENDING
    LDI r0, 0
    STORE r20, r0
    LDI r20, SCRATCH
    STRO r20, "Assembly produced 0 words"
    CALL write_line_to_buf
    LDI r1, 1
    CALL write_prompt
    JMP hk_ret

cmd_yes:
    LDI r1, 1
    ; Check RUN_PENDING -- if zero, nothing to confirm.
    LDI r20, RUN_PENDING
    LOAD r0, r20
    LDI r6, 0
    CMP r0, r6
    JZ r0, yes_nothing_pending

    ; Clear pending flag then jump into bytecode at 0x1000.
    LDI r20, RUN_PENDING
    LDI r6, 0
    STORE r20, r6
    LDI r20, SCRATCH
    STRO r20, "Running..."
    CALL write_line_to_buf
    LDI r1, 1
    RUNNEXT
    ; RUNNEXT never returns.
    JMP hk_ret

yes_nothing_pending:
    LDI r20, SCRATCH
    STRO r20, "Nothing to confirm. Use /run first."
    CALL write_line_to_buf
    LDI r1, 1
    CALL write_prompt
    JMP hk_ret

; =========================================
; WRITE_PROMPT -- write "> " on current row
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
; Copy chars from BUF[row*COLS+2 .. row*COLS+col-1] into SCRATCH
; Null terminate.
; =========================================
extract_cmd:
    PUSH r31
    LDI r1, 1

    ; Compute base = row * COLS
    LDI r20, CUR_ROW
    LOAD r6, r20
    LDI r7, COLS
    MUL r6, r7

    ; Get end position (cursor col)
    LDI r20, CUR_COL
    LOAD r7, r20

    ; Source starts at col 2 (skip "> ")
    LDI r20, BUF
    ADD r20, r6
    ADD r20, r1
    ADD r20, r1

    ; Destination
    LDI r21, SCRATCH

    ; Copy loop: copy chars from col 2 to cursor col
    LDI r22, 2            ; current column index
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
; Write null-terminated string from SCRATCH to current buffer row
; Then advance to next line
; =========================================
write_line_to_buf:
    PUSH r31
    LDI r1, 1

    ; Compute dest: BUF + row*COLS
    LDI r20, CUR_ROW
    LOAD r2, r20
    LDI r3, COLS
    MUL r2, r3
    LDI r20, BUF
    ADD r20, r2

    ; Source
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
    LDI r20, CUR_COL
    LOAD r0, r20
    ; Don't go past column 2 (the "> " prompt)
    CMPI r0, 2
    JNZ r0, hk_ret       ; if col <= 2, do nothing (JNZ after CMPI means not-equal)
    ; Actually: CMPI r0, 2 sets r0 to 0 if equal. So JNZ r0 means "if not 2"
    ; We want to proceed only if r0 > 2
    LDI r1, 1
    LDI r20, CUR_COL
    LOAD r0, r20
    CMPI r0, 2
    JZ r0, hk_ret        ; at prompt, do nothing

    SUBI r0, 1
    STORE r20, r0
    ; Clear char at new position
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
; Shift all text rows up by 1, clear last row
; =========================================
scroll_up:
    PUSH r31
    LDI r1, 1
    LDI r10, 0
scroll_loop:
    CMPI r10, 29
    BGE r0, scroll_clear

    ; Source: BUF + (row+1)*COLS
    LDI r20, BUF
    LDI r0, 0
    ADD r0, r10
    ADD r0, r1
    LDI r11, COLS
    MUL r0, r11
    ADD r20, r0

    ; Dest: BUF + row*COLS
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
; UTILITY: advance_to_null (r19)
; =========================================
advance_to_null:
    PUSH r31
adv_loop:
    LOAD r12, r19
    LDI r13, 0
    CMP r12, r13
    JZ r0, adv_done
    ADDI r19, 1
    JMP adv_loop
adv_done:
    POP r31
    RET

; =========================================
; UTILITY: copy_until_null (r18 -> r19)
; =========================================
copy_until_null:
    PUSH r31
cun_loop:
    LOAD r12, r18
    LDI r13, 0
    CMP r12, r13
    JZ r0, cun_done
    STORE r19, r12
    ADDI r18, 1
    ADDI r19, 1
    JMP cun_loop
cun_done:
    LDI r12, 0
    STORE r19, r12
    POP r31
    RET

; =========================================
; UTILITY: clear_buf (r12=start, r16=count)
; =========================================
clear_buf:
    PUSH r31
    PUSH r12
    PUSH r16
    LDI r13, 0
cb_loop:
    JZ r16, cb_done
    STORE r12, r13
    ADDI r12, 1
    SUBI r16, 1
    JMP cb_loop
cb_done:
    POP r16
    POP r12
    POP r31
    RET

hk_ret:
    POP r31
    RET
