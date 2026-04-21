; taskbar.asm -- Desktop Taskbar for Geometry OS
;
; Phase 72: Persistent taskbar at bottom of screen with running process
; icons, frame counter (clock), and Start button.
;
; Proves: PROCLS (process listing), MOUSEQ (mouse input), HITSET/HITQ
;         (clickable regions), DRAWTEXT (text rendering), LS (file listing).
;
; RAM Layout:
;   0x4000-0x407F  Process PID buffer (from PROCLS)
;   0x4080-0x47FF  File list buffer (from LS)
;   0x5000-0x50FF  String scratch buffers
;   0x5100         Process count
;   0x5104         File count
;   0x5108         Start menu open (0=closed, 1=open)
;   0x510C         Scroll offset for file list
;   0x5110         Selected file index
;   0x5114         Tick counter backup
;
; Taskbar: y=240..255 (bottom 16 pixels)
; Start button: x=0..47, y=240..255
; Process info: x=48..200
; Clock: x=200..255

#define PID_BUF      0x4000
#define FILE_BUF     0x4080
#define STR_BUF      0x5000
#define PROC_COUNT   0x5100
#define FILE_COUNT   0x5104
#define MENU_OPEN    0x5108
#define SCROLL_OFF   0x510C
#define SEL_INDEX    0x5110
#define TICK_BAK     0x5114
#define TBAR_Y       240
#define TBAR_H       16

; ==========================================
; INIT
; ==========================================
LDI r30, 0xFD00           ; stack pointer
LDI r1, 1
LDI r7, 0                 ; zero constant

; Clear state
LDI r20, MENU_OPEN
STORE r20, r7
LDI r20, SCROLL_OFF
STORE r20, r7
LDI r20, SEL_INDEX
STORE r20, r7

; ==========================================
; MAIN LOOP
; ==========================================
main_loop:
  ; -- Draw desktop area (above taskbar) --
  LDI r0, 0x181828
  FILL r0

  ; -- Draw taskbar background --
  LDI r0, 0x2D2D3D        ; dark blue-gray
  LDI r10, 0
  LDI r11, TBAR_Y
  LDI r12, 256
  LDI r13, TBAR_H
  RECTF r10, r11, r12, r13, r0

  ; -- Draw Start button --
  LDI r0, 0x4444AA         ; blue
  LDI r10, 1
  LDI r11, TBAR_Y
  ADDI r11, 1
  LDI r12, 46
  LDI r13, 14
  RECTF r10, r11, r12, r13, r0

  ; Write "Start" to STR_BUF
  LDI r10, STR_BUF
  LDI r0, 83               ; S
  STORE r10, r0
  ADD r10, r1
  LDI r0, 116              ; t
  STORE r10, r0
  ADD r10, r1
  LDI r0, 97               ; a
  STORE r10, r0
  ADD r10, r1
  LDI r0, 114              ; r
  STORE r10, r0
  ADD r10, r1
  LDI r0, 116              ; t
  STORE r10, r0
  ADD r10, r1
  LDI r0, 0                ; null
  STORE r10, r0

  ; Draw Start text
  LDI r10, 8
  LDI r11, TBAR_Y
  ADDI r11, 4
  LDI r12, STR_BUF
  LDI r13, 0xFFFFFF
  LDI r14, 0
  DRAWTEXT r10, r11, r12, r13, r14

  ; -- Get running processes via PROCLS --
  LDI r10, PID_BUF
  PROCLS r10
  ; r0 = process count
  LDI r20, PROC_COUNT
  STORE r20, r0

  ; -- Draw process count on taskbar as "P:N" --
  LDI r10, STR_BUF
  LDI r0, 80               ; P
  STORE r10, r0
  ADD r10, r1
  LDI r0, 58               ; :
  STORE r10, r0
  ADD r10, r1
  ; Convert process count to ASCII digit
  LDI r20, PROC_COUNT
  LOAD r20, r20
  LDI r0, 48               ; 0
  ADD r0, r20
  STORE r10, r0
  ADD r10, r1
  LDI r0, 0
  STORE r10, r0

  LDI r10, 52
  LDI r11, TBAR_Y
  ADDI r11, 4
  LDI r12, STR_BUF
  LDI r13, 0x00FF00
  LDI r14, 0
  DRAWTEXT r10, r11, r12, r13, r14

  ; -- Draw frame counter (clock) at right side --
  ; TICKS / 60 = seconds, seconds / 60 = minutes
  LDI r20, 0xFFE
  LOAD r20, r20             ; TICKS
  LDI r21, 60
  DIV r20, r21              ; seconds
  LDI r21, 60
  DIV r20, r21              ; minutes
  LDI r21, 10
  MOD r20, r21              ; minutes mod 10
  LDI r0, 48
  ADD r0, r20               ; ASCII digit for minutes
  LDI r10, STR_BUF
  STORE r10, r0
  ADD r10, r1
  LDI r0, 58                ; :
  STORE r10, r0
  ADD r10, r1
  ; seconds
  LDI r20, 0xFFE
  LOAD r20, r20
  LDI r21, 60
  DIV r20, r21              ; total seconds
  LDI r21, 10
  MOD r20, r21              ; seconds mod 10
  LDI r0, 48
  ADD r0, r20
  STORE r10, r0
  ADD r10, r1
  LDI r0, 0
  STORE r10, r0

  LDI r10, 232
  LDI r11, TBAR_Y
  ADDI r11, 4
  LDI r12, STR_BUF
  LDI r13, 0xAAAAFF
  LDI r14, 0
  DRAWTEXT r10, r11, r12, r13, r14

  ; -- Register hit region for Start button --
  LDI r10, 1               ; x
  LDI r11, TBAR_Y
  ADDI r11, 1              ; y
  LDI r12, 46              ; w
  LDI r13, 14              ; h
  HITSET r10, r11, r12, r13, 1

  ; -- Check mouse --
  MOUSEQ r10                ; r10=mx, r11=my, r12=btn

  ; Check for click (btn==2)
  LDI r13, 2
  CMP r12, r13
  BNE_check:
  ; BNE not available -- use JNZ on difference
  MOV r15, r12
  SUB r15, r13              ; r15 = btn - 2
  JNZ r15, no_click

  ; Something was clicked -- check what
  HITQ r14                  ; r14 = hit region id

  ; Start button (id==1)?
  LDI r15, 1
  CMP r14, r15
  JNZ r0, check_menu_click

  ; Toggle menu
  LDI r20, MENU_OPEN
  LOAD r21, r20
  LDI r15, 1
  CMP r21, r15
  JNZ r0, open_menu
  ; Close menu
  LDI r0, 0
  STORE r20, r0
  JMP no_click

open_menu:
  LDI r0, 1
  STORE r20, r0
  ; Refresh file list
  LDI r10, FILE_BUF
  LS r10
  LDI r20, FILE_COUNT
  STORE r20, r0
  LDI r20, SCROLL_OFF
  LDI r0, 0
  STORE r20, r0
  LDI r20, SEL_INDEX
  STORE r20, r0
  JMP no_click

check_menu_click:
  ; Menu item (id >= 10)? If r14 < 10, skip
  LDI r15, 10
  CMP r14, r15
  BLT r0, no_click

  ; Close menu on selection
  LDI r20, MENU_OPEN
  LDI r0, 0
  STORE r20, r0
  JMP no_click

no_click:
  ; -- Draw menu if open --
  LDI r20, MENU_OPEN
  LOAD r20, r20
  LDI r15, 1
  CMP r20, r15
  JNZ r0, skip_menu

  ; Menu background
  LDI r0, 0x303050
  LDI r10, 0
  LDI r11, 144
  LDI r12, 120
  LDI r13, 96
  RECTF r10, r11, r12, r13, r0

  ; Menu border
  LDI r0, 0x6666AA
  LDI r10, 0
  LDI r11, 144
  LDI r12, 120
  LDI r13, 96
  RECT r10, r11, r12, r13, r0

  ; Menu title "Apps"
  LDI r10, STR_BUF
  LDI r0, 65                ; A
  STORE r10, r0
  ADD r10, r1
  LDI r0, 112               ; p
  STORE r10, r0
  ADD r10, r1
  LDI r0, 112               ; p
  STORE r10, r0
  ADD r10, r1
  LDI r0, 115               ; s
  STORE r10, r0
  ADD r10, r1
  LDI r0, 0
  STORE r10, r0
  LDI r10, 4
  LDI r11, 148
  LDI r12, STR_BUF
  LDI r13, 0xFFFFFF
  LDI r14, 0
  DRAWTEXT r10, r11, r12, r13, r14

  ; Draw file list entries (up to 4 visible)
  LDI r22, 0                ; visible entry counter
  LDI r23, 4                ; max visible
  LDI r24, FILE_BUF          ; filename ptr
  LDI r20, FILE_COUNT
  LOAD r20, r20             ; total files

draw_entries:
  CMP r22, r23
  BGE r0, entries_done
  CMP r20, r7
  JZ r0, entries_done

  ; Copy filename to STR_BUF
  LDI r25, STR_BUF
  LDI r26, 12               ; max chars
copy_name:
  LOAD r27, r24
  STORE r25, r27
  JZ r27, name_done
  ADD r24, r1
  ADD r25, r1
  SUBI r26, 1
  JZ r26, name_done
  JMP copy_name

name_done:
  ; Draw filename
  LDI r10, 4
  LDI r11, 164
  MOV r12, r22
  LDI r28, 14
  MUL r12, r28
  ADD r11, r12              ; y = 164 + entry * 14
  LDI r12, STR_BUF
  LDI r13, 0xCCCCFF
  LDI r14, 0
  DRAWTEXT r10, r11, r12, r13, r14

  ; Advance r24 past null
  LOAD r27, r24
  JNZ r27, no_adv
  ADD r24, r1
no_adv:

  ADDI r22, 1
  SUBI r20, 1
  JMP draw_entries

entries_done:
  ; Menu background hit region
  LDI r10, 0
  LDI r11, 144
  LDI r12, 120
  LDI r13, 96
  HITSET r10, r11, r12, r13, 5

skip_menu:
  FRAME
  JMP main_loop
