; lib/stdlib.asm -- Standard Library: string operations, memory operations, heap allocator
;
; Calling convention:
;   Arguments: r1-r5 (r0 = return value)
;   Caller-saved: r1-r9
;   Callee-saved: r10-r25
;   Temp/reserved: r27-r29 (preprocessor), r30=SP, r31=return PC
;
; String format: null-terminated, one character per u32 word (low byte)
;
; Heap layout:
;   HEAP_START (0xC000) is the heap base.
;   RAM[HEAP_START] = next free offset (bump allocator pointer)
;   malloc: bump the pointer, return old pointer
;   free: no-op (bump allocator -- memory reused only on process restart)

; ═══════════════════════════════════════════════════════════════
; strlen -- compute length of null-terminated string
;   r1 = string address
;   returns r0 = length (number of chars before null)
; ═══════════════════════════════════════════════════════════════
strlen:
    LDI r0, 0
strlen_loop:
    LOAD r2, r1
    JZ r2, strlen_done
    LDI r2, 1
    ADD r1, r2
    LDI r2, 1
    ADD r0, r2
    JMP strlen_loop
strlen_done:
    RET

; ═══════════════════════════════════════════════════════════════
; strcmp -- compare two null-terminated strings
;   r1 = string A address, r2 = string B address
;   returns r0 = 0 if equal, 1 if A>B, 0xFFFFFFFF if A<B
; ═══════════════════════════════════════════════════════════════
strcmp:
strcmp_loop:
    LOAD r3, r1
    LOAD r4, r2
    JZ r3, strcmp_check_b
    JZ r4, strcmp_a_bigger
    CMP r3, r4
    LDI r5, 0xFFFFFFFF
    CMP r0, r5
    JZ r0, strcmp_a_less
    LDI r5, 1
    ADD r1, r5
    ADD r2, r5
    JMP strcmp_loop
strcmp_check_b:
    JZ r4, strcmp_equal
    JMP strcmp_a_less
strcmp_a_bigger:
    LDI r0, 1
    RET
strcmp_a_less:
    LDI r0, 0xFFFFFFFF
    RET
strcmp_equal:
    LDI r0, 0
    RET

; ═══════════════════════════════════════════════════════════════
; strcpy -- copy null-terminated string from src to dst
;   r1 = destination address, r2 = source address
;   returns r0 = destination address (unchanged)
; ═══════════════════════════════════════════════════════════════
strcpy:
    MOV r0, r1
strcpy_loop:
    LOAD r3, r2
    STORE r1, r3
    JZ r3, strcpy_done
    LDI r3, 1
    ADD r1, r3
    ADD r2, r3
    JMP strcpy_loop
strcpy_done:
    RET

; ═══════════════════════════════════════════════════════════════
; strcat -- concatenate null-terminated string src onto dst
;   r1 = destination address, r2 = source address
;   returns r0 = destination address
; ═══════════════════════════════════════════════════════════════
strcat:
    MOV r0, r1
strcat_find_end:
    LOAD r3, r1
    JZ r3, strcat_copy
    LDI r3, 1
    ADD r1, r3
    JMP strcat_find_end
strcat_copy:
    LOAD r3, r2
    STORE r1, r3
    JZ r3, strcat_done
    LDI r3, 1
    ADD r1, r3
    ADD r2, r3
    JMP strcat_copy
strcat_done:
    RET

; ═══════════════════════════════════════════════════════════════
; memset -- fill memory region with a value
;   r1 = address, r2 = count (words), r3 = value
;   returns r0 = address
; ═══════════════════════════════════════════════════════════════
memset:
    MOV r0, r1
    JZ r2, memset_done
memset_loop:
    STORE r1, r3
    LDI r4, 1
    ADD r1, r4
    LDI r4, 1
    SUB r2, r4
    JNZ r2, memset_loop
memset_done:
    RET

; ═══════════════════════════════════════════════════════════════
; memcpy -- copy memory region
;   r1 = destination, r2 = source, r3 = count (words)
;   returns r0 = destination
; ═══════════════════════════════════════════════════════════════
memcpy:
    MOV r0, r1
    JZ r3, memcpy_done
memcpy_loop:
    LOAD r4, r2
    STORE r1, r4
    LDI r4, 1
    ADD r1, r4
    ADD r2, r4
    LDI r4, 1
    SUB r3, r4
    JNZ r3, memcpy_loop
memcpy_done:
    RET

; ═══════════════════════════════════════════════════════════════
; itoa -- convert unsigned integer to decimal string
;   r1 = value, r2 = output buffer address
;   returns r0 = buffer address
; ═══════════════════════════════════════════════════════════════
itoa:
    MOV r10, r1            ; save value
    MOV r11, r2            ; save buffer
    LDI r0, 0
    STORE r11, r0          ; null terminator
    JNZ r10, itoa_nonzero
    LDI r0, 48             ; '0'
    STORE r11, r0
    MOV r0, r11
    RET
itoa_nonzero:
    MOV r12, r11           ; r12 = write pos (for digits in reverse)
itoa_loop:
    JZ r10, itoa_reverse
    MOV r1, r10
    LDI r2, 10
    MOD r1, r2             ; r1 = value % 10
    LDI r2, 10
    DIV r10, r2            ; r10 = value / 10
    LDI r3, 48
    ADD r1, r3             ; r1 = digit + '0'
    STORE r12, r1
    LDI r3, 1
    ADD r12, r3
    JMP itoa_loop
itoa_reverse:
    ; Reverse digits in-place: r11=start, r12=past-end
    MOV r1, r11            ; dst = start
    LDI r3, 1
    SUB r12, r3            ; r12 = last digit pos
    MOV r14, r12           ; src = last digit pos
itoa_rev_loop:
    LOAD r2, r14
    STORE r1, r2
    LDI r3, 1
    ADD r1, r3             ; dst++
    CMP r14, r1
    LDI r3, 0xFFFFFFFF
    CMP r0, r3             ; if CMP == -1, src < dst, done
    JZ r0, itoa_done
    LDI r3, 1
    SUB r14, r3            ; src--
    JMP itoa_rev_loop
itoa_done:
    LDI r0, 0
    STORE r1, r0           ; null terminate
    MOV r0, r11
    RET

; ═══════════════════════════════════════════════════════════════
; malloc -- allocate N words from heap (bump allocator)
;   r1 = number of words to allocate
;   returns r0 = address of allocated block (0 if out of memory)
; ═══════════════════════════════════════════════════════════════
#define HEAP_START 0xC000
#define HEAP_SIZE 0x3000

malloc:
    LDI r2, HEAP_START
    LOAD r3, r2            ; r3 = current offset
    JNZ r3, malloc_inited
    LDI r3, 1              ; first alloc: offset starts at 1
    STORE r2, r3
malloc_inited:
    LOAD r3, r2
    MOV r4, r3
    ADD r4, r1             ; r4 = new offset
    LDI r5, HEAP_SIZE
    CMP r4, r5
    LDI r6, 1
    CMP r0, r6             ; r4 > HEAP_SIZE?
    JZ r0, malloc_fail
    STORE r2, r4           ; update pointer
    LDI r0, HEAP_START
    ADD r0, r3             ; return HEAP_START + old_offset
    RET
malloc_fail:
    LDI r0, 0
    RET

; ═══════════════════════════════════════════════════════════════
; free -- free allocated memory (no-op for bump allocator)
;   r1 = address to free (ignored)
; ═══════════════════════════════════════════════════════════════
free:
    RET
