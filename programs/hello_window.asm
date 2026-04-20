; hello_window.asm -- Token-to-Pixel GUI "Hello World"
;
; Proves the full loop: RECTF draws pixels, HITSET makes them semantic,
; HITQ reads the cursor. This is the minimum-viable GUI in Geometry OS.
;
; Layout (256x256 screen):
;   +-----------------------------+
;   | #555555 title bar (30px)    |
;   | "Hello"                     |
;   +-----------------------------+
;   |                             |
;   |   [ #2266FF OK button ]     |
;   |                             |
;   +-----------------------------+
;
; After HALT:
;   - vm.screen has the rendered UI
;   - vm.hit_regions has 1 entry (the OK button) with id=1
;   - A host that calls vm.push_mouse(x, y) and re-runs HITQ gets the id back

; --- background fill (dark grey) ---
LDI r0, 0x202020
FILL r0

; --- title bar: RECTF at (0,0) 256x30, colour #555555 ---
LDI r1, 0
LDI r2, 0
LDI r3, 256
LDI r4, 30
LDI r5, 0x555555
RECTF r1, r2, r3, r4, r5

; --- title text "Hello" at (100, 10) ---
LDI r9, 0x2000
LDI r0, 72         ; H
STORE r9, r0
LDI r9, 0x2001
LDI r0, 101        ; e
STORE r9, r0
LDI r9, 0x2002
LDI r0, 108        ; l
STORE r9, r0
LDI r9, 0x2003
LDI r0, 108        ; l
STORE r9, r0
LDI r9, 0x2004
LDI r0, 111        ; o
STORE r9, r0
LDI r9, 0x2005
LDI r0, 0          ; NUL
STORE r9, r0
LDI r1, 100
LDI r2, 10
LDI r3, 0x2000
TEXT r1, r2, r3

; --- OK button body: RECTF at (88, 110) 80x28, colour #2266FF ---
LDI r1, 88
LDI r2, 110
LDI r3, 80
LDI r4, 28
LDI r5, 0x2266FF
RECTF r1, r2, r3, r4, r5

; --- register the same rect as hit-region id=1 ---
; r1..r4 still hold x,y,w,h from the RECTF above
HITSET r1, r2, r3, r4, 1

; --- button label "OK" at (120, 118) ---
LDI r9, 0x2010
LDI r0, 79         ; O
STORE r9, r0
LDI r9, 0x2011
LDI r0, 75         ; K
STORE r9, r0
LDI r9, 0x2012
LDI r0, 0
STORE r9, r0
LDI r1, 120
LDI r2, 118
LDI r3, 0x2010
TEXT r1, r2, r3

; --- demo: query cursor, put matched id in r10 ---
; (Host sets cursor via vm.push_mouse(x,y); without that, r10 = 0.)
HITQ r10

HALT
