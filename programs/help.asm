; help.asm -- Help Viewer for Geometry OS
;
; Shows keyboard shortcuts, opcode reference, and shell commands.
; Reads help text from RAM (pre-loaded). Scrollable with arrow keys.
;
; Proves: TEXT, DRAWTEXT, RECTF, STRO, IKEY, FRAME, 
;         scrolling, keyboard navigation.
;
; RAM Layout:
;   0x6000-0x60FF  Text buffers
;   0x6100         Scroll offset
;   0x6101         Max scroll
;
; No colons in comments (assembler pitfall)

#define TICKS   0xFFE
#define BUF     0x6000
#define SCROLL  0x6100
#define MAXSCRL 0x6101

; Init
LDI r30, 0xFD00
LDI r1, 1

LDI r20, SCROLL
LDI r21, 0
STORE r20, r21
LDI r20, MAXSCRL
LDI r21, 12
STORE r20, r21

; =========================================
; Main Loop
; =========================================
main_loop:
    IKEY r10
    JZ r10, no_input

    ; Up arrow (A=65) = scroll up
    LDI r11, 65
    CMP r10, r11
    JZ r0, scroll_up

    ; Down arrow (B=66) = scroll down
    LDI r11, 66
    CMP r10, r11
    JZ r0, scroll_down

    ; ESC (27) = quit
    LDI r11, 27
    CMP r10, r11
    JZ r0, help_quit

    JMP no_input

scroll_up:
    LDI r20, SCROLL
    LOAD r15, r20
    JZ r15, no_input
    LDI r11, 1
    SUB r15, r11
    STORE r20, r15
    JMP no_input

scroll_down:
    LDI r20, SCROLL
    LOAD r15, r20
    LDI r20, MAXSCRL
    LOAD r16, r20
    CMP r15, r16
    BGE r0, no_input
    LDI r20, SCROLL
    LOAD r15, r20
    LDI r11, 1
    ADD r15, r11
    STORE r20, r15
    JMP no_input

no_input:
    CALL render_help
    FRAME
    JMP main_loop

help_quit:
    HALT

; =========================================
; render_help -- draw help screen
; =========================================
render_help:
    PUSH r31

    ; Background
    LDI r1, 0x0D1B2A
    FILL r1

    ; Title bar
    LDI r1, 0
    LDI r2, 0
    LDI r3, 256
    LDI r4, 24
    LDI r5, 0x1B3A5C
    RECTF r1, r2, r3, r4, r5

    LDI r20, BUF
    STRO r20, "Geometry OS Help"
    LDI r1, 56
    LDI r2, 6
    LDI r3, BUF
    LDI r4, 0xFFFFFF
    LDI r5, 0x1B3A5C
    DRAWTEXT r1, r2, r3, r4, r5

    ; Content area background
    LDI r1, 8
    LDI r2, 28
    LDI r3, 240
    LDI r4, 216
    LDI r5, 0x101828
    RECTF r1, r2, r3, r4, r5

    ; Load scroll offset
    LDI r20, SCROLL
    LOAD r12, r20

    ; Draw help lines based on scroll offset
    ; Each line is 16px apart, starting at y=34
    ; We draw up to 13 visible lines

    ; Line 0 (relative to scroll)
    LDI r13, 0            ; line index
    LDI r14, 34           ; y position

help_line_loop:
    ; Check if we've drawn enough lines (13 visible)
    LDI r15, 13
    CMP r13, r15
    BGE r0, help_lines_done

    ; Compute which help entry to show: scroll_offset + line_index
    MOV r15, r12
    ADD r15, r13

    ; Dispatch on help entry index
    JZ r15, hl_keyboard
    LDI r11, 1
    CMP r15, r11
    JZ r0, hl_keys2
    LDI r11, 2
    CMP r15, r11
    JZ r0, hl_opcodes1
    LDI r11, 3
    CMP r15, r11
    JZ r0, hl_opcodes2
    LDI r11, 4
    CMP r15, r11
    JZ r0, hl_opcodes3
    LDI r11, 5
    CMP r15, r11
    JZ r0, hl_shell
    LDI r11, 6
    CMP r15, r11
    JZ r0, hl_memory
    LDI r11, 7
    CMP r15, r11
    JZ r0, hl_tips1
    LDI r11, 8
    CMP r15, r11
    JZ r0, hl_tips2
    LDI r11, 9
    CMP r15, r11
    JZ r0, hl_tips3
    LDI r11, 10
    CMP r15, r11
    JZ r0, hl_tips4
    LDI r11, 11
    CMP r15, r11
    JZ r0, hl_tips5
    LDI r11, 12
    CMP r15, r11
    JZ r0, hl_tips6
    JMP hl_blank

hl_keyboard:
    LDI r20, BUF
    STRO r20, "Keyboard Shortcuts"
    LDI r4, 0x8888FF
    JMP hl_draw

hl_keys2:
    LDI r20, BUF
    STRO r20, "F5=Run F8=Asm F6=Step ESC=Back"
    LDI r4, 0xAAAACC
    JMP hl_draw

hl_opcodes1:
    LDI r20, BUF
    STRO r20, "Opcodes (113 total)"
    LDI r4, 0x8888FF
    JMP hl_draw

hl_opcodes2:
    LDI r20, BUF
    STRO r20, "LDI LOAD STORE MOV ADD SUB MUL"
    LDI r4, 0xAAAACC
    JMP hl_draw

hl_opcodes3:
    LDI r20, BUF
    STRO r20, "DIV AND OR XOR SHL SHR MOD NEG"
    LDI r4, 0xAAAACC
    JMP hl_draw

hl_shell:
    LDI r20, BUF
    STRO r20, "Shell: help ls load run edit regs"
    LDI r4, 0xAAAACC
    JMP hl_draw

hl_memory:
    LDI r20, BUF
    STRO r20, "64K RAM  0x000-0x3FF=Source"
    LDI r4, 0xAAAACC
    JMP hl_draw

hl_tips1:
    LDI r20, BUF
    STRO r20, "Tips"
    LDI r4, 0x8888FF
    JMP hl_draw

hl_tips2:
    LDI r20, BUF
    STRO r20, "r0 reserved for CMP results"
    LDI r4, 0xAAAACC
    JMP hl_draw

hl_tips3:
    LDI r20, BUF
    STRO r20, "r30=SP r31=LR (CALL/RET)"
    LDI r4, 0xAAAACC
    JMP hl_draw

hl_tips4:
    LDI r20, BUF
    STRO r20, "No colons in .asm comments"
    LDI r4, 0xFF8888
    JMP hl_draw

hl_tips5:
    LDI r20, BUF
    STRO r20, "FRAME for animation loops"
    LDI r4, 0xAAAACC
    JMP hl_draw

hl_tips6:
    LDI r20, BUF
    STRO r20, "IKEY reads keyboard each frame"
    LDI r4, 0xAAAACC
    JMP hl_draw

hl_blank:
    LDI r20, BUF
    STRO r20, ""
    LDI r4, 0xAAAACC

hl_draw:
    LDI r1, 16
    MOV r2, r14
    LDI r3, BUF
    LDI r5, 0x101828
    DRAWTEXT r1, r2, r3, r4, r5

    ; Next line
    LDI r15, 1
    ADD r13, r15
    LDI r15, 16
    ADD r14, r15
    JMP help_line_loop

help_lines_done:
    ; Footer
    LDI r1, 0
    LDI r2, 244
    LDI r3, 256
    LDI r4, 12
    LDI r5, 0x0A0A1A
    RECTF r1, r2, r3, r4, r5

    LDI r20, BUF
    STRO r20, "Up/Down: Scroll  ESC: Back"
    LDI r1, 52
    LDI r2, 246
    LDI r3, BUF
    LDI r4, 0x6666AA
    LDI r5, 0x0A0A1A
    DRAWTEXT r1, r2, r3, r4, r5

    POP r31
    RET
