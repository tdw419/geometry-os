; self_analysis.asm -- Recursive Self-Analysis Program
;
; The OS uses its own LLM to reason about its own screen state.
; Phase 111: "AI inside AI" -- a program that creates visual output,
; then uses SCREENP to sample what it drew, builds a description,
; calls the LLM opcode to analyze it, and writes the result to a file.
;
; Memory layout:
;   0x1000-0x10FF  Screen description buffer (built at runtime)
;   0x1400-0x17FF  LLM prompt buffer (1K)
;   0x1800-0x1FFF  LLM response buffer (2K)
;   0x2000-0x20FF  Working text buffer
;   0x2100-0x211F  File path buffer ("/tmp/screen_analysis.txt")
;   0x2200-0x22FF  Summary text buffer
;
; Registers:
;   r1:  constant 1
;   r30: stack pointer

#define PROMPT_BUF  0x1400
#define RESP_BUF    0x1800
#define TEXT_BUF    0x2000
#define PATH_BUF    0x2100
#define SUMMARY_BUF 0x2200

LDI r1, 1
LDI r30, 0xFD00

; ── PHASE 1: Draw content on screen ──

; Dark background
LDI r0, 0x0A0A14
FILL r0

; Title bar (purple)
LDI r1, 0
LDI r2, 0
LDI r3, 256
LDI r4, 20
LDI r5, 0x2D0050
RECTF r1, r2, r3, r4, r5

; Title text
LDI r20, TEXT_BUF
STRO r20, "Recursive Self-Analysis"
LDI r1, 4
LDI r2, 4
LDI r3, TEXT_BUF
LDI r4, 0xFFFFFF
LDI r5, 0x2D0050
DRAWTEXT r1, r2, r3, r4, r5

; Draw 4 colored quadrants so there's something to detect
; Top-left: red block
LDI r1, 10
LDI r2, 30
LDI r3, 50
LDI r4, 50
LDI r5, 0xFF3333
RECTF r1, r2, r3, r4, r5

; Top-right: green block
LDI r1, 196
LDI r2, 30
LDI r3, 50
LDI r4, 50
LDI r5, 0x33FF33
RECTF r1, r2, r3, r4, r5

; Bottom-left: blue block
LDI r1, 10
LDI r2, 176
LDI r3, 50
LDI r4, 50
LDI r5, 0x3333FF
RECTF r1, r2, r3, r4, r5

; Bottom-right: yellow block
LDI r1, 196
LDI r2, 176
LDI r3, 50
LDI r4, 50
LDI r5, 0xFFFF33
RECTF r1, r2, r3, r4, r5

; Status line: "Sampling..."
LDI r20, TEXT_BUF
STRO r20, "Sampling screen..."
LDI r1, 4
LDI r2, 22
LDI r3, TEXT_BUF
LDI r4, 0xAAAAAA
LDI r5, 0x2D0050
DRAWTEXT r1, r2, r3, r4, r5

FRAME

; ── PHASE 2: Sample screen pixels ──
; Sample a 16x16 grid (every 16 pixels). For each quadrant,
; count non-background (non-0x0A0A14) pixels.

LDI r1, 1

; Quadrant counters: r4=TL, r5=TR, r6=BL, r7=BR
LDI r4, 0
LDI r5, 0
LDI r6, 0
LDI r7, 0

; r8 = row counter (0..15)
LDI r8, 0
sample_row:
    ; r9 = col counter (0..15)
    LDI r9, 0
sample_col:
    ; Compute actual pixel position: x = col*16, y = row*16
    MOV r10, r9
    SHLI r10, 4           ; x = col * 16
    MOV r11, r8
    SHLI r11, 4           ; y = row * 16

    ; Read pixel at (x, y)
    SCREENP r12, r10, r11  ; r12 = pixel color

    ; Check if pixel != background (0x0A0A14)
    LDI r13, 0x0A0A14
    CMP r12, r13
    JZ r0, sample_next    ; background pixel, skip

    ; Non-background pixel -- increment appropriate quadrant
    ; TL: x < 128 and y < 128
    CMPI r10, 128
    BGE r0, sample_check_right
    CMPI r11, 128
    BGE r0, sample_check_bl
    ADDI r4, 1            ; TL++
    JMP sample_next

sample_check_right:
    CMPI r11, 128
    BGE r0, sample_check_br
    ADDI r5, 1            ; TR++
    JMP sample_next

sample_check_bl:
    ADDI r6, 1            ; BL++
    JMP sample_next

sample_check_br:
    ADDI r7, 1            ; BR++
    ; fall through to sample_next

sample_next:
    ADDI r9, 1
    CMPI r9, 16
    JNZ r0, sample_col    ; more columns

    ADDI r8, 1
    CMPI r8, 16
    JNZ r0, sample_row    ; more rows

; ── PHASE 3: Build LLM prompt ──
; Describe the screen state in natural language.
; Store quadrant counts as text in PROMPT_BUF.

LDI r20, PROMPT_BUF
STRO r20, "You are analyzing a 256x256 pixel screen. "
CALL advance_to_null
STRO r20, "I sampled a 16x16 grid of pixels and counted "
CALL advance_to_null
STRO r20, "non-background pixels per quadrant:\n"
CALL advance_to_null

; Append TL count
STRO r20, "Top-left: "
CALL advance_to_null
MOV r0, r4
CALL append_number
STRO r20, " pixels\n"
CALL advance_to_null

; Append TR count
STRO r20, "Top-right: "
CALL advance_to_null
MOV r0, r5
CALL append_number
STRO r20, " pixels\n"
CALL advance_to_null

; Append BL count
STRO r20, "Bottom-left: "
CALL advance_to_null
MOV r0, r6
CALL append_number
STRO r20, " pixels\n"
CALL advance_to_null

; Append BR count
STRO r20, "Bottom-right: "
CALL advance_to_null
MOV r0, r7
CALL append_number
STRO r20, " pixels\n"
CALL advance_to_null

STRO r20, "The screen has a purple title bar at top, "
CALL advance_to_null
STRO r20, "and four colored blocks: red (TL), green (TR), "
CALL advance_to_null
STRO r20, "blue (BL), yellow (BR). "
CALL advance_to_null
STRO r20, "Describe what you see in one sentence."
CALL advance_to_null

; ── PHASE 4: Update status ──
LDI r20, TEXT_BUF
STRO r20, "Calling LLM..."
LDI r1, 4
LDI r2, 22
LDI r3, TEXT_BUF
LDI r4, 0xFFFF00
LDI r5, 0x2D0050
DRAWTEXT r1, r2, r3, r4, r5
FRAME

; ── PHASE 5: Call LLM ──
LDI r2, PROMPT_BUF
LDI r3, RESP_BUF
LDI r4, 512
LLM r2, r3, r4
; r0 = response length

; Save response length
MOV r15, r0

; ── PHASE 6: Display response on screen ──
LDI r20, TEXT_BUF
STRO r20, "LLM Response:"
LDI r1, 4
LDI r2, 80
LDI r3, TEXT_BUF
LDI r4, 0x00FF88
LDI r5, 0x0A0A14
DRAWTEXT r1, r2, r3, r4, r5

; Draw the LLM response text (first 32 chars fit on one line)
LDI r1, 4
LDI r2, 92
LDI r3, RESP_BUF
LDI r4, 0xDDDDDD
LDI r5, 0x0A0A14
DRAWTEXT r1, r2, r3, r4, r5

; ── PHASE 7: Write analysis to file ──
; Prepare file path
LDI r20, PATH_BUF
STRO r20, "/tmp/screen_analysis.txt"

; Open for writing
LDI r1, PATH_BUF
LDI r2, 1            ; mode = write
OPEN r1, r2
; r0 = fd

; Check for error
LDI r13, 0xFFFFFFFF
CMP r0, r13
JZ r0, write_failed

; Save fd
MOV r14, r0

; Write the response to file
MOV r1, r14          ; fd
LDI r2, RESP_BUF     ; buf addr
MOV r3, r15          ; length
WRITESTR r1, r2

; Close file
MOV r1, r14
CLOSE r1

; Show success
LDI r20, SUMMARY_BUF
STRO r20, "Analysis saved to /tmp/screen_analysis.txt"
LDI r1, 4
LDI r2, 140
LDI r3, SUMMARY_BUF
LDI r4, 0x33FF33
LDI r5, 0x0A0A14
DRAWTEXT r1, r2, r3, r4, r5
JMP show_counts

write_failed:
    LDI r20, SUMMARY_BUF
    STRO r20, "File write failed (no sandbox?)"
    LDI r1, 4
    LDI r2, 140
    LDI r3, SUMMARY_BUF
    LDI r4, 0xFF3333
    LDI r5, 0x0A0A14
    DRAWTEXT r1, r2, r3, r4, r5

show_counts:
    ; Show quadrant counts on screen
    LDI r20, SUMMARY_BUF
    STRO r20, "TL:"
    CALL advance_to_null
    MOV r0, r4
    CALL append_number
    STRO r20, " TR:"
    CALL advance_to_null
    MOV r0, r5
    CALL append_number
    STRO r20, " BL:"
    CALL advance_to_null
    MOV r0, r6
    CALL append_number
    STRO r20, " BR:"
    CALL advance_to_null
    MOV r0, r7
    CALL append_number

    LDI r1, 4
    LDI r2, 155
    LDI r3, SUMMARY_BUF
    LDI r4, 0xBBBBBB
    LDI r5, 0x0A0A14
    DRAWTEXT r1, r2, r3, r4, r5

; Final status
LDI r20, TEXT_BUF
STRO r20, "Self-analysis complete."
LDI r1, 4
LDI r2, 22
LDI r3, TEXT_BUF
LDI r4, 0x33FF33
LDI r5, 0x2D0050
DRAWTEXT r1, r2, r3, r4, r5
FRAME

; Halt
LDI r0, 0
EXIT r0

; =========================================
; Subroutines
; =========================================

; advance_to_null -- advance r20 to null terminator
; r20: pointer to string in RAM
advance_to_null:
    PUSH r22
advance_loop:
    LOAD r22, r20
    CMPI r22, 0
    JZ r0, advance_done
    ADDI r20, 1
    JMP advance_loop
advance_done:
    POP r22
    RET

; append_number -- append decimal number to string at r20
; r0: number to append (0-255)
; r20: current position (advanced past written digits)
append_number:
    PUSH r2
    PUSH r3
    PUSH r6
    PUSH r22

    ; Handle 0 special case
    CMPI r0, 0
    JNZ r0, append_nonzero
    LDI r22, 48           ; '0'
    STORE r20, r22
    ADDI r20, 1
    JMP append_done

append_nonzero:
    ; Extract digits into stack (reversed)
    LDI r6, 0             ; digit count
    MOV r2, r0            ; working copy
append_extract:
    CMPI r2, 0
    JZ r0, append_write
    ; r2 % 10
    MOV r3, r2
    LDI r22, 10
    DIV r3, r22           ; r3 = r2 / 10
    MUL r3, r22           ; r3 = (r2/10)*10
    SUB r2, r3            ; r2 = r2 % 10 (digit)
    ADDI r2, 48           ; ASCII digit
    PUSH r2               ; push digit onto stack
    ADDI r6, 1
    MOV r2, r0
    LDI r22, 10
    DIV r2, r22           ; r2 = r0 / 10
    MOV r0, r2            ; update r0 for next iteration
    MOV r2, r0
    JMP append_extract

append_write:
    ; Pop digits in correct order
    CMPI r6, 0
    JZ r0, append_done
    POP r22
    STORE r20, r22
    ADDI r20, 1
    SUBI r6, 1
    JMP append_write

append_done:
    LDI r22, 0
    STORE r20, r22        ; null terminate
    POP r22
    POP r6
    POP r3
    POP r2
    RET
