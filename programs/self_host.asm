; self_host.asm -- Enhanced Self-Hosting Pixel Assembler (Phase 70)
;
; Demonstrates the full self-hosting pipeline:
; 1. Write assembly source to RAM character by character
; 2. Assemble at runtime using ASM opcode (0x4B)
; 3. Check for errors via RAM[0xFFD]
; 4. Execute the assembled bytecode
;
; The generated program uses: #define, labels, LDI, MOV, ADD, AND,
; CMP, BLT, PSET, HALT -- all assembled from inside the VM.

LDI r1, 0x0800       ; destination for source text

; Write "#define W 256"
LDI r2, 35
CALL w
LDI r2, 100
CALL w
LDI r2, 101
CALL w
LDI r2, 102
CALL w
LDI r2, 105
CALL w
LDI r2, 110
CALL w
LDI r2, 101
CALL w
LDI r2, 32
CALL w
LDI r2, 87
CALL w
LDI r2, 32
CALL w
LDI r2, 50
CALL w
LDI r2, 53
CALL w
LDI r2, 54
CALL w
LDI r2, 10
CALL w

; Write "LDI r5, 0"
LDI r2, 76
CALL w
LDI r2, 68
CALL w
LDI r2, 73
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 53
CALL w
LDI r2, 44
CALL w
LDI r2, 32
CALL w
LDI r2, 48
CALL w
LDI r2, 10
CALL w

; Write "LDI r7, 1"
LDI r2, 76
CALL w
LDI r2, 68
CALL w
LDI r2, 73
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 55
CALL w
LDI r2, 44
CALL w
LDI r2, 32
CALL w
LDI r2, 49
CALL w
LDI r2, 10
CALL w

; Write "LDI r8, W"
LDI r2, 76
CALL w
LDI r2, 68
CALL w
LDI r2, 73
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 56
CALL w
LDI r2, 44
CALL w
LDI r2, 32
CALL w
LDI r2, 87
CALL w
LDI r2, 10
CALL w

; Write "loop:"
LDI r2, 108
CALL w
LDI r2, 111
CALL w
LDI r2, 111
CALL w
LDI r2, 112
CALL w
LDI r2, 58
CALL w
LDI r2, 10
CALL w

; Write "  LDI r2, 0"
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 76
CALL w
LDI r2, 68
CALL w
LDI r2, 73
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 50
CALL w
LDI r2, 44
CALL w
LDI r2, 32
CALL w
LDI r2, 48
CALL w
LDI r2, 10
CALL w

; Write "  xloop:"
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 120
CALL w
LDI r2, 108
CALL w
LDI r2, 111
CALL w
LDI r2, 111
CALL w
LDI r2, 112
CALL w
LDI r2, 58
CALL w
LDI r2, 10
CALL w

; Write "    MOV r3, r5"
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 77
CALL w
LDI r2, 79
CALL w
LDI r2, 86
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 51
CALL w
LDI r2, 44
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 53
CALL w
LDI r2, 10
CALL w

; Write "    ADD r3, r2"
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 65
CALL w
LDI r2, 68
CALL w
LDI r2, 68
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 51
CALL w
LDI r2, 44
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 50
CALL w
LDI r2, 10
CALL w

; Write "    LDI r9, 255"
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 76
CALL w
LDI r2, 68
CALL w
LDI r2, 73
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 57
CALL w
LDI r2, 44
CALL w
LDI r2, 32
CALL w
LDI r2, 50
CALL w
LDI r2, 53
CALL w
LDI r2, 53
CALL w
LDI r2, 10
CALL w

; Write "    AND r3, r9"
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 65
CALL w
LDI r2, 78
CALL w
LDI r2, 68
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 51
CALL w
LDI r2, 44
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 57
CALL w
LDI r2, 10
CALL w

; Write "    PSET r2, r5, r3"
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 80
CALL w
LDI r2, 83
CALL w
LDI r2, 69
CALL w
LDI r2, 84
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 50
CALL w
LDI r2, 44
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 53
CALL w
LDI r2, 44
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 51
CALL w
LDI r2, 10
CALL w

; Write "    ADD r2, r7"
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 65
CALL w
LDI r2, 68
CALL w
LDI r2, 68
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 50
CALL w
LDI r2, 44
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 55
CALL w
LDI r2, 10
CALL w

; Write "    CMP r2, r8"
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 67
CALL w
LDI r2, 77
CALL w
LDI r2, 80
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 50
CALL w
LDI r2, 44
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 56
CALL w
LDI r2, 10
CALL w

; Write "    BLT r0, xloop"
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 66
CALL w
LDI r2, 76
CALL w
LDI r2, 84
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 48
CALL w
LDI r2, 44
CALL w
LDI r2, 32
CALL w
LDI r2, 120
CALL w
LDI r2, 108
CALL w
LDI r2, 111
CALL w
LDI r2, 111
CALL w
LDI r2, 112
CALL w
LDI r2, 10
CALL w

; Write "  ADD r5, r7"
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 65
CALL w
LDI r2, 68
CALL w
LDI r2, 68
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 53
CALL w
LDI r2, 44
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 55
CALL w
LDI r2, 10
CALL w

; Write "  CMP r5, r8"
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 67
CALL w
LDI r2, 77
CALL w
LDI r2, 80
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 53
CALL w
LDI r2, 44
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 56
CALL w
LDI r2, 10
CALL w

; Write "  BLT r0, loop"
LDI r2, 32
CALL w
LDI r2, 32
CALL w
LDI r2, 66
CALL w
LDI r2, 76
CALL w
LDI r2, 84
CALL w
LDI r2, 32
CALL w
LDI r2, 114
CALL w
LDI r2, 48
CALL w
LDI r2, 44
CALL w
LDI r2, 32
CALL w
LDI r2, 108
CALL w
LDI r2, 111
CALL w
LDI r2, 111
CALL w
LDI r2, 112
CALL w
LDI r2, 10
CALL w

; Write "HALT"
LDI r2, 72
CALL w
LDI r2, 65
CALL w
LDI r2, 76
CALL w
LDI r2, 84
CALL w
LDI r2, 10
CALL w

; Null-terminate the source text
LDI r2, 0
CALL w

; Assemble the source text
LDI r5, 0x0800
LDI r6, 0x1000
ASM r5, r6

; Check for assembly error
LDI r5, 0xFFD
LOAD r7, r5
LDI r8, 0xFFFFFFFF
CMP r7, r8
JZ r0, asm_error

; Run the compiled code at 0x1000
JMP 0x1000

asm_error:
    LDI r0, 0xFF0000
    FILL r0
    HALT

; ---- Subroutine: write character r2 at address in r1, advance r1 ----
w:
    STORE r1, r2
    LDI r3, 1
    ADD r1, r3
    RET
