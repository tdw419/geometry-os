LDI r1, 10
LDI r2, 10
LDI r3, str
TEXT r1, r2, r3
HALT
str:
.DB "Hello", 0
