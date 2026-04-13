; cat.asm -- Phase 25: Filesystem test program
; Reads a file named "hello.txt" from the VFS and displays it on screen.
;
; Usage: The file must exist in .geometry_os/fs/hello.txt
;
; Layout:
;   r1 = path address (0x1000)
;   r2 = open mode
;   r3 = fd
;   r4 = read buffer address (0x1100)
;   r5 = read length (32 bytes at a time)
;   r6 = bytes read
;   r7 = screen x cursor
;   r8 = screen y cursor
;   r9 = TEXT address temp
;   r10 = color (white)
;   r11 = check for error
;   r20 = 0 (zero constant)
;   r21 = 0xFFFFFFFF (error sentinel)

#define COLOR_WHITE 0xFFFFFF
#define BUF_ADDR 0x1100
#define PATH_ADDR 0x1000
#define READ_LEN 32

start:
    LDI r20, 0
    LDI r21, 0xFFFFFFFF
    LDI r10, COLOR_WHITE
    LDI r7, 2           ; x = 2
    LDI r8, 2           ; y = 2

    ; Store filename "hello.txt" at PATH_ADDR
    LDI r1, PATH_ADDR
    LDI r9, 'h'
    STORE r1, r9
    LDI r9, 'e'
    LDI r1, PATH_ADDR + 1
    STORE r1, r9
    LDI r9, 'l'
    LDI r1, PATH_ADDR + 2
    STORE r1, r9
    LDI r9, 'l'
    LDI r1, PATH_ADDR + 3
    STORE r1, r9
    LDI r9, 'o'
    LDI r1, PATH_ADDR + 4
    STORE r1, r9
    LDI r9, '.'
    LDI r1, PATH_ADDR + 5
    STORE r1, r9
    LDI r9, 't'
    LDI r1, PATH_ADDR + 6
    STORE r1, r9
    LDI r9, 'x'
    LDI r1, PATH_ADDR + 7
    STORE r1, r9
    LDI r9, 't'
    LDI r1, PATH_ADDR + 8
    STORE r1, r9
    LDI r9, 0           ; null terminator
    LDI r1, PATH_ADDR + 9
    STORE r1, r9

    ; OPEN path, mode(read=0)
    LDI r1, PATH_ADDR
    LDI r2, 0           ; read mode
    OPEN r1, r2
    ; r0 = fd or error
    MOV r3, r0           ; save fd
    CMP r3, r21
    BLT r3, open_ok      ; if fd < 0xFFFFFFFF, success (actually check != -1)

    ; Error: display "ERR" on screen
    LDI r9, 'E'
    LDI r1, 0x1100
    STORE r1, r9
    LDI r9, 'R'
    LDI r1, 0x1101
    STORE r1, r9
    LDI r9, 'R'
    LDI r1, 0x1102
    STORE r1, r9
    LDI r9, 0
    LDI r1, 0x1103
    STORE r1, r9
    TEXT r7, r8, 0x1100
    HALT

open_ok:
    ; Read loop: read READ_LEN bytes at a time and display
read_loop:
    LDI r4, BUF_ADDR
    LDI r5, READ_LEN
    READ r3, r4, r5      ; r0 = bytes read
    MOV r6, r0           ; save count

    ; Check for error or EOF
    CMP r6, r21
    BLT r6, display_data  ; if count != error, display
    JZ r6, done           ; if count == 0, EOF

display_data:
    ; Display the read data as text on screen
    ; Use TEXT opcode to render the buffer
    LDI r9, BUF_ADDR
    TEXT r7, r8, r9

    ; Move y cursor down
    LDI r9, 10
    ADD r8, r9

    ; Continue reading
    JZ r6, done          ; if 0 bytes, we're done
    JMP read_loop

done:
    ; Close the file
    CLOSE r3
    HALT
