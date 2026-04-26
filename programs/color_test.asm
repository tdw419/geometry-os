; color_test.asm -- Test ANSI SGR color rendering
; Writes text in various ANSI colors to verify color buffer works

LDI r1, 1
LDI r30, 0xFD00

; Fill screen dark
LDI r0, 0x0A0A0A
FILL r0

; Simulate what host_term.asm does:
; Write text directly to BUF (0x4000) and colors to COLOR_BUF (0x7800)

; Row 0: "ANSI Color Test" in default gray
LDI r20, 0x4000
STRO r20, "ANSI Color Test"

; Row 1: Red text
LDI r20, 0x4050
STRO r20, "Red text here"
LDI r6, 0x4050
LDI r7, 0x7850
LDI r8, 13
LDI r9, 0xCD0000
call_fill_red:
    STORE r7, r9
    ADD r6, r1
    ADD r7, r1
    SUB r8, r1
    CMPI r8, 0
    JNZ r0, call_fill_red

; Row 2: Green text
LDI r20, 0x40A0
STRO r20, "Green text here"
LDI r6, 0x40A0
LDI r7, 0x78A0
LDI r8, 15
LDI r9, 0x00CD00
call_fill_green:
    STORE r7, r9
    ADD r6, r1
    ADD r7, r1
    SUB r8, r1
    CMPI r8, 0
    JNZ r0, call_fill_green

; Row 3: Blue text
LDI r20, 0x40F0
STRO r20, "Blue text here"
LDI r6, 0x40F0
LDI r7, 0x78F0
LDI r8, 14
LDI r9, 0x0000EE
call_fill_blue:
    STORE r7, r9
    ADD r6, r1
    ADD r7, r1
    SUB r8, r1
    CMPI r8, 0
    JNZ r0, call_fill_blue

; Row 4: Yellow text
LDI r20, 0x4140
STRO r20, "Yellow text here"
LDI r6, 0x4140
LDI r7, 0x7940
LDI r8, 16
LDI r9, 0xCDCD00
call_fill_yellow:
    STORE r7, r9
    ADD r6, r1
    ADD r7, r1
    SUB r8, r1
    CMPI r8, 0
    JNZ r0, call_fill_yellow

; Row 5: Cyan text
LDI r20, 0x4190
STRO r20, "Cyan text here"
LDI r6, 0x4190
LDI r7, 0x7990
LDI r8, 14
LDI r9, 0x00CDCD
call_fill_cyan:
    STORE r7, r9
    ADD r6, r1
    ADD r7, r1
    SUB r8, r1
    CMPI r8, 0
    JNZ r0, call_fill_cyan

HALT
