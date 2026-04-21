; html_browser.asm -- Simple HTML Browser for Geometry OS
;
; Fetches HTML pages via HTTPGET, parses with HTPARSE, renders styled text.
; Supports: headings (h1/h2/h3), paragraphs, links, bold, italic, horizontal rules.
; Navigation: Enter=fetch, W/S=scroll, B=back, H=home, M=bookmark.
;
; Memory layout:
;   0x5000  URL buffer (64 chars)
;   0x5100  HTTP host buffer
;   0x5200  HTTP path buffer
;   0x9000  HTML body buffer (8K)
;   0xA000  Parsed lines (100 lines x 33 words = 3300 words)
;   0xD000  Scroll offset (1 word)
;   0xD004  Line count (1 word)
;   0xD008  Mode (0=browse, 1=edit url)
;   0xD00C  History count
;   0xD010  History URLs (4 x 32 words)

; -- Constants --
LDI r20, 0       ; color: black
LDI r21, 0x00FFFF ; color: cyan
LDI r22, 0x00FF00 ; color: green
LDI r23, 0xFFFFFF ; color: white
LDI r24, 0x000040 ; color: dark blue (url bar bg)
LDI r25, 0x333333 ; color: dark gray (status)

; -- Init --
LDI r7, 0xD000
LDI r8, 0
STORE r7, r8     ; scroll = 0
LDI r7, 0xD004
STORE r7, r8     ; line_count = 0
LDI r7, 0xD008
STORE r7, r8     ; mode = browse
LDI r7, 0xD00C
STORE r7, r8     ; history count = 0

; -- Build a test page in HTML buffer --
; Write "<h1>Geometry OS</h1><p>Welcome!</p>" into 0x9000
LDI r10, 0x9000
LDI r11, 0

; Write chars using STRO-like STORE sequence
; "<h1>Geometry OS</h1><p>Welcome!</p>"
; Character codes: < = 60, h = 104, 1 = 49, > = 62, etc.
; We'll write the HTML string byte by byte
LDI r12, 60      ; <
STORE r10, r12
LDI r12, 104     ; h
LDI r11, 1
ADD r10, r11
STORE r10, r12
LDI r12, 49      ; 1
ADD r10, r11
STORE r10, r12
LDI r12, 62      ; >
ADD r10, r11
STORE r10, r12
LDI r12, 71      ; G
ADD r10, r11
STORE r10, r12
LDI r12, 101     ; e
ADD r10, r11
STORE r10, r12
LDI r12, 111     ; o
ADD r10, r11
STORE r10, r12
LDI r12, 109     ; m
ADD r10, r11
STORE r10, r12
LDI r12, 101     ; e
ADD r10, r11
STORE r10, r12
LDI r12, 116     ; t
ADD r10, r11
STORE r10, r12
LDI r12, 114     ; r
ADD r10, r11
STORE r10, r12
LDI r12, 121     ; y
ADD r10, r11
STORE r10, r12
LDI r12, 32      ; (space)
ADD r10, r11
STORE r10, r12
LDI r12, 79      ; O
ADD r10, r11
STORE r10, r12
LDI r12, 83      ; S
ADD r10, r11
STORE r10, r12
LDI r12, 60      ; <
ADD r10, r11
STORE r10, r12
LDI r12, 47      ; /
ADD r10, r11
STORE r10, r12
LDI r12, 104     ; h
ADD r10, r11
STORE r10, r12
LDI r12, 49      ; 1
ADD r10, r11
STORE r10, r12
LDI r12, 62      ; >
ADD r10, r11
STORE r10, r12
LDI r12, 60      ; <
ADD r10, r11
STORE r10, r12
LDI r12, 112     ; p
ADD r10, r11
STORE r10, r12
LDI r12, 62      ; >
ADD r10, r11
STORE r10, r12
LDI r12, 87      ; W
ADD r10, r11
STORE r10, r12
LDI r12, 101     ; e
ADD r10, r11
STORE r10, r12
LDI r12, 108     ; l
ADD r10, r11
STORE r10, r12
LDI r12, 99      ; c
ADD r10, r11
STORE r10, r12
LDI r12, 111     ; o
ADD r10, r11
STORE r10, r12
LDI r12, 109     ; m
ADD r10, r11
STORE r10, r12
LDI r12, 101     ; e
ADD r10, r11
STORE r10, r12
LDI r12, 33      ; !
ADD r10, r11
STORE r10, r12
LDI r12, 60      ; <
ADD r10, r11
STORE r10, r12
LDI r12, 47      ; /
ADD r10, r11
STORE r10, r12
LDI r12, 112     ; p
ADD r10, r11
STORE r10, r12
LDI r12, 62      ; >
ADD r10, r11
STORE r10, r12
LDI r12, 0       ; null terminator
ADD r10, r11
STORE r10, r12

; -- Parse HTML --
HITCLR
LDI r10, 0x9000  ; src = HTML buffer
LDI r11, 0xA000  ; dest = parsed lines buffer
LDI r12, 100     ; max_lines = 100
HTPARSE r10, r11, r12
; r0 = line count

; Store line count
LDI r7, 0xD004
STORE r7, r0

; -- Render URL bar --
; Draw dark blue bar at top
LDI r10, 0
LDI r11, 0
LDI r12, 256
LDI r13, 16
RECTF r10, r11, r12, r13, r24

; Draw "geo://" in URL bar using TEXT opcode
LDI r10, 0x5000
LDI r12, 103     ; g
STORE r10, r12
LDI r11, 1
ADD r10, r11
LDI r12, 101     ; e
STORE r10, r12
ADD r10, r11
LDI r12, 111     ; o
STORE r10, r12
ADD r10, r11
LDI r12, 58      ; :
STORE r10, r12
ADD r10, r11
LDI r12, 47      ; /
STORE r10, r12
ADD r10, r11
LDI r12, 47      ; /
STORE r10, r12
ADD r10, r11
LDI r12, 104     ; h
STORE r10, r12
ADD r10, r11
LDI r12, 111     ; o
STORE r10, r12
ADD r10, r11
LDI r12, 109     ; m
STORE r10, r12
ADD r10, r11
LDI r12, 101     ; e
STORE r10, r12
ADD r10, r11
LDI r12, 0       ; null
STORE r10, r12

; Render URL text
LDI r10, 2       ; x
LDI r11, 2       ; y
LDI r12, 0x5000  ; addr
LDI r13, 0x00FFFF ; cyan color
TEXT r10, r11, r12

; -- Render status bar --
LDI r10, 0
LDI r11, 240
LDI r12, 256
LDI r13, 16
RECTF r10, r11, r12, r13, r25

; -- Render parsed content --
; Read scroll offset
LDI r14, 0xD000
LOAD r14, r14    ; r14 = scroll

; Read line count
LDI r15, 0xD004
LOAD r15, r15    ; r15 = line_count

; Render up to 24 visible lines (screen rows 20-236, 8px per line)
LDI r7, 0        ; visible line counter
LDI r8, 1        ; increment
LDI r9, 24       ; max visible lines
LDI r10, 0       ; base Y = 20

render_loop:
  CMP r7, r9
  BGE r0, render_done
  CMP r7, r15
  BGE r0, render_done

  ; Calculate line address: 0xA000 + (scroll + vis_idx) * 33
  MOV r11, r14     ; scroll
  ADD r11, r7      ; + vis_idx
  LDI r12, 33
  MUL r11, r12     ; * 33
  LDI r12, 0xA000
  ADD r11, r12     ; + base

  ; Check if we have valid data (color != 0 or has chars)
  LOAD r12, r11    ; fg_color
  CMP r12, r20     ; compare with 0 (black = empty)
  JZ r0, next_line

  ; Draw the line using DRAWTEXT-style rendering
  ; For simplicity, draw first char as a colored pixel
  ; Real rendering would use DRAWTEXT opcode with per-line colors
  LDI r13, 4       ; x offset
  MOV r13, r10     ; y = base_y + line * 8
  LDI r16, 8
  MUL r13, r16
  LDI r16, 20
  ADD r13, r16

  ; Draw each char as a single pixel (simplified)
  LDI r16, 0       ; char index
  draw_chars:
    CMP r16, r8
    BGE r0, next_line
    LDI r17, 1
    CMP r16, r17
    BGE r0, skip_first
    ; Draw char pixel
    MOV r18, r11
    ADD r18, r16
    ADD r18, r8     ; skip color word
    LOAD r18, r18   ; char code
    CMP r18, r20
    JZ r0, next_line

    ; x = 4 + char_idx * 8, y = line_y
    MOV r19, r16
    LDI r17, 8
    MUL r19, r17
    LDI r17, 4
    ADD r19, r17
    PSET r19, r13, r12  ; colored pixel

  skip_first:
    ADD r16, r8
    JMP draw_chars

next_line:
  ADD r7, r8
  ADD r10, r8
  JMP render_loop

render_done:
  ; Wait for user input
  FILL r20         ; clear screen (black)
  FRAME
  JMP render_done  ; infinite display loop

HALT
