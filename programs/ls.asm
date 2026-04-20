; ls.asm -- List VFS directory contents
;
; Uses the LS syscall to enumerate files from the VFS base directory.
; Displays filenames as colored text on screen using DRAWTEXT.
; Outputs entry count at RAM[0x7800].
;
; Memory layout:
;   0x5000-0x50FF - text buffer
;   0x5100-0x5FFF - LS buffer (3840 words)
;   0x7800 - entry count

    ; --- Fill screen dark ---
    LDI r1, 0x000022
    FILL r1

    ; --- Draw title bar ---
    LDI r1, 0x000066
    LDI r15, 0
    LDI r16, 0
    LDI r17, 256
    LDI r18, 12
    RECTF r15, r16, r17, r18, r1

    ; --- Title text ---
    LDI r20, 0x5000
    STRO r20, "Directory:"
    LDI r10, 4
    LDI r11, 2
    LDI r12, 0x5000
    LDI r13, 0xFFFFFF
    LDI r14, 0x000066
    DRAWTEXT r10, r11, r12, r13, r14

    ; --- Call LS to list files ---
    LDI r5, 0x5100
    LS r5                   ; r0 = entry count

    ; Store entry count at 0x7800
    LDI r6, 0x7800
    STORE r6, r0
    MOV r4, r0              ; r4 = count (safe from CMP)

    ; --- Display filenames from LS buffer ---
    ; LS writes null-terminated filenames sequentially, extra null at end
    LDI r11, 16             ; y start
    LDI r12, 0x5100         ; buffer pointer
    LDI r7, 1               ; increment
    LDI r9, 14              ; line spacing
    LDI r8, 232             ; y limit

next_file:
    CMP r11, r8
    BGE r0, footer

    ; Read first char
    LOAD r2, r12
    LDI r3, 0
    CMP r2, r3
    JZ r0, footer

    ; Draw this filename
    LDI r10, 4
    LDI r13, 0xCCCCCC
    LDI r14, 0x000022
    DRAWTEXT r10, r11, r12, r13, r14

    ; Advance pointer past null terminator
skip_chars:
    LOAD r2, r12
    LDI r3, 0
    CMP r2, r3
    JZ r0, skip_done
    ADD r12, r7
    JMP skip_chars

skip_done:
    ADD r12, r7             ; skip the null
    ADD r11, r9
    JMP next_file

footer:
    ; --- Footer ---
    LDI r1, 0x004400
    LDI r15, 0
    LDI r16, 244
    LDI r17, 256
    LDI r18, 12
    RECTF r15, r16, r17, r18, r1

    ; Build count string at 0x5000
    LDI r20, 0x5000
    ADDI r4, 48             ; ASCII digit for count
    STORE r20, r4
    LDI r7, 1
    ADD r20, r7
    STRO r20, " file(s)"

    LDI r10, 4
    LDI r11, 246
    LDI r12, 0x5000
    LDI r13, 0xFFFFFF
    LDI r14, 0x004400
    DRAWTEXT r10, r11, r12, r13, r14

    HALT
