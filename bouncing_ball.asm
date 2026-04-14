LDI r1, 128      ; x position
LDI r2, 128      ; y position
LDI r3, 2        ; dx velocity
LDI r4, 2        ; dy velocity
LDI r5, 256      ; screen width/height
LDI r7, 10       ; ball radius
LDI r8, 0x00FF00 ; green color
LDI r9, 0x000000 ; black for clear
loop:
FILL r9        ; clear screen
CIRCLE r1, r2, r7, r8  ; draw ball
ADD r1, r3     ; x += dx
ADD r2, r4     ; y += dy
; check x upper boundary
CMP r1, r5
BLT r0, checkxlow
NEG r3
checkxlow:
; check x lower boundary
LDI r10, 0
CMP r1, r10
BGE r0, checkyhigh
NEG r3
checkyhigh:
; check y upper boundary
CMP r2, r5
BLT r0, checkylow
NEG r4
checkylow:
; check y lower boundary
LDI r10, 0
CMP r2, r10
BGE r0, donecheck
NEG r4
donecheck:
FRAME          ; update display
JMP loop
