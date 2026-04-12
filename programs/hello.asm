; Hello World -- render text on the VM screen using TEXT opcode
; Writes "Hello, World!" to RAM at 0x2000, then calls TEXT to draw it.

; --- build the string at 0x2000 ---
LDI r9, 0x2000
LDI r0, 72
STORE r9, r0
LDI r9, 0x2001
LDI r0, 101
STORE r9, r0
LDI r9, 0x2002
LDI r0, 108
STORE r9, r0
LDI r9, 0x2003
LDI r0, 108
STORE r9, r0
LDI r9, 0x2004
LDI r0, 111
STORE r9, r0
LDI r9, 0x2005
LDI r0, 44
STORE r9, r0
LDI r9, 0x2006
LDI r0, 32
STORE r9, r0
LDI r9, 0x2007
LDI r0, 87
STORE r9, r0
LDI r9, 0x2008
LDI r0, 111
STORE r9, r0
LDI r9, 0x2009
LDI r0, 114
STORE r9, r0
LDI r9, 0x200A
LDI r0, 108
STORE r9, r0
LDI r9, 0x200B
LDI r0, 100
STORE r9, r0
LDI r9, 0x200C
LDI r0, 33
STORE r9, r0
LDI r9, 0x200D
LDI r0, 0
STORE r9, r0
; --- draw it ---
LDI r1, 90
LDI r2, 120
LDI r3, 0x2000
TEXT r1, r2, r3
HALT
