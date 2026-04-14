; canvas_counter.asm
; Increments a digit on the canvas grid each frame.
; Demonstrates that the grid IS the display.

  LDI r1, 0x8000    ; Top-left corner of canvas
  LDI r2, 0x30      ; ASCII '0'
  LDI r3, 1         ; Increment
  LDI r4, 0x3A      ; ASCII ':' (one past '9')

loop:
  STORE r1, r2      ; Update canvas
  FRAME             ; Wait for next frame
  ADD r2, r3        ; Increment character
  CMP r2, r4        ; Reached 10?
  BLT r0, loop      ; No, continue
  LDI r2, 0x30      ; Yes, reset to '0'
  JMP loop
