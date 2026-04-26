; Phase 137: Host Filesystem Read Demo
; Reads the first 4 bytes of a test file and draws green if "GEOS" found.
;
; Setup: Create ~/.geos_host_file_test.txt with content "GEOS"
; This program opens it for reading, reads 4 bytes, and checks for 'G' (0x47)

; FSOPEN path_reg, mode_reg  -- open for reading
LDI r1, filepath
LDI r2, 0           ; read mode
FSOPEN r1, r2

; r0 has handle. Error codes are >= 0xFFFFFF00.
; Simple check: r0 < 0x80000000 means success (handle is small)
LDI r11, 0x80000000
CMP r0, r11
BGE r0, fail        ; if handle >= 0x80000000, it's an error
MOV r10, r0         ; save handle

; FSREAD handle, buf, len
LDI r2, 0x3000      ; buffer
LDI r3, 4           ; read 4 bytes
MOV r1, r10
FSREAD r1, r2, r3
MOV r12, r0         ; bytes read

; FSCLOSE handle
MOV r1, r10
FSCLOSE r1

; Check if we read at least 1 byte
LDI r11, 0x80000000
CMP r12, r11
BGE r0, fail        ; bytes_read >= 0x80000000 means error

; Check first byte is 'G' (0x47)
LDI r13, 0x3000
LOAD r14, r13       ; r14 = first byte
LDI r15, 0x47       ; 'G'
CMP r14, r15
JZ r0, success
JMP fail

success:
LDI r1, 96
LDI r2, 96
LDI r3, 64
LDI r4, 64
LDI r5, 0x00FF00
RECTF r1, r2, r3, r4, r5
HALT

fail:
LDI r1, 96
LDI r2, 96
LDI r3, 64
LDI r4, 64
LDI r5, 0xFF0000
RECTF r1, r2, r3, r4, r5
HALT

filepath:
.asciz "~/.geos_host_file_test.txt"
