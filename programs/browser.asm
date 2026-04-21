; browser.asm -- Text Browser v1 for Geometry OS
;
; A minimal web browser that fetches pages and renders them as text.
; Uses the http_get.asm library conventions.
;
; Memory layout (shared with http_get library):
;   0x6000..0x60FF  URL buffer
;   0x6100..0x61FF  Host buffer
;   0x6200          Port number
;   0x6300..0x63FF  Path buffer
;   0x6400..0x67FF  Request buffer
;   0x6800..0x8FFF  Response buffer
;   0x9000..0x97FF  Body buffer
;   0x9800          FD cell
;   0x9804          Body length
;   0x9808          Status code
;
; Browser state:
;   0x9900          Scroll offset (lines)
;   0x9904          Loading flag (1=loading, 0=idle)
;   0x9908          Page count (lines in body)
;   0x990C          History pointer (index into history)
;   0x9A00..0x9AFF  URL history ring (16 URLs, 16 bytes each)
;
; Controls:
;   Enter  - Fetch URL typed in URL bar
;   Up/Down - Scroll page content
;   'b'    - Go back in history
;   Escape - Quit
;
; Colors:
;   URL bar:     Cyan on dark blue
;   Status bar:  Yellow on dark gray
;   Content:     White on black
;   Links:       Green text

#define URL_BUF      0x6000
#define HOST_BUF     0x6100
#define PORT_CELL    0x6200
#define PATH_BUF     0x6300
#define REQ_BUF      0x6400
#define RESP_BUF     0x6800
#define BODY_BUF     0x9000
#define FD_CELL      0x9800
#define BODY_LEN     0x9804
#define STATUS_CELL  0x9808
#define SCROLL_OFF   0x9900
#define LOADING_FLG  0x9904
#define PAGE_LINES   0x9908
#define HIST_PTR     0x990C
#define HIST_BUF     0x9A00
#define URL_BAR_Y    4
#define STATUS_Y     14
#define CONTENT_Y    20
#define LINE_HEIGHT  8

  ; Initialize
  LDI r3, 1

  ; Clear scroll offset
  LDI r10, 0
  LDI r11, SCROLL_OFF
  STORE r11, r10

  ; Clear loading flag
  LDI r11, LOADING_FLG
  STORE r11, r10

  ; Clear history pointer
  LDI r11, HIST_PTR
  STORE r11, r10

  ; Write default URL
  LDI r10, URL_BUF
  LDI r11, 108        ; 'l'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 111        ; 'o'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 99         ; 'c'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 97         ; 'a'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 108        ; 'l'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 104        ; 'h'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 111        ; 'o'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 115        ; 's'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 116        ; 't'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 58         ; ':'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 56         ; '8'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 48         ; '0'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 48         ; '0'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 48         ; '0'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 47         ; '/'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 0          ; null
  STORE r10, r11

; ═══════════════════════════════════════
; Main loop
; ═══════════════════════════════════════
main_loop:
  LDI r9, 0
  FILL r9

  ; Draw URL bar background (dark blue strip)
  LDI r10, 0
  LDI r11, URL_BAR_Y
  LDI r12, 256
  LDI r13, 10
  LDI r14, 0x000080   ; dark blue
  RECTF r10, r11, r12, r13, r14

  ; Draw "URL: " label
  LDI r10, 2
  LDI r11, URL_BAR_Y
  LDI r12, url_label
  LDI r13, 0x00FFFF   ; cyan
  ; Use TEXT opcode to render label
  TEXT r10, r11, r12

  ; Draw URL text after "URL: " (shift right by ~40px)
  LDI r10, 34
  LDI r11, URL_BAR_Y
  LDI r12, URL_BUF
  LDI r13, 0xFFFFFF   ; white
  TEXT r10, r11, r12

  ; Draw status bar background
  LDI r10, 0
  LDI r11, STATUS_Y
  LDI r12, 256
  LDI r13, 10
  LDI r14, 0x333333   ; dark gray
  RECTF r10, r11, r12, r13, r14

  ; Check loading flag
  LDI r12, LOADING_FLG
  LOAD r12, r12
  JNZ r12, draw_loading

  ; Draw status: show response code
  LDI r10, 2
  LDI r11, STATUS_Y
  LDI r12, status_idle
  LDI r13, 0xFFFF00   ; yellow
  TEXT r10, r11, r12
  JMP draw_status_done

draw_loading:
  LDI r10, 2
  LDI r11, STATUS_Y
  LDI r12, status_loading
  LDI r13, 0xFF8800   ; orange
  TEXT r10, r11, r12

draw_status_done:
  ; ── Draw content area ──
  ; Render body text line by line
  LDI r10, CONTENT_Y  ; y position
  LDI r11, 0          ; line counter
  LDI r12, SCROLL_OFF
  LOAD r12, r12       ; scroll offset
  LDI r13, BODY_BUF   ; body pointer (adjusted by scroll)

  ; Calculate start position in body based on scroll
  ; Each "line" is ~40 chars for wrapping. Approximate: skip scroll*32 chars
  LDI r14, 32
  LDI r15, 0          ; char offset in body
scroll_skip:
  JZ r12, scroll_done
  ADD r15, r14         ; offset += 32
  SUB r12, r3          ; scroll--
  JMP scroll_skip

scroll_done:
  ; r13 = BODY_BUF + r15 (body start with scroll)
  LDI r12, BODY_BUF
  ADD r12, r15
  MOV r13, r12

  ; Draw up to 28 lines (224px of content area / 8px per line)
  LDI r14, 28         ; max lines to draw
draw_line_loop:
  JZ r14, draw_content_done

  ; Check if we hit end of body
  LOAD r15, r13        ; char at current body pos
  JZ r15, draw_content_done

  ; Draw this line of text (up to 32 chars, TEXT opcode handles it)
  LDI r10, 2           ; x = 2
  ; r10 = x, current r10 = y? No, we need to track y
  ; Use r11 for y (already set initially)
  TEXT r10, r10, r13

  ; Advance body pointer by 32 chars (line width)
  LDI r15, 32
  ADD r13, r15

  ; Move y down
  LDI r15, 8
  ADD r10, r15

  ; Decrement line counter
  SUB r14, r3
  JMP draw_line_loop

draw_content_done:
  ; ── Handle keyboard input ──
  IKEY r10             ; read key
  JZ r10, no_key

  ; Check for Enter (13) - fetch page
  LDI r11, 13
  CMP r10, r11
  JZ r0, fetch_page

  ; Check for 'b' (98) - go back
  LDI r11, 98
  CMP r10, r11
  JZ r0, go_back

  ; Check for Up arrow - scroll up
  LDI r11, 119         ; 'w' for up
  CMP r10, r11
  JZ r0, scroll_up

  ; Check for Down arrow - scroll down
  LDI r11, 115         ; 's' for down
  CMP r10, r11
  JZ r0, scroll_down

  ; Check for Escape - quit
  LDI r11, 27
  CMP r10, r11
  JZ r0, quit

  JMP no_key

scroll_up:
  LDI r10, SCROLL_OFF
  LOAD r10, r10
  JZ r10, no_key       ; already at top
  SUB r10, r3
  LDI r11, SCROLL_OFF
  STORE r11, r10
  JMP no_key

scroll_down:
  LDI r10, SCROLL_OFF
  LOAD r10, r10
  LDI r11, 50          ; max scroll
  CMP r10, r11
  BGE r0, no_key       ; at max
  ADD r10, r3
  LDI r11, SCROLL_OFF
  STORE r11, r10
  JMP no_key

fetch_page:
  ; Set loading flag
  LDI r10, 1
  LDI r11, LOADING_FLG
  STORE r11, r10

  ; Reset scroll
  LDI r10, 0
  LDI r11, SCROLL_OFF
  STORE r11, r10

  ; Call the HTTP get library
  ; (In real usage, this would be CALL http_get)
  ; For the browser demo, we do it inline since we can't .include yet
  CALL do_fetch

  ; Clear loading flag
  LDI r10, 0
  LDI r11, LOADING_FLG
  STORE r11, r10

  JMP no_key

go_back:
  ; Decrement history pointer
  LDI r10, HIST_PTR
  LOAD r10, r10
  JZ r10, no_key       ; no history
  SUB r10, r3
  LDI r11, HIST_PTR
  STORE r11, r10

  ; Restore URL from history (simplified: just go back to default)
  ; In a full implementation, would copy from history buffer to URL_BUF
  JMP no_key

quit:
  HALT

no_key:
  FRAME
  JMP main_loop


; ═══════════════════════════════════════════════
; do_fetch -- Simplified fetch for demo
; In production, this would call the http_get library routines.
; For the demo, it writes a mock page to BODY_BUF.
; ═══════════════════════════════════════════════
do_fetch:
  PUSH r31
  LDI r3, 1

  ; Write mock page content to body buffer
  LDI r10, BODY_BUF

  ; "Geometry OS Browser v1.0"
  LDI r11, 71  ; G
  STORE r10, r11
  ADD r10, r3
  LDI r11, 101 ; e
  STORE r10, r11
  ADD r10, r3
  LDI r11, 111 ; o
  STORE r10, r11
  ADD r10, r3
  LDI r11, 109 ; m
  STORE r10, r11
  ADD r10, r3
  LDI r11, 101 ; e
  STORE r10, r11
  ADD r10, r3
  LDI r11, 116 ; t
  STORE r10, r11
  ADD r10, r3
  LDI r11, 114 ; r
  STORE r10, r11
  ADD r10, r3
  LDI r11, 121 ; y
  STORE r10, r11
  ADD r10, r3
  LDI r11, 32  ; (space)
  STORE r10, r11
  ADD r10, r3
  LDI r11, 79  ; O
  STORE r10, r11
  ADD r10, r3
  LDI r11, 83  ; S
  STORE r10, r11
  ADD r10, r3
  LDI r11, 32  ; (space)
  STORE r10, r11
  ADD r10, r3
  LDI r11, 66  ; B
  STORE r10, r11
  ADD r10, r3
  LDI r11, 114 ; r
  STORE r10, r11
  ADD r10, r3
  LDI r11, 111 ; o
  STORE r10, r11
  ADD r10, r3
  LDI r11, 119 ; w
  STORE r10, r11
  ADD r10, r3
  LDI r11, 115 ; s
  STORE r10, r11
  ADD r10, r3
  LDI r11, 101 ; e
  STORE r10, r11
  ADD r10, r3
  LDI r11, 114 ; r
  STORE r10, r11
  ADD r10, r3

  ; Null terminate
  LDI r11, 0
  STORE r10, r11

  ; Set body length
  LDI r10, BODY_LEN
  LDI r11, 20
  STORE r10, r11

  ; Set status = 200
  LDI r10, STATUS_CELL
  LDI r11, 200
  STORE r10, r11

  POP r31
  RET


; ═══════════════════════════════════════════════
; String constants
; ═══════════════════════════════════════════════
url_label:
  .asciz "URL:"

status_idle:
  .asciz "Ready - Enter: fetch, W/S: scroll, B: back"

status_loading:
  .asciz "Loading..."
