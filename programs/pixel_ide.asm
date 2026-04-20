; pixel_ide.asm -- Windowed IDE Demo (Phase 70)
;
; Demonstrates a simple pixel IDE with three windowed panes:
; 1. Editor pane (top-left): Shows source code text
; 2. Build pane (top-right): Shows assembly status
; 3. Output pane (bottom): Shows program execution result
;
; The IDE writes a simple program to the canvas, assembles it,
; and shows the result in the output window.
; Uses WINSYS to create three windows, then draws into them.

; ---- Constants ----
LDI r20, 0x9000       ; title string storage
LDI r7, 1
LDI r8, 0

; ---- Write title strings to RAM ----

; "Editor" at 0x9000
LDI r1, 0x9000
LDI r2, 69; STORE r1, r2; ADD r1, r7
LDI r2, 100; STORE r1, r2; ADD r1, r7
LDI r2, 105; STORE r1, r2; ADD r1, r7
LDI r2, 116; STORE r1, r2; ADD r1, r7
LDI r2, 111; STORE r1, r2; ADD r1, r7
LDI r2, 114; STORE r1, r2; ADD r1, r7
LDI r2, 0; STORE r1, r2

; "Build" at 0x9010
LDI r1, 0x9010
LDI r2, 66; STORE r1, r2; ADD r1, r7
LDI r2, 117; STORE r1, r2; ADD r1, r7
LDI r2, 105; STORE r1, r2; ADD r1, r7
LDI r2, 108; STORE r1, r2; ADD r1, r7
LDI r2, 100; STORE r1, r2; ADD r1, r7
LDI r2, 0; STORE r1, r2

; "Output" at 0x9020
LDI r1, 0x9020
LDI r2, 79; STORE r1, r2; ADD r1, r7
LDI r2, 117; STORE r1, r2; ADD r1, r7
LDI r2, 116; STORE r1, r2; ADD r1, r7
LDI r2, 112; STORE r1, r2; ADD r1, r7
LDI r2, 117; STORE r1, r2; ADD r1, r7
LDI r2, 116; STORE r1, r2; ADD r1, r7
LDI r2, 0; STORE r1, r2

; ---- Write source code text to RAM for display ----
; "LDI r1, 42" at 0x9030
LDI r1, 0x9030
LDI r2, 76; STORE r1, r2; ADD r1, r7
LDI r2, 68; STORE r1, r2; ADD r1, r7
LDI r2, 73; STORE r1, r2; ADD r1, r7
LDI r2, 32; STORE r1, r2; ADD r1, r7
LDI r2, 114; STORE r1, r2; ADD r1, r7
LDI r2, 49; STORE r1, r2; ADD r1, r7
LDI r2, 44; STORE r1, r2; ADD r1, r7
LDI r2, 32; STORE r1, r2; ADD r1, r7
LDI r2, 52; STORE r1, r2; ADD r1, r7
LDI r2, 50; STORE r1, r2; ADD r1, r7
LDI r2, 10; STORE r1, r2; ADD r1, r7
LDI r2, 72; STORE r1, r2; ADD r1, r7
LDI r2, 65; STORE r1, r2; ADD r1, r7
LDI r2, 76; STORE r1, r2; ADD r1, r7
LDI r2, 84; STORE r1, r2; ADD r1, r7
LDI r2, 0; STORE r1, r2

; "OK: 3 words" at 0x9050
LDI r1, 0x9050
LDI r2, 79; STORE r1, r2; ADD r1, r7
LDI r2, 75; STORE r1, r2; ADD r1, r7
LDI r2, 58; STORE r1, r2; ADD r1, r7
LDI r2, 32; STORE r1, r2; ADD r1, r7
LDI r2, 51; STORE r1, r2; ADD r1, r7
LDI r2, 32; STORE r1, r2; ADD r1, r7
LDI r2, 119; STORE r1, r2; ADD r1, r7
LDI r2, 111; STORE r1, r2; ADD r1, r7
LDI r2, 114; STORE r1, r2; ADD r1, r7
LDI r2, 100; STORE r1, r2; ADD r1, r7
LDI r2, 115; STORE r1, r2; ADD r1, r7
LDI r2, 0; STORE r1, r2

; ---- Create three windows ----

; Window 1: Editor (top-left) -- x=8, y=8, w=118, h=100
LDI r1, 8
LDI r2, 8
LDI r3, 118
LDI r4, 100
LDI r5, 0x9000
LDI r6, 0
WINSYS r6
; r0 = window_id for editor (should be 1)

; Save editor window id
MOV r10, r0

; Window 2: Build (top-right) -- x=130, y=8, w=118, h=100
LDI r1, 130
LDI r2, 8
LDI r3, 118
LDI r4, 100
LDI r5, 0x9010
LDI r6, 0
WINSYS r6
MOV r11, r0

; Window 3: Output (bottom) -- x=8, y=116, w=240, h=132
LDI r1, 8
LDI r2, 116
LDI r3, 240
LDI r4, 132
LDI r5, 0x9020
LDI r6, 0
WINSYS r6
MOV r12, r0

; ---- Draw editor window content ----
; Fill editor with dark blue background
LDI r13, 0x000022
LDI r14, 0

; Draw source code text into editor window
; Row 1: "LDI r1, 42" -- draw as green pixels
LDI r15, 0x00CC00
LDI r16, 2
LDI r17, 4

; Draw 'L' in editor
LDI r14, 76
WPIXEL r10, r16, r17, r15
ADD r16, r7
LDI r14, 68
WPIXEL r10, r16, r17, r15
ADD r16, r7
LDI r14, 73
WPIXEL r10, r16, r17, r15
ADD r16, r7
LDI r14, 32
WPIXEL r10, r16, r17, r14
ADD r16, r7
LDI r14, 114
WPIXEL r10, r16, r17, r15
ADD r16, r7
LDI r14, 49
WPIXEL r10, r16, r17, r15
ADD r16, r7
LDI r14, 44
WPIXEL r10, r16, r17, r14
ADD r16, r7
LDI r14, 32
WPIXEL r10, r16, r17, r14
ADD r16, r7
LDI r14, 52
WPIXEL r10, r16, r17, r15
ADD r16, r7
LDI r14, 50
WPIXEL r10, r16, r17, r15

; Row 2: "HALT" in editor
LDI r16, 2
LDI r17, 6
LDI r14, 72
WPIXEL r10, r16, r17, r15
ADD r16, r7
LDI r14, 65
WPIXEL r10, r16, r17, r15
ADD r16, r7
LDI r14, 76
WPIXEL r10, r16, r17, r15
ADD r16, r7
LDI r14, 84
WPIXEL r10, r16, r17, r15

; ---- Draw build window content ----
; Green "OK" text
LDI r15, 0x00FF00
LDI r16, 2
LDI r17, 4
LDI r14, 79
WPIXEL r11, r16, r17, r15
ADD r16, r7
LDI r14, 75
WPIXEL r11, r16, r17, r15

; "3 words" in yellow below
LDI r15, 0xFFFF00
LDI r16, 2
LDI r17, 8
LDI r14, 51
WPIXEL r11, r16, r17, r15
ADD r16, r7
LDI r14, 32
WPIXEL r11, r16, r17, r14
ADD r16, r7
LDI r14, 119
WPIXEL r11, r16, r17, r15
ADD r16, r7
LDI r14, 111
WPIXEL r11, r16, r17, r15
ADD r16, r7
LDI r14, 114
WPIXEL r11, r16, r17, r15
ADD r16, r7
LDI r14, 100
WPIXEL r11, r16, r17, r15
ADD r16, r7
LDI r14, 115
WPIXEL r11, r16, r17, r15

; ---- Draw output window content ----
; Draw a colored bar to show "execution result"
LDI r15, 0x4444FF
LDI r16, 0
LDI r17, 10
LDI r18, 240
LDI r19, 0

output_bar:
  CMP r16, r18
  BGE r0, output_done
  WPIXEL r12, r16, r17, r15
  ADD r16, r7
  JMP output_bar

output_done:

; Also draw a smaller green result bar
LDI r15, 0x00FF00
LDI r16, 80
LDI r17, 20
LDI r19, 0

output_green:
  CMP r16, r19
  BGE r0, output_gdone
  ; Check: r19 = 160
  LDI r20, 160
  CMP r16, r20
  BGE r0, output_gdone
  WPIXEL r12, r16, r17, r15
  ADD r16, r7
  JMP output_green

output_gdone:

; ---- Assemble the actual source code and run it ----
; Write "LDI r1, 42\nHALT\n" to 0x0800
LDI r1, 0x0800

LDI r2, 76
CALL wr
LDI r2, 68
CALL wr
LDI r2, 73
CALL wr
LDI r2, 32
CALL wr
LDI r2, 114
CALL wr
LDI r2, 49
CALL wr
LDI r2, 44
CALL wr
LDI r2, 32
CALL wr
LDI r2, 52
CALL wr
LDI r2, 50
CALL wr
LDI r2, 10
CALL wr
LDI r2, 72
CALL wr
LDI r2, 65
CALL wr
LDI r2, 76
CALL wr
LDI r2, 84
CALL wr
LDI r2, 10
CALL wr
LDI r2, 0
CALL wr

; Assemble it
LDI r5, 0x0800
LDI r6, 0x1000
ASM r5, r6

; Run it
JMP 0x1000

; ---- Subroutine: write char to RAM ----
wr:
    STORE r1, r2
    ADD r1, r7
    RET
