; pulse.asm -- Self-Animating Pulse Bar for Geometry OS
;
; Proves: self-driven state change with zero external input.
;         No IKEY, no HITQ -- purely FRAME-driven animation.
;
; RAM Layout:
;   0x200        tick counter (increments every frame)
;   0x204        bar width (oscillates via triangle wave from tick)
;   0xFFE        frame_count (written by FRAME opcode, read-only for us)
;
; The animation:
;   - A colored bar grows right then shrinks left, cycling forever
;   - Tick increments each frame, bar_width = triangle_wave(tick % 200)
;   - Triangle wave: ramp up 0->100 in 100 frames, ramp down 100->0 in 100 frames
;   - Color shifts with tick for visual feedback
;
; Registers:
;   r1  - constant 1
;   r2  - scratch
;   r3  - scratch (tick, temp)
;   r5  - color
;   r6  - bar width
;   r7  - divisor/modulus (200, 100)
;   r8  - phase (tick % 200)
;   r9  - frame_count from RAM[0xFFE]
;

#define TICK      0x200
#define BAR_W     0x204
#define SCRATCH   0x300
#define FCOUNT    0xFFE

; ── INIT ──────────────────────────────────────
LDI r1, 1

; tick = 0
LDI r2, TICK
LDI r3, 0
STORE r2, r3

; bar_width = 0
LDI r2, BAR_W
LDI r3, 0
STORE r2, r3

; ── MAIN LOOP ─────────────────────────────────
main_loop:
    LDI r1, 1

    ; ── UPDATE STATE ──

    ; Increment tick
    LDI r20, TICK
    LOAD r3, r20           ; r3 = tick
    ADD r3, r1             ; r3 = tick + 1
    STORE r20, r3          ; tick++

    ; Compute phase = tick % 200
    LDI r8, 0
    ADD r8, r3             ; r8 = tick
    LDI r7, 200
    MOD r8, r7             ; r8 = tick % 200 (0..199)

    ; Triangle wave from phase:
    ;   if phase < 100: bar_width = phase (ramp up)
    ;   if phase >= 100: bar_width = 200 - phase (ramp down)
    ; CMPI sets r0: 0xFFFFFFFF if <, 0 if ==, 1 if >
    ; BGE branches when r0 != 0xFFFFFFFF (i.e., >= )
    CMPI r8, 100
    BGE r0, ramp_down       ; if phase >= 100, go to ramp_down

    ; ramp up: bar_width = phase
    LDI r6, 0
    ADD r6, r8             ; r6 = phase (0..99)
    JMP store_width

ramp_down:
    ; ramp down: bar_width = 200 - phase
    LDI r2, 200
    LDI r6, 0
    ADD r6, r8             ; r6 = phase
    SUB r2, r6             ; r2 = 200 - phase
    LDI r6, 0
    ADD r6, r2             ; r6 = bar_width

store_width:
    LDI r20, BAR_W
    STORE r20, r6          ; save bar_width to RAM

    ; ── DRAW ──

    ; Clear screen to dark blue-black
    LDI r2, 0x0D0D1A
    FILL r2

    ; Draw bar: filled rectangle at (78, 100) with width=bar_width, height=30
    ; Color cycles: use tick to shift hue
    ; Simple: alternate between two colors based on tick/50
    LDI r20, TICK
    LOAD r3, r20           ; r3 = tick
    LDI r7, 50
    LDI r2, 0
    ADD r2, r3
    MOD r2, r7             ; r2 = tick % 50

    ; Color: 0x2ECC71 (green) base, shift with tick
    ; r5 = 0x2ECC71 + (tick % 50) * small_offset
    ; Simpler: just use tick low bits to cycle RGB
    LDI r5, 0x2ECC71       ; base green
    ADD r5, r3             ; shift color with tick (wraps around nicely)

    ; Bar position: x=78, y=100, w=bar_width, h=30
    LDI r1, 78             ; x
    LDI r2, 100            ; y
    LDI r3, 0
    ADD r3, r6             ; w = bar_width (from above)
    LDI r4, 30             ; h
    RECTF r1, r2, r3, r4, r5

    ; Draw "PULSE" title text
    LDI r20, SCRATCH       ; scratch buffer
    LDI r2, 80             ; 'P'
    STORE r20, r2
    LDI r1, 1
    ADD r20, r1
    LDI r2, 85             ; 'U'
    STORE r20, r2
    ADD r20, r1
    LDI r2, 76             ; 'L'
    STORE r20, r2
    ADD r20, r1
    LDI r2, 83             ; 'S'
    STORE r20, r2
    ADD r20, r1
    LDI r2, 69             ; 'E'
    STORE r20, r2
    ADD r20, r1
    LDI r2, 0              ; null terminator
    STORE r20, r2

    LDI r1, 5
    LDI r2, 5
    LDI r3, SCRATCH
    TEXT r1, r2, r3

    ; Draw frame counter as 3-digit number at (5, 20)
    LDI r20, SCRATCH
    LDI r20, TICK
    LOAD r10, r20          ; r10 = tick value

    ; Convert tick to 3 ASCII digits at 0x200
    LDI r1, 1
    LDI r20, SCRATCH

    ; Hundreds
    LDI r6, 100
    LDI r11, 0
    ADD r11, r10
    DIV r11, r6            ; r11 = hundreds
    LDI r2, 48
    ADD r2, r11
    STORE r20, r2
    ADD r20, r1

    ; Tens
    LDI r12, 0
    ADD r12, r11
    LDI r2, 100
    MUL r12, r2            ; r12 = hundreds * 100
    LDI r13, 0
    ADD r13, r10
    SUB r13, r12           ; r13 = remainder
    LDI r6, 10
    LDI r14, 0
    ADD r14, r13
    DIV r14, r6            ; r14 = tens
    LDI r2, 48
    ADD r2, r14
    STORE r20, r2
    ADD r20, r1

    ; Ones
    LDI r15, 0
    ADD r15, r13
    LDI r6, 10
    MOD r15, r6            ; r15 = ones
    LDI r2, 48
    ADD r2, r15
    STORE r20, r2
    ADD r20, r1
    LDI r2, 0
    STORE r20, r2          ; null terminate

    LDI r1, 5
    LDI r2, 20
    LDI r3, SCRATCH
    TEXT r1, r2, r3

    ; ── FRAME ──
    FRAME

    JMP main_loop
