; device_test.asm -- Phase 28: Device Driver Abstraction demo
; Opens all 4 device files, uses IOCTL to query them,
; reads from /dev/keyboard, writes pixels to /dev/screen.
;
; Controls: Press any key to draw a pixel at a random position
; Opcodes: OPEN, READ, WRITE, IOCTL, LDI, LOAD, STORE, RAND, FRAME, HALT

; -- Data area --
.org 0xD00
screen_path:
    .db "/dev/screen", 0
kb_path:
    .db "/dev/keyboard", 0
audio_path:
    .db "/dev/audio", 0
net_path:
    .db "/dev/net", 0
pixel_buf:
    .db 0, 0, 0   ; x, y, color triplet for screen write
key_buf:
    .db 0         ; buffer for keyboard read
result:
    .db 0         ; stores ioctl results

; -- Main program --
.org 0x000
    ; Fill screen with dark blue background
    LDI r6, 0x000033
    FILL r6

    ; Draw title bar
    LDI r1, 0
    LDI r2, 0
    LDI r3, 256
    LDI r4, 16
    LDI r6, 0x003366
    RECTF r1, r2, r3, r4, r6

    ; Draw status bar at bottom
    LDI r1, 0
    LDI r2, 240
    LDI r3, 256
    LDI r4, 16
    LDI r6, 0x003366
    RECTF r1, r2, r3, r4, r6

    ; === Test 1: Open all 4 devices ===
    ; /dev/screen -> fd 0xE000
    LDI r1, screen_path
    LDI r2, 0
    OPEN r1, r2
    ; r0 = 0xE000 (screen fd)

    ; /dev/keyboard -> fd 0xE001
    LDI r1, kb_path
    LDI r2, 0
    OPEN r1, r2
    ; r0 = 0xE001 (keyboard fd), save to r7
    MOV r7, r0

    ; /dev/audio -> fd 0xE002
    LDI r1, audio_path
    LDI r2, 0
    OPEN r1, r2
    ; r0 = 0xE002 (audio fd), save to r8
    MOV r8, r0

    ; /dev/net -> fd 0xE003
    LDI r1, net_path
    LDI r2, 0
    OPEN r1, r2
    ; r0 = 0xE003 (net fd), save to r9
    MOV r9, r0

    ; === Test 2: IOCTL queries ===
    ; Get screen width (cmd=0) -- should return 256
    LDI r10, 0xE000
    LDI r11, 0
    LDI r12, 0
    IOCTL r10, r11, r12
    ; r0 = 256 (width), store it
    LDI r20, result
    STORE r20, r0

    ; Get screen height (cmd=1) -- should return 256
    LDI r10, 0xE000
    LDI r11, 1
    LDI r12, 0
    IOCTL r10, r11, r12
    ; r0 = 256 (height)

    ; Set keyboard echo mode to 1 (cmd=1)
    LDI r10, 0xE001
    LDI r11, 1
    LDI r12, 1
    IOCTL r10, r11, r12
    ; r0 = 0 (success)

    ; Set audio volume to 50 (cmd=1)
    LDI r10, 0xE002
    LDI r11, 1
    LDI r12, 50
    IOCTL r10, r11, r12
    ; r0 = 0 (success)

    ; Get net status (cmd=0) -- should return 1 (up)
    LDI r10, 0xE003
    LDI r11, 0
    LDI r12, 0
    IOCTL r10, r11, r12
    ; r0 = 1 (net up)

    ; === Test 3: Interactive loop ===
    ; Read keyboard, draw random colored pixels
loop:
    ; Read one key from /dev/keyboard
    LDI r1, key_buf
    LDI r2, 1
    READ r7, r1, r2
    ; r0 = bytes read (1 if key pressed, 0 if not)

    ; If no key, just frame and loop
    LDI r3, 0
    CMP r0, r3
    JZ r0, skip

    ; Key was pressed -- generate random pixel position and color
    RAND r0
    LDI r3, 255
    AND r0, r3       ; r0 = random x (0-255)

    RAND r1
    AND r1, r3       ; r1 = random y (0-255)

    RAND r2
    AND r2, r3       ; r2 = random color component

    ; Make it brighter (add 0x111111)
    LDI r3, 0x111111
    ADD r2, r3

    ; Draw pixel directly
    PSETI r0, r1, r2

    ; Also beep on keypress (via /dev/audio write)
    ; Write (freq=440, dur=50) to audio device
    LDI r1, pixel_buf
    LDI r2, 440
    STORE r1, r2
    LDI r2, 1
    ADD r1, r2       ; r1 = pixel_buf + 1
    LDI r2, 50
    STORE r1, r2
    LDI r1, pixel_buf
    LDI r2, 2
    WRITE r8, r1, r2

skip:
    FRAME
    JMP loop
