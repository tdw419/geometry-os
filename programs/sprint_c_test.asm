; Sprint C test: MOD, PUSH/POP, BLT/BGE
; Results stored at RAM[0x0200..0x0209]

; ── Test 1: MOD -- 17 % 5 = 2 ──
  LDI r0, 17
  LDI r1, 5
  MOD r0, r1        ; r0 = 17 % 5 = 2
  LDI r10, 0x0200
  STORE r10, r0     ; RAM[0x0200] = 2

; ── Test 2: MOD -- 10 % 3 = 1 ──
  LDI r0, 10
  LDI r1, 3
  MOD r0, r1        ; r0 = 10 % 3 = 1
  LDI r10, 0x0201
  STORE r10, r0     ; RAM[0x0201] = 1

; ── Test 3: PUSH/POP -- push 42, push 99, pop into r5, pop into r6 ──
  LDI r30, 0xFF00   ; set stack pointer to top of RAM
  LDI r2, 42
  LDI r3, 99
  PUSH r2           ; push 42
  PUSH r3           ; push 99
  POP r5            ; r5 = 99 (last in, first out)
  POP r6            ; r6 = 42
  LDI r10, 0x0202
  STORE r10, r5     ; RAM[0x0202] = 99
  LDI r10, 0x0203
  STORE r10, r6     ; RAM[0x0203] = 42

; ── Test 4: BLT -- if 3 < 10, store 1 at 0x0204 ──
  LDI r0, 3
  LDI r1, 10
  CMP r0, r1        ; r0 = -1 (3 < 10)
  BLT r0, blt_taken
  ; Not taken -- store 0
  LDI r10, 0x0204
  LDI r2, 0
  STORE r10, r2
  JMP blt_done
blt_taken:
  LDI r10, 0x0204
  LDI r2, 1
  STORE r10, r2     ; RAM[0x0204] = 1
blt_done:

; ── Test 5: BLT NOT taken -- if 10 < 3, branch (should not) ──
  LDI r0, 10
  LDI r1, 3
  CMP r0, r1        ; r0 = 1 (10 > 3)
  BLT r0, blt_fail
  ; Falls through (correct) -- store 0
  LDI r10, 0x0205
  LDI r2, 0
  STORE r10, r2     ; RAM[0x0205] = 0 (BLT correctly not taken)
  JMP blt_pass
blt_fail:
  LDI r10, 0x0205
  LDI r2, 1
  STORE r10, r2     ; RAM[0x0205] = 1 (WRONG)
blt_pass:

; ── Test 6: BGE -- if 10 >= 3, store 1 at 0x0206 ──
  LDI r0, 10
  LDI r1, 3
  CMP r0, r1        ; r0 = 1 (10 > 3, so >=)
  BGE r0, bge_taken
  ; Not taken -- store 0
  LDI r10, 0x0206
  LDI r2, 0
  STORE r10, r2
  JMP bge_done
bge_taken:
  LDI r10, 0x0206
  LDI r2, 1
  STORE r10, r2     ; RAM[0x0206] = 1
bge_done:

; ── Test 7: BGE with equal values -- if 5 >= 5, store 1 at 0x0207 ──
  LDI r0, 5
  LDI r1, 5
  CMP r0, r1        ; r0 = 0 (5 == 5, so >= is true)
  BGE r0, bge_eq_taken
  LDI r10, 0x0207
  LDI r2, 0
  STORE r10, r2
  JMP bge_eq_done
bge_eq_taken:
  LDI r10, 0x0207
  LDI r2, 1
  STORE r10, r2     ; RAM[0x0207] = 1
bge_eq_done:

  HALT
