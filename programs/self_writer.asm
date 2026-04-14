; self_writer.asm
; Pixel-Driving-Pixels Demo: A program that writes its successor to the canvas.
;
; HOW IT WORKS:
; 1. Stores assembly source code ("LDI r0, 42\nHALT\n") into the canvas buffer
;    at addresses 0x8000-0x800E using STORE instructions.
; 2. Calls ASMSELF (0x73) to compile the canvas text into bytecode at 0x1000.
; 3. Calls RUNNEXT (0x74) to execute the newly compiled code.
; 4. The successor program runs: LDI r0, 42 / HALT
; 5. Result: r0 = 42, proving the self-modification cycle worked.
;
; This is the core "pixel driving pixels" loop:
;   Program A writes Program B's source as pixels on the canvas.
;   ASMSELF turns those pixels back into executable code.
;   RUNNEXT executes the new code.
;   The new code is a true successor -- different from the original.

  LDI r8, 0x8000      ; canvas base address (row 0, col 0)
  LDI r6, 1           ; address increment

  ; --- Write "LDI r0, 42" to canvas row 0 ---
  LDI r7, 76          ; 'L'
  STORE r8, r7
  ADD r8, r6
  LDI r7, 68          ; 'D'
  STORE r8, r7
  ADD r8, r6
  LDI r7, 73          ; 'I'
  STORE r8, r7
  ADD r8, r6
  LDI r7, 32          ; ' '
  STORE r8, r7
  ADD r8, r6
  LDI r7, 114         ; 'r'
  STORE r8, r7
  ADD r8, r6
  LDI r7, 48          ; '0'
  STORE r8, r7
  ADD r8, r6
  LDI r7, 44          ; ','
  STORE r8, r7
  ADD r8, r6
  LDI r7, 32          ; ' '
  STORE r8, r7
  ADD r8, r6
  LDI r7, 52          ; '4'
  STORE r8, r7
  ADD r8, r6
  LDI r7, 50          ; '2'
  STORE r8, r7
  ADD r8, r6
  LDI r7, 10          ; '\n' (newline)
  STORE r8, r7
  ADD r8, r6

  ; --- Write "HALT" to canvas row 1 ---
  LDI r7, 72          ; 'H'
  STORE r8, r7
  ADD r8, r6
  LDI r7, 65          ; 'A'
  STORE r8, r7
  ADD r8, r6
  LDI r7, 76          ; 'L'
  STORE r8, r7
  ADD r8, r6
  LDI r7, 84          ; 'T'
  STORE r8, r7
  ADD r8, r6
  LDI r7, 10          ; '\n'
  STORE r8, r7

  ; --- Compile and execute the successor ---
  ASMSELF             ; compile canvas text -> bytecode at 0x1000
  RUNNEXT             ; jump to 0x1000 and execute
  HALT                ; safety halt (successor HALTs first)
