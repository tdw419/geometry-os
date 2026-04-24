; chatbot.asm - ELIZA-style Pattern Matcher on Canvas
;
; A simple chatbot that reads input from the canvas buffer,
; matches keyword patterns, and writes responses back to the canvas.
;
; Input: User writes text on the canvas grid (row 0-3)
; Output: Chatbot writes response on the canvas (row 4-7)
;
; Pattern matching:
;   - Scans input for keywords (HELLO, NAME, HOW, BYE, HELP, etc.)
;   - First matching keyword determines the response
;   - If no keyword matches, uses a default response
;
; Demonstrates: canvas buffer text I/O, string scanning,
;   pattern matching, self-contained text processing
;
; Canvas layout (32 cols x 128 rows):
;   Row 0: "geo> " prompt + user input
;   Row 4-5: Chatbot response text
;
; Register usage:
;   r7  = constant 1
;   r14 = 1 (address increment)
;   r8  = read pointer (scans input)
;   r9  = write pointer (writes response)
;   r10 = current character
;   r11 = temp for comparison
;   r12 = keyword match flag (0=no match, 1=match)

LDI r7, 1
LDI r14, 1
LDI r30, 0xFF00         ; initialize stack pointer

; ===== Step 1: Scan input for keywords =====
; Input starts at canvas row 0 (offset 0x8000)
; Skip "geo> " prefix (5 chars) if present

LDI r8, 0x8000          ; start of canvas = row 0

; Skip past "geo> " if it starts with 'g' (0x67)
LOAD r10, r8
LDI r11, 103            ; 'g'
CMP r10, r11
JNZ r0, scan_start
; Skip 5 chars ("geo> ")
LDI r11, 5
ADD r8, r11

scan_start:
; ===== Check keyword: HELLO (0x48='H') =====
LDI r12, 0              ; no match yet
LOAD r10, r8
LDI r11, 72             ; 'H'
CMP r10, r11
JNZ r0, check_name
; Check 'E' at next position
MOV r11, r8
ADD r11, r7
LOAD r10, r11
LDI r11, 69             ; 'E'
CMP r10, r11
JNZ r0, check_name
; Check 'L'
MOV r11, r8
LDI r3, 2
ADD r11, r3
LOAD r10, r11
LDI r11, 76             ; 'L'
CMP r10, r11
JNZ r0, check_name
; Check 'L'
MOV r11, r8
LDI r3, 3
ADD r11, r3
LOAD r10, r11
LDI r11, 76             ; 'L'
CMP r10, r11
JNZ r0, check_name
; Check 'O'
MOV r11, r8
LDI r3, 4
ADD r11, r3
LOAD r10, r11
LDI r11, 79             ; 'O'
CMP r10, r11
JNZ r0, check_name
; Match! Response: "Hello! I am GEO bot."
LDI r12, 1
CALL write_hello
JMP respond_done

check_name:
; ===== Check keyword: NAME (0x4E='N') =====
LOAD r10, r8
LDI r11, 78             ; 'N'
CMP r10, r11
JNZ r0, check_how
MOV r11, r8
ADD r11, r7
LOAD r10, r11
LDI r11, 65             ; 'A'
CMP r10, r11
JNZ r0, check_how
MOV r11, r8
LDI r3, 2
ADD r11, r3
LOAD r10, r11
LDI r11, 77             ; 'M'
CMP r10, r11
JNZ r0, check_how
MOV r11, r8
LDI r3, 3
ADD r11, r3
LOAD r10, r11
LDI r11, 69             ; 'E'
CMP r10, r11
JNZ r0, check_how
; Match! "I am GEO, the canvas bot."
LDI r12, 1
CALL write_name_resp
JMP respond_done

check_how:
; ===== Check keyword: HOW (0x48='H') =====
LOAD r10, r8
LDI r11, 72             ; 'H'
CMP r10, r11
JNZ r0, check_bye
MOV r11, r8
ADD r11, r7
LOAD r10, r11
LDI r11, 79             ; 'O'
CMP r10, r11
JNZ r0, check_bye
MOV r11, r8
LDI r3, 2
ADD r11, r3
LOAD r10, r11
LDI r11, 87             ; 'W'
CMP r10, r11
JNZ r0, check_bye
; Match! "I run on pixels. All is well!"
LDI r12, 1
CALL write_how_resp
JMP respond_done

check_bye:
; ===== Check keyword: BYE (0x42='B') =====
LOAD r10, r8
LDI r11, 66             ; 'B'
CMP r10, r11
JNZ r0, check_help
MOV r11, r8
ADD r11, r7
LOAD r10, r11
LDI r11, 89             ; 'Y'
CMP r10, r11
JNZ r0, check_help
MOV r11, r8
LDI r3, 2
ADD r11, r3
LOAD r10, r11
LDI r11, 69             ; 'E'
CMP r10, r11
JNZ r0, check_help
; Match! "Goodbye! Pixels forever."
LDI r12, 1
CALL write_bye_resp
JMP respond_done

check_help:
; ===== Check keyword: HELP (0x48='H') =====
; Already checked H for HELLO and HOW, but HELP has different 2nd char
; Actually we already consumed H->HOW check. Let me check HELP differently.
; HELP starts with 'H','E' like HELLO but has 'L','P' at pos 2,3
; We already checked HELLO (H,E,L,L,O) above and jumped if matched.
; For HELP: check H,E,L,P
LOAD r10, r8
LDI r11, 72             ; 'H'
CMP r10, r11
JNZ r0, default_resp
MOV r11, r8
ADD r11, r7
LOAD r10, r11
LDI r11, 69             ; 'E'
CMP r10, r11
JNZ r0, default_resp
MOV r11, r8
LDI r3, 2
ADD r11, r3
LOAD r10, r11
LDI r11, 76             ; 'L'
CMP r10, r11
JNZ r0, default_resp
MOV r11, r8
LDI r3, 3
ADD r11, r3
LOAD r10, r11
LDI r11, 80             ; 'P'
CMP r10, r11
JNZ r0, default_resp
; Match! "Say HELLO, NAME, HOW, or BYE"
LDI r12, 1
CALL write_help_resp
JMP respond_done

default_resp:
; ===== Default response =====
CALL write_default

respond_done:
HALT

; ===== Response subroutines =====
; All write to canvas row 4 (offset 4*32 = 128 from 0x8000)

; Write a null-terminated string stored inline in RAM to canvas
; Uses STRO-like approach but manually
; r9 = canvas write address

write_hello:
  PUSH r31
  LDI r9, 0x8080        ; row 4 = offset 128
  ; "Hello! I am GEO bot."
  LDI r2, 72            ; 'H'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 101           ; 'e'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 108           ; 'l'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 108           ; 'l'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 111           ; 'o'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 33            ; '!'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 73            ; 'I'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 97            ; 'a'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 109           ; 'm'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 71            ; 'G'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 69            ; 'E'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 79            ; 'O'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 98            ; 'b'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 111           ; 'o'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 116           ; 't'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 46            ; '.'
  STORE r9, r2
  POP r31
  RET

write_name_resp:
  PUSH r31
  LDI r9, 0x8080
  ; "I am GEO, the canvas bot."
  LDI r2, 73            ; 'I'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 97            ; 'a'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 109           ; 'm'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 71            ; 'G'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 69            ; 'E'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 79            ; 'O'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 44            ; ','
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 116           ; 't'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 104           ; 'h'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 101           ; 'e'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 99            ; 'c'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 97            ; 'a'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 110           ; 'n'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 118           ; 'v'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 97            ; 'a'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 115           ; 's'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 98            ; 'b'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 111           ; 'o'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 116           ; 't'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 46            ; '.'
  STORE r9, r2
  POP r31
  RET

write_how_resp:
  PUSH r31
  LDI r9, 0x8080
  ; "I run on pixels. All is well!"
  LDI r2, 73            ; 'I'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 114           ; 'r'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 117           ; 'u'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 110           ; 'n'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 111           ; 'o'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 110           ; 'n'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 112           ; 'p'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 105           ; 'i'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 120           ; 'x'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 101           ; 'e'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 108           ; 'l'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 115           ; 's'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 46            ; '.'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 65            ; 'A'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 108           ; 'l'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 108           ; 'l'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 105           ; 'i'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 115           ; 's'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 119           ; 'w'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 101           ; 'e'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 108           ; 'l'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 108           ; 'l'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 33            ; '!'
  STORE r9, r2
  POP r31
  RET

write_bye_resp:
  PUSH r31
  LDI r9, 0x8080
  ; "Goodbye! Pixels forever."
  LDI r2, 71            ; 'G'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 111           ; 'o'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 111           ; 'o'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 100           ; 'd'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 98            ; 'b'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 121           ; 'y'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 101           ; 'e'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 33            ; '!'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 80            ; 'P'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 105           ; 'i'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 120           ; 'x'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 101           ; 'e'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 108           ; 'l'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 115           ; 's'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 102           ; 'f'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 111           ; 'o'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 114           ; 'r'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 101           ; 'e'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 118           ; 'v'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 101           ; 'e'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 114           ; 'r'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 46            ; '.'
  STORE r9, r2
  POP r31
  RET

write_help_resp:
  PUSH r31
  LDI r9, 0x8080
  ; "Say HELLO NAME HOW or BYE"
  LDI r2, 83            ; 'S'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 97            ; 'a'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 121           ; 'y'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 72            ; 'H'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 69            ; 'E'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 76            ; 'L'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 76            ; 'L'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 79            ; 'O'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 78            ; 'N'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 65            ; 'A'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 77            ; 'M'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 69            ; 'E'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 72            ; 'H'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 79            ; 'O'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 87            ; 'W'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 111           ; 'o'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 114           ; 'r'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 66            ; 'B'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 89            ; 'Y'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 69            ; 'E'
  STORE r9, r2
  POP r31
  RET

write_default:
  PUSH r31
  LDI r9, 0x8080
  ; "I do not understand. Try HELP"
  LDI r2, 73            ; 'I'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 100           ; 'd'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 111           ; 'o'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 110           ; 'n'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 111           ; 'o'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 116           ; 't'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 117           ; 'u'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 110           ; 'n'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 100           ; 'd'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 101           ; 'e'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 114           ; 'r'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 115           ; 's'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 116           ; 't'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 97            ; 'a'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 110           ; 'n'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 100           ; 'd'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 46            ; '.'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 84            ; 'T'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 114           ; 'r'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 121           ; 'y'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 32            ; ' '
  STORE r9, r2
  ADD r9, r14
  LDI r2, 72            ; 'H'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 69            ; 'E'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 76            ; 'L'
  STORE r9, r2
  ADD r9, r14
  LDI r2, 80            ; 'P'
  STORE r9, r2
  POP r31
  RET
