LDI r10, 0
LDI r1, 1
LDI r5, 256
y_loop:
LDI r2, 0
x_loop:
MOV r6, r10
SHL r6, r1
PSET r2, r10, r6
ADD r2, r1
CMP r2, r5
BLT r0, x_loop
ADD r10, r1
CMP r10, r5
BLT r0, y_loop
HALT
