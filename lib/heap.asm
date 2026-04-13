; Geometry OS Heap Allocator -- lib/heap.asm
; Simple bump allocator with header tracking.
;
; Usage: .include "heap.asm" in your program
;
; Memory layout:
;   0xC000: _heap_ptr (next free address, u32, initialized by _lib_heap_init)
;   0xC004-0xDFFF: heap data region (16KB - 4 bytes = 16380 words)
;
; Block layout: [size][magic][data...data]
;   size  = number of data words (not including header)
;   magic = 0xDEAD when allocated
;
; Call _lib_heap_init once before any allocation.

; ── _lib_heap_init ─────────────────────────────────────────────────
; Input:  none
; Output: none
; Clobbers: r0, r1
_lib_heap_init:
  LDI r0, 0xC004       ; first free address (after ptr word + 3 padding)
  LDI r1, 0xC000
  STORE r1, r0         ; RAM[0xC000] = 0xC004
  RET

; ── _lib_heap_alloc ────────────────────────────────────────────────
; Allocate N words from the heap.
; Input:  r0 = number of u32 words to allocate (must be > 0)
; Output: r0 = pointer to data area, or 0 on failure (out of memory)
; Clobbers: r1, r4, r5
_lib_heap_alloc:
  JZ r0, _lib_ha_fail  ; size 0 = fail

  MOV r5, r0           ; r5 = requested size

  ; Read heap_ptr from 0xC000
  LDI r4, 0xC000
  LOAD r4, r4           ; r4 = current heap_ptr

  ; new_end = heap_ptr + 2 (header) + size
  LDI r0, 2
  ADD r0, r5           ; r0 = 2 + size
  ADD r0, r4           ; r0 = new_end

  ; Save old heap_ptr for return value
  PUSH r4              ; push old heap_ptr

  ; Bounds check: new_end must be <= 0xE000
  ; Subtract: if new_end > 0xE000, underflow wraps to huge number
  ; Use: new_end - 0xE000. If result is 0 or small, OK. If huge, fail.
  ; But we can't easily check "huge" without CMP clobbering.
  ; Practical approach: use CMP on the end address vs limit.
  ; We have end in r0. Save it, load limit, CMP.
  PUSH r0              ; push new_end
  LDI r0, 0xE000       ; limit
  POP r1               ; r1 = new_end
  CMP r1, r0           ; r1 = -1/0/1 (new_end vs limit)
  ; If r1 == 1, new_end > limit => fail
  LDI r0, 1
  CMP r1, r0
  JZ r1, _lib_ha_ofail ; out of memory

  ; Success: write header and update ptr
  POP r4               ; r4 = old heap_ptr
  ; Write size at old_ptr
  STORE r4, r5         ; RAM[old_ptr] = size
  ; Write magic at old_ptr + 1
  LDI r0, 1
  ADD r0, r4           ; r0 = old_ptr + 1
  LDI r1, 0xDEAD
  STORE r0, r1         ; RAM[old_ptr+1] = 0xDEAD

  ; Update heap_ptr = old_ptr + 2 + size
  LDI r0, 0xC000
  LDI r1, 2
  ADD r1, r5           ; r1 = 2 + size
  ADD r4, r1           ; r4 = new heap_ptr
  STORE r0, r4

  ; Return data pointer = old_heap_ptr + 2
  LDI r0, 0xC000
  LOAD r0, r0           ; r0 = new heap_ptr
  LDI r1, 2
  ADD r1, r5
  SUB r0, r1           ; r0 = old_heap_ptr
  LDI r1, 2
  ADD r0, r1           ; r0 = data pointer
  RET

_lib_ha_ofail:
  POP r4               ; clean up stack
_lib_ha_fail:
  LDI r0, 0
  RET

; ── _lib_heap_free ─────────────────────────────────────────────────
; Note: This is a bump allocator. Free is a no-op.
; For a teaching OS, this is acceptable. A real free-list allocator
; would be significantly more complex.
; Input:  r0 = pointer (ignored)
; Output: none
_lib_heap_free:
  RET

; ── _lib_heap_available ────────────────────────────────────────────
; Returns the number of free words remaining in the heap.
; Input:  none
; Output: r0 = free words remaining
; Clobbers: r1
_lib_heap_available:
  LDI r0, 0xC000
  LOAD r0, r0           ; r0 = heap_ptr
  LDI r1, 0xE000
  SUB r1, r0           ; r1 = 0xE000 - heap_ptr
  MOV r0, r1
  RET
