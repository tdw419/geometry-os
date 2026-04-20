; about.asm -- System Information Panel for Geometry OS
;
; Shows: version, opcode count, RAM size, screen size,
;        process count, uptime (frames), color palette.
;
; Proves: TEXT, DRAWTEXT, RECTF, STRO, RAM introspection,
;         frame counter (TICKS), integer-to-string conversion.
;
; RAM Layout:
;   0x6000-0x60FF  String buffers for text
;   0x6100-0x61FF  Number conversion buffer
;   0x6200-0x623C  Palette color table (15 entries)
;
; Screen Layout (256x256):
;   Row 0-23    Title bar (dark navy)
;   Row 24-220  Info panel
;   Row 220-255 Footer + palette bar
;
; No colons in comments (assembler pitfall)

#define TICKS   0xFFE
#define BUF     0x6000
#define NUMBUF  0x6100
#define PAL     0x6200

; Init stack
LDI r30, 0xFD00
LDI r1, 1

; =========================================
; Init palette table in RAM (15 colors)
; =========================================
LDI r20, PAL

LDI r21, 0xFF0000
STORE r20, r21
ADD r20, r1

LDI r21, 0xFF4400
STORE r20, r21
ADD r20, r1

LDI r21, 0xFF8800
STORE r20, r21
ADD r20, r1

LDI r21, 0xFFCC00
STORE r20, r21
ADD r20, r1

LDI r21, 0xFFFF00
STORE r20, r21
ADD r20, r1

LDI r21, 0x88FF00
STORE r20, r21
ADD r20, r1

LDI r21, 0x00FF44
STORE r20, r21
ADD r20, r1

LDI r21, 0x00FFAA
STORE r20, r21
ADD r20, r1

LDI r21, 0x00FFFF
STORE r20, r21
ADD r20, r1

LDI r21, 0x0088FF
STORE r20, r21
ADD r20, r1

LDI r21, 0x0044FF
STORE r20, r21
ADD r20, r1

LDI r21, 0x4400FF
STORE r20, r21
ADD r20, r1

LDI r21, 0x8800FF
STORE r20, r21
ADD r20, r1

LDI r21, 0xCC00FF
STORE r20, r21
ADD r20, r1

LDI r21, 0xFF00AA
STORE r20, r21

; =========================================
; Draw background
; =========================================
LDI r1, 0x0D1B2A
FILL r1

; Title bar
LDI r1, 0
LDI r2, 0
LDI r3, 256
LDI r4, 24
LDI r5, 0x1B3A5C
RECTF r1, r2, r3, r4, r5

; Title text
LDI r20, BUF
STRO r20, "About Geometry OS"
LDI r1, 64
LDI r2, 6
LDI r3, BUF
LDI r4, 0xFFFFFF
LDI r5, 0x1B3A5C
DRAWTEXT r1, r2, r3, r4, r5

; =========================================
; Info panel background
; =========================================
LDI r1, 12
LDI r2, 28
LDI r3, 232
LDI r4, 188
LDI r5, 0x141428
RECTF r1, r2, r3, r4, r5

; =========================================
; Info lines (label on left, value on right)
; =========================================

; -- Version --
LDI r20, BUF
STRO r20, "Version"
LDI r1, 24
LDI r2, 36
LDI r3, BUF
LDI r4, 0x8888CC
LDI r5, 0x141428
DRAWTEXT r1, r2, r3, r4, r5

LDI r20, BUF
STRO r20, "v1.0.0"
LDI r1, 140
LDI r2, 36
LDI r3, BUF
LDI r4, 0xFFFFFF
LDI r5, 0x141428
DRAWTEXT r1, r2, r3, r4, r5

; -- Opcodes --
LDI r20, BUF
STRO r20, "Opcodes"
LDI r1, 24
LDI r2, 54
LDI r3, BUF
LDI r4, 0x8888CC
LDI r5, 0x141428
DRAWTEXT r1, r2, r3, r4, r5

LDI r20, BUF
STRO r20, "113"
LDI r1, 140
LDI r2, 54
LDI r3, BUF
LDI r4, 0xFFFFFF
LDI r5, 0x141428
DRAWTEXT r1, r2, r3, r4, r5

; -- Registers --
LDI r20, BUF
STRO r20, "Registers"
LDI r1, 24
LDI r2, 72
LDI r3, BUF
LDI r4, 0x8888CC
LDI r5, 0x141428
DRAWTEXT r1, r2, r3, r4, r5

LDI r20, BUF
STRO r20, "32 (r0-r31)"
LDI r1, 140
LDI r2, 72
LDI r3, BUF
LDI r4, 0xFFFFFF
LDI r5, 0x141428
DRAWTEXT r1, r2, r3, r4, r5

; -- RAM --
LDI r20, BUF
STRO r20, "RAM"
LDI r1, 24
LDI r2, 90
LDI r3, BUF
LDI r4, 0x8888CC
LDI r5, 0x141428
DRAWTEXT r1, r2, r3, r4, r5

LDI r20, BUF
STRO r20, "65536 x u32 (64K)"
LDI r1, 140
LDI r2, 90
LDI r3, BUF
LDI r4, 0xFFFFFF
LDI r5, 0x141428
DRAWTEXT r1, r2, r3, r4, r5

; -- Screen --
LDI r20, BUF
STRO r20, "Screen"
LDI r1, 24
LDI r2, 108
LDI r3, BUF
LDI r4, 0x8888CC
LDI r5, 0x141428
DRAWTEXT r1, r2, r3, r4, r5

LDI r20, BUF
STRO r20, "256 x 256 pixels"
LDI r1, 140
LDI r2, 108
LDI r3, BUF
LDI r4, 0xFFFFFF
LDI r5, 0x141428
DRAWTEXT r1, r2, r3, r4, r5

; -- Colors --
LDI r20, BUF
STRO r20, "Colors"
LDI r1, 24
LDI r2, 126
LDI r3, BUF
LDI r4, 0x8888CC
LDI r5, 0x141428
DRAWTEXT r1, r2, r3, r4, r5

LDI r20, BUF
STRO r20, "24-bit RGB (16M)"
LDI r1, 140
LDI r2, 126
LDI r3, BUF
LDI r4, 0xFFFFFF
LDI r5, 0x141428
DRAWTEXT r1, r2, r3, r4, r5

; -- Uptime (read TICKS and display) --
LDI r20, BUF
STRO r20, "Uptime"
LDI r1, 24
LDI r2, 144
LDI r3, BUF
LDI r4, 0x8888CC
LDI r5, 0x141428
DRAWTEXT r1, r2, r3, r4, r5

; Read TICKS counter
LDI r20, TICKS
LOAD r15, r20            ; r15 = frame count

; Convert to decimal string
LDI r20, NUMBUF
CALL int_to_str

LDI r1, 140
LDI r2, 144
LDI r3, NUMBUF
LDI r4, 0x44FF44
LDI r5, 0x141428
DRAWTEXT r1, r2, r3, r4, r5

; Append " frames"
LDI r20, BUF
STRO r20, " frames"
LDI r1, 180
LDI r2, 144
LDI r3, BUF
LDI r4, 0x44FF44
LDI r5, 0x141428
DRAWTEXT r1, r2, r3, r4, r5

; =========================================
; Separator line
; =========================================
LDI r1, 24
LDI r2, 168
LDI r3, 208
LDI r4, 1
LDI r5, 0x333366
RECTF r1, r2, r3, r4, r5

; =========================================
; Footer text
; =========================================
LDI r20, BUF
STRO r20, "Geometry OS -- Pixel VM"
LDI r1, 48
LDI r2, 174
LDI r3, BUF
LDI r4, 0x6666AA
LDI r5, 0x141428
DRAWTEXT r1, r2, r3, r4, r5

LDI r20, BUF
STRO r20, "github.com/tdw419/geometry-os"
LDI r1, 36
LDI r2, 188
LDI r3, BUF
LDI r4, 0x4488BB
LDI r5, 0x141428
DRAWTEXT r1, r2, r3, r4, r5

; =========================================
; Color palette bar (bottom decoration)
; =========================================
LDI r1, 0
LDI r2, 218
LDI r3, 256
LDI r4, 38
LDI r5, 0x0A0A1A
RECTF r1, r2, r3, r4, r5

; Draw 15 color squares
LDI r10, 8       ; x start
LDI r11, 226    ; y
LDI r13, 0      ; index
LDI r14, 15     ; count

pal_loop:
    ; Load color from palette table
    LDI r20, PAL
    ADD r20, r13
    LOAD r22, r20

    ; Draw square
    MOV r1, r10
    MOV r2, r11
    LDI r3, 14
    LDI r4, 14
    MOV r5, r22
    RECTF r1, r2, r3, r4, r5

    ; Advance x
    LDI r15, 16
    ADD r10, r15

    ; Next index
    LDI r15, 1
    ADD r13, r15

    ; Check if done
    CMP r13, r14
    BLT r0, pal_loop

; Palette label
LDI r20, BUF
STRO r20, "Color Palette"
LDI r1, 80
LDI r2, 244
LDI r3, BUF
LDI r4, 0x666688
LDI r5, 0x0A0A1A
DRAWTEXT r1, r2, r3, r4, r5

HALT

; =========================================
; int_to_str subroutine
; Converts r15 (u32) to decimal string at r20
; Destroys r15-r19. Returns string null-terminated.
; =========================================
int_to_str:
    PUSH r31
    LDI r16, 0          ; digit count

    ; Check if zero
    JZ r15, its_zero

    ; Extract digits in reverse onto stack
its_loop:
    MOV r18, r15
    LDI r17, 10
    MOD r18, r17        ; r18 = digit (r15 % 10)
    LDI r19, 48
    ADD r18, r19        ; ASCII '0' + digit
    PUSH r18             ; save digit
    LDI r17, 10
    DIV r15, r17        ; r15 = r15 / 10
    LDI r19, 1
    ADD r16, r19        ; digit count++
    JNZ r15, its_loop

    ; Pop digits into buffer in correct order
its_write:
    POP r18
    STORE r20, r18
    LDI r19, 1
    ADD r20, r19
    LDI r19, 1
    SUB r16, r19
    JNZ r16, its_write

    ; Null terminate
    LDI r18, 0
    STORE r20, r18
    POP r31
    RET

its_zero:
    LDI r18, 48         ; '0'
    STORE r20, r18
    LDI r19, 1
    ADD r20, r19
    LDI r18, 0
    STORE r20, r18
    POP r31
    RET
