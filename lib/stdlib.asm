; lib/stdlib.asm -- Standard Library for Geometry OS
;
; CALLING CONVENTION:
;   r0 = return value
;   r1-r5 = arguments (caller-saved)
;   r6-r9 = temporaries (caller-saved)
;   r10-r25 = callee-saved (library functions preserve these)
;   r26 = heap pointer (bump allocator)
;   r27-r29 = reserved for preprocessor macros
;   r30 = stack pointer
;   r31 = call return address
;
; LINKING:
;   .include "lib/stdlib.asm"  (from project root)
;   .include "stdlib.asm"      (if lib/ is in assembler lib_dir)
;
; HEAP:
;   MALLOC opcode (0x6F): r1 = size in words -> r0 = address (0 on failure)
;   FREE opcode (0x70): r1 = address -> frees the block
;   Bump allocator: heap_init / malloc_bump / free_nop

; =====================================================================
; HEAP ALLOCATOR (bump allocator)
; =====================================================================

; ── heap_init ──────────────────────────────────────────────────────
; Initialize the bump allocator heap pointer.
; Args: r1 = heap base address
heap_init:
    MOV r26, r1
    RET

; ── malloc_bump ────────────────────────────────────────────────────
; Bump allocator: allocate N words from the heap.
; Args: r1 = number of words to allocate
; Returns: r0 = address of block
malloc_bump:
    MOV r0, r26
    ADD r26, r1
    RET

; ── free_nop ──────────────────────────────────────────────────────
; No-op free (bump allocator cannot free individual blocks).
; Args: r1 = address (ignored)
free_nop:
    RET

; =====================================================================
; STRING OPERATIONS
; =====================================================================

; ── strlen ────────────────────────────────────────────────────────
; Count characters in a null-terminated string in RAM.
; Args: r1 = string address
; Returns: r0 = length
strlen:
    PUSH r6
    LDI r0, 0
    MOV r6, r1
strlen_loop:
    LOAD r2, r6
    JZ r2, strlen_done
    ADD r0, 1
    ADD r6, 1
    JMP strlen_loop
strlen_done:
    POP r6
    RET

; ── strcmp ────────────────────────────────────────────────────────
; Compare two null-terminated strings.
; Args: r1 = addr A, r2 = addr B
; Returns: r0 = 0 (equal), 1 (A > B), 0xFFFFFFFF (A < B)
strcmp:
    PUSH r6
    PUSH r7
    MOV r6, r1
    MOV r7, r2
strcmp_loop:
    LOAD r3, r6          ; char A
    LOAD r4, r7          ; char B
    ; Check if A is null
    JZ r3, strcmp_a_null
    ; A is not null, check if B is null
    JZ r4, strcmp_a_gt   ; A has char, B is null -> A > B
    ; Both have chars, compare them
    CMP r3, r4           ; r0 = -1/0/1
    JZ r0, strcmp_same   ; equal chars, continue
    ; Different: r0 already has the right value (-1 or 1)
    JMP strcmp_done
strcmp_same:
    ADD r6, 1
    ADD r7, 1
    JMP strcmp_loop
strcmp_a_null:
    ; A is null. If B is also null -> equal, else A < B
    JZ r4, strcmp_eq
    LDI r0, 0xFFFFFFFF
    JMP strcmp_done
strcmp_a_gt:
    LDI r0, 1
    JMP strcmp_done
strcmp_eq:
    LDI r0, 0
strcmp_done:
    POP r7
    POP r6
    RET

; ── strcpy ────────────────────────────────────────────────────────
; Copy null-terminated string src to dest.
; Args: r1 = dest, r2 = src
; Returns: r0 = dest
strcpy:
    PUSH r6
    PUSH r7
    MOV r6, r1
    MOV r7, r2
    MOV r0, r1
strcpy_loop:
    LOAD r3, r7
    STORE r6, r3
    JZ r3, strcpy_done
    ADD r6, 1
    ADD r7, 1
    JMP strcpy_loop
strcpy_done:
    POP r7
    POP r6
    RET

; ── strcat ────────────────────────────────────────────────────────
; Concatenate src onto end of dest.
; Args: r1 = dest, r2 = src
; Returns: r0 = dest
strcat:
    PUSH r6
    PUSH r7
    MOV r6, r1
    MOV r7, r2
    MOV r0, r1
strcat_find_end:
    LOAD r3, r6
    JZ r3, strcat_copy
    ADD r6, 1
    JMP strcat_find_end
strcat_copy:
    LOAD r3, r7
    STORE r6, r3
    JZ r3, strcat_done
    ADD r6, 1
    ADD r7, 1
    JMP strcat_copy
strcat_done:
    POP r7
    POP r6
    RET

; =====================================================================
; MEMORY OPERATIONS
; =====================================================================

; ── memset ────────────────────────────────────────────────────────
; Fill memory with a value.
; Args: r1 = addr, r2 = value, r3 = count (words)
; Returns: r0 = addr
memset:
    PUSH r6
    PUSH r7
    MOV r6, r1
    MOV r7, r3
    MOV r0, r1
memset_loop:
    JZ r7, memset_done
    STORE r6, r2
    ADD r6, 1
    SUB r7, 1
    JMP memset_loop
memset_done:
    POP r7
    POP r6
    RET

; ── memcpy ────────────────────────────────────────────────────────
; Copy memory region src to dest.
; Args: r1 = dest, r2 = src, r3 = count (words)
; Returns: r0 = dest
memcpy:
    PUSH r6
    PUSH r7
    PUSH r8
    MOV r6, r1
    MOV r7, r2
    MOV r8, r3
    MOV r0, r1
memcpy_loop:
    JZ r8, memcpy_done
    LOAD r4, r7
    STORE r6, r4
    ADD r6, 1
    ADD r7, 1
    SUB r8, 1
    JMP memcpy_loop
memcpy_done:
    POP r8
    POP r7
    POP r6
    RET
