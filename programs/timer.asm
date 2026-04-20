; timer.asm -- Countdown Timer with Alarm for Geometry OS
;
; Phase 75: Countdown timer. Enter time, press start, alarm on zero.
; Proves FRAME timing, BEEP audio, IKEY keyboard, RAM state.
;
; Controls:
;   1-9   = Set minutes (1-9)
;   Space = Start/Pause
;   R     = Reset to zero
;
; Timing:
;   60 frames = 1 second
;   Timer stored in centiseconds (100 per second)
;   When timer reaches 0, play alarm via BEEP
;
; RAM Layout:
;   0x4000 = timer centiseconds remaining
;   0x4004 = running flag (0=stopped, 1=running)
;   0x4008 = frame accumulator (0-59)
;   0x400C = alarm triggered flag (0=no, 1=yes)
;   0x4010 = initial time (for display)
;   0x5000 = text buffer
;
; NOTE: No colons in comments (assembler pitfall)

#define TIMER_CS    0x4000
#define RUNNING     0x4004
#define FRAME_ACC   0x4008
#define ALARMED     0x400C
#define INIT_TIME   0x4010
#define TXT_BUF     0x5000

; ── INIT ──────────────────────────────────────────
    LDI r1, 1
    LDI r2, 0

    ; Default timer = 60 seconds = 6000 centiseconds (1 minute)
    LDI r20, TIMER_CS
    LDI r3, 6000
    STORE r20, r3
    LDI r20, INIT_TIME
    STORE r20, r3

    LDI r20, RUNNING
    STORE r20, r2
    LDI r20, FRAME_ACC
    STORE r20, r2
    LDI r20, ALARMED
    STORE r20, r2

    ; Init stack
    LDI r30, 0xFD00

; ── MAIN LOOP ─────────────────────────────────────
main_loop:
    LDI r1, 1

    ; Read keyboard
    IKEY r5

    ; Check for digit keys 1-9 to set minutes
    CMPI r5, 49         ; '1'
    BLT r0, not_digit
    CMPI r5, 57         ; '9'
    BGE r0, not_digit

    ; Set timer to (key - 48) minutes = (key - 48) * 6000 cs
    SUBI r5, 48         ; minutes (1-9)
    LDI r6, 6000
    MUL r5, r6          ; total cs
    LDI r20, TIMER_CS
    STORE r20, r5
    LDI r20, INIT_TIME
    STORE r20, r5
    LDI r20, ALARMED
    LDI r2, 0
    STORE r20, r2
    JMP main_loop

not_digit:
    ; Space (32) = start/pause
    CMPI r5, 32
    JNZ r0, not_space

    ; Don't start if timer is 0 and alarm was triggered
    LDI r20, ALARMED
    LOAD r6, r20
    CMPI r6, 1
    JZ r0, main_loop

    ; Toggle running
    LDI r20, RUNNING
    LOAD r6, r20
    CMPI r6, 0
    JNZ r0, was_running
    LDI r6, 1
    STORE r20, r6
    JMP main_loop
was_running:
    LDI r6, 0
    STORE r20, r6
    JMP main_loop

not_space:
    ; R (82) = reset
    CMPI r5, 82
    JNZ r0, not_reset
    LDI r2, 0
    LDI r20, RUNNING
    STORE r20, r2
    LDI r20, FRAME_ACC
    STORE r20, r2
    LDI r20, ALARMED
    STORE r20, r2
    ; Restore initial time
    LDI r20, INIT_TIME
    LOAD r3, r20
    LDI r20, TIMER_CS
    STORE r20, r3
    JMP main_loop

not_reset:
    ; Update timer if running
    LDI r20, RUNNING
    LOAD r6, r20
    CMPI r6, 0
    JZ r0, skip_update

    ; frame_acc++
    LDI r20, FRAME_ACC
    LOAD r6, r20
    ADDI r6, 1
    STORE r20, r6

    ; Check if 60 frames passed
    CMPI r6, 60
    BLT r0, skip_update

    ; Reset frame acc
    LDI r6, 0
    LDI r20, FRAME_ACC
    STORE r20, r6

    ; timer_cs -= 100
    LDI r20, TIMER_CS
    LOAD r6, r20
    SUBI r6, 100
    STORE r20, r6

    ; Check if timer reached 0
    CMPI r6, 0
    BGE r0, skip_update  ; still positive, keep going

    ; Timer hit zero - stop and alarm
    LDI r6, 0
    LDI r20, TIMER_CS
    STORE r20, r6
    LDI r20, RUNNING
    STORE r20, r6
    LDI r6, 1
    LDI r20, ALARMED
    STORE r20, r6

    ; Play alarm sound (880Hz, 500ms)
    LDI r10, 880
    LDI r11, 500
    BEEP r10, r11

skip_update:
    ; Render
    CALL render

    FRAME
    JMP main_loop

; ── RENDER ────────────────────────────────────────
render:
    PUSH r31
    LDI r1, 1

    ; Background
    LDI r2, 0x1A0A2E
    FILL r2

    ; Title bar
    LDI r2, 0x2A1A3E
    LDI r3, 0
    LDI r4, 0
    LDI r5, 256
    LDI r6, 24
    RECTF r3, r4, r5, r6, r2

    ; Title text
    LDI r20, TXT_BUF
    STRO r20, "TIMER"
    LDI r3, 100
    LDI r4, 6
    LDI r20, TXT_BUF
    TEXT r3, r4, r20

    ; Main display panel
    LDI r2, 0x0A0616
    LDI r3, 30
    LDI r4, 40
    LDI r5, 196
    LDI r6, 80
    RECTF r3, r4, r5, r6, r2

    ; Compute time from centiseconds
    LDI r20, TIMER_CS
    LOAD r10, r20

    ; Minutes = timer / 6000
    LDI r11, 6000
    MOV r12, r10
    DIV r12, r11

    ; Remaining = timer % 6000
    MOV r13, r10
    MOD r13, r11

    ; Seconds = remaining / 100
    LDI r14, 100
    MOV r15, r13
    DIV r15, r14

    ; Centiseconds = remaining % 100
    MOV r16, r13
    MOD r16, r14

    ; Build MM:SS.CC string
    LDI r20, TXT_BUF
    MOV r12, r12
    CALL fmt_2digit
    LDI r2, 0x3A       ; ':'
    STORE r20, r2
    ADDI r20, 1
    MOV r12, r15
    CALL fmt_2digit
    LDI r2, 0x2E       ; '.'
    STORE r20, r2
    ADDI r20, 1
    MOV r12, r16
    CALL fmt_2digit
    LDI r2, 0
    STORE r20, r2

    ; Draw time - green normally, red when alarm
    LDI r20, ALARMED
    LOAD r6, r20
    CMPI r6, 1
    JNZ r0, time_normal
    LDI r5, 0xFF2222   ; red when alarm
    JMP draw_time
time_normal:
    LDI r5, 0x00FF00   ; green normally
draw_time:
    LDI r3, 60
    LDI r4, 65
    LDI r20, TXT_BUF
    DRAWTEXT r3, r4, r20, r5, r5

    ; Status panel
    LDI r2, 0x0D0D1A
    LDI r3, 30
    LDI r4, 130
    LDI r5, 196
    LDI r6, 50
    RECTF r3, r4, r5, r6, r2

    ; Show state text
    LDI r20, ALARMED
    LOAD r6, r20
    CMPI r6, 1
    JNZ r0, not_alarm_show

    ; "ALARM!" text
    LDI r20, TXT_BUF
    STRO r20, "ALARM!"
    LDI r3, 85
    LDI r4, 145
    LDI r5, 0xFF4444
    DRAWTEXT r3, r4, r20, r5, r5
    JMP status_done

not_alarm_show:
    LDI r20, RUNNING
    LOAD r6, r20
    CMPI r6, 0
    JNZ r0, timer_running

    ; "PAUSED" text
    LDI r20, TXT_BUF
    STRO r20, "Press 1-9 or Space"
    LDI r3, 45
    LDI r4, 145
    LDI r5, 0x888888
    DRAWTEXT r3, r4, r20, r5, r5
    JMP status_done

timer_running:
    LDI r20, TXT_BUF
    STRO r20, "COUNTING DOWN..."
    LDI r3, 50
    LDI r4, 145
    LDI r5, 0x44FF44
    DRAWTEXT r3, r4, r20, r5, r5

status_done:
    ; Bottom info bar
    LDI r2, 0x0A0A1A
    LDI r3, 0
    LDI r4, 220
    LDI r5, 256
    LDI r6, 36
    RECTF r3, r4, r5, r6, r2

    ; Controls hint
    LDI r20, TXT_BUF
    STRO r20, "1-9:mins  Space:start  R:reset"
    LDI r3, 20
    LDI r4, 228
    LDI r5, 0x666666
    DRAWTEXT r3, r4, r20, r5, r5

    POP r31
    RET

; ── FMT 2 DIGIT ───────────────────────────────────
fmt_2digit:
    PUSH r31
    LDI r21, 10
    MOV r22, r12
    DIV r22, r21
    MOV r23, r12
    MOD r23, r21
    ADDI r22, 0x30
    STORE r20, r22
    ADDI r20, 1
    ADDI r23, 0x30
    STORE r20, r23
    ADDI r20, 1
    POP r31
    RET
