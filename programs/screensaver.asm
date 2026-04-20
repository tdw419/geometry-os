; screensaver.asm -- Multi-effect screensaver for Geometry OS
; Phase 64 demo: uses MIN/MAX/CLAMP opcodes
; Any key press exits. Effects cycle every ~10 seconds.

#define TICKS_PER_EFFECT 600
#define NUM_EFFECTS      3
#define NUM_STARS        40
#define TICKS_ADDR       0xFFE
#define STARS_BASE       0x1000

    LDI r0, 0

    ; Initialize stars: x,y pairs at STARS_BASE
    LDI r10, NUM_STARS
    LDI r11, STARS_BASE
init_stars:
    RAND r12
    ANDI r12, 0xFF
    STORE r11, r12
    ADDI r11, 1
    RAND r12
    ANDI r12, 0xFF
    STORE r11, r12
    ADDI r11, 1
    SUBI r10, 1
    JNZ r10, init_stars

main_loop:
    IKEY r0
    JNZ r0, exit_screensaver

    ; Current effect = (ticks / TICKS_PER_EFFECT) % NUM_EFFECTS
    LDI r25, TICKS_ADDR
    LOAD r21, r25
    LDI r10, TICKS_PER_EFFECT
    DIV r21, r10
    LDI r10, NUM_EFFECTS
    MOD r21, r10

    ; Clear screen (black)
    LDI r10, 0
    FILL r10

    ; Dispatch
    CMPI r21, 0
    JZ r0, do_starfield
    CMPI r21, 1
    JZ r0, do_bounce
    JMP do_plasma

; ── Starfield ─────────────────────────────────
do_starfield:
    LDI r10, NUM_STARS
    LDI r11, STARS_BASE
star_loop:
    LOAD r12, r11        ; x
    ADDI r11, 1
    LOAD r13, r11        ; y
    ADDI r13, 1
    ANDI r13, 0xFF       ; wrap
    STORE r11, r13
    SUBI r11, 1

    ; Brightness via MIN/MAX
    LDI r14, 0x444444
    MAX r13, r14
    LDI r14, 0xCCCCCC
    MIN r13, r14

    PSET r12, r13, r13
    ADDI r11, 2
    SUBI r10, 1
    JNZ r10, star_loop
    JMP frame_end

; ── Bouncing logo ─────────────────────────────
do_bounce:
    LDI r25, TICKS_ADDR
    LOAD r10, r25
    ANDI r10, 0x7F
    ADDI r10, 10

    LDI r25, TICKS_ADDR
    LOAD r11, r25
    ; Slow y: divide by 3 via LDI + DIV
    LDI r26, 3
    DIV r11, r26
    ANDI r11, 0x7F
    ADDI r11, 10

    ; CLAMP to stay on screen
    LDI r12, 10
    LDI r13, 200
    CLAMP r10, r12, r13
    CLAMP r11, r12, r13

    ; Green logo
    LDI r14, 0x00FF00
    LDI r15, 15
    LDI r16, 20
    RECTF r10, r11, r15, r16, r14

    ; Shadow
    LDI r14, 0x005500
    ADDI r10, 2
    ADDI r11, 2
    RECTF r10, r11, r15, r16, r14
    JMP frame_end

; ── Plasma ────────────────────────────────────
do_plasma:
    LDI r25, TICKS_ADDR
    LOAD r10, r25        ; time
    LDI r11, 0           ; y
plasma_y:
    LDI r12, 0           ; x
plasma_x:
    ; Red channel
    MOV r13, r12
    ADD r13, r11
    ADD r13, r10
    ANDI r13, 0xFF
    SHLI r13, 16

    ; Green channel
    MOV r14, r12
    ADD r14, r11
    ADD r14, r10
    ADDI r14, 85
    ANDI r14, 0xFF
    LDI r15, 200
    MIN r14, r15
    SHLI r14, 8
    ADD r13, r14

    ; Blue channel
    MOV r14, r12
    ADD r14, r11
    ADD r14, r10
    ADDI r14, 170
    ANDI r14, 0xFF
    LDI r15, 200
    MIN r14, r15
    ADD r13, r14

    PSET r12, r11, r13

    ADDI r12, 4
    CMPI r12, 256
    JNZ r0, plasma_x

    ADDI r11, 4
    CMPI r11, 256
    JNZ r0, plasma_y
    JMP frame_end

frame_end:
    FRAME
    JMP main_loop

exit_screensaver:
    LDI r10, 0
    FILL r10
    FRAME
    HALT
