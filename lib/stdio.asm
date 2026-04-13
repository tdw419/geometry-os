; lib/stdio.asm -- Standard Library: formatted I/O
;
; Calling convention:
;   Arguments: r1-r5 (r0 = return value)
;   Caller-saved: r1-r9
;   Callee-saved: r10-r25
;
; Requires: lib/stdlib.asm (for strlen, itoa)
; Requires: lib/stdlib.asm must be included BEFORE this file

; ═══════════════════════════════════════════════════════════════
; Cursor state stored in RAM:
;   0xFB0 = cursor X position
;   0xFB1 = cursor Y position
;   0xFB2 = text color (default white)
; ═══════════════════════════════════════════════════════════════
#define CURSOR_X 0xFB0
#define CURSOR_Y 0xFB1
#define CURSOR_COLOR 0xFB2

; ═══════════════════════════════════════════════════════════════
; stdio_init -- initialize cursor position and color
; ═══════════════════════════════════════════════════════════════
stdio_init:
    LDI r9, CURSOR_X
    LDI r0, 2
    STORE r9, r0           ; x = 2
    LDI r9, CURSOR_Y
    LDI r0, 0
    STORE r9, r0           ; y = 0
    LDI r9, CURSOR_COLOR
    LDI r0, 0xFFFFFF
    STORE r9, r0           ; color = white
    RET

; ═══════════════════════════════════════════════════════════════
; print_str -- print null-terminated string to screen at cursor
;   r1 = string address
;   Advances cursor position
; ═══════════════════════════════════════════════════════════════
print_str:
    LDI r9, CURSOR_X
    LOAD r10, r9           ; r10 = cursor x
    LDI r9, CURSOR_Y
    LOAD r11, r9           ; r11 = cursor y
print_str_loop:
    LOAD r2, r1
    JZ r2, print_str_done
    ; Handle newline
    LDI r3, 10
    CMP r2, r3
    JNZ r0, print_str_char
    ; Newline: advance y, reset x
    LDI r3, 8
    ADD r11, r3
    LDI r10, 2
    LDI r3, 1
    ADD r1, r3
    JMP print_str_loop
print_str_char:
    ; Render character at cursor position using TEXT
    ; TEXT needs x_reg, y_reg, addr_reg -- but it reads from RAM addr
    ; We need to draw a single char. Use PSETI-based approach.
    ; Actually, we can store the char at a temp location and use TEXT.
    LDI r9, 0xFB8          ; temp char location
    STORE r9, r2           ; store char
    LDI r9, 0
    LDI r3, 1
    ADD r9, r3             ; 0xFB9
    STORE r9, r3           ; null terminator at 0xFB9... wait, we need 0 not 1
    ; Actually let's be more careful
    LDI r9, 0xFB9
    LDI r3, 0
    STORE r9, r3           ; null terminator
    ; Now TEXT cursor_x, cursor_y, 0xFB8
    LDI r2, 0xFB8
    ; But TEXT takes register numbers... we need to load values into registers
    MOV r12, r10           ; save x
    MOV r13, r11           ; save y
    LDI r9, CURSOR_X
    STORE r9, r12          ; (already there, but TEXT reads regs not RAM)
    ; TEXT opcode reads from registers: TEXT x_reg, y_reg, addr_reg
    ; We need: TEXT with actual register values
    ; Since we have x in r10, y in r11, and addr in r2:
    ; Move to registers that TEXT can use
    MOV r3, r10            ; r3 = x
    MOV r4, r11            ; r4 = y  
    LDI r5, 0xFB8          ; r5 = char addr
    TEXT r3, r4, r5        ; draw char at cursor
    ; Advance cursor: x += 6 (char width + gap)
    LDI r3, 6
    ADD r10, r3
    ; Line wrap
    LDI r3, 250
    CMP r10, r3
    LDI r3, 1
    CMP r0, r3             ; x > 250?
    JNZ r0, print_str_next
    LDI r10, 2
    LDI r3, 8
    ADD r11, r3
print_str_next:
    LDI r3, 1
    ADD r1, r3             ; advance string pointer
    JMP print_str_loop
print_str_done:
    ; Save cursor position back
    LDI r9, CURSOR_X
    STORE r9, r10
    LDI r9, CURSOR_Y
    STORE r9, r11
    RET

; ═══════════════════════════════════════════════════════════════
; print_int -- print unsigned integer as decimal
;   r1 = value
;   Uses internal buffer at 0xFB4 (8 words)
; ═══════════════════════════════════════════════════════════════
print_int:
    LDI r2, 0xFB4          ; buffer for digits
    CALL itoa              ; convert to string at 0xFB4
    LDI r1, 0xFB4
    CALL print_str         ; print the string
    RET

; ═══════════════════════════════════════════════════════════════
; print_newline -- advance cursor to next line
; ═══════════════════════════════════════════════════════════════
print_newline:
    LDI r9, CURSOR_X
    LDI r0, 2
    STORE r9, r0           ; x = 2 (left margin)
    LDI r9, CURSOR_Y
    LOAD r0, r9
    LDI r3, 8
    ADD r0, r3             ; y += 8 (line height)
    STORE r9, r0
    RET
