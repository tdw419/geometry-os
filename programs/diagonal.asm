; DIAGONAL_LINE -- Draw a diagonal line from (0,0) to (255,255)
; Uses PSET (register-based) in a loop
; Test: pixels at (i,i) for i in 0..255 should be green (0x00FF00)

LDI r0, 0            ; r0 = i (x and y counter)
LDI r1, 1            ; r1 = increment
LDI r2, 0x00FF00     ; r2 = green color (for PSET color register)
LDI r3, 256          ; r3 = limit
; PSET xr, yr, cr: set pixel at (regs[xr], regs[yr]) to regs[cr]
; We want PSET r0, r0, r2 -> pixel at (r0, r0) with color r2

loop:
PSET r0, r0, r2      ; pixel at (i, i) with green
ADD r0, r1           ; i++
; Check i == 256
LDI r4, 0
ADD r4, r0
SUB r4, r3
JZ r4, done
JMP loop

done:
HALT
