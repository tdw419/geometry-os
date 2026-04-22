; oracle_autodev.asm -- LLM Oracle Architect for autonomous development
;
; The Oracle building becomes the autodev architect. It uses the LLM opcode
; to decide what to build next and displays the decision on screen.
;
; This is the in-VM visualization layer for the Oracle-driven autodev.
; The actual build execution happens externally (Hermes workers), but
; this program shows the Oracle "thinking" in real time.
;
; Memory layout:
;   0x3000-0x30FF: Roadmap status summary (injected by host before launch)
;   0x3300-0x36FF: LLM response buffer
;   0x3700-0x371F: Status line
;   0x3800-0x381F: Title
;   0x3900-0x391F: Oracle prompt (built at runtime)
;
; Registers:
;   r0: CMP result (reserved)
;   r1: Key input
;   r7: Constant 1
;   r8: State (0=idle, 1=thinking, 2=result)

; ===== Initialize =====
LDI r7, 1
LDI r8, 0            ; state: idle

; Title
LDI r12, 0x3800
STRO r12, "ORACLE ARCHITECT v1.0"

; Build the Oracle prompt
LDI r12, 0x3900
STRO r12, "You are the Geometry OS Oracle. Read the roadmap state below. Which planned phase to build next? Reply with just the phase ID and a one-line reason."

; Status
LDI r12, 0x3700
STRO r12, "SPACE=ask Oracle"

; ===== Main Loop =====
main_loop:
    FILL r0

    ; -- Title bar --
    LDI r12, 0x1A0066    ; oracle purple
    LDI r13, 0
    LDI r14, 0
    LDI r15, 256
    LDI r16, 8
    RECTF r13, r14, r15, r16, r12

    ; Title text
    LDI r13, 2
    LDI r14, 1
    LDI r15, 0x3800
    TEXT r13, r14, r15

    ; -- Separator --
    LDI r12, 0x333355
    LDI r13, 0
    LDI r14, 9
    LDI r15, 256
    LDI r16, 1
    RECTF r13, r14, r15, r16, r12

    ; -- State dispatch --
    JNZ r8, not_idle
    JMP state_idle
not_idle:
    LDI r20, 1
    SUB r20, r8, r20
    JNZ r20, not_thinking
    JMP state_thinking
not_thinking:
    JMP state_result

state_idle:
    ; Show roadmap summary
    LDI r13, 2
    LDI r14, 12
    LDI r15, 0x3000
    TEXT r13, r14, r15

    ; Status
    LDI r13, 2
    LDI r14, 28
    LDI r15, 0x3700
    TEXT r13, r14, r15

    ; Wait for SPACE
    IKEY r1
    LDI r12, 32
    CMP r1, r12
    JNZ r0, main_loop

    ; SPACE pressed -- call Oracle
    LDI r8, 1
    LDI r12, 0x3700
    STRO r12, "Oracle thinking..."
    JMP main_loop

state_thinking:
    ; Call LLM: prompt from 0x3900, response to 0x3300, max 768 bytes
    LDI r3, 0x3900
    LDI r4, 0x3300
    LDI r5, 768
    LLM r3, r4, r5

    ; r0 = response length (0 = error)
    JNZ r0, oracle_ok

    ; Oracle failed
    LDI r8, 0
    LDI r12, 0x3700
    STRO r12, "Oracle silent. SPACE=retry"
    JMP main_loop

oracle_ok:
    LDI r8, 2
    LDI r12, 0x3700
    STRO r12, "SPACE=again  ESC=back"
    JMP main_loop

state_result:
    ; Show the Oracle's response
    LDI r13, 2
    LDI r14, 12
    LDI r15, 0x3300
    TEXT r13, r14, r15

    ; Status
    LDI r13, 2
    LDI r14, 28
    LDI r15, 0x3700
    TEXT r13, r14, r15

    ; Wait for key
    IKEY r1
    LDI r12, 32
    CMP r1, r12
    JNZ r0, check_esc
    ; SPACE = ask again
    LDI r8, 1
    LDI r12, 0x3700
    STRO r12, "Oracle thinking..."
    JMP main_loop

check_esc:
    LDI r12, 27
    CMP r1, r12
    JNZ r0, main_loop
    ; ESC = back to idle
    LDI r8, 0
    LDI r12, 0x3700
    STRO r12, "SPACE=ask Oracle"
    JMP main_loop
