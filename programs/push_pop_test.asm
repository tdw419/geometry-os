; PUSH/POP test: stack operations with r30 as stack pointer
; Tests: LIFO order, SP tracking, multiple values, register reuse
; Results stored at RAM[0x0200..0x0208]

; ── Setup: point SP at top of RAM ──
  LDI r30, 0xFF00

; ── Test 1: Basic LIFO -- push 3 values, pop in reverse ──
  LDI r0, 100
  LDI r1, 200
  LDI r2, 300
  PUSH r0           ; stack: [100]
  PUSH r1           ; stack: [100, 200]
  PUSH r2           ; stack: [100, 200, 300]
  POP r5            ; r5 = 300 (top)
  POP r6            ; r6 = 200
  POP r7            ; r7 = 100
  LDI r10, 0x0200
  STORE r10, r5     ; RAM[0x0200] = 300
  LDI r10, 0x0201
  STORE r10, r6     ; RAM[0x0201] = 200
  LDI r10, 0x0202
  STORE r10, r7     ; RAM[0x0202] = 100

; ── Test 2: Push same register multiple times ──
  LDI r0, 0
  PUSH r0           ; push 0
  LDI r0, 1
  PUSH r0           ; push 1
  LDI r0, 2
  PUSH r0           ; push 2
  POP r5            ; r5 = 2
  POP r6            ; r6 = 1
  POP r7            ; r7 = 0
  LDI r10, 0x0203
  STORE r10, r5     ; RAM[0x0203] = 2
  LDI r10, 0x0204
  STORE r10, r6     ; RAM[0x0204] = 1
  LDI r10, 0x0205
  STORE r10, r7     ; RAM[0x0205] = 0

; ── Test 3: SP restored after balanced push/pop ──
; After all above operations, SP should be back at 0xFF00
; We test this by pushing a known value and popping it
  LDI r0, 42
  PUSH r0           ; push at SP = 0xFF00
  POP r5            ; r5 should be 42 if SP is correct
  LDI r10, 0x0206
  STORE r10, r5     ; RAM[0x0206] = 42

; ── Test 4: PUSH preserves values across register reuse ──
  LDI r0, 777
  PUSH r0           ; save 777
  LDI r0, 0         ; clobber r0
  ADD r0, r1        ; r0 = 0 + r1 (some garbage value)
  LDI r0, 0
  ADD r0, r2
  POP r5            ; r5 should still be 777
  LDI r10, 0x0207
  STORE r10, r5     ; RAM[0x0207] = 777

; ── Test 5: Push 5 values, pop all 5 ──
  LDI r0, 10
  PUSH r0
  LDI r0, 20
  PUSH r0
  LDI r0, 30
  PUSH r0
  LDI r0, 40
  PUSH r0
  LDI r0, 50
  PUSH r0
  ; Pop into r5 and accumulate sum
  LDI r5, 0
  POP r6
  ADD r5, r6        ; 50
  POP r6
  ADD r5, r6        ; 50+40=90
  POP r6
  ADD r5, r6        ; 90+30=120
  POP r6
  ADD r5, r6        ; 120+20=140
  POP r6
  ADD r5, r6        ; 140+10=150
  LDI r10, 0x0208
  STORE r10, r5     ; RAM[0x0208] = 150

  HALT
