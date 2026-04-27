; netmon.asm -- Phase 141: Network Packet Viewer
;
; Visual network packet monitor. Reads from /dev/net device,
; displays scrolling packet log with hex data and rate stats.
; Simulates traffic by writing random data to the net port.
;
; Layout:
;   Row 0-1: Title bar
;   Row 2-29: Scrolling packet log (colored bars per packet)
;   Row 30-31: Status bar with packet count and rate
;
; Opcodes: LDI, MOV, ADD, SUB, MUL, DIV, AND, OR, SHL, SHR, SAR
;          MOD, CMP, JZ, JNZ, BLT, BGE, JMP, FILL, RECTF, PSET
;          DRAWTEXT, TEXT, IKEY, RAND, FRAME, OPEN, READ, IOCTL
;          STORE, LOAD

; == INIT ==
    LDI r5, 1
    LDI r6, 0
    LDI r8, 0             ; total packets
    LDI r10, 0            ; rate counter
    LDI r11, 0            ; displayed rate
    LDI r12, 0            ; buffer head (0-31)
    LDI r22, 0x5000       ; packet buffer (32 entries, 4 words each)
    LDI r23, 32           ; max packets

    ; Open /dev/net
    LDI r1, net_path
    LDI r2, 0
    OPEN r1, r2
    MOV r7, r0            ; r7 = net fd

; == MAIN LOOP ==
main_loop:
    ; Dark background
    LDI r1, 0x080810
    FILL r1

    ; == SIMULATE PACKETS (every ~32 frames) ==
    LDI r1, 0xFFE
    LOAD r1, r1
    LDI r2, 31
    AND r1, r2
    LDI r2, 15
    CMP r1, r2
    JNZ r0, no_sim
    RAND r1
    LDI r2, 0xFFC
    STORE r2, r1
no_sim:

    ; == READ FROM /dev/net ==
    LDI r1, 0xFFC
    LDI r2, 1
    READ r7, r1, r2
    LDI r1, 0
    CMP r0, r1
    JZ r0, no_pkt

    ; Store packet
    LDI r1, 0xFFC
    LOAD r24, r1           ; data word

    ; Buffer[head*4 + 0] = data
    MOV r1, r12
    LDI r2, 2
    SHL r1, r2
    MOV r2, r22
    ADD r2, r1
    STORE r2, r24

    ; Buffer[head*4 + 1] = top byte (for color)
    MOV r1, r5
    ADD r2, r1
    MOV r3, r24
    LDI r4, 24
    SHR r3, r4             ; top byte
    STORE r2, r3

    ; Buffer[head*4 + 2] = TICKS at receipt
    MOV r1, r5
    ADD r2, r1
    LDI r3, 0xFFE
    LOAD r3, r3
    STORE r2, r3

    ; Advance head
    ADD r12, r5
    LDI r1, 32
    CMP r12, r1
    BLT r0, no_wrap
    LDI r12, 0
no_wrap:
    ADD r8, r5
    ADD r10, r5

no_pkt:
    ; == RATE UPDATE ==
    LDI r1, 0xFFE
    LOAD r14, r1
    LDI r1, 63
    AND r14, r1
    LDI r1, 2
    CMP r14, r1
    BGE r0, no_rate
    MOV r11, r10
    LDI r10, 0
no_rate:

    ; == TITLE BAR ==
    LDI r1, 0
    LDI r2, 0
    LDI r3, 256
    LDI r4, 14
    LDI r13, 0x003366
    RECTF r1, r2, r3, r4, r13

    LDI r16, 4
    LDI r17, 3
    LDI r18, title_str
    LDI r19, 0xFFFFFF
    LDI r20, 0x003366
    DRAWTEXT r16, r17, r18, r19, r20

    ; Net status indicator
    LDI r1, 0xE003
    LDI r2, 0
    LDI r3, 0
    IOCTL r1, r2, r3
    LDI r1, 0
    CMP r0, r1
    JZ r0, net_down_label

    ; Net status indicator
    LDI r16, 90
    LDI r17, 3
    LDI r18, up_str
    LDI r19, 0x00FF00
    LDI r20, 0x003366
    DRAWTEXT r16, r17, r18, r19, r20
    JMP pkt_draw

net_down_label:
    LDI r16, 90
    LDI r17, 3
    LDI r18, down_str
    LDI r19, 0xFF0000
    LDI r20, 0x003366
    DRAWTEXT r16, r17, r18, r19, r20

pkt_draw:
    ; == DRAW PACKET LOG ==
    ; Show last 28 packets as colored bars
    LDI r25, 0             ; display row (0-27)
    LDI r17, 16            ; start y

pkt_row:
    LDI r1, 0
    CMP r8, r1
    JZ r0, show_empty

    ; Determine entry index
    LDI r1, 28
    CMP r8, r1
    BGE r0, full_buf
    ; Partial: show rows 0..total-1
    CMP r25, r8
    BLT r0, do_draw_entry
    JMP skip_row

full_buf:
    ; entry = (head - 28 + row) & 31
    MOV r1, r12
    LDI r2, 28
    SUB r1, r2
    ADD r1, r25
    LDI r2, 31
    AND r1, r2

do_draw_entry:
    ; r1 = entry index
    LDI r2, 2
    SHL r1, r2             ; entry * 4
    MOV r3, r22
    ADD r3, r1             ; buffer base + offset

    ; Load color byte (word 1)
    MOV r2, r3
    ADD r2, r5
    LOAD r13, r2           ; color index (top byte of data)

    ; Generate color from the byte: hue mapping
    ; Use top 3 bits for hue channel selection
    LDI r4, 0xE0
    AND r13, r4            ; top 3 bits
    LDI r4, 5
    SHR r13, r4            ; shift to bits 8-10 range
    ; Create a visible color
    LDI r4, 0x44
    OR r13, r4             ; ensure minimum brightness

    ; Draw row bar
    LDI r1, 2
    LDI r2, 256
    LDI r4, 7
    RECTF r1, r17, r2, r4, r13

    ; Draw a bright accent line on left
    MOV r1, r3
    LOAD r15, r1           ; data word
    LDI r14, 16
    SHR r15, r14
    LDI r14, 0xF
    AND r15, r14
    LDI r14, 2
    MUL r15, r14
    ADD r15, r5             ; width = 3..33
    LDI r14, 0x00CCFF      ; cyan accent
    RECTF r1, r17, r15, r4, r14

skip_row:
    LDI r1, 8
    ADD r17, r1
    ADD r25, r5
    LDI r1, 28
    CMP r25, r1
    BLT r0, pkt_row
    JMP draw_stat

show_empty:
    LDI r16, 80
    LDI r17, 130
    LDI r18, empty_str
    LDI r19, 0x555555
    LDI r20, 0
    DRAWTEXT r16, r17, r18, r19, r20

draw_stat:
    ; == STATUS BAR ==
    LDI r1, 0
    LDI r2, 242
    LDI r3, 256
    LDI r4, 14
    LDI r13, 0x002244
    RECTF r1, r2, r3, r4, r13

    ; Build "PKT:N" at 0x6000
    LDI r1, 0x6000
    LDI r13, 80            ; 'P'
    STORE r1, r13
    ADD r1, r5
    LDI r13, 75
    STORE r1, r13
    ADD r1, r5
    LDI r13, 84
    STORE r1, r13
    ADD r1, r5
    LDI r13, 58
    STORE r1, r13
    ADD r1, r5
    LDI r13, 32
    STORE r1, r13
    ADD r1, r5

    ; Decimal convert r8
    MOV r2, r8
    LDI r3, 0x6020
    LDI r13, 0
    STORE r3, r13
    ; Special case: if r8 == 0, write '0'
    LDI r13, 0
    CMP r2, r13
    JNZ r0, dec_nonzero
    LDI r13, 48
    SUB r3, r5
    STORE r3, r13
    JMP dec_copy
dec_nonzero:
    LDI r27, 10
    MOV r28, r2
    DIV r28, r27
    MUL r28, r27
    MOV r29, r2
    SUB r29, r28
    LDI r28, 48
    ADD r29, r28
    SUB r3, r5
    STORE r3, r29
    LDI r28, 10
    DIV r2, r28
    LDI r28, 0
    CMP r2, r28
    JNZ r0, dec_nonzero
dec_copy:
    LDI r1, 0x6005
dec_cp:
    LOAD r2, r3
    LDI r27, 0
    CMP r2, r27
    JZ r0, dec_cp_done
    STORE r1, r2
    ADD r1, r5
    ADD r3, r5
    JMP dec_cp
dec_cp_done:
    LDI r13, 0
    STORE r1, r13

    LDI r16, 4
    LDI r17, 244
    LDI r18, 0x6000
    LDI r19, 0xFFFFFF
    LDI r20, 0x002244
    DRAWTEXT r16, r17, r18, r19, r20

    ; Build "RATE:N/s" at 0x6030
    LDI r1, 0x6030
    LDI r13, 82
    STORE r1, r13
    ADD r1, r5
    LDI r13, 65
    STORE r1, r13
    ADD r1, r5
    LDI r13, 84
    STORE r1, r13
    ADD r1, r5
    LDI r13, 69
    STORE r1, r13
    ADD r1, r5
    LDI r13, 58
    STORE r1, r13
    ADD r1, r5
    LDI r13, 32
    STORE r1, r13
    ADD r1, r5

    ; Decimal convert r11
    MOV r2, r11
    LDI r3, 0x6050
    LDI r13, 0
    STORE r3, r13
    LDI r13, 0
    CMP r2, r13
    JNZ r0, rdec_nonzero
    LDI r13, 48
    SUB r3, r5
    STORE r3, r13
    JMP rdec_copy
rdec_nonzero:
    LDI r27, 10
    MOV r28, r2
    DIV r28, r27
    MUL r28, r27
    MOV r29, r2
    SUB r29, r28
    LDI r28, 48
    ADD r29, r28
    SUB r3, r5
    STORE r3, r29
    LDI r28, 10
    DIV r2, r28
    LDI r28, 0
    CMP r2, r28
    JNZ r0, rdec_nonzero
rdec_copy:
    LDI r1, 0x6037
rdec_cp:
    LOAD r2, r3
    LDI r27, 0
    CMP r2, r27
    JZ r0, rdec_done
    STORE r1, r2
    ADD r1, r5
    ADD r3, r5
    JMP rdec_cp
rdec_done:
    LDI r13, 47
    STORE r1, r13
    ADD r1, r5
    LDI r13, 115
    STORE r1, r13
    ADD r1, r5
    LDI r13, 0
    STORE r1, r13

    LDI r16, 130
    LDI r17, 244
    LDI r18, 0x6030
    LDI r19, 0xFFFF00
    LDI r20, 0x002244
    DRAWTEXT r16, r17, r18, r19, r20

    FRAME
    JMP main_loop

; == DATA ==
.org 0x4000
net_path:
    .asciz "/dev/net"
    .db 0

title_str:
    .asciz "NETMON - Packet Viewer"
    .db 0

up_str:
    .asciz "UP"
    .db 0

down_str:
    .asciz "DOWN"
    .db 0

empty_str:
    .asciz "NO PACKETS YET"
    .db 0
