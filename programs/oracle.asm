; oracle.asm -- Map-aware AI Oracle building
; Walk in, ask questions about the world. The LLM knows your position and
; nearby buildings. Type a question, press Tab to ask. Response appears below.
; Uses LLM opcode (0x9C) to send map-aware prompts to external AI.
;
; Memory layout:
;   0x3000-0x30FF: Context string (player pos, nearby buildings)
;   0x3100-0x32FF: User input buffer (typed question)
;   0x3300-0x36FF: Response buffer (LLM response, up to 1K)
;   0x3700-0x37FF: Status line
;   0x3800-0x3820: Title text
;   0x3900-0x39FF: Full prompt (context + question) sent to LLM
;   0x3E00-0x3FFF: Temp digit buffer / scratch
;
; Registers:
;   r0: CMP result (reserved)
;   r1: Key input
;   r3: Prompt addr (0x3900)
;   r4: Response addr (0x3300)
;   r5: Max response length (768)
;   r7: Constant 1
;   r8: Constant 9 (Tab)
;   r9: Constant 10 (Enter)
;   r10: Constant 8 (Backspace)
;   r11: Constant 27 (Escape)
;   r12-r20: scratch

; ===== Initialize =====
LDI r3, 0x3900          ; full prompt addr
LDI r4, 0x3300          ; response buffer
LDI r5, 768             ; max response length
LDI r7, 1
LDI r8, 9               ; Tab
LDI r9, 10              ; Enter
LDI r10, 8              ; Backspace
LDI r11, 27             ; Escape

; Build map context prefix
CALL build_context

; Clear user input and response
LDI r12, 0x3100
LDI r16, 256
CALL clear_buf
LDI r12, 0x3300
LDI r16, 768
CALL clear_buf

; Status
LDI r12, 0x3700
STRO r12, "Tab=Ask  Esc=Exit  Enter=Clear"

; ===== Main Loop =====
main_loop:
    FILL r0

    ; -- Title bar --
    LDI r12, 0x1A0033   ; dark purple
    LDI r13, 0
    LDI r14, 0
    LDI r15, 256
    LDI r16, 14
    RECTF r13, r14, r15, r16, r12

    LDI r12, 0x3800
    STRO r12, "Oracle - AI Guide"
    LDI r13, 4
    LDI r14, 3
    LDI r15, 0x3800
    TEXT r13, r14, r15

    ; -- Context info line --
    LDI r13, 4
    LDI r14, 16
    LDI r15, 0x3000
    TEXT r13, r14, r15

    ; -- Separator --
    LDI r12, 0x333344
    LDI r13, 0
    LDI r14, 28
    LDI r15, 256
    LDI r16, 1
    RECTF r13, r14, r15, r16, r12

    ; -- User input --
    LDI r13, 4
    LDI r14, 32
    LDI r15, 0x3100
    TEXT r13, r14, r15

    ; -- Response area --
    LDI r13, 4
    LDI r14, 48
    CALL draw_response

    ; -- Status bar --
    LDI r12, 0x111122
    LDI r13, 0
    LDI r14, 248
    LDI r15, 256
    LDI r16, 8
    RECTF r13, r14, r15, r16, r12
    LDI r13, 4
    LDI r14, 248
    LDI r12, 0x3700
    TEXT r13, r14, r12

    ; -- Input --
    IKEY r1
    LDI r12, 0
    ADD r12, r1
    JZ r12, ml_continue

    CMP r1, r11
    JZ r0, done
    CMP r1, r8
    JZ r0, send_to_llm
    CMP r1, r9
    JZ r0, handle_enter
    CMP r1, r10
    JZ r0, handle_backspace

    ; Regular char: append to input buffer
    LDI r12, 0x3100
    CALL find_null
    LDI r13, 0x32FF
    CMP r12, r13
    BGE r0, ml_continue
    STORE r12, r1

ml_continue:
    FRAME
    JMP main_loop

; ===== Send to LLM =====
send_to_llm:
    ; Status: thinking
    LDI r12, 0x3700
    STRO r12, "Thinking..."

    ; Build full prompt at 0x3900:
    ;   1. Copy context from 0x3000
    ;   2. Append "\n\nQuestion: "
    ;   3. Append user input from 0x3100
    ;   4. Append "\n\nAnswer concisely."

    ; Step 1: copy context
    LDI r18, 0x3000
    LDI r19, 0x3900
    CALL copy_until_null

    ; Step 2: append \n\nQuestion:
    SUBI r19, 1         ; back over null
    LDI r12, 10
    STORE r19, r12
    ADDI r19, 1
    STORE r19, r12
    ADDI r19, 1
    STRO r19, "Question: "
    CALL advance_to_null

    ; Step 3: copy user input
    LDI r18, 0x3100
    CALL copy_until_null

    ; Step 4: append \n\nAnswer concisely.
    SUBI r19, 1
    LDI r12, 10
    STORE r19, r12
    ADDI r19, 1
    STORE r19, r12
    ADDI r19, 1
    STRO r19, "Answer concisely."
    CALL advance_to_null

    ; Call LLM: reads from 0x3900, writes to 0x3300
    LDI r3, 0x3900
    LLM r3, r4, r5

    ; Clear input
    LDI r12, 0x3100
    LDI r16, 256
    CALL clear_buf

    ; Restore status
    LDI r12, 0x3700
    STRO r12, "Tab=Ask  Esc=Exit  Enter=Clear"

    JMP ml_continue

; ===== Handle Enter: clear input + response =====
handle_enter:
    LDI r12, 0x3100
    LDI r16, 256
    CALL clear_buf
    LDI r12, 0x3300
    LDI r16, 768
    CALL clear_buf
    JMP ml_continue

; ===== Handle Backspace =====
handle_backspace:
    LDI r12, 0x3100
    CALL find_null
    LDI r13, 0x3100
    CMP r12, r13
    JZ r0, ml_continue
    SUBI r12, 1
    LDI r13, 0
    STORE r12, r13
    JMP ml_continue

; ===== Build context string at 0x3000 =====
; Uses r19 as output pointer, r12-r18 as scratch
build_context:
    PUSH r31
    LDI r19, 0x3000

    ; "Pos: ("
    STRO r19, "Pos: ("
    CALL advance_to_null

    ; Player X from RAM[0x7808]
    LDI r12, 0x7808
    LOAD r13, r12
    LDI r12, 0
    ADD r12, r13
    CALL write_decimal

    ; ","
    CALL advance_to_null
    STRO r19, ","
    CALL advance_to_null

    ; Player Y from RAM[0x7809]
    LDI r12, 0x7809
    LOAD r13, r12
    LDI r12, 0
    ADD r12, r13
    CALL write_decimal

    ; ") "
    CALL advance_to_null
    STRO r19, ") "
    CALL advance_to_null

    ; Check nearby building
    LDI r12, 0x7588
    LOAD r13, r12
    LDI r14, 1
    CMP r13, r14
    JNZ r0, no_nearby

    ; "Near: "
    STRO r19, "Near: "
    CALL advance_to_null

    ; Read building name: index at 0x7584
    LDI r12, 0x7584
    LOAD r15, r12        ; r15 = building index
    LDI r14, 4
    MUL r15, r14
    LDI r14, 0x7500
    ADD r15, r14         ; r15 = 0x7500 + index*4
    ADDI r15, 3          ; offset to name_addr field
    LOAD r16, r15        ; r16 = name string address
    ; Copy name chars to output
    LDI r12, 0
copy_bname:
    MOV r17, r16
    ADD r17, r12
    LOAD r13, r17
    LDI r18, 0
    CMP r13, r18
    JZ r0, ctx_done
    STORE r19, r13
    ADDI r19, 1
    ADDI r12, 1
    JMP copy_bname

no_nearby:
    STRO r19, "Explore the world!"

ctx_done:
    LDI r12, 0
    STORE r19, r12
    POP r31
    RET

; ===== Write decimal to string at r19 =====
; r12 = number to write
write_decimal:
    PUSH r31
    PUSH r12
    LDI r13, 0
    CMP r12, r13
    JNZ r0, wd_nonzero
    LDI r13, 48
    STORE r19, r13
    ADDI r19, 1
    JMP wd_done

wd_nonzero:
    LDI r14, 0x3E00     ; temp digit buffer
    LDI r15, 0          ; digit count
wd_loop:
    LDI r13, 0
    CMP r12, r13
    JZ r0, wd_reverse
    LDI r13, 10
    MOV r16, r12
    MOD r16, r13
    ADDI r16, 48
    STORE r14, r16
    ADDI r14, 1
    ADDI r15, 1
    DIV r12, r13
    JMP wd_loop

wd_reverse:
    SUBI r14, 1
wd_rev_loop:
    LDI r13, 0
    CMP r15, r13
    JZ r0, wd_done
    LOAD r16, r14
    STORE r19, r16
    ADDI r19, 1
    SUBI r14, 1
    SUBI r15, 1
    JMP wd_rev_loop

wd_done:
    LDI r12, 0
    STORE r19, r12
    POP r12
    POP r31
    RET

; ===== Advance r19 to null terminator =====
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

; ===== Copy from r18 to r19 until null =====
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

; ===== Find null terminator at r12 =====
find_null:
    PUSH r31
fn_loop:
    LOAD r16, r12
    LDI r13, 0
    CMP r16, r13
    JZ r0, fn_done
    ADDI r12, 1
    JMP fn_loop
fn_done:
    POP r31
    RET

; ===== Clear buffer: r12=start, r16=count =====
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

; ===== Draw response text =====
; r13 = start x, r14 = start y
; Delegates to TEXT opcode, which wraps at column ~250 and handles \n.
draw_response:
    PUSH r31
    PUSH r12
    LDI r12, 0x3300     ; response buffer
    TEXT r13, r14, r12
    POP r12
    POP r31
    RET

done:
    HALT
