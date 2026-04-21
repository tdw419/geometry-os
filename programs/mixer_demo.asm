; mixer_demo.asm -- Multi-Channel Music Demo (Phase 99)
;
; Two processes send notes to a mixer daemon on different channels.
; Demonstrates multi-process IPC with concurrent audio.
;
; Architecture:
;   Parent --- MSGSND ---> Mixer (daemon) <--- MSGSND --- Child
;   Parent plays melody on channel 0
;   Child plays bass on channel 1
;
; The mixer receives notes from both processes and plays them.

    LDI r30, 0xFF00         ; SP

    ; ── Spawn mixer daemon ──
    LDI r5, mixer_daemon
    SPAWN r5
    LDI r5, 0xFFA
    LOAD r5, r5             ; r5 = mixer PID

    ; ── Spawn melody player on channel 0 ──
    ; Share mixer PID via shared RAM (0xF00)
    LDI r10, 0xF00
    STORE r10, r5           ; RAM[0xF00] = mixer PID
    LDI r6, melody_child
    SPAWN r6

    ; ── Parent: play bass line on channel 1 ──
    ; Bass: C3, G3, A3, F3
    LDI r1, 1               ; channel 1
    LDI r2, 131             ; C3
    LDI r3, 600
    LDI r4, 70
    MSGSND r5
    FRAME

    LDI r1, 1
    LDI r2, 196             ; G3
    LDI r3, 600
    LDI r4, 70
    MSGSND r5
    FRAME

    LDI r1, 1
    LDI r2, 220             ; A3
    LDI r3, 600
    LDI r4, 70
    MSGSND r5
    FRAME

    LDI r1, 1
    LDI r2, 175             ; F3
    LDI r3, 800
    LDI r4, 80
    MSGSND r5
    FRAME

    ; Let children finish
    FRAME
    FRAME
    FRAME

    ; Kill mixer and halt
    KILL r5
    HALT

; ── Melody Child (channel 0) ────────────────────────────────────
.org 0x400
melody_child:
    LDI r30, 0xFF00         ; SP

    ; Read mixer PID from shared RAM
    LDI r5, 0xF00
    LOAD r5, r5             ; r5 = mixer PID

    ; Play melody on channel 0
    LDI r1, 0               ; channel 0
    LDI r2, 523             ; C5
    LDI r3, 300
    LDI r4, 90
    MSGSND r5
    FRAME

    LDI r1, 0
    LDI r2, 587             ; D5
    LDI r3, 300
    LDI r4, 90
    MSGSND r5
    FRAME

    LDI r1, 0
    LDI r2, 659             ; E5
    LDI r3, 300
    LDI r4, 90
    MSGSND r5
    FRAME

    HALT

; ── Mixer Daemon ────────────────────────────────────────────────
.org 0x800
mixer_daemon:
    LDI r30, 0xFF00         ; SP
    LDI r7, 1               ; constant 1
    LDI r8, 4               ; max channels
    LDI r9, 0x200           ; channel table base

    ; Initialize channel table
    LDI r10, 0
    LDI r12, 0x200
    LDI r14, 0x210
mix_init:
    STORE r12, r10
    ADD r12, r7
    CMP r12, r14
    BLT r0, mix_init

mixer_loop:
    MSGRCV                  ; r0=sender, r1=channel, r2=freq, r3=dur, r4=vol
    CMP r1, r8
    BGE r0, mixer_loop      ; skip invalid channel

    ; Store in channel table
    MOV r10, r1
    LDI r11, 4
    MUL r10, r11
    ADD r10, r9
    STORE r10, r2           ; freq
    MOV r11, r10
    ADD r11, r7
    STORE r11, r3           ; dur
    MOV r11, r10
    LDI r12, 2
    ADD r11, r12
    STORE r11, r4           ; vol
    MOV r11, r10
    LDI r12, 3
    ADD r11, r12
    STORE r11, r7           ; active = 1

    BEEP r2, r3
    FRAME
    JMP mixer_loop
