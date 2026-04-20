; hexdump.asm -- Hex viewer for RAM contents
;
; Reads bytes from a RAM buffer and displays them as hex values
; on the screen using DRAWTEXT. Demonstrates byte-to-hex conversion.
;
; Input: data at 0x6000 (pre-loaded sample text)
; Output: hex display on screen, byte count at RAM[0x7800]
;
; Format: 8 bytes per row, hex pairs separated by spaces
; e.g. "48 65 6C 6C 6F 20 57 6F" for "Hello Wo"

    ; --- Fill screen ---
    LDI r1, 0x000011
    FILL r1

    ; --- Pre-load sample text at 0x6000 ---
    LDI r20, 0x6000
    STRO r20, "Hello, Geometry OS!"

    ; --- Count bytes ---
    LDI r10, 0x6000
    LDI r11, 0             ; byte count

count_loop:
    LOAD r2, r10
    LDI r3, 0
    CMP r2, r3
    JZ r0, count_done
    ADDI r11, 1
    ADDI r10, 1
    JMP count_loop

count_done:
    ; Store byte count
    LDI r6, 0x7800
    STORE r6, r11

    ; --- Draw header ---
    LDI r1, 0x440044       ; purple header
    LDI r15, 0
    LDI r16, 0
    LDI r17, 256
    LDI r18, 12
    RECTF r15, r16, r17, r18, r1

    LDI r20, 0x5000
    STRO r20, "Hex Dump"
    LDI r10, 4
    LDI r11, 2
    LDI r12, 0x5000
    LDI r13, 0xFFFFFF
    LDI r14, 0x440044
    DRAWTEXT r10, r11, r12, r13, r14

    ; --- Convert each byte to hex and display ---
    ; We display 8 bytes per row as "XX XX XX XX XX XX XX XX"
    ; Each hex pair = 2 hex chars, space between = 3 chars per byte, 8 bytes = 23 chars + null = 24

    LDI r20, 0x6000        ; source pointer
    LDI r11, 16            ; y position
    LDI r9, 14             ; line spacing
    LDI r7, 1
    LDI r8, 8              ; bytes per row

row_loop:
    ; Build hex string at 0x5000
    LDI r21, 0x5000        ; output pointer
    LDI r22, 0             ; byte counter for this row

byte_loop:
    ; Read source byte
    LOAD r2, r20
    LDI r3, 0
    CMP r2, r3
    JZ r0, flush_row       ; end of data

    ; Convert low nibble to hex char
    LDI r4, 0xF
    MOV r5, r2
    AND r5, r4             ; low nibble
    ADDI r5, 48            ; '0' + nibble
    ; Adjust for A-F (10-15)
    LDI r6, 10
    CMP r5, r6
    BLT r0, lo_digit_ok
    ADDI r5, 7             ; skip past ':' to 'A'
lo_digit_ok:

    ; Convert high nibble to hex char
    LDI r4, 4
    MOV r6, r2
    SHR r6, r4             ; high nibble
    ADDI r6, 48
    LDI r3, 10
    CMP r6, r3
    BLT r0, hi_digit_ok
    ADDI r6, 7
hi_digit_ok:

    ; Store high nibble first, then low
    STORE r21, r6          ; high nibble char
    ADD r21, r7
    STORE r21, r5          ; low nibble char
    ADD r21, r7

    ; Add space (except after last byte)
    ADDI r22, 1
    LDI r3, 8
    CMP r22, r3
    BGE r0, no_space
    LDI r3, 32             ; space
    STORE r21, r3
    ADD r21, r7
no_space:

    ADD r20, r7             ; advance source
    JMP byte_loop

flush_row:
    ; Null terminate
    LDI r3, 0
    STORE r21, r3

    ; Draw the row if it has any bytes
    LDI r3, 0
    CMP r22, r3
    JZ r0, dump_done

    LDI r10, 4
    LDI r12, 0x5000
    LDI r13, 0x00CC00     ; green hex
    LDI r14, 0x000011
    DRAWTEXT r10, r11, r12, r13, r14

    ADD r11, r9            ; next row
    JMP row_loop

dump_done:
    ; --- Footer with byte count ---
    LDI r1, 0x440044
    LDI r15, 0
    LDI r16, 244
    LDI r17, 256
    LDI r18, 12
    RECTF r15, r16, r17, r18, r1

    LDI r20, 0x5000
    STRO r20, "Bytes:"
    LDI r10, 4
    LDI r11, 246
    LDI r12, 0x5000
    LDI r13, 0xFFFFFF
    LDI r14, 0x440044
    DRAWTEXT r10, r11, r12, r13, r14

    HALT
