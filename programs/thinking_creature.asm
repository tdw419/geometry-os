; thinking_creature.asm -- One LLM-driven creature on a simple world
;
; A red creature wanders the map. Every ~15 moves its plan runs out and it
; calls the LLM opcode to pick a fresh direction (N, S, E, or W). The game
; freezes briefly while the LLM responds -- that's expected, LLM is blocking.
;
; Controls: arrow keys move the white player (viewport center).
;
; Memory:
;   0x7000 creature world_x
;   0x7001 creature world_y
;   0x7002 plan_dir   (0=up, 1=down, 2=left, 3=right)
;   0x7003 plan_steps (countdown; 0 -> call LLM)
;   0x7800 camera_x
;   0x7801 camera_y
;   0x7802 frame_counter
;   0xB000 LLM prompt buffer
;   0xC000 LLM response buffer
;   0xFFB  key bitmask

; ===== Constants in registers =====
LDI r7, 1                ; the ubiquitous +1
LDI r8, 64               ; TILES per axis
LDI r9, 4                ; tile pixels
LDI r10, 0xFFB           ; key bitmask port
LDI r11, 0x7800          ; camera_x addr
LDI r12, 0x7801          ; camera_y addr
LDI r13, 0x7802          ; frame_counter addr
LDI r28, 32              ; viewport center

; ===== Write prompt string once to 0xB000 =====
LDI r17, 0xB000
STRO r17, "Pick one direction. Reply with a single letter: N, S, E, or W."

; ===== Init creature at (34, 33) =====
LDI r17, 0x7000
LDI r18, 34
STORE r17, r18           ; creature_x
LDI r17, 0x7001
LDI r18, 33
STORE r17, r18           ; creature_y
LDI r17, 0x7002
LDI r18, 0
STORE r17, r18           ; plan_dir = 0 (up)
LDI r17, 0x7003
LDI r18, 0
STORE r17, r18           ; plan_steps = 0 -> will trigger LLM on first update

; ===== Init camera at (0, 0) so player starts at world (32, 32) =====
LDI r17, 0
STORE r11, r17
STORE r12, r17
STORE r13, r17           ; frame_counter = 0

main_loop:
LDI r28, 32              ; reload center (clobbered by subroutines)

; --- Increment frame counter ---
LOAD r17, r13
ADD r17, r7
STORE r13, r17

; --- Read camera ---
LOAD r14, r11            ; camera_x
LOAD r15, r12            ; camera_y

; --- Read keys ---
LOAD r16, r10            ; key bitmask

; Up (bit 0)
MOV r17, r16
LDI r18, 1
AND r17, r18
JZ r17, no_up
SUB r15, r7
no_up:

; Down (bit 1)
MOV r17, r16
LDI r18, 2
AND r17, r18
JZ r17, no_down
ADD r15, r7
no_down:

; Left (bit 2)
MOV r17, r16
LDI r18, 4
AND r17, r18
JZ r17, no_left
SUB r14, r7
no_left:

; Right (bit 3)
MOV r17, r16
LDI r18, 8
AND r17, r18
JZ r17, no_right
ADD r14, r7
no_right:

STORE r11, r14
STORE r12, r15

; ===== Update creature (every 8 frames) =====
LOAD r22, r13
LDI r17, 7
AND r22, r17
JNZ r22, skip_creature

; plan_steps == 0? -> call LLM
LDI r17, 0x7003
LOAD r18, r17            ; r18 = plan_steps
JNZ r18, do_move

CALL think_creature
JMP skip_creature        ; don't also move this tick

do_move:
CALL move_creature

skip_creature:

; ===== Render =====
LDI r17, 0
FILL r17

; Tile grid
LOAD r22, r13            ; frame_counter for animation
LDI r1, 0

render_y:
  LDI r2, 0
render_x:
    MOV r3, r14
    ADD r3, r2           ; world_x
    MOV r4, r15
    ADD r4, r1           ; world_y

    ; Hash for biome (coarse)
    MOV r5, r3
    LDI r18, 3
    SHR r5, r18
    LDI r18, 99001
    MUL r5, r18
    MOV r6, r4
    LDI r18, 3
    SHR r6, r18
    LDI r18, 79007
    MUL r6, r18
    XOR r5, r6
    LDI r18, 1103515245
    MUL r5, r18
    LDI r18, 28
    SHR r5, r18          ; biome 0..15

    LDI r18, 3
    CMP r5, r18
    BLT r0, tile_water
    LDI r18, 8
    CMP r5, r18
    BLT r0, tile_grass
    LDI r18, 12
    CMP r5, r18
    BLT r0, tile_forest
    JMP tile_rock

tile_water:
    LDI r17, 0x0055AA
    JMP tile_draw
tile_grass:
    LDI r17, 0x33AA22
    JMP tile_draw
tile_forest:
    LDI r17, 0x116600
    JMP tile_draw
tile_rock:
    LDI r17, 0x888888

tile_draw:
    MOV r3, r2
    MUL r3, r9
    MOV r4, r1
    MUL r4, r9
    RECTF r3, r4, r9, r9, r17

    ADD r2, r7
    MOV r18, r2
    SUB r18, r8
    JZ r18, next_row
    JMP render_x

next_row:
    ADD r1, r7
    MOV r18, r1
    SUB r18, r8
    JZ r18, render_done
    JMP render_y

render_done:

; --- Draw creature ---
LDI r17, 0x7000
LOAD r3, r17
LDI r17, 0x7001
LOAD r4, r17
LDI r17, 0xFF3322        ; bright red
CALL draw_entity

; --- Draw player at viewport center ---
LDI r3, 128
LDI r4, 128
LDI r17, 0xFFFFFF
RECTF r3, r4, r9, r9, r17

FRAME
JMP main_loop


; ===== Subroutine: move_creature =====
; Steps the creature one tile in its plan_dir, then plan_steps--.
; Clobbers: r3, r4, r17, r18
move_creature:
  LDI r17, 0x7000
  LOAD r3, r17           ; world_x
  LDI r17, 0x7001
  LOAD r4, r17           ; world_y
  LDI r17, 0x7002
  LOAD r18, r17          ; plan_dir

  ; Branch on dir
  JZ r18, mv_up
  LDI r19, 1
  CMP r18, r19
  JZ r0, mv_down
  LDI r19, 2
  CMP r18, r19
  JZ r0, mv_left
  JMP mv_right

mv_up:
  SUB r4, r7
  JMP mv_write
mv_down:
  ADD r4, r7
  JMP mv_write
mv_left:
  SUB r3, r7
  JMP mv_write
mv_right:
  ADD r3, r7

mv_write:
  LDI r17, 0x7000
  STORE r17, r3
  LDI r17, 0x7001
  STORE r17, r4

  ; plan_steps--
  LDI r17, 0x7003
  LOAD r18, r17
  SUB r18, r7
  STORE r17, r18
  RET


; ===== Subroutine: think_creature =====
; Calls LLM, parses N/S/E/W from response, updates plan_dir + plan_steps.
; Falls back to a RAND direction if no recognizable letter appears.
think_creature:
  ; r_prompt, r_response, r_max
  LDI r3, 0xB000
  LDI r4, 0xC000
  LDI r5, 256
  LLM r3, r4, r5         ; r0 = response length

  ; Default: random direction if parse fails
  RAND r20
  LDI r18, 3
  AND r20, r18           ; r20 = 0..3

  ; Scan response buffer for first N/S/E/W letter
  LDI r21, 0xC000        ; cursor
  MOV r22, r0            ; remaining chars to scan
  JZ r22, tc_done        ; empty response -> keep random

tc_scan:
  LOAD r23, r21          ; char
  JZ r23, tc_done        ; null terminator -> stop

  LDI r18, 78            ; 'N'
  CMP r23, r18
  JZ r0, tc_pick_up
  LDI r18, 110           ; 'n'
  CMP r23, r18
  JZ r0, tc_pick_up

  LDI r18, 83            ; 'S'
  CMP r23, r18
  JZ r0, tc_pick_down
  LDI r18, 115           ; 's'
  CMP r23, r18
  JZ r0, tc_pick_down

  LDI r18, 69            ; 'E'
  CMP r23, r18
  JZ r0, tc_pick_right
  LDI r18, 101           ; 'e'
  CMP r23, r18
  JZ r0, tc_pick_right

  LDI r18, 87            ; 'W'
  CMP r23, r18
  JZ r0, tc_pick_left
  LDI r18, 119           ; 'w'
  CMP r23, r18
  JZ r0, tc_pick_left

  ADD r21, r7
  SUB r22, r7
  JNZ r22, tc_scan
  JMP tc_done

tc_pick_up:
  LDI r20, 0
  JMP tc_done
tc_pick_down:
  LDI r20, 1
  JMP tc_done
tc_pick_left:
  LDI r20, 2
  JMP tc_done
tc_pick_right:
  LDI r20, 3

tc_done:
  ; Commit plan_dir
  LDI r17, 0x7002
  STORE r17, r20
  ; plan_steps = 15
  LDI r17, 0x7003
  LDI r18, 15
  STORE r17, r18
  RET


; ===== Subroutine: draw_entity =====
; Draws a 4x4 rect at (world_x, world_y) in r3, r4 with color in r17 if on-screen.
; Clobbers: r3, r4, r5, r6, r18
draw_entity:
  LOAD r5, r11            ; camera_x
  MOV r6, r3
  SUB r6, r5              ; r6 = world_x - camera_x (wraps if negative)
  LDI r18, 64
  CMP r6, r18
  BGE r0, de_done
  MOV r3, r6
  LDI r18, 4
  MUL r3, r18             ; pixel x

  LOAD r5, r12            ; camera_y
  MOV r6, r4
  SUB r6, r5
  LDI r18, 64
  CMP r6, r18
  BGE r0, de_done
  MOV r4, r6
  LDI r18, 4
  MUL r4, r18             ; pixel y

  RECTF r3, r4, r9, r9, r17

de_done:
  RET
