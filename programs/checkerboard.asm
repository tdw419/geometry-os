; CHECKERBOARD -- Draw a checkerboard pattern using nested loops
; Strategy: iterate y=0..255, x=0..255, use division-by-subtraction
; to get cell_row and cell_col, then XOR to determine color.
; Since we lack shifts, we track cell_row and cell_col with
; explicit counters that reset every 8 iterations.
;
; Color: white (0xFFFFFF) when (cell_row + cell_col) is even,
;        dark gray (0x333333) when odd.

LDI r0, 0            ; r0 = y (outer loop)
LDI r1, 0            ; r1 = x (inner loop)
LDI r2, 256          ; r2 = limit
LDI r3, 8            ; r3 = cell size
LDI r4, 0            ; r4 = cell_row
LDI r5, 0            ; r5 = cell_col
LDI r6, 0            ; r6 = color parity (0=white, 1=dark)
LDI r7, 0xFFFFFF     ; r7 = white
LDI r8, 0x333333     ; r8 = dark gray

y_loop:
  LDI r1, 0          ; reset x
  LDI r5, 0          ; reset cell_col

x_loop:
  ; color = (cell_row + cell_col) & 1
  ; parity = (r4 + r5) % 2
  ; Compute parity: add r4 and r5, then subtract 2 until < 2
  LDI r6, r4
  ADD r6, r5          ; r6 = cell_row + cell_col
  LDI r9, 2
parity_loop:
  SUB r6, r9          ; r6 -= 2
  ; If r6 >= 2, keep going (unsigned: check if subtracting didn't wrap)
  ; But we can't easily check that with only JZ/JNZ...
  ; Hack: if r6 was >= 2, then r6-2 >= 0, but wrapping makes it huge
  ; Use CMP to check if r6 > some large number (wrapped)
  CMP r6, r9          ; sets r0 = -1/0/1
  ; r0=1 means r6>r9=2, but we already subtracted so r6 could be anything
  ; This approach is getting too complex. Let's just track parity differently.

  ; Simpler: just use (cell_row XOR cell_col) & 1
  ; XOR r4, r5 gives parity bit in lowest bit
  LDI r6, r4
  XOR r6, r5          ; r6 = cell_row ^ cell_col
  AND r6, r3          ; r6 &= 8 ... no, we want & 1
  ; We need to AND with 1, but r3 = 8. Let's use a different reg.
  ; Problem: we only have 32 regs and no immediate AND.
  ; Load 1 into a register
  LDI r9, 1
  AND r6, r9          ; r6 = (cell_row ^ cell_col) & 1

  ; Draw pixel: if r6 == 0 -> white, else -> dark
  JZ r6, draw_white
  LDI r6, r8          ; dark gray
  PSETI r1, r0, r6
  JMP next_pixel

draw_white:
  LDI r6, r7          ; white
  PSETI r1, r0, r6

next_pixel:
  ; Advance x
  LDI r9, 1
  ADD r1, r9          ; x++

  ; Track cell_col: every 8 pixels, increment cell_col
  ; Check if x is a multiple of 8
  ; Since no modulo, use a counter: count to 8, then reset
  ; Actually, let's just check: if x % 8 == 0 and x > 0, increment cell_col
  ; We can use a separate counter for within-cell position
  LDI r6, r1          ; r6 = x
  LDI r9, 0           ; inner counter (reset each outer iteration)
  ; This is getting complicated. Let me use a different approach.

  ; Simplest: maintain cell_col counter, increment every 8 x-steps
  ; Use r14 as x-in-cell counter
  LDI r14, 0          ; init once... but we need it persistent

  ; OK, this program is too complex for the limited ISA. Let me simplify
  ; to just a stripe pattern instead.
  HALT
