; smart_term.asm -- Terminal with AI assist
; Type text, press Tab to ask the LLM, response appears on screen.
; Uses LLM opcode (0x9C) to send prompts to external AI.
;
; Memory layout:
;   0x2000-0x20FF: Input buffer (prompt typed by user)
;   0x2100-0x24FF: Response buffer (LLM response, up to 1K chars)
;   0x2500-0x25FF: Status line text
;
; Registers:
;   r0: CMP result (reserved)
;   r1: Key input from IKEY
;   r2: Cursor position (offset into input buffer)
;   r3: Input buffer base (0x2000)
;   r4: Response buffer base (0x2100)
;   r5: Max response length (256)
;   r6: Response length (from LLM opcode)
;   r7: Constant 1
;   r8: Constant 9 (Tab ASCII)
;   r9: Constant 10 (Enter/LF)
;   r10: Constant 8 (Backspace)
;   r11: Constant 27 (Escape)
;   r12: Loop temp
;   r13: Screen Y for text rendering
;   r14: Constant 32 (space)
;   r15: Tick counter for blinking cursor
;   r16: Color for prompt text

; Initialize constants
LDI r3, 0x2000          ; input buffer base
LDI r4, 0x2100          ; response buffer base
LDI r5, 256             ; max response length
LDI r7, 1
LDI r8, 9               ; Tab
LDI r9, 10              ; LF
LDI r10, 8              ; Backspace
LDI r11, 27             ; Escape
LDI r14, 32             ; Space
LDI r15, 0              ; tick counter
LDI r16, 0x00FF00       ; green color for prompt

; Write title bar
LDI r12, 0
CALL draw_title

; Write prompt indicator "> " at top
LDI r12, 0x2000
LDI r13, 0
CALL clear_input

; Write status line
LDI r12, 0x2500
LDI r13, 0x00FFFF       ; cyan
LDI r14, 28             ; screen Y position
CALL write_status

; Main loop
main_loop:
    FILL r0             ; clear screen (black)
    ; Redraw on every frame

    ; Draw title
    CALL draw_title

    ; Draw input prompt "> " in green
    LDI r12, 0x00FF00   ; green
    LDI r13, 2           ; x
    LDI r14, 4           ; y (below title)
    LDI r15, 0x2000      ; input buffer
    CALL draw_text_line

    ; Draw response in white below input
    LDI r12, 0xFFFFFF   ; white
    LDI r13, 2           ; x
    LDI r14, 6           ; y (below prompt)
    LDI r15, 0x2100      ; response buffer
    CALL draw_text_line

    ; Draw status bar
    LDI r12, 0x00FFFF   ; cyan
    LDI r13, 2           ; x
    LDI r14, 28          ; y (bottom area)
    LDI r15, 0x2500      ; status buffer
    CALL draw_text_line

    ; Read keyboard
    IKEY r1
    LDI r12, 0
    ADD r12, r1
    JZ r12, main_loop_continue

    ; Check Escape
    CMP r1, r11
    JZ r0, done

    ; Check Tab (send to LLM)
    CMP r1, r8
    JZ r0, send_to_llm

    ; Check Enter (clear input)
    CMP r1, r9
    JZ r0, handle_enter

    ; Check Backspace
    CMP r1, r10
    JZ r0, handle_backspace

    ; Regular character - store in input buffer
    ; Find end of current input (null terminator)
    LDI r12, 0x2000
    CALL find_end
    ; r12 = address of null terminator
    ; Check buffer not full
    LDI r13, 0x20FF
    CMP r12, r13
    BGE r0, main_loop_continue
    ; Store character
    STORE r12, r1

main_loop_continue:
    FRAME
    JMP main_loop

; --- Send prompt to LLM ---
send_to_llm:
    ; Update status to "Thinking..."
    LDI r12, 0x2500
    LDI r13, 84           ; 'T'
    STORE r12, r13
    LDI r13, 104          ; 'h'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 105          ; 'i'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 110          ; 'n'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 107          ; 'k'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 105          ; 'i'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 110          ; 'n'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 103          ; 'g'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 46           ; '.'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 46           ; '.'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 46           ; '.'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 0            ; null terminate
    ADD r12, r7
    STORE r12, r13

    ; Call LLM opcode: LLM prompt_addr, response_addr, max_len
    LLM r3, r4, r5

    ; r0 = response length
    ; Clear input buffer for next prompt
    LDI r12, 0x2000
    CALL clear_input

    ; Update status
    LDI r12, 0x2500
    LDI r13, 68            ; 'D'
    STORE r12, r13
    LDI r13, 111           ; 'o'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 110           ; 'n'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 101           ; 'e'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 46            ; '.'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 0
    ADD r12, r7
    STORE r12, r13

    JMP main_loop_continue

; --- Handle Enter: clear input and response ---
handle_enter:
    LDI r12, 0x2000
    CALL clear_input
    LDI r12, 0x2100
    CALL clear_input
    JMP main_loop_continue

; --- Handle Backspace ---
handle_backspace:
    LDI r12, 0x2000
    CALL find_end
    ; If at buffer start, ignore
    LDI r13, 0x2000
    CMP r12, r13
    JZ r0, main_loop_continue
    ; Go back one and null-terminate
    SUB r12, r7
    LDI r13, 0
    STORE r12, r13
    JMP main_loop_continue

; --- Draw title bar ---
draw_title:
    PUSH r31
    ; Draw title background
    LDI r12, 0x222244     ; dark purple
    LDI r13, 0
    LDI r14, 0
    LDI r15, 256
    LDI r16, 12
    RECTF r13, r14, r15, r16, r12

    ; Draw "Smart Terminal - Press Tab for AI, Esc to quit"
    ; Use TEXT opcode to render title
    ; Title is stored inline -- write it to RAM first
    LDI r12, 0x2600
    LDI r13, 83           ; 'S'
    STORE r12, r13
    LDI r13, 109          ; 'm'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 97           ; 'a'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 114          ; 'r'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 116          ; 't'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 32           ; ' '
    ADD r12, r7
    STORE r12, r13
    LDI r13, 84           ; 'T'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 101          ; 'e'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 114          ; 'r'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 109          ; 'm'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 0            ; null terminate
    ADD r12, r7
    STORE r12, r13

    ; Render with TEXT opcode
    LDI r12, 0x2600       ; source addr
    LDI r13, 4            ; x
    LDI r14, 2            ; y
    LDI r15, 0xFFFF00     ; yellow
    TEXT r13, r14, r12

    POP r31
    RET

; --- Write status line ---
write_status:
    ; "Tab=AI  Esc=Quit  Enter=Clear"
    PUSH r31
    LDI r12, 0x2500
    LDI r13, 84           ; 'T'
    STORE r12, r13
    LDI r13, 97           ; 'a'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 98           ; 'b'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 61           ; '='
    ADD r12, r7
    STORE r12, r13
    LDI r13, 65           ; 'A'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 73           ; 'I'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 32           ; ' '
    ADD r12, r7
    STORE r12, r13
    LDI r13, 32           ; ' '
    ADD r12, r7
    STORE r12, r13
    LDI r13, 69           ; 'E'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 115          ; 's'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 99           ; 'c'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 61           ; '='
    ADD r12, r7
    STORE r12, r13
    LDI r13, 81           ; 'Q'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 117          ; 'u'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 105          ; 'i'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 116          ; 't'
    ADD r12, r7
    STORE r12, r13
    LDI r13, 0            ; null terminate
    ADD r12, r7
    STORE r12, r13
    POP r31
    RET

; --- Draw text line from RAM buffer ---
; r12 = color, r13 = screen x, r14 = screen y, r15 = RAM addr
draw_text_line:
    PUSH r31
    PUSH r15
    PUSH r12
draw_text_loop:
    LOAD r16, r15
    LDI r12, 0
    CMP r16, r12
    JZ r0, draw_text_done
    PSET r13, r14, r12
    ADD r13, r7
    ADD r15, r7
    JMP draw_text_loop
draw_text_done:
    POP r12
    POP r15
    POP r31
    RET

; --- Find null terminator in buffer ---
; r12 = start address. Returns r12 = address of null.
find_end:
    PUSH r31
    PUSH r7
find_end_loop:
    LOAD r16, r12
    LDI r13, 0
    CMP r16, r13
    JZ r0, find_end_done
    ADD r12, r7
    JMP find_end_loop
find_end_done:
    POP r7
    POP r31
    RET

; --- Clear input buffer (fill with 0) ---
; r12 = buffer start address
clear_input:
    PUSH r31
    PUSH r7
    PUSH r12
    LDI r13, 0
    LDI r16, 256           ; max clear length
clear_loop:
    STORE r12, r13
    ADD r12, r7
    LDI r14, 0
    ADD r14, r12
    LDI r15, 0x2100
    ; Only clear up to response buffer size
    CMP r14, r15
    BGE r0, clear_done
    JMP clear_loop
clear_done:
    POP r12
    POP r7
    POP r31
    RET

done:
    HALT
