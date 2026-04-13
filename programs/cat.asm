; cat.asm -- Read a file and display its contents on screen
; Opens "hello.txt" from the VFS and renders it using TEXT opcode.
; Uses OPEN, READ, CLOSE opcodes from Phase 25 (Filesystem).
;
; Before running: create a file in .geometry_os/fs/hello.txt
;
; Usage: Load and run in Geometry OS. The file contents appear at (2, 10).

; ── Store filename "hello.txt" at 0x1000 (one ASCII byte per RAM cell) ──
LDI r9, 0x1000
LDI r0, 104      ; h
STORE r9, r0
LDI r9, 0x1001
LDI r0, 101      ; e
STORE r9, r0
LDI r9, 0x1002
LDI r0, 108      ; l
STORE r9, r0
LDI r9, 0x1003
LDI r0, 108      ; l
STORE r9, r0
LDI r9, 0x1004
LDI r0, 111      ; o
STORE r9, r0
LDI r9, 0x1005
LDI r0, 46       ; .
STORE r9, r0
LDI r9, 0x1006
LDI r0, 116      ; t
STORE r9, r0
LDI r9, 0x1007
LDI r0, 120      ; x
STORE r9, r0
LDI r9, 0x1008
LDI r0, 116      ; t
STORE r9, r0
LDI r9, 0x1009
LDI r0, 0        ; null terminator
STORE r9, r0

; ── Open file for reading (mode 0 = read) ──
LDI r1, 0x1000   ; r1 = pointer to filename
LDI r2, 0        ; r2 = mode (read)
OPEN r1, r2       ; r0 = file descriptor (or 0xFFFFFFFF on error)

; ── Check for open error ──
LDI r3, 0xFFFFFFFF
CMP r0, r3
JZ halt          ; if fd == error, skip to halt

; ── Save fd, then read up to 200 bytes ──
MOV r5, r0       ; r5 = saved fd
LDI r3, 0x2000   ; r3 = read buffer address
LDI r4, 200      ; r4 = max bytes to read
READ r5, r3, r4  ; r0 = bytes actually read

; ── Null-terminate the buffer ──
LDI r6, 0x2000   ; buffer start
ADD r0, r6       ; r0 = buffer start + bytes_read = end address
LDI r7, 0
STORE r0, r7     ; write null terminator

; ── Display file contents using TEXT opcode ──
LDI r1, 2        ; x position
LDI r2, 10       ; y position
LDI r3, 0x2000   ; pointer to string buffer
TEXT r1, r2, r3

; ── Close the file ──
CLOSE r5

halt:
HALT
