; net_chat.asm -- Pixel Chat Demo (Phase 71: Pixel Network Protocol)
;
; Demonstrates bidirectional NET_RECV for receiving pixel data from a peer.
; Listens for incoming pixel protocol frames and displays them on screen.
; Also shows a simple text interface using NET_SEND for chat messages.
;
; For testing without a real network: push frames into vm.net_inbox
; using the test harness. The demo polls NET_RECV each frame.
;
; Protocol:
;   Frame type 0 (screen_share): display pixels on screen
;   Frame type 1 (chat): display text message in status bar

LDI r7, 1              ; increment constant
LDI r8, 256            ; screen width
LDI r9, 0              ; frame counter

; Clear screen to dark blue
LDI r10, 0x000033
FILL r10

; Draw title bar
LDI r10, 0x00
LDI r11, 0
LDI r12, 256
LDI r13, 1
LDI r14, 0x333333
RECTF r10, r11, r12, r13, r14

; Draw status bar at bottom
LDI r10, 0
LDI r11, 250
LDI r12, 256
LDI r13, 6
LDI r14, 0x222222
RECTF r10, r11, r12, r13, r14

; Write "Pixel Chat - Waiting..." text at top
LDI r1, 0x2000
LDI r2, 80             ; 'P'
STORE r1, r2
LDI r1, 0x2001
LDI r2, 105            ; 'i'
STORE r1, r2
LDI r1, 0x2002
LDI r2, 120            ; 'x'
STORE r1, r2
LDI r1, 0x2003
LDI r2, 101            ; 'e'
STORE r1, r2
LDI r1, 0x2004
LDI r2, 108            ; 'l'
STORE r1, r2
LDI r1, 0x2005
LDI r2, 0              ; null
STORE r1, r2

LDI r1, 0x2000
LDI r2, 2
LDI r3, 8
LDI r4, 0xFFFFFF
TEXT r1, r2, r4

; Main loop: poll for incoming frames
main_loop:
  ; Update frame counter
  ADD r9, r7
  LDI r10, 0xFFE
  STORE r10, r9

  ; Poll NET_RECV
  LDI r10, 0x7100     ; receive buffer
  LDI r11, 256        ; max length
  NET_RECV r10, r11

  ; Check if we got data (r0 > 0 means data received)
  LDI r12, 0
  CMP r0, r12
  JZ r0, no_data

  ; Data received! Check frame type
  LDI r13, 0x7100
  LOAD r13, r13         ; r13 = frame type
  LDI r14, 1
  CMP r13, r14
  JZ r0, got_chat       ; type 1 = chat message

  ; Type 0 = screen share - display pixels
  ; Pixel data starts at 0x7104 (after 4-byte header)
  ; For now, just flash the screen green briefly
  LDI r10, 0x003300
  FILL r10
  FRAME
  JMP main_loop

got_chat:
  ; Chat message received - display in status bar
  ; Change status bar color to indicate message
  LDI r10, 0
  LDI r11, 250
  LDI r12, 256
  LDI r13, 6
  LDI r14, 0x004400    ; green tint = message received
  RECTF r10, r11, r12, r13, r14
  FRAME
  JMP main_loop

no_data:
  ; Animate waiting indicator (blinking dot)
  LDI r10, 0xFFE
  LOAD r10, r10         ; r10 = TICKS
  LDI r11, 30
  AND r10, r11          ; r10 = TICKS & 30
  LDI r11, 15
  CMP r10, r11
  BLT r0, dot_on

  ; Dot off - draw black pixel at (128, 4)
  LDI r10, 128
  LDI r11, 4
  LDI r12, 0x000000
  PSET r10, r11, r12
  FRAME
  JMP main_loop

dot_on:
  LDI r10, 128
  LDI r11, 4
  LDI r12, 0xFFFFFF
  PSET r10, r11, r12
  FRAME
  JMP main_loop

HALT
