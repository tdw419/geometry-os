; debugger.asm -- Visual Register & Trace Debugger for Geometry OS
;
; Phase 76: Debugger UI
;
; Shows: register values (r0-r31 in hex), PC counter, trace buffer entries.
; Controls: S=step, C=continue, R=reset trace, T=toggle trace
; Proves trace buffer (SNAP_TRACE / TRACE_READ) integration.
;
; The debugger demonstrates reading the trace buffer from assembly programs.
; Since registers are not directly introspectable at runtime, we use the
; trace buffer to show PC values and opcode history.
;
; RAM Layout:
;   0x6000-0x60FF   String buffers (STRO targets)
;   0x6100          Step mode (0=running, 1=step)
;   0x6101          Trace enabled flag
;   0x6200-0x62FF   Hex scratch buffer
;   0x6400-0x640F   Hex digit lookup (16 ASCII chars)
;   0x6500-0x6513   Trace entry read buffer (20 words)
;   0x6600          Frame counter (local)
;
; Screen Layout (256x256):
;   Row 0-15:     Title bar (dark red)
;   Row 16-80:    Trace buffer info (entries, opcodes)
;   Row 80-150:   PC history from trace
;   Row 150-200:  Register snapshot from last trace entry
;   Row 220-255:  Status bar

#define TICKS     0xFFE
#define KEY_PORT  0xFFF
#define BUF       0x6000
#define STEP_MODE 0x6100
#define TRACE_ON  0x6101
#define HEX_BUF   0x6200
#define HEX_TABLE 0x6400
#define TRACE_RD  0x6500
#define FRAME_CNT 0x6600

LDI r30, 0xFD00
LDI r1, 1

; =========================================
; Init hex lookup table
; "0123456789ABCDEF"
; =========================================
LDI r20, HEX_TABLE
LDI r21, 48           ; '0'
STORE r20, r21
ADD r20, r1
LDI r21, 49
STORE r20, r21
ADD r20, r1
LDI r21, 50
STORE r20, r21
ADD r20, r1
LDI r21, 51
STORE r20, r21
ADD r20, r1
LDI r21, 52
STORE r20, r21
ADD r20, r1
LDI r21, 53
STORE r20, r21
ADD r20, r1
LDI r21, 54
STORE r20, r21
ADD r20, r1
LDI r21, 55
STORE r20, r21
ADD r20, r1
LDI r21, 56
STORE r20, r21
ADD r20, r1
LDI r21, 57
STORE r20, r21
ADD r20, r1
LDI r21, 65           ; 'A'
STORE r20, r21
ADD r20, r1
LDI r21, 66
STORE r20, r21
ADD r20, r1
LDI r21, 67
STORE r20, r21
ADD r20, r1
LDI r21, 68
STORE r20, r21
ADD r20, r1
LDI r21, 69
STORE r20, r21
ADD r20, r1
LDI r21, 70
STORE r20, r21

; Init state
LDI r20, STEP_MODE
LDI r2, 1
STORE r20, r2
LDI r20, TRACE_ON
LDI r2, 0
STORE r20, r2
LDI r20, FRAME_CNT
LDI r2, 0
STORE r20, r2

; Start trace recording
LDI r2, 1
SNAP_TRACE r2

; =========================================
; MAIN LOOP
; =========================================
main_loop:
  ; Increment frame counter
  LDI r20, FRAME_CNT
  LOAD r2, r20
  ADD r2, r1
  STORE r20, r2

  ; Read keyboard
  IKEY r10

  ; Handle keys
  LDI r11, 83          ; 'S' = Step mode
  CMP r10, r11
  JZ r0, key_step

  LDI r11, 67          ; 'C' = Continue
  CMP r10, r11
  JZ r0, key_continue

  LDI r11, 82          ; 'R' = Reset trace
  CMP r10, r11
  JZ r0, key_reset

  LDI r11, 84          ; 'T' = Toggle trace
  CMP r10, r11
  JZ r0, key_trace

  JMP draw_frame

key_step:
  LDI r20, STEP_MODE
  LDI r2, 1
  STORE r20, r2
  JMP draw_frame

key_continue:
  LDI r20, STEP_MODE
  LDI r2, 0
  STORE r20, r2
  JMP draw_frame

key_reset:
  ; Clear trace
  LDI r2, 2
  SNAP_TRACE r2
  ; Re-enable
  LDI r20, TRACE_ON
  LOAD r2, r20
  SNAP_TRACE r2
  JMP draw_frame

key_trace:
  LDI r20, TRACE_ON
  LOAD r2, r20
  LDI r11, 1
  XOR r2, r11
  STORE r20, r2
  SNAP_TRACE r2
  JMP draw_frame

; =========================================
; DRAW FRAME
; =========================================
draw_frame:
  ; Background
  LDI r3, 0x0A0A1E
  FILL r3

  ; Title bar
  LDI r3, 0
  LDI r4, 0
  LDI r5, 256
  LDI r6, 16
  LDI r7, 0x8B0000
  RECTF r3, r4, r5, r6, r7

  ; Title
  LDI r20, BUF
  STRO r20, "GO Debugger"
  LDI r3, 60
  LDI r4, 3
  LDI r5, BUF
  LDI r6, 0xFFFFFF
  LDI r7, 0x8B0000
  DRAWTEXT r3, r4, r5, r6, r7

  ; --- Trace count ---
  LDI r20, BUF
  STRO r20, "Trace entries:"
  LDI r3, 4
  LDI r4, 20
  LDI r5, BUF
  LDI r6, 0x00FFFF
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  ; Query trace count (mode 0)
  LDI r2, 0
  TRACE_READ r2        ; r0 = count
  MOV r8, r0           ; save count
  CALL val_to_hex      ; convert r0 to HEX_BUF
  LDI r3, 110
  LDI r4, 20
  LDI r5, HEX_BUF
  LDI r6, 0xFFFF00
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  ; --- Read last trace entry (PC + opcode) ---
  ; mode 1: r2=index, r3=dest addr -> writes 20 words
  CMP r8, r1
  BLT r0, no_trace_entries

  ; Read last entry (index = count - 1)
  SUB r8, r1           ; r8 = count - 1 (last index)
  MOV r2, r8
  LDI r3, TRACE_RD
  LDI r9, 1
  TRACE_READ r9        ; mode 1 read

  ; Show PC from trace entry: TRACE_RD+2 = PC
  LDI r20, BUF
  STRO r20, "Last PC:"
  LDI r3, 4
  LDI r4, 34
  LDI r5, BUF
  LDI r6, 0x00FF00
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  LDI r20, TRACE_RD
  ADD r20, r1
  ADD r20, r1           ; +2 = PC
  LOAD r0, r20
  CALL val_to_hex
  LDI r3, 70
  LDI r4, 34
  LDI r5, HEX_BUF
  LDI r6, 0xFFFF00
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  ; Show opcode from trace entry: TRACE_RD+19 = opcode
  LDI r20, BUF
  STRO r20, "Opcode:"
  LDI r3, 4
  LDI r4, 48
  LDI r5, BUF
  LDI r6, 0x00FF00
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  LDI r20, TRACE_RD
  LDI r21, 19
  ADD r20, r21
  LOAD r0, r20
  CALL val_to_hex
  LDI r3, 70
  LDI r4, 48
  LDI r5, HEX_BUF
  LDI r6, 0xFFFF00
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  ; Show register r0 from trace: TRACE_RD+3 = regs[0]
  LDI r20, BUF
  STRO r20, "r0:"
  LDI r3, 4
  LDI r4, 62
  LDI r5, BUF
  LDI r6, 0x88FF88
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  LDI r20, TRACE_RD
  LDI r21, 3
  ADD r20, r21
  LOAD r0, r20
  CALL val_to_hex
  LDI r3, 28
  LDI r4, 62
  LDI r5, HEX_BUF
  LDI r6, 0xFFFF00
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  ; Show register r1 from trace: TRACE_RD+4 = regs[1]
  LDI r20, BUF
  STRO r20, "r1:"
  LDI r3, 100
  LDI r4, 62
  LDI r5, BUF
  LDI r6, 0x88FF88
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  LDI r20, TRACE_RD
  LDI r21, 4
  ADD r20, r21
  LOAD r0, r20
  CALL val_to_hex
  LDI r3, 124
  LDI r4, 62
  LDI r5, HEX_BUF
  LDI r6, 0xFFFF00
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  JMP show_stack

no_trace_entries:
  LDI r20, BUF
  STRO r20, "(no trace data)"
  LDI r3, 4
  LDI r4, 34
  LDI r5, BUF
  LDI r6, 0x666666
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

show_stack:
  ; --- Stack pointer (r30 = 0xFD00) ---
  LDI r20, BUF
  STRO r20, "SP: FD00"
  LDI r3, 4
  LDI r4, 80
  LDI r5, BUF
  LDI r6, 0x00FF00
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  ; --- Step counter (TICKS) ---
  LDI r20, BUF
  STRO r20, "Ticks:"
  LDI r3, 4
  LDI r4, 94
  LDI r5, BUF
  LDI r6, 0x00FFFF
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  LDI r20, TICKS
  LOAD r0, r20
  CALL val_to_hex
  LDI r3, 52
  LDI r4, 94
  LDI r5, HEX_BUF
  LDI r6, 0xFFFF00
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  ; --- Frame counter ---
  LDI r20, BUF
  STRO r20, "Frame:"
  LDI r3, 120
  LDI r4, 94
  LDI r5, BUF
  LDI r6, 0x00FFFF
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  LDI r20, FRAME_CNT
  LOAD r0, r20
  CALL val_to_hex
  LDI r3, 172
  LDI r4, 94
  LDI r5, HEX_BUF
  LDI r6, 0xFFFF00
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  ; --- Opcode count from trace ---
  ; Count HALT opcodes (0x00) to prove TRACE_READ mode 2
  LDI r20, BUF
  STRO r20, "Opcode histogram:"
  LDI r3, 4
  LDI r4, 112
  LDI r5, BUF
  LDI r6, 0xFF88FF
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  ; Count FRAME opcodes (0x02) in trace
  LDI r2, 2            ; target opcode = FRAME
  LDI r9, 2
  TRACE_READ r9        ; mode 2: r0 = count of matching opcode
  MOV r8, r0
  CALL val_to_hex
  LDI r20, BUF
  STRO r20, "FRAME:"
  LDI r3, 4
  LDI r4, 126
  LDI r5, BUF
  LDI r6, 0x88FF88
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  LDI r3, 60
  LDI r4, 126
  LDI r5, HEX_BUF
  LDI r6, 0xFFFF00
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  ; Count IKEY opcodes (0x48)
  LDI r2, 0x48
  LDI r9, 2
  TRACE_READ r9
  MOV r8, r0
  CALL val_to_hex
  LDI r20, BUF
  STRO r20, "IKEY:"
  LDI r3, 110
  LDI r4, 126
  LDI r5, BUF
  LDI r6, 0x88FF88
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  LDI r3, 152
  LDI r4, 126
  LDI r5, HEX_BUF
  LDI r6, 0xFFFF00
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  ; --- Register snapshot display ---
  LDI r20, BUF
  STRO r20, "-- Register Snapshot --"
  LDI r3, 4
  LDI r4, 146
  LDI r5, BUF
  LDI r6, 0xFFAA00
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  ; Show current live register values using RAM stores
  ; r30 = SP (we know it's 0xFD00)
  LDI r20, BUF
  STRO r20, "r30(SP):FD00 r31(LR):"
  LDI r3, 4
  LDI r4, 160
  LDI r5, BUF
  LDI r6, 0x88FF88
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  ; Show LR (r31) value
  MOV r0, r31
  CALL val_to_hex
  LDI r3, 160
  LDI r4, 160
  LDI r5, HEX_BUF
  LDI r6, 0xFFFF00
  LDI r7, 0x0A0A1E
  DRAWTEXT r3, r4, r5, r6, r7

  ; --- Status bar ---
  LDI r3, 0
  LDI r4, 236
  LDI r5, 256
  LDI r6, 20
  LDI r7, 0x1A1A2E
  RECTF r3, r4, r5, r6, r7

  ; Mode
  LDI r20, STEP_MODE
  LOAD r8, r20
  LDI r11, 0
  CMP r8, r11
  JZ r0, mode_run

  LDI r20, BUF
  STRO r20, "[STEP]"
  LDI r3, 4
  LDI r4, 238
  LDI r5, BUF
  LDI r6, 0xFF4444
  LDI r7, 0x1A1A2E
  DRAWTEXT r3, r4, r5, r6, r7
  JMP status_trace_bar

mode_run:
  LDI r20, BUF
  STRO r20, "[RUN]"
  LDI r3, 4
  LDI r4, 238
  LDI r5, BUF
  LDI r6, 0x44FF44
  LDI r7, 0x1A1A2E
  DRAWTEXT r3, r4, r5, r6, r7

status_trace_bar:
  LDI r20, TRACE_ON
  LOAD r8, r20
  LDI r11, 0
  CMP r8, r11
  JZ r0, trc_off

  LDI r20, BUF
  STRO r20, "TRC:ON"
  LDI r3, 70
  LDI r4, 238
  LDI r5, BUF
  LDI r6, 0x44FF44
  LDI r7, 0x1A1A2E
  DRAWTEXT r3, r4, r5, r6, r7
  JMP hints

trc_off:
  LDI r20, BUF
  STRO r20, "TRC:OFF"
  LDI r3, 70
  LDI r4, 238
  LDI r5, BUF
  LDI r6, 0x666666
  LDI r7, 0x1A1A2E
  DRAWTEXT r3, r4, r5, r6, r7

hints:
  LDI r20, BUF
  STRO r20, "S:Step C:Run R:Reset T:Trace"
  LDI r3, 140
  LDI r4, 238
  LDI r5, BUF
  LDI r6, 0x888888
  LDI r7, 0x1A1A2E
  DRAWTEXT r3, r4, r5, r6, r7

  FRAME
  JMP main_loop

; =========================================
; val_to_hex: Convert r0 to 8-char hex string at HEX_BUF
; Clobbers r14-r19
; =========================================
val_to_hex:
  PUSH r31
  LDI r20, HEX_BUF
  LDI r21, 8
  LDI r22, 28
  LDI r23, 4
  LDI r24, 0xF
  LDI r7, 1
  LDI r14, 0

vhex_loop:
  MOV r15, r0
  MOV r16, r22
  SHR r15, r16
  AND r15, r24

  LDI r16, HEX_TABLE
  ADD r16, r15
  LOAD r16, r16

  MOV r17, r20
  ADD r17, r14
  STORE r17, r16

  SUB r22, r23
  ADD r14, r7
  LDI r18, 8
  CMP r14, r18
  BLT r0, vhex_loop

  ; Null terminate
  MOV r17, r20
  ADD r17, r14
  LDI r18, 0
  STORE r17, r18

  POP r31
  RET
