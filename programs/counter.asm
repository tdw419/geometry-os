; counter.asm -- Interactive Counter App for Geometry OS
;
; Proves: bytecode persistence, HITSET/HITQ hit-routing, FRAME loop,
;         RECTF drawing, TEXT rendering, arithmetic (DIV/MOD/ADD/SUB).
;
; RAM Layout:
;   0x100        counter value (u32, starts at 0)
;   0x200-0x20F  scratch buffer for number text (null-terminated)
;
; Registers:
;   r0  - scratch / CMP result (clobbered by CMPI)
;   r1  - constant 1 (reloaded as needed)
;   r2  - scratch
;   r3  - scratch
;   r5  - color
;   r6  - divisor (10, 100)
;   r10 - counter value / hit query result
;   r11 - hundreds digit
;   r12 - hundreds * 100
;   r13 - remainder after hundreds
;   r14 - tens digit
;   r15 - ones digit
;   r20 - RAM pointer
;
; NOTE: The assembler's label detector uses line.find(':'), so we
;       avoid colons inside quoted strings to prevent misparse.

#define COUNT   0x100
#define SCRATCH 0x200

; ── INIT ──────────────────────────────────────
LDI r1, 1

; counter = 0
LDI r20, COUNT
LDI r2, 0
STORE r20, r2

; Fill screen dark
LDI r2, 0x1A1A2E
FILL r2

; Register hit regions
LDI r1, 60
LDI r2, 150
LDI r3, 40
LDI r4, 40
HITSET r1, r2, r3, r4, 1       ; [+] button, id=1

LDI r1, 156
LDI r2, 150
LDI r3, 40
LDI r4, 40
HITSET r1, r2, r3, r4, 2       ; [-] button, id=2

; ── MAIN LOOP ─────────────────────────────────
main_loop:
    LDI r1, 1

    ; ── DRAW ──

    ; Clear screen
    LDI r2, 0x1A1A2E
    FILL r2

    ; Draw [+] button (green)
    LDI r1, 60
    LDI r2, 150
    LDI r3, 40
    LDI r4, 40
    LDI r5, 0x2ECC71
    RECTF r1, r2, r3, r4, r5

    ; Draw [-] button (red)
    LDI r1, 156
    LDI r2, 150
    LDI r3, 40
    LDI r4, 40
    LDI r5, 0xE74C3C
    RECTF r1, r2, r3, r4, r5

    ; "+" label at center of green button
    LDI r20, SCRATCH
    LDI r2, 43           ; '+'
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2        ; null terminate

    LDI r1, 75
    LDI r2, 162
    LDI r3, SCRATCH
    TEXT r1, r2, r3

    ; "-" label at center of red button
    LDI r20, SCRATCH
    LDI r2, 45           ; '-'
    STORE r20, r2
    LDI r2, 0
    ADD r20, r1
    STORE r20, r2

    LDI r1, 171
    LDI r2, 162
    LDI r3, SCRATCH
    TEXT r1, r2, r3

    ; ── Render counter value ──
    ; Build "Count XXX" in scratch buffer (no colon to avoid label misparse)
    ; Restore r1 = 1 (clobbered by button label rendering)
    LDI r1, 1
    ; "Count " = 67 111 117 110 116 32
    LDI r20, SCRATCH
    LDI r2, 67            ; 'C'
    STORE r20, r2
    ADD r20, r1
    LDI r2, 111           ; 'o'
    STORE r20, r2
    ADD r20, r1
    LDI r2, 117           ; 'u'
    STORE r20, r2
    ADD r20, r1
    LDI r2, 110           ; 'n'
    STORE r20, r2
    ADD r20, r1
    LDI r2, 116           ; 't'
    STORE r20, r2
    ADD r20, r1
    LDI r2, 32            ; ' '
    STORE r20, r2
    ADD r20, r1

    ; Load counter
    LDI r2, COUNT
    LOAD r10, r2          ; r10 = counter value

    ; Hundreds: r10 / 100
    LDI r6, 100
    LDI r11, 0
    ADD r11, r10          ; r11 = counter
    DIV r11, r6           ; r11 = hundreds digit
    LDI r2, 48
    ADD r2, r11           ; ASCII '0' + hundreds
    STORE r20, r2
    ADD r20, r1           ; advance pointer

    ; Remainder = counter - hundreds*100
    LDI r12, 0
    ADD r12, r11          ; r12 = hundreds
    LDI r2, 100
    MUL r12, r2           ; r12 = hundreds * 100
    LDI r13, 0
    ADD r13, r10          ; r13 = counter
    SUB r13, r12          ; r13 = remainder (0..99)

    ; Tens: remainder / 10
    LDI r6, 10
    LDI r14, 0
    ADD r14, r13          ; r14 = remainder
    DIV r14, r6           ; r14 = tens digit
    LDI r2, 48
    ADD r2, r14           ; ASCII '0' + tens
    STORE r20, r2
    ADD r20, r1

    ; Ones: remainder % 10
    LDI r15, 0
    ADD r15, r13          ; r15 = remainder
    LDI r6, 10
    MOD r15, r6           ; r15 = ones digit
    LDI r2, 48
    ADD r2, r15           ; ASCII '0' + ones
    STORE r20, r2
    ADD r20, r1

    ; Null terminate
    LDI r2, 0
    STORE r20, r2

    ; Render counter text centered
    LDI r1, 70
    LDI r2, 80
    LDI r3, SCRATCH
    TEXT r1, r2, r3

    ; ── FRAME ──
    FRAME

    ; ── HIT CHECK ──
    HITQ r10
    JZ r10, main_loop     ; no click → loop

    ; r10 = hit region id
    CMPI r10, 1
    JNZ r0, check_minus

    ; id=1 → increment
    LDI r20, COUNT
    LOAD r2, r20
    LDI r1, 1
    ADD r2, r1
    STORE r20, r2
    JMP main_loop

check_minus:
    CMPI r10, 2
    JNZ r0, main_loop    ; unknown id → ignore
    ; id=2 → decrement
    LDI r20, COUNT
    LOAD r2, r20
    LDI r1, 1
    SUB r2, r1
    STORE r20, r2
    JMP main_loop
