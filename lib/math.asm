; lib/math.asm -- Standard Library: math operations
;
; Calling convention:
;   Arguments: r1-r5 (r0 = return value)
;   Caller-saved: r1-r9
;   Callee-saved: r10-r25

; ═══════════════════════════════════════════════════════════════
; abs -- absolute value
;   r1 = value
;   returns r0 = |value|
; ═══════════════════════════════════════════════════════════════
abs:
    MOV r0, r1
    ; Check if negative (bit 31 set means negative in signed interpretation)
    LDI r2, 0x80000000
    AND r2, r0
    JZ r2, abs_done        ; positive, done
    ; Negate: 2's complement
    LDI r2, 0
    SUB r2, r0
    MOV r0, r2
abs_done:
    RET

; ═══════════════════════════════════════════════════════════════
; min -- minimum of two values
;   r1 = a, r2 = b
;   returns r0 = (a < b) ? a : b
; ═══════════════════════════════════════════════════════════════
min:
    CMP r1, r2
    LDI r3, 0xFFFFFFFF
    CMP r0, r3             ; CMP == -1 means r1 < r2
    JZ r0, min_a
    MOV r0, r2             ; r2 <= r1
    RET
min_a:
    MOV r0, r1
    RET

; ═══════════════════════════════════════════════════════════════
; max -- maximum of two values
;   r1 = a, r2 = b
;   returns r0 = (a > b) ? a : b
; ═══════════════════════════════════════════════════════════════
max:
    CMP r1, r2
    LDI r3, 1
    CMP r0, r3             ; CMP == 1 means r1 > r2
    JZ r0, max_a
    MOV r0, r2             ; r2 >= r1
    RET
max_a:
    MOV r0, r1
    RET

; ═══════════════════════════════════════════════════════════════
; clamp -- clamp value to range [lo, hi]
;   r1 = value, r2 = lo, r3 = hi
;   returns r0 = clamped value
; ═══════════════════════════════════════════════════════════════
clamp:
    ; if value < lo, return lo
    CMP r1, r2
    LDI r4, 0xFFFFFFFF
    CMP r0, r4
    JZ r0, clamp_lo
    ; if value > hi, return hi
    CMP r1, r3
    LDI r4, 1
    CMP r0, r4
    JZ r0, clamp_hi
    MOV r0, r1
    RET
clamp_lo:
    MOV r0, r2
    RET
clamp_hi:
    MOV r0, r3
    RET

; ═══════════════════════════════════════════════════════════════
; sqrt_approx -- integer square root via Newton's method
;   r1 = value (unsigned)
;   returns r0 = floor(sqrt(value))
; ═══════════════════════════════════════════════════════════════
sqrt_approx:
    ; Handle 0
    JNZ r1, sqrt_nonzero
    LDI r0, 0
    RET
sqrt_nonzero:
    ; Initial guess: value / 2 (or value >> 1)
    MOV r0, r1
    LDI r2, 1
    SHR r0, r2             ; guess = value >> 1
    ; Newton's method: x = (x + value/x) / 2
    ; Repeat until convergence
    MOV r10, r1            ; save value
    LDI r11, 0             ; iteration counter
    LDI r12, 16            ; max iterations
sqrt_loop:
    ; prev = guess
    MOV r13, r0            ; r13 = previous guess
    ; x = (guess + value/guess) / 2
    JZ r0, sqrt_done       ; avoid div by zero
    MOV r1, r10            ; r1 = value
    MOV r2, r0             ; r2 = guess
    DIV r1, r2             ; r0 = value / guess  -- WAIT, DIV uses r10,r2 -> r10 = r10/r2
    ; Actually DIV opcode: DIV rd, rs means rd = rd / rs
    ; So we need: DIV value_reg, guess_reg
    MOV r1, r10            ; r1 = value
    MOV r2, r0             ; r2 = guess  
    DIV r1, r2             ; r1 = value / guess
    ADD r1, r0             ; r1 = (value/guess) + guess
    LDI r2, 1
    SHR r1, r2             ; r1 = ((value/guess) + guess) / 2
    MOV r0, r1             ; guess = new value
    ; Check convergence: if guess == prev, done
    CMP r0, r13
    JZ r0, sqrt_done       ; CMP == 0 means equal
    ; Also check iteration limit
    LDI r2, 1
    ADD r11, r2
    CMP r11, r12
    LDI r2, 1
    CMP r0, r2             ; if counter >= max_iter
    JNZ r0, sqrt_loop
sqrt_done:
    RET

; ═══════════════════════════════════════════════════════════════
; dist2 -- squared distance between two points
;   r1 = x1, r2 = y1, r3 = x2, r4 = y2
;   returns r0 = (x2-x1)^2 + (y2-y1)^2
; ═══════════════════════════════════════════════════════════════
dist2:
    ; dx = x2 - x1
    MOV r0, r3
    SUB r0, r1             ; r0 = dx
    MOV r5, r0             ; r5 = dx
    MUL r0, r5             ; r0 = dx * dx
    MOV r5, r0             ; r5 = dx^2
    ; dy = y2 - y1
    MOV r0, r4
    SUB r0, r2             ; r0 = dy
    MOV r6, r0             ; r6 = dy
    MUL r0, r6             ; r0 = dy * dy
    ADD r0, r5             ; r0 = dx^2 + dy^2
    RET
