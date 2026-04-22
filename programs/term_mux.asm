; term_mux.asm -- Phase 103: Terminal Multiplexer
;
; A minimal terminal multiplexer that manages 2 sessions.
; Each session is a child process with stdin/stdout connected via pipes.
; The muxer reads keyboard, routes to active session, and renders output.
;
; Session protocol (shared RAM at 0xF00-0xF03):
;   0xF00: active_session (0 or 1)
;   0xF01: session_0_output_len (words written to output buffer)
;   0xF02: session_1_output_len
;   0xF03: reserved
;
; Output buffers:
;   0x5000-0x5FFF: session 0 scrollback (4K words)
;   0x6000-0x6FFF: session 1 scrollback (4K words)
;
; Keyboard routing: muxer reads IKEY, writes to active session's stdin pipe
;
; Controls:
;   Ctrl+1 (ASCII 1): switch to session 0
;   Ctrl+2 (ASCII 2): switch to session 1
;   Ctrl+N (ASCII 14): create new session (spawns echo_test)
;   All other keys: forwarded to active session

.org 0x000

; ═══════════════════════════════════════════════════════════════
; Initialize
; ═══════════════════════════════════════════════════════════════
    LDI r7, 1
    LDI r8, 0xFF00
    MOV r30, r8              ; SP = 0xFF00

    ; Clear screen
    LDI r0, 0
    FILL r0

    ; Initialize shared state
    LDI r10, 0xF00
    LDI r0, 0
    STORE r10, r0            ; active_session = 0
    ADD r10, r7
    STORE r10, r0            ; session_0_output_len = 0
    ADD r10, r7
    STORE r10, r0            ; session_1_output_len = 0

    ; Initialize scrollback buffers to zero
    ; NOTE: r0 is CMP result register, so use r9 for zero
    LDI r9, 0
    LDI r11, 0x5000
    LDI r12, 0x7000
init_buffers:
    STORE r11, r9
    ADD r11, r7
    CMP r11, r12
    BLT r0, init_buffers

    ; Create pipe pair for session 0
    ; PIPE creates read_fd and write_fd
    LDI r1, 20               ; r20 = read_fd
    LDI r2, 21               ; r21 = write_fd
    PIPE r20, r21

    ; Store pipe fds in RAM
    ; Session 0 stdin pipe: write_fd in r21 (muxer writes to child stdin)
    ; Session 0 stdout pipe: read_fd in r20 (muxer reads child stdout)
    LDI r10, 0x4000
    STORE r10, r20           ; RAM[0x4000] = session_0_stdout_read_fd
    ADD r10, r7
    STORE r10, r21           ; RAM[0x4001] = session_0_stdin_write_fd

    ; Store child PID
    LDI r10, 0x4002
    LDI r0, 0
    STORE r10, r0            ; session_0_pid = 0 (not spawned yet)

    ; Create pipe pair for session 1
    LDI r1, 22
    LDI r2, 23
    PIPE r22, r23

    LDI r10, 0x4010
    STORE r10, r22           ; RAM[0x4010] = session_1_stdout_read_fd
    ADD r10, r7
    STORE r10, r23           ; RAM[0x4011] = session_1_stdin_write_fd

    LDI r10, 0x4012
    LDI r0, 0
    STORE r10, r0            ; session_1_pid = 0

    ; Draw status bar at top
    CALL draw_status_bar

    ; ═══════════════════════════════════════════════════════════════
    ; Main loop: read keyboard, route to session, render output
    ; ═══════════════════════════════════════════════════════════════
main_loop:
    IKEY r10                 ; read key from keyboard port

    ; Check for session switch keys
    LDI r11, 1               ; Ctrl+1
    CMP r10, r11
    JZ r0, switch_to_0

    LDI r11, 2               ; Ctrl+2
    CMP r10, r11
    JZ r0, switch_to_1

    ; If key > 0, write to active session's stdin pipe
    JZ r10, read_output

    ; Get active session
    LDI r12, 0xF00
    LOAD r12, r12            ; r12 = active_session

    ; Get stdin_write_fd for active session
    LDI r13, 0x4001          ; base + 1 for stdin_write_fd
    LDI r14, 16              ; offset between sessions (0x4011 - 0x4001)
    MUL r12, r14
    ADD r13, r12             ; r13 = &session[active].stdin_write_fd
    LOAD r13, r13            ; r13 = stdin_write_fd

    ; Write key to pipe: WRITE fd_reg, buf_reg, len_reg
    LDI r15, 0x4080          ; temp buffer for key
    STORE r15, r10
    LDI r16, 1               ; length = 1 word
    WRITE r13, r15, r16

read_output:
    ; Read from both sessions' stdout pipes and render active one
    CALL read_session_output
    CALL render_active_session

    ; Yield frame
    FRAME
    JMP main_loop

; ═══════════════════════════════════════════════════════════════
; Switch to session 0
; ═══════════════════════════════════════════════════════════════
switch_to_0:
    LDI r10, 0xF00
    LDI r11, 0
    STORE r10, r11
    CALL draw_status_bar
    JMP read_output

; ═══════════════════════════════════════════════════════════════
; Switch to session 1
; ═══════════════════════════════════════════════════════════════
switch_to_1:
    LDI r10, 0xF00
    LDI r11, 1
    STORE r10, r11
    CALL draw_status_bar
    JMP read_output

; ═══════════════════════════════════════════════════════════════
; Draw status bar at top of screen
; ═══════════════════════════════════════════════════════════════
draw_status_bar:
    PUSH r31
    ; Background bar
    LDI r0, 0
    LDI r1, 0
    LDI r2, 256
    LDI r3, 12
    LDI r4, 0x1A1A2E        ; dark blue
    RECTF r0, r1, r2, r3, r4

    ; Draw "S0" indicator (session 0)
    LDI r0, 0xF00
    LOAD r0, r0              ; active_session
    JZ r0, s0_active

    ; Session 0 inactive - dim color
    LDI r5, 0x444466
    JMP draw_s0
s0_active:
    LDI r5, 0x44FF44         ; bright green for active
draw_s0:
    ; Draw "0" at position (4, 3)
    LDI r0, 4
    LDI r1, 3
    PSET r0, r1, r5
    LDI r0, 5
    PSET r0, r1, r5

    ; Draw "1" indicator (session 1)
    LDI r0, 0xF00
    LOAD r0, r0
    LDI r11, 1
    CMP r0, r11
    JZ r0, s1_active
    LDI r5, 0x444466         ; dim
    JMP draw_s1
s1_active:
    LDI r5, 0x44FF44         ; bright green
draw_s1:
    LDI r0, 8
    LDI r1, 3
    PSET r0, r1, r5
    LDI r0, 9
    PSET r0, r1, r5

    ; Draw separator
    LDI r5, 0x888888
    LDI r0, 7
    LDI r1, 2
    LDI r2, 7
    LDI r3, 10
    LINE r0, r1, r2, r3, r5

    POP r31
    RET

; ═══════════════════════════════════════════════════════════════
; Read output from both sessions' stdout pipes
; ═══════════════════════════════════════════════════════════════
read_session_output:
    PUSH r31

    ; Read from session 0 stdout pipe
    LDI r10, 0x4000
    LOAD r10, r10            ; r10 = session_0_stdout_read_fd
    LDI r11, 0x4090          ; read buffer
    LDI r12, 1               ; read 1 word
    READ r10, r11, r12

    ; Read from session 1 stdout pipe
    LDI r10, 0x4010
    LOAD r10, r10            ; r10 = session_1_stdout_read_fd
    LDI r11, 0x40A0          ; read buffer
    LDI r12, 1               ; read 1 word
    READ r10, r11, r12

    POP r31
    RET

; ═══════════════════════════════════════════════════════════════
; Render active session's scrollback to screen (below status bar)
; ═══════════════════════════════════════════════════════════════
render_active_session:
    PUSH r31
    ; Get active session
    LDI r10, 0xF00
    LOAD r10, r10            ; r10 = active_session

    ; Calculate buffer base address
    LDI r11, 0x5000          ; session 0 buffer
    JZ r10, render_buf
    LDI r11, 0x6000          ; session 1 buffer

render_buf:
    ; Simple render: draw first 244 rows of scrollback as colored pixels
    ; (each word = one pixel, rendered at y+12 to leave room for status bar)
    LDI r12, 0               ; row counter
    LDI r13, 256              ; width
    LDI r14, 244              ; max rows to render
    LDI r15, 12               ; y offset (below status bar)

render_row:
    LDI r16, 0               ; col counter

render_col:
    ; Compute buffer address
    MOV r17, r12
    MUL r17, r13             ; row * 256
    ADD r17, r16             ; + col
    MOV r18, r11
    ADD r18, r17             ; buf + offset
    LOAD r18, r18            ; pixel value

    ; Only draw non-zero pixels
    JZ r18, skip_pixel

    ; Draw pixel
    MOV r19, r16             ; x = col
    MOV r20, r12
    ADD r20, r15             ; y = row + offset
    PSET r19, r20, r18

skip_pixel:
    ADD r16, r7              ; col++
    CMP r16, r13
    BLT r0, render_col

    ADD r12, r7              ; row++
    CMP r12, r14
    BLT r0, render_row

    POP r31
    RET

HALT
