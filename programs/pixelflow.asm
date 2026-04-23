; pixelflow.asm -- Neural network visualizer for Geometry OS
; Connects to pixelflow server via /tmp/pixelflow.sock
; Shows weight matrices as pixel patterns and generated text
;
; RAM layout:
;   0x4000 = mode (0=weights, 1=logits, 2=generate)
;   0x4004 = scroll offset
;   0x4008 = generation result buffer (256 words = 256 chars)
;   0x4100 = status text buffer
;   0x4200 = server response buffer

#define MODE        0x4000
#define SCROLL      0x4004
#define GEN_BUF     0x4008
#define STATUS_BUF  0x4100
#define RESP_BUF    0x4200

START:
    ; Init mode to weights view
    LDI r1, 1
    LDI r10, MODE
    STORE r10, r1
    
    ; Init scroll
    LDI r1, 0
    LDI r10, SCROLL
    STORE r10, r1
    
    ; Title
    LDI r20, STATUS_BUF
    STRO r20, "PIXELFLOW NEURAL ENGINE"

main_loop:
    ; Clear screen
    LDI r2, 0x111122
    FILL r2

    ; Draw header bar
    LDI r5, 0x00FF00     ; green header
    LDI r10, 0
    LDI r11, 0
    LDI r12, 256
    LDI r13, 16
    RECTF r10, r11, r12, r13, r5

    ; Title text
    LDI r10, 4
    LDI r11, 2
    LDI r15, STATUS_BUF
    DRAWTEXT r10, r11, r15, r16, r5

    ; Draw weight visualization area (green border)
    LDI r5, 0x00FF00
    LDI r10, 4
    LDI r11, 20
    LDI r12, 248
    LDI r13, 180
    RECTF r10, r11, r12, r13, r5

    ; Inner area (dark)
    LDI r5, 0x0A0A1A
    LDI r10, 6
    LDI r11, 22
    LDI r12, 244
    LDI r13, 176
    RECTF r10, r11, r12, r13, r5

    ; Draw simulated weight pattern (diagonal stripes = placeholder)
    ; In real version, this reads from pixelflow server
    LDI r5, 0x334455
    LDI r6, 8            ; y start
    draw_row_loop:
        LDI r7, 10       ; x start
        draw_col_loop:
            ; Compute color based on position (simulated weight heatmap)
            ; Blue channel = position-based pattern
            MOV r8, r7
            ADD r8, r6
            LDI r9, 0xFF
            AND r8, r9       ; mod 256
            SHLI r8, 16       ; blue channel
            OR r8, r5        ; mix with base
            
            ; Draw 2x2 pixel block
            LDI r10, 2
            RECTF r7, r6, r10, r10, r8
            
            ADDI r7, 3
            LDI r15, 250
            CMP r7, r15
            BLT r0, draw_col_loop
        
        ADDI r6, 3
        LDI r15, 198
        CMP r6, r15
        BLT r0, draw_row_loop

    ; Draw logits bar chart at bottom
    LDI r5, 0x444444
    LDI r10, 4
    LDI r11, 204
    LDI r12, 248
    LDI r13, 40
    RECTF r10, r11, r12, r13, r5

    ; Simulated logits bars (5 bars)
    LDI r16, 0       ; bar counter
    bar_loop:
        ; Bar color: gradient from green to red
        SHLI r16, 5
        LDI r17, 0x00FF00
        ADD r17, r16     ; shift hue
        MOV r5, r17
        
        ; Bar position
        LDI r1, 1
        SHLI r16, 5       ; x = counter * 32
        ADD r16, r1      ; +1
        ADD r16, r1      ; +1 = +2
        LDI r10, 8       ; x start
        ADD r10, r16
        
        ; Bar height (simulated)
        LDI r11, 230
        LDI r12, 20      ; bar width
        LDI r13, 10      ; bar height (would be logits)
        
        RECTF r10, r11, r12, r13, r5
        
        ADDI r16, 1
        LDI r15, 8
        CMP r16, r15
        BLT r0, bar_loop

    ; Status text at bottom
    LDI r10, 4
    LDI r11, 246
    LDI r15, STATUS_BUF
    DRAWTEXT r10, r11, r15, r16, r2

    ; Render frame
    FRAME
    
    ; Check for key input
    IKEY r5
    JZ r5, main_loop
    
    ; ESC = exit
    LDI r15, 27
    CMPI r5, 27
    JZ r0, exit_app
    
    ; Tab = switch mode
    LDI r15, 9
    CMPI r5, 9
    JNZ r0, main_loop
    
    ; Toggle mode
    LDI r10, MODE
    LOAD r11, r10
    LDI r1, 1
    ADD r11, r1
    LDI r15, 3
    CMPI r11, 3
    BLT r0, store_mode
    LDI r11, 0
    store_mode:
    STORE r10, r11
    
    ; Update status text
    LDI r20, STATUS_BUF
    LDI r10, MODE
    LOAD r11, r10
    CMPI r11, 0
    JNZ r0, try_logits
    STRO r20, "MODE: WEIGHTS"
    JMP main_loop
    try_logits:
    CMPI r11, 1
    JNZ r0, try_gen
    STRO r20, "MODE: LOGITS"
    JMP main_loop
    try_gen:
    STRO r20, "MODE: GENERATE"

    JMP main_loop

exit_app:
    HALT