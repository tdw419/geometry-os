; evolving_counter.asm
; Pixel-Driving-Pixels Demo: A live frame counter on the canvas grid.
;
; HOW IT WORKS:
; 1. Each frame, reads TICKS from RAM[0xFFE]
; 2. Converts the count to 4 decimal ASCII digits (0000-9999)
; 3. Writes the digits to canvas buffer at 0x8000 (row 0, cols 0-3)
; 4. The grid becomes a live dashboard -- digits visibly change each frame
;
; The count digits ARE the program's visible state. No separate output.
; This demonstrates that the grid IS the display.
;
; Register usage:
;   r10 = TICKS value (number to convert)
;   r11 = digit extractor (temp copy)
;   r12 = divisor (10)
;   r13 = ASCII '0' offset
;   r14 = current canvas address
;   r15 = address increment
;   r16 = digit position (4,3,2,1 counting down)
;   r17 = digit value (0-9)
;   r18 = temp for division chain
;   r1  = loop counter
;   r5  = temp for comparisons

  LDI r12, 10          ; divisor for decimal extraction
  LDI r13, 0x30        ; ASCII '0'
  LDI r15, 1           ; address increment

main_loop:
  ; Read TICKS
  LDI r1, 0xFFE
  LOAD r10, r1         ; r10 = current frame count

  ; We extract 4 digits: thousands, hundreds, tens, ones
  ; Starting from the leftmost digit (most significant)
  ; Canvas address starts at 0x8000 (row 0, col 0)
  LDI r14, 0x8003      ; start at rightmost position (col 3)

  ; Extract ones digit: r10 % 10
  MOV r11, r10
  MOD r11, r12         ; r11 = ones digit (0-9)
  ADD r11, r13         ; convert to ASCII
  STORE r14, r11       ; write to canvas col 3
  SUB r14, r15         ; move left to col 2

  ; Extract tens digit: (r10 / 10) % 10
  MOV r11, r10
  DIV r11, r12         ; r11 = r10 / 10
  MOV r18, r11         ; save quotient
  MOD r11, r12         ; r11 = tens digit (0-9)
  ADD r11, r13         ; convert to ASCII
  STORE r14, r11       ; write to canvas col 2
  SUB r14, r15         ; move left to col 1

  ; Extract hundreds digit: (r10 / 100) % 10
  MOV r11, r18         ; restore r10/10
  DIV r11, r12         ; r11 = r10 / 100
  MOV r18, r11         ; save quotient
  MOD r11, r12         ; r11 = hundreds digit (0-9)
  ADD r11, r13         ; convert to ASCII
  STORE r14, r11       ; write to canvas col 1
  SUB r14, r15         ; move left to col 0

  ; Extract thousands digit: (r10 / 1000) % 10
  MOV r11, r18         ; restore r10/100
  DIV r11, r12         ; r11 = r10 / 1000
  MOD r11, r12         ; r11 = thousands digit (0-9)
  ADD r11, r13         ; convert to ASCII
  STORE r14, r11       ; write to canvas col 0

  FRAME                ; display this frame
  JMP main_loop        ; repeat forever
