; net_demo.asm -- TCP Networking Demo for Geometry OS
; Connects to an echo server, sends a message, receives the echo, displays it.
;
; Usage: Start an echo server first:
;   python3 -c "import socket; s=socket.socket(); s.bind(('127.0.0.1', 8888)); s.listen(1); c,_=s.accept(); c.settimeout(5); [c.sendall(c.recv(4096)) for _ in iter(int,1)]"
;   Then run this program with F5.
;
; Memory layout:
;   0x7000..0x7010  server IP string ("127.0.0.1")
;   0x7100..0x71FF  send buffer
;   0x7200..0x72FF  receive buffer

#define IP_ADDR      0x7000
#define SEND_BUF     0x7100
#define RECV_BUF     0x7200
#define PORT         8888

  ; Draw title
  LDI r1, 10
  LDI r2, 10
  LDI r3, title
  TEXT r1, r2, r3

  ; Draw "Connecting..." message
  LDI r1, 10
  LDI r2, 30
  LDI r3, msg_connecting
  TEXT r1, r2, r3

  ; Store IP address string at 0x7000
  LDI r1, IP_ADDR
  LDI r2, 49        ; '1'
  STORE r1, r2
  LDI r2, 50        ; '2'
  LDI r3, IP_ADDR
  ADD r3, r3, r1
  ADD r3, r3, r1
  STORE r3, r2      ; 0x7002 = '2'
  LDI r2, 55        ; '7'
  LDI r3, IP_ADDR
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  STORE r3, r2      ; 0x7003 = '7'
  LDI r2, 46        ; '.'
  LDI r3, IP_ADDR
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  STORE r3, r2      ; 0x7004 = '.'
  LDI r2, 48        ; '0'
  LDI r3, IP_ADDR
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  STORE r3, r2      ; 0x7005 = '0'
  LDI r2, 46        ; '.'
  LDI r3, IP_ADDR
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  STORE r3, r2      ; 0x7006 = '.'
  LDI r2, 48        ; '0'
  LDI r3, IP_ADDR
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  STORE r3, r2      ; 0x7007 = '0'
  LDI r2, 46        ; '.'
  LDI r3, IP_ADDR
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  STORE r3, r2      ; 0x7008 = '.'
  LDI r2, 49        ; '1'
  LDI r3, IP_ADDR
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  STORE r3, r2      ; 0x7009 = '1'
  LDI r2, 0         ; null terminator
  LDI r3, IP_ADDR
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  ADD r3, r3, r1
  STORE r3, r2      ; 0x700A = null

  ; CONNECT addr_reg, port_reg, fd_reg
  LDI r1, IP_ADDR
  LDI r2, PORT
  CONNECT r1, r2, r3

  ; Check if connect succeeded (r0 == 0)
  CMP r0, r0
  JNZ connect_fail

  ; Draw "Connected!" message
  LDI r1, 10
  LDI r2, 40
  LDI r3, msg_connected
  TEXT r1, r2, r3

  ; Store "Hello from Geometry OS!" in send buffer
  LDI r1, SEND_BUF
  LDI r2, 72        ; 'H'
  STORE r1, r2
  LDI r2, 101       ; 'e'
  ADD r1, r1, r3
  STORE r1, r2      ; 'e'
  LDI r2, 108       ; 'l'
  ADD r1, r1, r3
  STORE r1, r2      ; 'l'
  LDI r2, 108       ; 'l'
  ADD r1, r1, r3
  STORE r1, r2      ; 'l'
  LDI r2, 111       ; 'o'
  ADD r1, r1, r3
  STORE r1, r2      ; 'o'
  LDI r2, 32        ; ' '
  ADD r1, r1, r3
  STORE r1, r2      ; ' '
  LDI r2, 102       ; 'f'
  ADD r1, r1, r3
  STORE r1, r2      ; 'f'
  LDI r2, 114       ; 'r'
  ADD r1, r1, r3
  STORE r1, r2      ; 'r'
  LDI r2, 111       ; 'o'
  ADD r1, r1, r3
  STORE r1, r2      ; 'o'
  LDI r2, 109       ; 'm'
  ADD r1, r1, r3
  STORE r1, r2      ; 'm'
  LDI r2, 32        ; ' '
  ADD r1, r1, r3
  STORE r1, r2      ; ' '
  LDI r2, 71        ; 'G'
  ADD r1, r1, r3
  STORE r1, r2      ; 'G'
  LDI r2, 101       ; 'e'
  ADD r1, r1, r3
  STORE r1, r2      ; 'e'
  LDI r2, 111       ; 'o'
  ADD r1, r1, r3
  STORE r1, r2      ; 'o'
  LDI r2, 109       ; 'm'
  ADD r1, r1, r3
  STORE r1, r2      ; 'm'
  LDI r2, 101       ; 'e'
  ADD r1, r1, r3
  STORE r1, r2      ; 'e'
  LDI r2, 116       ; 't'
  ADD r1, r1, r3
  STORE r1, r2      ; 't'
  LDI r2, 114       ; 'r'
  ADD r1, r1, r3
  STORE r1, r2      ; 'r'
  LDI r2, 121       ; 'y'
  ADD r1, r1, r3
  STORE r1, r2      ; 'y'
  LDI r2, 32        ; ' '
  ADD r1, r1, r3
  STORE r1, r2      ; ' '
  LDI r2, 79        ; 'O'
  ADD r1, r1, r3
  STORE r1, r2      ; 'O'
  LDI r2, 83        ; 'S'
  ADD r1, r1, r3
  STORE r1, r2      ; 'S'
  LDI r2, 33        ; '!'
  ADD r1, r1, r3
  STORE r1, r2      ; '!'

  ; Draw "Sent:" message
  LDI r1, 10
  LDI r2, 50
  LDI r3, msg_sent
  TEXT r1, r2, r3

  ; Draw the sent message
  LDI r1, 50
  LDI r2, 50
  LDI r3, SEND_BUF
  TEXT r1, r2, r3

  ; SOCKSEND fd_reg, buf_reg, len_reg, sent_reg
  ; r3 = fd from CONNECT
  LDI r1, SEND_BUF
  LDI r2, 23        ; length of "Hello from Geometry OS!"
  SOCKSEND r3, r1, r2, r4

  ; Wait a moment for echo (100 frames = ~1.6 seconds)
  LDI r10, 100
wait_loop:
  FRAME
  SUB r10, r10, r3
  CMP r10, r3
  JNZ wait_loop
  CMP r10, r3
  JZ wait_done
  JMP wait_loop

wait_done:
  ; SOCKRECV fd_reg, buf_reg, max_len_reg, recv_reg
  LDI r1, RECV_BUF
  LDI r2, 200
  SOCKRECV r3, r1, r2, r4

  ; Draw "Received:" message
  LDI r1, 10
  LDI r2, 60
  LDI r3, msg_recv
  TEXT r1, r2, r3

  ; Draw the received message
  LDI r1, 60
  LDI r2, 60
  LDI r3, RECV_BUF
  TEXT r1, r2, r3

  ; Disconnect
  DISCONNECT r3

  ; Draw "Done!" message
  LDI r1, 10
  LDI r2, 80
  LDI r3, msg_done
  TEXT r1, r2, r3

  HALT

connect_fail:
  ; Draw error message
  LDI r1, 10
  LDI r2, 40
  LDI r3, msg_error
  TEXT r1, r2, r3
  HALT

title:
  TEXTI "=== TCP Networking Demo ==="

msg_connecting:
  TEXTI "Connecting to 127.0.0.1:8888..."

msg_connected:
  TEXTI "Connected! (fd stored in r3)"

msg_sent:
  TEXTI "Sent:"

msg_recv:
  TEXTI "Echo:"

msg_done:
  TEXTI "Done! Connection closed."

msg_error:
  TEXTI "ERROR: Could not connect."
