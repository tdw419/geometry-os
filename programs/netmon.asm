; netmon.asm -- Network Packet Monitor for Geometry OS (Phase 141)
;
; Shows: received packets with header info and hex dump of payload
; Interactive: W/S scrolls, C clears log, Q quits
; Uses NET_RECV to poll for packets each frame.
;
; RAM Layout:
;   0x6000-0x60FF  String buffers
;   0x6100-0x61FF  Number formatting buffer
;   0x6200         Scroll position (u32)
;   0x6201         Total packet count (u32)
;   0x6202         Packets in current second (u32)
;   0x6203         Last TICKS for rate calc (u32)
;   0x6204         Packets/sec display value (u32)
;   0x6300-0x63FF  Packet ring buffer (8 slots x 32 words)
;     Each slot: [type, width, height, flags, data_len, 0, 0, 0, data_word_0..23]
;   0x6400         Ring buffer write index
;   0x6500-0x65FF  NET_RECV temporary buffer
;
; Uses: NET_RECV, DRAWTEXT, RECTF, FILL, FRAME, IKEY, STRO

#define TICKS   0xFFE
#define BUF     0x6000
#define NUMBUF  0x6100
#define SCROLL  0x6200
#define PKT_TOT 0x6201
#define PKT_SEC 0x6202
#define LAST_T  0x6203
#define PKT_RATE 0x6204
#define RING    0x6300
#define RING_WI 0x6400
#define RECVBUF 0x6500
#define SLOT_SZ 32
#define MAX_SLOTS 8

; Colors
#define COL_BG    0x0D1B2A
#define COL_TITLE 0x1B3A5C
#define COL_PANEL 0x1B2838
#define COL_FG    0xFFFFFF
#define COL_LABEL 0x8888CC
#define COL_GREEN 0x44FF44
#define COL_YELLOW 0xFFFF44
#define COL_RED    0xFF4444
#define COL_CYAN   0x44FFFF
#define COL_GRAY   0x666688
#define COL_HEX    0xAABB44

; Init stack pointer
LDI r30, 0xFD00

; Init all counters to zero
LDI r15, 0
LDI r20, SCROLL
STORE r20, r15
LDI r20, PKT_TOT
STORE r20, r15
LDI r20, PKT_SEC
STORE r20, r15
LDI r20, LAST_T
STORE r20, r15
LDI r20, PKT_RATE
STORE r20, r15
LDI r20, RING_WI
STORE r20, r15

; Constants used throughout
LDI r7, 1
LDI r8, 0

; =========================================
; Main loop
; =========================================
main_loop:
    ; Clear screen
    LDI r1, COL_BG
    FILL r1

    ; Poll for incoming packets
    CALL poll_packets

    ; Update rate counter once per second
    CALL update_rate

    ; --- Title bar ---
    LDI r1, 0
    LDI r2, 0
    LDI r3, 256
    LDI r4, 18
    LDI r5, COL_TITLE
    RECTF r1, r2, r3, r4, r5

    ; Title
    LDI r20, BUF
    STRO r20, "Network Monitor"
    LDI r1, 56
    LDI r2, 3
    LDI r3, BUF
    LDI r4, COL_FG
    LDI r5, COL_TITLE
    DRAWTEXT r1, r2, r3, r4, r5

    ; Packet count in title
    LDI r20, BUF
    STRO r20, "Pkts:"
    LDI r1, 180
    LDI r2, 3
    LDI r3, BUF
    LDI r4, COL_CYAN
    LDI r5, COL_TITLE
    DRAWTEXT r1, r2, r3, r4, r5

    ; Show total count
    LDI r20, PKT_TOT
    LOAD r13, r20
    LDI r20, NUMBUF
    CALL int_to_dec
    LDI r1, 212
    LDI r2, 3
    LDI r3, NUMBUF
    LDI r4, COL_FG
    LDI r5, COL_TITLE
    DRAWTEXT r1, r2, r3, r4, r5

    ; --- Separator ---
    LDI r1, 0
    LDI r2, 18
    LDI r3, 256
    LDI r4, 1
    LDI r5, COL_LABEL
    RECTF r1, r2, r3, r4, r5

    ; --- Column headers ---
    LDI r20, BUF
    STRO r20, "Typ W  H  Fl Len  Hex Dump (first 6 words)"
    LDI r1, 4
    LDI r2, 20
    LDI r3, BUF
    LDI r4, COL_LABEL
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    ; Separator
    LDI r1, 0
    LDI r2, 28
    LDI r3, 256
    LDI r4, 1
    LDI r5, COL_GRAY
    RECTF r1, r2, r3, r4, r5

    ; --- Draw packet entries ---
    CALL draw_packets

    ; --- Footer ---
    LDI r1, 0
    LDI r2, 236
    LDI r3, 256
    LDI r4, 20
    LDI r5, COL_PANEL
    RECTF r1, r2, r3, r4, r5

    ; Rate display
    LDI r20, BUF
    STRO r20, "Rate:"
    LDI r1, 4
    LDI r2, 239
    LDI r3, BUF
    LDI r4, COL_CYAN
    LDI r5, COL_PANEL
    DRAWTEXT r1, r2, r3, r4, r5

    LDI r20, PKT_RATE
    LOAD r13, r20
    LDI r20, NUMBUF
    CALL int_to_dec
    LDI r1, 40
    LDI r2, 239
    LDI r3, NUMBUF
    LDI r4, COL_FG
    LDI r5, COL_PANEL
    DRAWTEXT r1, r2, r3, r4, r5

    LDI r20, BUF
    STRO r20, "pkts/s"
    LDI r1, 56
    LDI r2, 239
    LDI r3, BUF
    LDI r4, COL_GRAY
    LDI r5, COL_PANEL
    DRAWTEXT r1, r2, r3, r4, r5

    ; Controls hint
    LDI r20, BUF
    STRO r20, "W/S-Scroll  C-Clear  Q-Quit"
    LDI r1, 108
    LDI r2, 239
    LDI r3, BUF
    LDI r4, COL_GRAY
    LDI r5, COL_PANEL
    DRAWTEXT r1, r2, r3, r4, r5

    ; --- Handle keyboard input ---
    IKEY r10
    ; W = scroll up
    LDI r11, 87
    CMP r10, r11
    JZ r0, do_scroll_up
    ; S = scroll down
    LDI r11, 83
    CMP r10, r11
    JZ r0, do_scroll_down
    ; C = clear log
    LDI r11, 67
    CMP r10, r11
    JZ r0, do_clear
    ; Q = quit
    LDI r11, 81
    CMP r10, r11
    JZ r0, do_quit
    JMP input_done

do_scroll_up:
    LDI r20, SCROLL
    LOAD r15, r20
    LDI r11, 1
    CMP r15, r11
    BLT r0, input_done
    SUB r15, r11
    STORE r20, r15
    JMP input_done

do_scroll_down:
    LDI r20, SCROLL
    LOAD r15, r20
    LDI r11, 1
    ADD r15, r11
    LDI r11, 6
    CMP r15, r11
    BGE r0, input_done
    LDI r20, SCROLL
    STORE r20, r15
    JMP input_done

do_clear:
    LDI r15, 0
    LDI r20, PKT_TOT
    STORE r20, r15
    LDI r20, RING_WI
    STORE r20, r15
    LDI r20, SCROLL
    STORE r20, r15
    JMP input_done

do_quit:
    HALT

input_done:
    FRAME
    JMP main_loop

; =========================================
; Poll for incoming packets via NET_RECV
; =========================================
poll_packets:
    PUSH r31

    ; Try to receive a packet
    LDI r1, RECVBUF
    LDI r2, 240
    NET_RECV r1, r2
    ; r0 = words received (0 = nothing)

    JZ r0, poll_none

    ; Got one! Store in ring buffer at RING + (write_idx * SLOT_SZ)
    LDI r20, RING_WI
    LOAD r15, r20

    ; Compute slot address
    LDI r11, SLOT_SZ
    MUL r15, r11
    LDI r11, RING
    ADD r15, r11
    ; r15 = slot base

    ; Copy header (4 words from RECVBUF)
    LDI r12, RECVBUF
    LDI r14, 1

    ; word 0: type
    LOAD r13, r12
    STORE r15, r13
    ADD r15, r14
    ADD r12, r14
    ; word 1: width
    LOAD r13, r12
    STORE r15, r13
    ADD r15, r14
    ADD r12, r14
    ; word 2: height
    LOAD r13, r12
    STORE r15, r13
    ADD r15, r14
    ADD r12, r14
    ; word 3: flags
    LOAD r13, r12
    STORE r15, r13
    ADD r15, r14
    ADD r12, r14
    ; word 4: data_len = r0 from NET_RECV
    STORE r15, r0
    ADD r15, r14
    ADD r12, r14
    ; words 5-7: padding (zero)
    STORE r15, r8
    ADD r15, r14
    STORE r15, r8
    ADD r15, r14
    STORE r15, r8
    ADD r15, r14

    ; Copy data words from RECVBUF+8 (after header copy consumed 4+1+3=8 words)
    ; Actually RECVBUF layout: [type, width, height, flags, data...]
    ; r12 is now at RECVBUF+4, r15 is at slot+8
    ; Need to skip the data_len word at RECVBUF+4 (already read as r0)
    ; r12 = RECVBUF + 4, advance to RECVBUF + 4 + 1 = RECVBUF + 5
    ADD r12, r14

    ; Copy up to 24 data words
    LDI r16, 0
copy_loop:
    LDI r11, 24
    CMP r16, r11
    BGE r0, copy_done
    LOAD r13, r12
    STORE r15, r13
    ADD r15, r14
    ADD r12, r14
    ADD r16, r14
    JMP copy_loop

copy_done:
    ; Advance write index with wrap
    LDI r20, RING_WI
    LOAD r15, r20
    ADD r15, r7
    LDI r11, MAX_SLOTS
    CMP r15, r11
    BLT r0, no_wrap
    LDI r15, 0
no_wrap:
    STORE r20, r15

    ; Increment totals
    LDI r20, PKT_TOT
    LOAD r15, r20
    ADD r15, r7
    STORE r20, r15

    LDI r20, PKT_SEC
    LOAD r15, r20
    ADD r15, r7
    STORE r20, r15

poll_none:
    POP r31
    RET

; =========================================
; Update rate counter (once per ~60 frames = 1 second)
; =========================================
update_rate:
    PUSH r31

    LDI r20, TICKS
    LOAD r15, r20       ; current ticks
    LDI r20, LAST_T
    LOAD r16, r20       ; last check time
    LDI r11, 60
    ADD r16, r11        ; r16 = last + 60
    CMP r15, r16
    BLT r0, rate_skip

    ; Time to update: copy sec count to rate display
    LDI r20, PKT_SEC
    LOAD r15, r20
    LDI r20, PKT_RATE
    STORE r20, r15

    ; Reset per-sec counter
    LDI r20, PKT_SEC
    STORE r20, r8

    ; Save current ticks
    LDI r20, TICKS
    LOAD r15, r20
    LDI r20, LAST_T
    STORE r20, r15

rate_skip:
    POP r31
    RET

; =========================================
; Draw packet entries from ring buffer
; =========================================
draw_packets:
    PUSH r31

    ; Check if we have any packets
    LDI r20, RING_WI
    LOAD r15, r20       ; total slots used
    JNZ r15, has_pkts

    ; Empty state -- show waiting message
    LDI r20, BUF
    STRO r20, "Waiting for packets..."
    LDI r1, 60
    LDI r2, 100
    LDI r3, BUF
    LDI r4, COL_GRAY
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    LDI r20, BUF
    STRO r20, "NET_RECV polls each frame"
    LDI r1, 62
    LDI r2, 112
    LDI r3, BUF
    LDI r4, COL_GRAY
    LDI r5, 0
    DRAWTEXT r1, r2, r3, r4, r5

    JMP dp_done

has_pkts:
    ; r15 = total packet slots, draw visible ones
    LDI r20, SCROLL
    LOAD r16, r20       ; scroll offset

    ; Draw up to 7 rows starting from scroll offset
    LDI r17, 0          ; visible row counter
    LDI r18, 0          ; slot counter

dp_loop:
    LDI r11, 7
    CMP r17, r11
    BGE r0, dp_done

    CMP r18, r15
    BGE r0, dp_done

    ; Skip slots before scroll offset
    CMP r18, r16
    BLT r0, dp_skip

    ; Draw this slot
    PUSH r31
    PUSH r15
    PUSH r16
    CALL draw_one_pkt
    POP r16
    POP r15
    POP r31

    LDI r11, 1
    ADD r17, r11

dp_skip:
    LDI r11, 1
    ADD r18, r11
    JMP dp_loop

dp_done:
    POP r31
    RET

; =========================================
; Draw one packet row
; r17 = visible row index, r18 = slot index
; =========================================
draw_one_pkt:
    ; Compute slot address: RING + slot_idx * SLOT_SZ
    MOV r15, r18
    LDI r11, SLOT_SZ
    MUL r15, r11
    LDI r11, RING
    ADD r15, r11

    ; Y position: 30 + visible_row * 28
    LDI r11, 28
    MOV r12, r17
    MUL r12, r11
    LDI r11, 30
    ADD r12, r11
    ; r12 = Y base

    ; Row background
    LDI r1, 2
    LDI r3, 252
    LDI r4, 26
    LDI r5, COL_PANEL
    RECTF r1, r12, r3, r4, r5

    ; Text Y = row_y + 3 (centered in 26px row)
    LDI r11, 3
    ADD r12, r11
    ; r12 = text Y

    ; --- Column 1: Type (byte 0) ---
    LOAD r13, r15
    LDI r11, 1
    ADD r15, r11

    ; Show type as single char
    ; 0 = S(screen), 1 = C(chat), 2 = F(file), else = ?
    LDI r14, 0
    CMP r13, r14
    JNZ r0, not_screen
    LDI r20, BUF
    STRO r20, "S"
    JMP show_type
not_screen:
    LDI r14, 1
    CMP r13, r14
    JNZ r0, not_chat
    LDI r20, BUF
    STRO r20, "C"
    JMP show_type
not_chat:
    LDI r14, 2
    CMP r13, r14
    JNZ r0, not_file
    LDI r20, BUF
    STRO r20, "F"
    JMP show_type
not_file:
    LDI r20, BUF
    STRO r20, "?"
show_type:
    LDI r1, 6
    LDI r3, BUF
    LDI r4, COL_CYAN
    LDI r5, COL_PANEL
    DRAWTEXT r1, r12, r3, r4, r5

    ; --- Column 2: Width ---
    LOAD r13, r15
    LDI r11, 1
    ADD r15, r11

    LDI r20, NUMBUF
    CALL int_to_dec
    LDI r1, 28
    LDI r3, NUMBUF
    LDI r4, COL_GREEN
    LDI r5, COL_PANEL
    DRAWTEXT r1, r12, r3, r4, r5

    ; --- Column 3: Height ---
    LOAD r13, r15
    LDI r11, 1
    ADD r15, r11

    LDI r20, NUMBUF
    CALL int_to_dec
    LDI r1, 52
    LDI r3, NUMBUF
    LDI r4, COL_GREEN
    LDI r5, COL_PANEL
    DRAWTEXT r1, r12, r3, r4, r5

    ; --- Column 4: Flags ---
    LOAD r13, r15
    LDI r11, 1
    ADD r15, r11

    LDI r20, NUMBUF
    CALL int_to_dec
    LDI r1, 76
    LDI r3, NUMBUF
    LDI r4, COL_YELLOW
    LDI r5, COL_PANEL
    DRAWTEXT r1, r12, r3, r4, r5

    ; --- Column 5: Data length ---
    LOAD r13, r15
    LDI r11, 1
    ADD r15, r11

    LDI r20, NUMBUF
    CALL int_to_dec
    LDI r1, 100
    LDI r3, NUMBUF
    LDI r4, COL_FG
    LDI r5, COL_PANEL
    DRAWTEXT r1, r12, r3, r4, r5

    ; --- Skip 3 padding words ---
    LDI r11, 3
    ADD r15, r11

    ; --- Column 6: Hex dump (first 6 data words) ---
    LDI r16, 0
    LDI r1, 130        ; x start
hex_word_loop:
    LDI r11, 6
    CMP r16, r11
    BGE r0, hex_done

    LOAD r13, r15
    LDI r11, 1
    ADD r15, r11

    LDI r20, NUMBUF
    CALL hex_to_str

    ; Compute x position: 130 + word_idx * 22
    LDI r11, 22
    MOV r3, r16
    MUL r3, r11
    ADD r3, r1
    LDI r6, NUMBUF
    LDI r4, COL_HEX
    LDI r5, COL_PANEL
    DRAWTEXT r3, r12, r6, r4, r5

    LDI r11, 1
    ADD r16, r11
    JMP hex_word_loop

hex_done:
    RET

; =========================================
; Convert unsigned int to decimal string
; r13 = number, r20 = buffer address
; Writes null-terminated string, returns buffer addr in r20
; =========================================
int_to_dec:
    PUSH r31
    PUSH r14
    PUSH r15
    PUSH r16

    ; Handle zero case
    LDI r11, 0
    CMP r13, r11
    JNZ r0, itd_nonzero
    LDI r11, 48         ; '0'
    STORE r20, r11
    LDI r11, 1
    ADD r20, r11
    LDI r11, 0
    STORE r20, r11
    JMP itd_done

itd_nonzero:
    ; We need to extract digits from most significant to least
    ; Use a second buffer to collect digits in reverse
    LDI r15, 0          ; digit count
    LDI r16, 0          ; temp
    LDI r14, NUMBUF
    ADD r14, r15        ; start at NUMBUF+0

    ; Divide by 10 repeatedly
itd_loop:
    LDI r11, 0
    CMP r13, r11
    JZ r0, itd_reverse

    ; r13 / 10 -> quotient in r13, remainder as digit
    LDI r11, 10
    ; Compute remainder via repeated subtraction (simpler than MOD)
    ; Actually we have MOD opcode
    MOV r16, r13
    MOD r16, r11        ; r16 = r13 % 10
    DIV r13, r11        ; r13 = r13 / 10

    ; Convert digit to ASCII
    ADD r16, r15
    SUB r16, r15
    LDI r11, 48
    ADD r16, r11        ; r16 = digit + '0'

    ; Store in reverse buffer (at end, we reverse)
    ; Actually, push onto stack for easy reversal
    PUSH r16

    LDI r11, 1
    ADD r15, r11
    JMP itd_loop

itd_reverse:
    ; r15 = number of digits, all pushed on stack
    ; Pop and write to buffer in correct order
    JZ r15, itd_done

itd_pop_loop:
    POP r16
    STORE r20, r16
    LDI r11, 1
    ADD r20, r11
    LDI r11, 1
    SUB r15, r11
    JNZ r15, itd_pop_loop

    ; Null terminate
    LDI r11, 0
    STORE r20, r11

itd_done:
    POP r16
    POP r15
    POP r14
    POP r31
    RET

; =========================================
; Convert u32 to hex string "0xXXXXXXXX"
; r13 = number, r20 = buffer address
; =========================================
hex_to_str:
    PUSH r31
    PUSH r15
    PUSH r16

    ; Write "0x" prefix
    LDI r11, 48         ; '0'
    STORE r20, r11
    LDI r11, 1
    ADD r20, r11
    LDI r11, 120        ; 'x'
    STORE r20, r11
    LDI r11, 1
    ADD r20, r11

    ; 8 hex nibbles, high to low
    LDI r16, 28         ; bit shift start

hts_nibble:
    LDI r11, 0
    CMP r16, r11
    BLT r0, hts_done

    ; Extract nibble
    MOV r12, r13
    MOV r14, r16
    SHR r12, r14
    LDI r14, 0xF
    AND r12, r14

    ; To ASCII
    LDI r14, 10
    CMP r12, r14
    BGE r0, hts_alpha
    LDI r14, 48         ; + '0'
    ADD r12, r14
    JMP hts_store
hts_alpha:
    LDI r14, 55         ; + 'A' - 10
    ADD r12, r14
hts_store:
    STORE r20, r12
    LDI r11, 1
    ADD r20, r11
    LDI r11, 4
    SUB r16, r11
    JMP hts_nibble

hts_done:
    ; Null terminate
    LDI r11, 0
    STORE r20, r11

    POP r16
    POP r15
    POP r31
    RET
