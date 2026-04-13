; preprocessor_advanced_test.asm
; Test register safety and INC/DEC

VAR score 0x4000

; Set r1 and r4 to specific values
LDI r1, 0x1234
LDI r4, 0x5678

; Use SET - should use r28/r29, not r1/r4
SET score, 100

; Use INC
INC score
; score should now be 101

; Use DEC
DEC score
; score should be back to 100

; Verify r1 and r4 are untouched
; r1 should be 0x1234, r4 should be 0x5678
GET r10, score

HALT
