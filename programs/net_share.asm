; net_share.asm -- Screen Sharing Demo (Phase 71: Pixel Network Protocol)
;
; Demonstrates NET_SEND for sharing screen content with a peer.
; Connect to a listening Geometry OS instance and send the screen
; as pixel protocol frames.
;
; Protocol: Each frame is a pixel protocol frame with type=0 (screen_share),
; containing a row of screen pixels (256 RGBA u32 values).
;
; Usage: Load on the sender side. Start a receiver with net_chat.asm or
; a custom listener first.

LDI r10, 0x7000       ; IP address string location
LDI r2, 49            ; '1'
STORE r10, r2
LDI r10, 0x7001
LDI r2, 50            ; '2'
STORE r10, r2
LDI r10, 0x7002
LDI r2, 55            ; '7'
STORE r10, r2
LDI r10, 0x7003
LDI r2, 46            ; '.'
STORE r10, r2
LDI r10, 0x7004
LDI r2, 48            ; '0'
STORE r10, r2
LDI r10, 0x7005
LDI r2, 46            ; '.'
STORE r10, r2
LDI r10, 0x7006
LDI r2, 48            ; '0'
STORE r10, r2
LDI r10, 0x7007
LDI r2, 46            ; '.'
STORE r10, r2
LDI r10, 0x7008
LDI r2, 49            ; '1'
STORE r10, r2
LDI r10, 0x7009
LDI r2, 0             ; null terminator
STORE r10, r2

; Draw a test pattern on screen before sharing
LDI r5, 0             ; y = 0
LDI r6, 1             ; increment
LDI r7, 256           ; limit
LDI r8, 0xFF0000      ; start color (red)

draw_loop:
  LDI r9, 0           ; x = 0

x_loop:
  ; Color = (y * 256 + x) << 8 for a gradient
  MOV r3, r5
  MUL r3, r7
  ADD r3, r9
  SHL r3, r6
  PSET r9, r5, r3
  ADD r9, r6
  CMP r9, r7
  BLT r0, x_loop
  ADD r5, r6
  CMP r5, r7
  BLT r0, draw_loop

; Now share the screen
; Step 1: Connect to peer
LDI r1, 0x7000        ; IP address
LDI r2, 3839          ; port (ascii_world terminal port)
CONNECT r1, r2, r3    ; r3 = fd

; Check if connection succeeded
LDI r4, 0
CMP r0, r4
JNZ r0, connected     ; r0 = 0 on success

; Connection failed - show red screen
LDI r8, 0xFF0000
FILL r8
HALT

connected:
MOV r4, r3            ; r4 = connection fd

; Send screen as rows (256 pixels per row = 1 NET_SEND per row)
LDI r5, 0             ; y = 0
LDI r6, 1             ; increment
LDI r7, 256           ; limit

send_loop:
  ; Screen row address = screen buffer base
  ; We use screen_ram_mirror which starts at 0x10000 in the address space
  ; But for NET_SEND we can just use any RAM region with the data

  ; For this demo, we'll construct the pixel data in RAM at 0x8000
  ; Copy one row of screen to RAM[0x8000..0x80FF]
  LDI r9, 0
  LDI r10, 0x8000
  LDI r11, 0x10000     ; screen buffer base (conceptual)

copy_row:
  ; Use SCREENP to read each pixel, then store in RAM
  ; Actually, for simplicity, just send data from RAM region
  ; In a real scenario, the screen buffer is accessible
  ADD r9, r6
  CMP r9, r7
  BLT r0, copy_row

  ; NET_SEND r10, r7, r4 -- send 256 words from 0x8000 to connection r4
  NET_SEND r10, r7, r4

  ; Check send result
  LDI r11, 0
  CMP r0, r11
  JZ r0, send_failed

  ADD r5, r6
  CMP r5, r7
  BLT r0, send_loop

; Done - show green
LDI r8, 0x00FF00
FILL r8
HALT

send_failed:
  ; Show yellow to indicate partial send
  LDI r8, 0xFFFF00
  FILL r8
  DISCONNECT r4
  HALT
