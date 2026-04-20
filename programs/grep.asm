; grep.asm -- Search for a pattern in RAM text
;
; Searches for all occurrences of a byte pattern in a text buffer.
; Pattern is pre-loaded at 0x6100, text at 0x6000.
; Outputs match count at RAM[0x7800].
; Displays matches on screen with highlighted text.
;
; Sample: search for "oo" in "Hello World\nFoo Bar\nBaz"
; Expected: 1 match (in "F[oo] Bar")

    ; --- Fill screen ---
    LDI r1, 0x000011
    FILL r1

    ; --- Pre-load text at 0x6000 ---
    LDI r20, 0x6000
    STRO r20, "Hello World"
    LDI r20, 0x600B
    LDI r2, 10
    STORE r20, r2
    LDI r20, 0x600C
    STRO r20, "Foo Bar"
    LDI r20, 0x6013
    LDI r2, 10
    STORE r20, r2
    LDI r20, 0x6014
    STRO r20, "Baz"

    ; --- Pre-load pattern at 0x6100 ---
    LDI r20, 0x6100
    STRO r20, "oo"

    ; --- Search ---
    LDI r10, 0x6000        ; text pointer
    LDI r11, 0             ; match count
    LDI r7, 1

search_loop:
    ; Read char at text pointer
    LOAD r2, r10
    ; Check for null (end of text)
    LDI r3, 0
    CMP r2, r3
    JZ r0, search_done

    ; Try to match pattern at this position
    LDI r15, 0x6100        ; pattern pointer
    MOV r16, r10            ; save start position

match_try:
    ; Read pattern char
    LOAD r2, r15
    ; If pattern char is null, full match
    LDI r3, 0
    CMP r2, r3
    JZ r0, found_match

    ; Read text char at current position
    LOAD r4, r16
    ; If text is null or newline, no match
    LDI r3, 0
    CMP r4, r3
    JZ r0, no_match

    ; Compare
    CMP r2, r4
    JNZ r0, no_match

    ; Match so far, advance both
    ADD r15, r7
    ADD r16, r7
    JMP match_try

found_match:
    ADDI r11, 1            ; match count++

no_match:
    ADDI r10, 1            ; advance text pointer
    JMP search_loop

search_done:
    ; Store match count
    LDI r6, 0x7800
    STORE r6, r11

    ; --- Draw header ---
    LDI r1, 0x006600       ; green header
    LDI r15, 0
    LDI r16, 0
    LDI r17, 256
    LDI r18, 12
    RECTF r15, r16, r17, r18, r1

    LDI r20, 0x5000
    STRO r20, "grep:"
    LDI r10, 4
    LDI r11, 2
    LDI r12, 0x5000
    LDI r13, 0xFFFFFF
    LDI r14, 0x006600
    DRAWTEXT r10, r11, r12, r13, r14

    ; Show pattern searched
    LDI r20, 0x5000
    STRO r20, "Pattern:"
    LDI r10, 4
    LDI r11, 16
    LDI r12, 0x5000
    LDI r13, 0xCCCCCC
    LDI r14, 0x000011
    DRAWTEXT r10, r11, r12, r13, r14

    ; Show the pattern itself
    LDI r10, 80
    LDI r11, 16
    LDI r12, 0x6100
    LDI r13, 0xFFFF00     ; yellow
    LDI r14, 0x000011
    DRAWTEXT r10, r11, r12, r13, r14

    ; Show match count
    LDI r20, 0x5000
    STRO r20, "Matches:"
    LDI r10, 4
    LDI r11, 32
    LDI r12, 0x5000
    LDI r13, 0xCCCCCC
    LDI r14, 0x000011
    DRAWTEXT r10, r11, r12, r13, r14

    ; Show the original text
    LDI r20, 0x5000
    STRO r20, "Text:"
    LDI r10, 4
    LDI r11, 48
    LDI r12, 0x5000
    LDI r13, 0xCCCCCC
    LDI r14, 0x000011
    DRAWTEXT r10, r11, r12, r13, r14

    LDI r10, 50
    LDI r11, 48
    LDI r12, 0x6000
    LDI r13, 0x888888
    LDI r14, 0x000011
    DRAWTEXT r10, r11, r12, r13, r14

    HALT
