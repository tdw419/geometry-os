; lib_test.asm -- Shared Library Test (Phase 100)
;
; Tests the shared library system by loading string.lib from VFS
; and calling strlen at a fixed known address.
;
; Library loading convention:
;   string.lib is loaded at address 0x5000
;   strlen is the first function (at offset 0)
;
; Register convention:
;   r10 = lib base address (0x5000)
;   r1  = argument to strlen (string address)

    LDI r30, 0xFF00         ; SP

    ; ── Write test string "Hello" at 0x6000 ──
    LDI r1, 0x6000
    LDI r2, 72              ; 'H'
    STORE r1, r2
    LDI r2, 101             ; 'e'
    LDI r3, 1
    ADD r1, r3
    STORE r1, r2
    LDI r2, 108             ; 'l'
    ADD r1, r3
    STORE r1, r2
    LDI r2, 108             ; 'l'
    ADD r1, r3
    STORE r1, r2
    LDI r2, 111             ; 'o'
    ADD r1, r3
    STORE r1, r2
    LDI r2, 0               ; null terminator
    ADD r1, r3
    STORE r1, r2

    ; ── Call strlen from library at 0x5000 ──
    LDI r1, 0x6000          ; string address
    PUSH r31
    CALL 0x5000             ; call strlen at fixed lib address
    POP r31

    ; ── Store result ──
    LDI r1, 0x7000
    STORE r1, r0            ; store strlen result

    HALT
