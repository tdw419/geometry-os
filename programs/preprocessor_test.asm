; preprocessor_test.asm
; Test the new abstraction layer

VAR score 0x4000
VAR player_x 0x4001

; Initialize variables
SET score, 10
SET player_x, 128

; Load back into registers
GET r10, score
GET r11, player_x

; Verify values in registers via PSET (visual check)
; If r10=10 and r11=128, it works
PSET r11, r10, r10

HALT
