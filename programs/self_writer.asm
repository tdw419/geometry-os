; self_writer.asm
; Pixel-Driving-Pixels Demo: A program that writes its successor to the canvas.
;
; HOW IT WORKS:
; 1. Copies successor source code from RAM data area to canvas buffer (0x8000+)
;    using a STORE loop -- each character becomes a visible glyph on the grid.
; 2. Calls ASMSELF (0x73) to compile the canvas text into bytecode at 0x1000.
; 3. Calls RUNNEXT (0x74) to execute the newly compiled code.
; 4. The successor runs: LDI r0, 42 / LDI r1, 1 / ADD r0, r1 / HALT
; 5. Result: r0 = 43, proving the self-modification cycle worked.
;
; This is the core "pixel driving pixels" loop:
;   Program A writes Program B's source as pixels on the canvas.
;   ASMSELF turns those pixels back into executable code.
;   RUNNEXT executes the new code.
;   The new code is a true successor -- different from the original.

  LDI r1, 0x8000      ; canvas base address (row 0, col 0)
  LDI r2, successor    ; address of successor source data
  LDI r3, 1           ; address increment

loop:
  LOAD r4, r2          ; load next char from successor data
  JZ r4, compile       ; null terminator means done writing
  STORE r1, r4         ; write char to canvas cell (visible as glyph!)
  ADD r1, r3           ; advance canvas pointer
  ADD r2, r3           ; advance source pointer
  JMP loop

compile:
  ASMSELF              ; compile canvas text -> bytecode at 0x1000
  RUNNEXT              ; jump to 0x1000 and execute successor
  HALT                 ; safety (successor HALTs first)

; Successor program source: "LDI r0, 42\nLDI r1, 1\nADD r0, r1\nHALT\n"
; Each character stored as a u32 byte value, null-terminated.
.org 0x200
successor:
  .byte 76, 68, 73, 32, 114, 48, 44, 32, 52, 50, 10
  .byte 76, 68, 73, 32, 114, 49, 44, 32, 49, 10
  .byte 65, 68, 68, 32, 114, 48, 44, 32, 114, 49, 10
  .byte 72, 65, 76, 84, 10, 0
