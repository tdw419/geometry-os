; sound_mixer.asm -- Sound Mixer Daemon with Music Player (Phase 99)
;
; Demonstrates IPC-based audio control using shared RAM polling.
; The parent process (music player) writes note data to shared RAM,
; and the mixer daemon (child process) polls and plays via BEEP.
;
; Shared RAM protocol (0xF00-0xF03, in identity-mapped page 3):
;   0xF00 = command flag (0 = no command, 1 = play note, 2 = shutdown)
;   0xF01 = frequency (Hz)
;   0xF02 = duration (ms)
;   0xF03 = channel (0-3)
;
; Channel table in mixer RAM (0x200):
;   [ch0_freq, ch0_dur, ch0_active,
;    ch1_freq, ch1_dur, ch1_active,
;    ch2_freq, ch2_dur, ch2_active,
;    ch3_freq, ch3_dur, ch3_active]
;
; Register convention (parent):
;   r10-r13 = shared RAM addresses
;   r2  = frequency
;   r3  = duration
;   r4  = channel
;
; Register convention (mixer daemon):
;   r7  = constant 1
;   r8  = constant 4 (max channels)
;   r9  = channel table base (0x200)
;   r10-r13 = shared RAM addresses

; ── Parent Process (Music Player) ────────────────────────────────
    LDI r30, 0xFF00         ; stack pointer

    ; ── Spawn mixer daemon ──
    LDI r5, mixer_daemon
    SPAWN r5

    ; ── Set up shared RAM address pointers ──
    LDI r10, 0xF00          ; command flag address
    LDI r11, 0xF01          ; frequency address
    LDI r12, 0xF02          ; duration address
    LDI r13, 0xF03          ; channel address

    ; ── Play melody via shared RAM ──
    ; Helper: write_note(freq, dur, channel) - polls until mixer is ready

    ; Note 1: C4 (262 Hz) on channel 0
    LDI r2, 262
    LDI r3, 400
    LDI r4, 0
    CALL write_note

    ; Note 2: E4 (330 Hz) on channel 1
    LDI r2, 330
    LDI r3, 400
    LDI r4, 1
    CALL write_note

    ; Note 3: G4 (392 Hz) on channel 0
    LDI r2, 392
    LDI r3, 400
    LDI r4, 0
    CALL write_note

    ; Note 4: C5 (523 Hz) on channel 2
    LDI r2, 523
    LDI r3, 600
    LDI r4, 2
    CALL write_note

    ; Note 5: A4 (440 Hz) on channel 3
    LDI r2, 440
    LDI r3, 500
    LDI r4, 3
    CALL write_note

    ; ── Send shutdown command ──
    LDI r1, 2               ; shutdown flag
    STORE r10, r1

    HALT

; ── write_note subroutine ───────────────────────────────────────
; Sends a note to the mixer via shared RAM
; Input: r2=freq, r3=dur, r4=channel
; Uses: r1 (scratch)
; Preserves: r2, r3, r4, r10-r13
write_note:
    PUSH r31
wait_ready:
    LOAD r1, r10            ; r1 = command flag
    JNZ r1, wait_ready      ; wait until mixer clears previous command

    ; Write note data to shared RAM
    STORE r11, r2            ; frequency
    STORE r12, r3            ; duration
    STORE r13, r4            ; channel
    LDI r1, 1
    STORE r10, r1            ; set command flag = 1

    POP r31
    RET

; ── Mixer Daemon (child process at page-aligned 0x400) ──────────
.org 0x400
mixer_daemon:
    LDI r30, 0xFF00         ; stack pointer
    LDI r7, 1               ; constant 1
    LDI r8, 4               ; max channels
    LDI r9, 0x200           ; channel table base

    ; Set up shared RAM address pointers
    LDI r10, 0xF00          ; command flag
    LDI r11, 0xF01          ; frequency
    LDI r12, 0xF02          ; duration
    LDI r13, 0xF03          ; channel

mixer_loop:
    ; Poll for commands
    LOAD r1, r10            ; r1 = command flag
    JZ r1, mixer_loop       ; no command, keep polling

    ; Check for shutdown (command == 2)
    LDI r2, 2
    CMP r1, r2
    JZ r0, mixer_shutdown

    ; Command == 1: play note
    ; Read note data from shared RAM
    LOAD r2, r11            ; frequency
    LOAD r3, r12            ; duration
    LOAD r4, r13            ; channel

    ; Validate channel (0-3)
    CMP r4, r8
    BGE r0, clear_cmd       ; invalid channel, skip

    ; Store in channel table
    ; offset = channel * 3, base = 0x200
    MOV r1, r4              ; r1 = channel
    LDI r5, 3
    MUL r1, r5              ; r1 = channel * 3
    ADD r1, r9              ; r1 = 0x200 + channel * 3

    STORE r1, r2            ; freq at base+0
    MOV r5, r1
    ADD r5, r7
    STORE r5, r3            ; dur at base+1
    MOV r5, r1
    LDI r6, 2
    ADD r5, r6
    STORE r5, r7            ; active = 1 at base+2

    ; Play the note via BEEP
    BEEP r2, r3

clear_cmd:
    ; Clear command flag so parent can send next note
    LDI r1, 0
    STORE r10, r1

    JMP mixer_loop

mixer_shutdown:
    LDI r1, 0
    STORE r10, r1
    HALT
