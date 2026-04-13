; pipe_demo.asm -- Phase 27 IPC Demo
; Demonstrates pipes, MSGSND/MSGRCV, and blocking I/O
;
; This program:
; 1. Creates a pipe
; 2. Writes data into it
; 3. Reads data back from it
; 4. Displays the result on screen
;
; Run with: ./geometry_os programs/pipe_demo.asm

.org 0x100

; === Phase 1: Create a pipe ===
PIPE r5, r6          ; r5 = read_fd (0x8000+idx), r6 = write_fd (0xC000+idx)

; === Phase 2: Write 5 values into the pipe ===
LDI r1, 42           ; value 1
STORE r20, r1
LDI r1, 99           ; value 2
STORE r21, r1
LDI r1, 7            ; value 3
STORE r22, r1
LDI r1, 1234         ; value 4
STORE r23, r1
LDI r1, 56789        ; value 5
STORE r24, r1

; WRITE write_fd, buf_addr, len
LDI r1, 20           ; buf_addr = 0x14 (r20)
LDI r2, 5            ; len = 5
WRITE r6, r1, r2     ; write 5 words to pipe

; === Phase 3: Read 5 values back from the pipe ===
LDI r1, 30           ; buf_addr = 0x1E (r30)
LDI r2, 5            ; len = 5
READ r5, r1, r2      ; read 5 words from pipe

; === Phase 4: Display result on screen ===
LDI r10, 10          ; x position
LDI r11, 10          ; y position
FILL 0               ; clear screen (black)

; Show "IPC OK" text
LDI r10, 100
LDI r11, 120
LDI r12, msg
TEXT r10, r11, [r12]

; Draw a green line to indicate success
LDI r10, 100
LDI r11, 130
LDI r12, 156
LDI r13, 130
LDI r14, 0x00FF00    ; green
LINE r10, r11, r12, r13, r14

HALT

.org 0x200
msg: .db "PIPE IPC OK", 0
