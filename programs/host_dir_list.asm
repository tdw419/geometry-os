; Phase 137: Directory Listing Demo
; Uses FSLS to list the home directory.
; Blue bar at top = success, red bar = failure.

; List home directory
LDI r1, dirpath
LDI r2, 0x3000      ; buffer for filenames
LDI r3, 1024        ; max bytes
FSLS r1, r2, r3
MOV r10, r0         ; save total bytes written

LDI r11, 0
CMP r10, r11
JZ r0, fail         ; no entries or error

; Blue bar = FSLS succeeded
LDI r1, 0
LDI r2, 0
LDI r3, 256
LDI r4, 8
LDI r5, 0x4444FF
RECTF r1, r2, r3, r4, r5

; Draw a green bar showing how many bytes of listing we got
; (capped at 128 pixels wide)
LDI r1, 0
LDI r2, 12
LDI r3, 128
LDI r4, 4
LDI r5, 0x00FF00
RECTF r1, r2, r3, r4, r5

; White bar showing data content (sample from buffer)
LDI r1, 0
LDI r2, 20
LDI r3, 128
LDI r4, 2
LDI r5, 0xFFFFFF
RECTF r1, r2, r3, r4, r5

HALT

fail:
; Red bar = error (sandbox violation or FSLS failure)
LDI r1, 0
LDI r2, 0
LDI r3, 256
LDI r4, 8
LDI r5, 0xFF0000
RECTF r1, r2, r3, r4, r5
HALT

dirpath:
.asciz "~"
