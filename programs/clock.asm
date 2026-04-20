; clock.asm -- Digital Clock App for Geometry OS
;
; Proves: FRAME (timing), TEXT (text rendering), RECTF (UI panels),
;         DIV/MOD (time math), LDI/LOAD/STORE (RAM state), STRO (strings).
;
; Features:
;   - Digital clock display (HH:MM:SS) derived from frame counter
;   - 60 frames = 1 simulated second
;   - Day counter from epoch
;   - Blinking colon separator
;   - Color-coded info panels
;   - Title bar and status info
;
; RAM Layout:
;   0x6000       blink toggle (0 or 1, flips each half-second)
;   0x6100-0x6140 scratch buffer for text rendering
;
; Screen Layout (256x256):
;   Row 0-19:     Title bar (dark navy 0x0D1B2A)
;   Row 20-39:    Date and day info panel (0x101828)
;   Row 40-119:   Main clock display (dark panel 0x060612)
;   Row 120-179:  Statistics panel (0x0D0D1A)
;   Row 180-239:  Large time display (dark panel 0x060612)
;   Row 240-255:  Status bar (0x0A0A1A)
;
; NOTE: No colons in comments (assembler pitfall)

#define BLINK   0x6000
#define TICKS   0xFFE
#define SCRATCH 0x6100

; =========================================
; INIT
; =========================================
LDI r1, 1
LDI r30, 0xFD00

; Init blink counter
LDI r20, BLINK
LDI r2, 0
STORE r20, r2

; =========================================
; MAIN LOOP
; =========================================
main_loop:
    LDI r1, 1

    ; Draw everything
    CALL render

    ; Signal frame
    FRAME

    ; Update blink toggle
    LDI r20, TICKS
    LOAD r6, r20
    LDI r2, 30
    DIV r6, r2
    LDI r2, 2
    MOD r6, r2
    LDI r20, BLINK
    STORE r20, r6

    JMP main_loop

; =========================================
; RENDER
; =========================================
render:
    PUSH r31
    LDI r1, 1

    ; ── Background ──
    LDI r2, 0x0A0A1E
    FILL r2

    ; ── Title bar ──
    LDI r1, 0
    LDI r2, 0
    LDI r3, 256
    LDI r4, 20
    LDI r5, 0x0D1B2A
    RECTF r1, r2, r3, r4, r5

    LDI r20, SCRATCH
    STRO r20, "GeoClock v1.0"
    LDI r2, 0
    STORE r20, r2
    LDI r1, 8
    LDI r2, 6
    LDI r3, SCRATCH
    TEXT r1, r2, r3

    ; ── Info panel (date/day) ──
    LDI r1, 0
    LDI r2, 20
    LDI r3, 256
    LDI r4, 20
    LDI r5, 0x101828
    RECTF r1, r2, r3, r4, r5

    ; Compute time values
    ; total_seconds = frame_count / 60
    LDI r20, TICKS
    LOAD r6, r20
    LDI r2, 60
    DIV r6, r2
    ; r6 = total_seconds

    ; hours = total_seconds / 3600
    LDI r7, 0
    ADD r7, r6
    LDI r2, 3600
    DIV r7, r2
    ; r7 = hours

    ; minutes = (total_seconds % 3600) / 60
    LDI r8, 0
    ADD r8, r6
    LDI r2, 3600
    MOD r8, r2
    LDI r2, 60
    DIV r8, r2
    ; r8 = minutes

    ; seconds = total_seconds % 60
    LDI r9, 0
    ADD r9, r6
    LDI r2, 60
    MOD r9, r2
    ; r9 = seconds

    ; days = hours / 24
    LDI r10, 0
    ADD r10, r7
    LDI r2, 24
    DIV r10, r2
    ; r10 = days

    ; hours within day = hours % 24
    LDI r7, 0
    ADD r7, r6
    LDI r2, 3600
    DIV r7, r2
    LDI r2, 24
    MOD r7, r2
    ; r7 = hours (0-23)

    ; Build "Day N  HH:MM:SS" string
    LDI r20, SCRATCH
    STRO r20, "Day "
    LDI r6, 0
    ADD r6, r10
    CALL write_number
    LDI r2, 32
    STORE r20, r2
    ADD r20, r1

    ; Write hours (2-digit with leading zero)
    CALL write_two_digit
    ; Colon
    LDI r20, BLINK
    LOAD r14, r20
    CMPI r14, 0
    JNZ r14, colon_on1
    LDI r2, 32
    STORE r20, r2
    ADD r20, r1
    LDI r2, 32
    STORE r20, r2
    ADD r20, r1
    JMP colon_done1
colon_on1:
    LDI r2, 58
    STORE r20, r2
    ADD r20, r1
    LDI r2, 58
    STORE r20, r2
    ADD r20, r1
colon_done1:

    ; Write minutes
    LDI r6, 0
    ADD r6, r8
    CALL write_two_digit
    ; Colon
    LDI r20, BLINK
    LOAD r14, r20
    CMPI r14, 0
    JNZ r14, colon_on2
    LDI r2, 32
    STORE r20, r2
    ADD r20, r1
    LDI r2, 32
    STORE r20, r2
    ADD r20, r1
    JMP colon_done2
colon_on2:
    LDI r2, 58
    STORE r20, r2
    ADD r20, r1
    LDI r2, 58
    STORE r20, r2
    ADD r20, r1
colon_done2:

    ; Write seconds
    LDI r6, 0
    ADD r6, r9
    CALL write_two_digit

    ; Null terminate
    LDI r2, 0
    STORE r20, r2

    ; Render time string at top of info panel
    LDI r1, 8
    LDI r2, 24
    LDI r3, SCRATCH
    TEXT r1, r2, r3

    ; ── Main clock panel ──
    LDI r1, 0
    LDI r2, 40
    LDI r3, 256
    LDI r4, 80
    LDI r5, 0x060612
    RECTF r1, r2, r3, r4, r5

    ; Large time display using TEXT at y=60
    ; Build "HH:MM:SS" string again for the large display
    LDI r20, SCRATCH
    LDI r6, 0
    ADD r6, r7
    CALL write_two_digit
    LDI r2, 58
    STORE r20, r2
    ADD r20, r1
    LDI r6, 0
    ADD r6, r8
    CALL write_two_digit
    LDI r2, 58
    STORE r20, r2
    ADD r20, r1
    LDI r6, 0
    ADD r6, r9
    CALL write_two_digit
    LDI r2, 0
    STORE r20, r2

    LDI r1, 80
    LDI r2, 65
    LDI r3, SCRATCH
    TEXT r1, r2, r3

    ; ── Statistics panel ──
    LDI r1, 0
    LDI r2, 120
    LDI r3, 256
    LDI r4, 60
    LDI r5, 0x0D0D1A
    RECTF r1, r2, r3, r4, r5

    ; Frame count
    LDI r20, SCRATCH
    STRO r20, "Frames: "
    LDI r20, TICKS
    LOAD r6, r20
    LDI r20, SCRATCH
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    CALL write_number
    LDI r2, 0
    STORE r20, r2
    LDI r1, 8
    LDI r2, 126
    LDI r3, SCRATCH
    TEXT r1, r2, r3

    ; Total seconds
    LDI r20, SCRATCH
    STRO r20, "Seconds: "
    LDI r20, TICKS
    LOAD r6, r20
    LDI r2, 60
    DIV r6, r2
    LDI r20, SCRATCH
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    CALL write_number
    LDI r2, 0
    STORE r20, r2
    LDI r1, 8
    LDI r2, 138
    LDI r3, SCRATCH
    TEXT r1, r2, r3

    ; Total minutes
    LDI r20, SCRATCH
    STRO r20, "Minutes: "
    LDI r20, TICKS
    LOAD r6, r20
    LDI r2, 3600
    DIV r6, r2
    LDI r20, SCRATCH
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    ADD r20, r1
    CALL write_number
    LDI r2, 0
    STORE r20, r2
    LDI r1, 8
    LDI r2, 150
    LDI r3, SCRATCH
    TEXT r1, r2, r3

    ; Tick rate info
    LDI r20, SCRATCH
    STRO r20, "60 frames = 1 sim second"
    LDI r2, 0
    STORE r20, r2
    LDI r1, 8
    LDI r2, 164
    LDI r3, SCRATCH
    TEXT r1, r2, r3

    ; ── Large seconds display panel ──
    LDI r1, 0
    LDI r2, 180
    LDI r3, 256
    LDI r4, 56
    LDI r5, 0x060612
    RECTF r1, r2, r3, r4, r5

    ; Elapsed time string
    LDI r20, SCRATCH
    STRO r20, "Elapsed: "
    LDI r6, 0
    ADD r6, r10
    CALL write_number
    LDI r2, 32
    STORE r20, r2
    ADD r20, r1
    LDI r2, 100            ; 'd'
    STORE r20, r2
    ADD r20, r1
    LDI r2, 32
    STORE r20, r2
    ADD r20, r1
    LDI r6, 0
    ADD r6, r7
    CALL write_two_digit
    LDI r2, 58
    STORE r20, r2
    ADD r20, r1
    LDI r6, 0
    ADD r6, r8
    CALL write_two_digit
    LDI r2, 58
    STORE r20, r2
    ADD r20, r1
    LDI r6, 0
    ADD r6, r9
    CALL write_two_digit
    LDI r2, 0
    STORE r20, r2
    LDI r1, 8
    LDI r2, 195
    LDI r3, SCRATCH
    TEXT r1, r2, r3

    ; ── Status bar ──
    LDI r1, 0
    LDI r2, 240
    LDI r3, 256
    LDI r4, 16
    LDI r5, 0x0A0A1A
    RECTF r1, r2, r3, r4, r5

    LDI r20, SCRATCH
    STRO r20, "Geometry OS Clock"
    LDI r2, 0
    STORE r20, r2
    LDI r1, 8
    LDI r2, 244
    LDI r3, SCRATCH
    TEXT r1, r2, r3

    POP r31
    RET

; =========================================
; WRITE_TWO_DIGIT: Write value in r6 as 2-digit decimal at SCRATCH[r20]
; Advances r20 by 2
; Expects r1 = 1
; =========================================
write_two_digit:
    PUSH r31
    LDI r1, 1

    ; Tens digit
    LDI r14, 0
    ADD r14, r6
    LDI r2, 10
    DIV r14, r2
    LDI r2, 48
    ADD r2, r14
    STORE r20, r2
    ADD r20, r1

    ; Ones digit
    LDI r14, 0
    ADD r14, r6
    LDI r2, 10
    MOD r14, r2
    LDI r2, 48
    ADD r2, r14
    STORE r20, r2
    ADD r20, r1

    POP r31
    RET

; =========================================
; WRITE_NUMBER: Write number r6 as decimal at SCRATCH[r20]
; Advances r20 past the digits
; Expects r1 = 1
; =========================================
write_number:
    PUSH r31
    LDI r1, 1

    ; Handle 0
    CMPI r6, 0
    JNZ r6, wn_not_zero
    LDI r2, 48
    STORE r20, r2
    ADD r20, r1
    POP r31
    RET

wn_not_zero:
    ; Push digits onto stack (ones first), then pop (most significant first)
    LDI r15, 0
    ADD r15, r6          ; r15 = remaining value
    LDI r14, 0           ; digit count

wn_push_loop:
    CMPI r15, 0
    JZ r15, wn_pop_loop
    LDI r16, 0
    ADD r16, r15
    LDI r2, 10
    MOD r16, r2          ; r16 = ones digit
    PUSH r16
    ADD r14, r1
    LDI r2, 10
    DIV r15, r2
    JMP wn_push_loop

wn_pop_loop:
    CMPI r14, 0
    JZ r14, wn_done
    POP r16
    LDI r2, 48
    ADD r2, r16
    STORE r20, r2
    ADD r20, r1
    SUB r14, r1
    JMP wn_pop_loop

wn_done:
    POP r31
    RET
