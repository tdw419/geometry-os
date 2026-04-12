; self_host.asm -- Self-hosting demo
;
; Writes assembly source into RAM, assembles it with the ASM opcode,
; then runs the compiled bytecode. This proves the VM can compile and
; execute programs generated at runtime.
;
; The generated program fills the screen with green (color 3) and halts.

; ---- Main code (must come first, starts at address 0) ----

; Write source text to 0x0800
; The program we generate: "LDI r0, 3\nFILL r0\nHALT\n"
LDI r1, 0x0800

LDI r2, 76         ; 'L'
CALL store_char
LDI r2, 68         ; 'D'
CALL store_char
LDI r2, 73         ; 'I'
CALL store_char
LDI r2, 32         ; ' '
CALL store_char
LDI r2, 114        ; 'r'
CALL store_char
LDI r2, 48         ; '0'
CALL store_char
LDI r2, 44         ; ','
CALL store_char
LDI r2, 32         ; ' '
CALL store_char
LDI r2, 51         ; '3'
CALL store_char
LDI r2, 10         ; newline
CALL store_char
LDI r2, 70         ; 'F'
CALL store_char
LDI r2, 73         ; 'I'
CALL store_char
LDI r2, 76         ; 'L'
CALL store_char
LDI r2, 76         ; 'L'
CALL store_char
LDI r2, 32         ; ' '
CALL store_char
LDI r2, 114        ; 'r'
CALL store_char
LDI r2, 48         ; '0'
CALL store_char
LDI r2, 10         ; newline
CALL store_char
LDI r2, 72         ; 'H'
CALL store_char
LDI r2, 65         ; 'A'
CALL store_char
LDI r2, 76         ; 'L'
CALL store_char
LDI r2, 84         ; 'T'
CALL store_char
LDI r2, 10         ; newline
CALL store_char
LDI r2, 0          ; null terminator
CALL store_char

; Assemble the source text
LDI r5, 0x0800     ; source address
LDI r6, 0x1000     ; destination for bytecode
ASM r5, r6

; Check for assembly error
LDI r5, 0xFFD      ; ASM result port
LOAD r7, r5
LDI r8, 0xFFFFFFFF
CMP r7, r8
; If r0 == 0 (CMP equal), it is an error
JZ r0, asm_error

; Success: run the compiled code at 0x1000
JMP 0x1000

asm_error:
    ; Fill screen with red to indicate assembly error
    LDI r0, 4
    FILL r0
    HALT

; ---- Subroutine: store character r2 at address in r1, advance r1 ----
store_char:
    STORE r1, r2
    LDI r3, 1
    ADD r1, r3
    RET
