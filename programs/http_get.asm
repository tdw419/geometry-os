; http_get.asm -- HTTP Client Library for Geometry OS
;
; Provides subroutines for fetching web pages over HTTP/1.0.
; Uses the TCP networking opcodes (CONNECT, SOCKSEND, SOCKRECV, DISCONNECT).
;
; Memory layout:
;   0x6000..0x60FF  URL buffer (null-terminated string, e.g. "example.com:80/index.html")
;   0x6100..0x61FF  Host buffer (parsed from URL, null-terminated)
;   0x6200          Port number (u32, default 80)
;   0x6300..0x63FF  Path buffer (parsed from URL, null-terminated, e.g. "/index.html")
;   0x6400..0x67FF  HTTP request buffer (GET request string)
;   0x6800..0x8FFF  HTTP response buffer (up to 10K)
;   0x9000..0x97FF  Body buffer (response body after header strip)
;   0x9800          Connection fd (u32)
;   0x9804          Body length (u32)
;   0x9808          Response status code (u32, e.g. 200)
;
; Usage:
;   1. Write URL string to RAM starting at 0x6000
;   2. CALL url_parse        -- parses host/port/path from URL
;   3. CALL http_connect     -- TCP connect to host:port
;   4. CALL http_send_get    -- sends "GET /path HTTP/1.0\r\nHost: host\r\n\r\n"
;   5. CALL http_recv_response -- reads response into response buffer
;   6. CALL http_strip_headers -- separates body from headers
;
; Register conventions:
;   r10-r14  -- subroutine arguments/temp (caller-saved)
;   r15-r19  -- subroutine local vars (caller-saved)
;   r20-r29  -- preserved across calls if needed (callee-saved by convention)
;   r30 = SP (grows down from 0xFF00)
;   r31 = LR (link register)
;   r0  = CMP result (DO NOT USE as general register!)

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
#define RESP_MAX     10240

; ═══════════════════════════════════════════════
; url_parse -- Parse URL into host, port, path
;
; Input:  URL string at URL_BUF (0x6000)
; Output: HOST_BUF (0x6100), PORT_CELL (0x6200), PATH_BUF (0x6300)
;
; Supported formats:
;   "host/path"           -> host, port=80, "/path"
;   "host:port/path"      -> host, port, "/path"
;   "host"                -> host, port=80, "/"
;   "host:port"           -> host, port, "/"
; ═══════════════════════════════════════════════
url_parse:
  PUSH r31

  ; Set constants FIRST
  LDI r3, 1           ; increment constant

  ; Initialize port to 80 (default)
  LDI r10, 80
  LDI r11, PORT_CELL
  STORE r11, r10

  ; Initialize path to "/" followed by null
  LDI r11, PATH_BUF
  LDI r10, 47         ; '/'
  STORE r11, r10
  LDI r10, 0
  ADD r11, r3         ; r11 = PATH_BUF + 1
  STORE r11, r10      ; path[1] = null

  ; ── Phase 1: Copy host part until ':' or '/' or null ──
  LDI r10, URL_BUF    ; src = URL start
  LDI r11, HOST_BUF   ; dst = host buffer
  LDI r12, 0          ; host_len = 0

url_parse_host_loop:
  ; Load char from URL
  LOAD r13, r10       ; r13 = url[pos]

  ; Check null -- end of URL
  JZ r13, url_parse_done

  ; Check '/' -- start of path
  LDI r14, 47         ; '/'
  CMP r13, r14
  JZ r0, url_parse_path_start

  ; Check ':' -- start of port
  LDI r14, 58         ; ':'
  CMP r13, r14
  JZ r0, url_parse_port

  ; Copy char to host buffer
  STORE r11, r13
  ADD r10, r3          ; src++
  ADD r11, r3          ; dst++
  ADD r12, r3          ; host_len++
  JMP url_parse_host_loop

url_parse_port:
  ; Skip the ':'
  ADD r10, r3

  ; Null-terminate host
  LDI r13, 0
  STORE r11, r13

  ; Parse port number
  LDI r11, 0          ; port = 0

url_parse_port_loop:
  LOAD r13, r10
  JZ r13, url_parse_port_done

  ; Check '/' -- start of path
  LDI r14, 47
  CMP r13, r14
  JNZ r0, url_parse_port_digit

  ; Found '/' after port -- save port first, then copy path
  LDI r14, PORT_CELL
  STORE r14, r11
  JMP url_parse_path_start

url_parse_port_digit:
  ; port = port * 10 + (char - '0')
  LDI r14, 10
  MUL r11, r14        ; port *= 10
  LDI r14, 48         ; '0'
  SUB r13, r14        ; digit = char - '0'
  ADD r11, r13        ; port += digit

  ADD r10, r3
  JMP url_parse_port_loop

url_parse_port_done:
  ; Store port
  LDI r13, PORT_CELL
  STORE r13, r11
  JMP url_parse_done

url_parse_path_start:
  ; Null-terminate host (safe even if already done)
  ; r11 might be host dst ptr or port value -- use r12 (host_len) instead
  LDI r13, HOST_BUF
  ADD r13, r12         ; r13 = HOST_BUF + host_len
  LDI r14, 0
  STORE r13, r14       ; null-terminate host

  ; Copy remaining URL to path buffer
  LDI r11, PATH_BUF   ; dst = path buffer

url_parse_path_loop:
  LOAD r13, r10       ; char from URL
  JZ r13, url_parse_done
  STORE r11, r13      ; copy to path
  ADD r10, r3
  ADD r11, r3
  JMP url_parse_path_loop

url_parse_done:
  POP r31
  RET


; ═══════════════════════════════════════════════
; http_connect -- Connect to the parsed host:port
;
; Input:  HOST_BUF, PORT_CELL
; Output: FD_CELL = connection fd, r0 = 0 on success
; ═══════════════════════════════════════════════
http_connect:
  PUSH r31

  LDI r10, HOST_BUF   ; addr_reg = pointer to host string
  LDI r11, PORT_CELL
  LOAD r11, r11       ; port_reg = port number

  ; We need to load host address and port into registers
  ; CONNECT addr_reg, port_reg, fd_reg
  ; But CONNECT reads from RAM[addr_reg] as IP string
  ; We need a temp register for the fd output
  LDI r12, FD_CELL    ; will store fd here

  ; CONNECT r10, r11, r12  -- but we need to use register numbers
  ; r10=addr, r11=port, r12=fd_out
  CONNECT r10, r11, r12

  ; Store fd to RAM
  LDI r13, FD_CELL
  STORE r13, r12

  POP r31
  RET


; ═══════════════════════════════════════════════
; http_send_get -- Build and send HTTP GET request
;
; Input:  HOST_BUF, PATH_BUF, FD_CELL
; Output: Request sent via TCP, r0 = 0 on success
;
; Builds: "GET /path HTTP/1.0\r\nHost: host\r\n\r\n"
; ═══════════════════════════════════════════════
http_send_get:
  PUSH r31

  ; Build request string in REQ_BUF (0x6400)
  LDI r10, REQ_BUF    ; dst = request buffer
  LDI r3, 1           ; increment

  ; ── "GET " ──
  LDI r11, 71         ; 'G'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 69         ; 'E'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 84         ; 'T'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 32         ; ' '
  STORE r10, r11
  ADD r10, r3

  ; ── Copy path ──
  LDI r11, PATH_BUF
http_send_path_loop:
  LOAD r12, r11       ; char from path
  JZ r12, http_send_path_done
  STORE r10, r12
  ADD r10, r3
  ADD r11, r3
  JMP http_send_path_loop

http_send_path_done:
  ; ── " HTTP/1.0\r\nHost: " ──
  ; ' '
  LDI r11, 32
  STORE r10, r11
  ADD r10, r3
  ; 'H'
  LDI r11, 72
  STORE r10, r11
  ADD r10, r3
  ; 'T'
  LDI r11, 84
  STORE r10, r11
  ADD r10, r3
  ; 'T'
  LDI r11, 84
  STORE r10, r11
  ADD r10, r3
  ; 'P'
  LDI r11, 80
  STORE r10, r11
  ADD r10, r3
  ; '/'
  LDI r11, 47
  STORE r10, r11
  ADD r10, r3
  ; '1'
  LDI r11, 49
  STORE r10, r11
  ADD r10, r3
  ; '.'
  LDI r11, 46
  STORE r10, r11
  ADD r10, r3
  ; '0'
  LDI r11, 48
  STORE r10, r11
  ADD r10, r3
  ; '\r'
  LDI r11, 13
  STORE r10, r11
  ADD r10, r3
  ; '\n'
  LDI r11, 10
  STORE r10, r11
  ADD r10, r3
  ; 'H'
  LDI r11, 72
  STORE r10, r11
  ADD r10, r3
  ; 'o'
  LDI r11, 111
  STORE r10, r11
  ADD r10, r3
  ; 's'
  LDI r11, 115
  STORE r10, r11
  ADD r10, r3
  ; 't'
  LDI r11, 116
  STORE r10, r11
  ADD r10, r3
  ; ':'
  LDI r11, 58
  STORE r10, r11
  ADD r10, r3
  ; ' '
  LDI r11, 32
  STORE r10, r11
  ADD r10, r3

  ; ── Copy host ──
  LDI r11, HOST_BUF
http_send_host_loop:
  LOAD r12, r11
  JZ r12, http_send_host_done
  STORE r10, r12
  ADD r10, r3
  ADD r11, r3
  JMP http_send_host_loop

http_send_host_done:
  ; ── "\r\n\r\n" (end of headers) ──
  LDI r11, 13         ; '\r'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 10         ; '\n'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 13         ; '\r'
  STORE r10, r11
  ADD r10, r3
  LDI r11, 10         ; '\n'
  STORE r10, r11
  ADD r10, r3

  ; Null-terminate request for safety
  LDI r11, 0
  STORE r10, r11

  ; ── Calculate request length ──
  LDI r11, REQ_BUF
  SUB r10, r11        ; r10 = request length (dst - base)

  ; ── Send via TCP ──
  LDI r11, FD_CELL
  LOAD r11, r11       ; r11 = fd
  LDI r12, REQ_BUF    ; r12 = buf addr
  ; r10 = length
  ; SOCKSEND fd_reg, buf_reg, len_reg, sent_reg
  SOCKSEND r11, r12, r10, r13

  POP r31
  RET


; ═══════════════════════════════════════════════
; http_recv_response -- Read HTTP response into buffer
;
; Input:  FD_CELL
; Output: RESP_BUF filled, BODY_LEN = total bytes received
;
; Reads in chunks until connection closed or buffer full.
; Uses a loop with FRAME yields between reads.
; ═══════════════════════════════════════════════
http_recv_response:
  PUSH r31

  LDI r10, 0          ; total_received = 0
  LDI r3, 1           ; increment

  ; Store initial body_len = 0
  LDI r11, BODY_LEN
  STORE r11, r10

http_recv_loop:
  ; Check if buffer is full
  LDI r11, RESP_MAX
  CMP r10, r11
  BGE r0, http_recv_done

  ; Calculate remaining buffer space
  LDI r11, RESP_MAX
  SUB r11, r10        ; remaining = RESP_MAX - total

  ; SOCKRECV fd, buf_addr, remaining, received
  LDI r12, FD_CELL
  LOAD r12, r12       ; r12 = fd

  ; Calculate write address: RESP_BUF + total_received
  LDI r13, RESP_BUF
  ADD r13, r10        ; r13 = buf addr

  ; r11 = remaining (max_len)
  SOCKRECV r12, r13, r11, r14

  ; Check result
  ; r0 = status: 0=OK, 6=would_block, 7=closed
  JZ r0, http_recv_got_data

  ; Connection closed -- done
  LDI r11, 7          ; NET_ERR_CONNECTION_CLOSED
  CMP r0, r11
  JZ r0, http_recv_done

  ; Would block -- yield a frame and retry
  FRAME
  JMP http_recv_loop

http_recv_got_data:
  ; r14 = bytes received
  ADD r10, r14        ; total += received

  ; Update body_len
  LDI r11, BODY_LEN
  STORE r11, r10

  ; Check if received 0 bytes (shouldn't happen, but safety)
  JZ r14, http_recv_done

  ; Continue reading
  JMP http_recv_loop

http_recv_done:
  ; Null-terminate response buffer at total_received position
  LDI r11, RESP_BUF
  ADD r11, r10
  LDI r12, 0
  STORE r11, r12

  POP r31
  RET


; ═══════════════════════════════════════════════
; http_strip_headers -- Find body start after \r\n\r\n
;
; Input:  RESP_BUF, BODY_LEN
; Output: BODY_BUF filled with body content, BODY_LEN updated
;         STATUS_CELL = HTTP status code (200, 404, etc.)
;
; HTTP response format:
;   "HTTP/1.0 200 OK\r\nheaders...\r\n\r\nbody"
;   Status code is bytes 9-11 of first line (3 digits)
; ═══════════════════════════════════════════════
http_strip_headers:
  PUSH r31

  ; ── Parse status code from first line ──
  ; Status is at offset 9 (after "HTTP/1.0 ")
  LDI r10, RESP_BUF
  LDI r3, 1
  LDI r11, 9
  ADD r10, r11        ; r10 points to status code digits

  ; Parse 3-digit status: code = d1*100 + d2*10 + d3
  LOAD r12, r10       ; first digit
  LDI r11, 48
  SUB r12, r11        ; d1 = char - '0'
  LDI r11, 100
  MUL r12, r11        ; d1 * 100
  LDI r13, STATUS_CELL
  STORE r13, r12      ; partial status

  ADD r10, r3
  LOAD r12, r10       ; second digit
  LDI r11, 48
  SUB r12, r11        ; d2 = char - '0'
  LDI r11, 10
  MUL r12, r11        ; d2 * 10
  LDI r13, STATUS_CELL
  LOAD r13, r13
  ADD r13, r12
  LDI r12, STATUS_CELL
  STORE r12, r13      ; status += d2*10

  ADD r10, r3
  LOAD r12, r10       ; third digit
  LDI r11, 48
  SUB r12, r11        ; d3 = char - '0'
  LDI r13, STATUS_CELL
  LOAD r13, r13
  ADD r13, r12
  LDI r12, STATUS_CELL
  STORE r12, r13      ; status += d3

  ; ── Find \r\n\r\n delimiter ──
  LDI r10, RESP_BUF   ; scan position
  LDI r11, BODY_LEN
  LOAD r11, r11       ; response length
  LDI r12, 0          ; bytes scanned

http_strip_scan:
  ; Check if we've scanned enough (need at least 4 more bytes for \r\n\r\n)
  CMP r12, r11
  BGE r0, http_strip_no_body

  ; Check for \r\n\r\n pattern
  LOAD r13, r10       ; char[pos]
  LDI r14, 13         ; '\r'
  CMP r13, r14
  JNZ r0, http_strip_next

  ; Found \r, check \n\r\n
  MOV r15, r10
  ADD r15, r3
  LOAD r13, r15       ; char[pos+1]
  LDI r14, 10         ; '\n'
  CMP r13, r14
  JNZ r0, http_strip_next

  ADD r15, r3
  LOAD r13, r15       ; char[pos+2]
  LDI r14, 13         ; '\r'
  CMP r13, r14
  JNZ r0, http_strip_next

  ADD r15, r3
  LOAD r13, r15       ; char[pos+3]
  LDI r14, 10         ; '\n'
  CMP r13, r14
  JNZ r0, http_strip_next

  ; Found \r\n\r\n at pos! Body starts at pos+4
  ADD r10, r3
  ADD r10, r3
  ADD r10, r3
  ADD r10, r3          ; r10 = body start

  ; Copy body to BODY_BUF
  LDI r13, BODY_BUF   ; dst
  LDI r15, 0          ; body count

http_strip_copy:
  LOAD r14, r10       ; char from body
  JZ r14, http_strip_copy_done
  STORE r13, r14      ; copy to body buffer
  ADD r10, r3
  ADD r13, r3
  ADD r15, r3
  JMP http_strip_copy

http_strip_copy_done:
  ; Null-terminate body
  LDI r14, 0
  STORE r13, r14

  ; Update BODY_LEN to actual body length
  LDI r10, BODY_LEN
  STORE r10, r15

  POP r31
  RET

http_strip_next:
  ADD r10, r3
  ADD r12, r3
  JMP http_strip_scan

http_strip_no_body:
  ; No body found -- set body_len = 0, null-terminate body buf
  LDI r10, BODY_BUF
  LDI r11, 0
  STORE r10, r11
  LDI r10, BODY_LEN
  STORE r10, r11

  POP r31
  RET


; ═══════════════════════════════════════════════
; http_close -- Disconnect and clean up
; ═══════════════════════════════════════════════
http_close:
  PUSH r31
  LDI r10, FD_CELL
  LOAD r10, r10
  DISCONNECT r10
  POP r31
  RET


; ═══════════════════════════════════════════════
; http_get -- Full HTTP GET in one call
;
; Input:  URL string at URL_BUF (0x6000)
; Output: BODY_BUF (0x9000), BODY_LEN, STATUS_CELL
;         r0 = 0 on success
;
; This is the main entry point combining all steps.
; ═══════════════════════════════════════════════
http_get:
  PUSH r31

  ; Step 1: Parse URL
  CALL url_parse

  ; Step 2: Connect
  CALL http_connect

  ; Check connection result (r0 should be NET_OK = 0)
  JNZ r0, http_get_fail

  ; Step 3: Send GET request
  CALL http_send_get

  ; Check send result
  JNZ r0, http_get_close_fail

  ; Step 4: Receive response
  CALL http_recv_response

  ; Step 5: Strip headers, extract body
  CALL http_strip_headers

  ; Step 6: Close connection
  CALL http_close

  ; Success
  LDI r0, 0
  POP r31
  RET

http_get_close_fail:
  CALL http_close

http_get_fail:
  ; r0 already has error code
  POP r31
  RET
