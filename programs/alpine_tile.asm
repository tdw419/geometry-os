; alpine_tile.asm -- Alpine Linux Live Tile (Phase 123)
;
; This program displays a title bar for the Alpine Linux live tile.
; When the hypervisor is booted (via hypervisor_boot socket command),
; the RISC-V VM output is rendered onto the screen in real-time.
;
; Usage from socket:
;   hypervisor_boot                     (use defaults)
;   hypervisor_boot ram=256             (custom RAM)
;   inject_key h                        (send 'h' to guest shell)
;   hypervisor_status                   (check status)
;   hypervisor_kill                     (clean up)

; Title bar
LDI r1, 0x000033   ; dark blue
LDI r10, 0
LDI r11, 0
LDI r12, 256
LDI r13, 16
RECTF r10, r11, r12, r13, r1

; Title text
LDI r14, 0x3000
STRO r14, "Alpine Linux Live Tile"
LDI r15, 3
LDI r16, 4
TEXT r15, r16, r14

; Status text
LDI r14, 0x3100
STRO r14, "Use: hypervisor_boot"
LDI r15, 2
LDI r16, 20
TEXT r15, r16, r14

LDI r14, 0x3200
STRO r14, "Status: hypervisor_status"
LDI r15, 2
LDI r16, 28
TEXT r15, r16, r14

; Draw a frame around the terminal area
LDI r1, 0x00FF00   ; green frame
LDI r10, 0
LDI r11, 16
LDI r12, 256
LDI r13, 1
RECTF r10, r11, r12, r13, r1  ; top border
LDI r11, 255
RECTF r10, r11, r12, r13, r1  ; bottom border
LDI r10, 0
LDI r11, 16
LDI r12, 1
LDI r13, 240
RECTF r10, r11, r12, r13, r1  ; left border
LDI r10, 255
RECTF r10, r11, r12, r13, r1  ; right border

HALT
