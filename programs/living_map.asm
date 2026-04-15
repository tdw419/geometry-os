; living_map.asm -- Stateful infinite world with wandering creatures
;
; Extends infinite_map.asm with:
;   Layer 1: Sparse state overlay (footstep trails)
;   Layer 3: Simulated creatures (wander AI, no SPAWN needed)
;
; Player is always at viewport center (tile 32,32).
; Footstep trails mark visited tiles as "path" color.
; Creatures wander the world via random walk, drawn on top of terrain.
;
; Memory:
;   RAM[0x7800] = camera_x (tile coordinates)
;   RAM[0x7801] = camera_y (tile coordinates)
;   RAM[0x7802] = frame_counter
;   RAM[0x7803] = player_world_x (camera_x + 32)
;   RAM[0x7804] = player_world_y (camera_y + 32)
;   RAM[0x7805] = prev_player_x (for detecting movement)
;   RAM[0x7806] = prev_player_y
;   RAM[0x7807] = state_count (number of state entries, 0..64)
;   RAM[0x7808] = state_head (circular buffer write index)
;   RAM[0x9000..0x92FF] = state table (64 entries * 3 words: world_x, world_y, color)
;   RAM[0x7000..0x7002] = creature 0 (world_x, world_y, dir_seed)
;   RAM[0x7004..0x7006] = creature 1
;   RAM[0x7008..0x700A] = creature 2
;   RAM[0xFFB]  = key bitmask (host writes each frame)

; ===== Constants =====
LDI r7, 1               ; constant 1
LDI r8, 64              ; TILES per axis
LDI r9, 4               ; TILE_SIZE pixels
LDI r10, 0xFFB          ; key bitmask port
LDI r11, 0x7800         ; camera_x address
LDI r12, 0x7801         ; camera_y address
LDI r13, 0x7802         ; frame_counter address
LDI r28, 32             ; center offset

; ===== Initialize state table =====
LDI r17, 0x7807
LDI r18, 0
STORE r17, r18          ; state_count = 0
LDI r17, 0x7808
STORE r17, r18          ; state_head = 0

; ===== Initialize player position =====
LDI r17, 0x7803
LDI r18, 32
STORE r17, r18          ; player_world_x = 32
LDI r17, 0x7804
STORE r17, r18          ; player_world_y = 32
LDI r17, 0x7805
STORE r17, r18          ; prev_player_x = 32
LDI r17, 0x7806
STORE r17, r18          ; prev_player_y = 32

; ===== Main Loop =====
main_loop:
LDI r28, 32             ; center offset (reloaded because r28 gets clobbered)

; --- Increment frame counter ---
LOAD r17, r13
ADD r17, r7
STORE r13, r17          ; frame_counter++

; --- Read camera position ---
LOAD r14, r11           ; r14 = camera_x
LOAD r15, r12           ; r15 = camera_y

; --- Read key bitmask ---
LOAD r16, r10           ; r16 = key bitmask

; --- Store previous player position ---
LDI r17, 0x7803
LOAD r18, r17
LDI r17, 0x7805
STORE r17, r18          ; prev_player_x = player_world_x
LDI r17, 0x7804
LOAD r18, r17
LDI r17, 0x7806
STORE r17, r18          ; prev_player_y = player_world_y

; --- Process Up (bit 0) ---
MOV r17, r16
LDI r18, 1
AND r17, r18
JZ r17, no_up
SUB r15, r7
no_up:

; --- Process Down (bit 1) ---
MOV r17, r16
LDI r18, 2
AND r17, r18
JZ r17, no_down
ADD r15, r7
no_down:

; --- Process Left (bit 2) ---
MOV r17, r16
LDI r18, 4
AND r17, r18
JZ r17, no_left
SUB r14, r7
no_left:

; --- Process Right (bit 3) ---
MOV r17, r16
LDI r18, 8
AND r17, r18
JZ r17, no_right
ADD r14, r7
no_right:

; --- Store updated camera ---
STORE r11, r14
STORE r12, r15

; --- Compute and store player world position ---
MOV r17, r14
ADD r17, r28
LDI r18, 0x7803
STORE r18, r17          ; player_world_x = camera_x + 32
MOV r17, r15
ADD r17, r28
LDI r18, 0x7804
STORE r18, r17          ; player_world_y = camera_y + 32

; --- Check if player moved, add footstep trail ---
LDI r17, 0x7803
LOAD r19, r17           ; r19 = player_world_x (current)
LDI r17, 0x7805
LOAD r18, r17           ; r18 = prev_player_x
CMP r19, r18
JNZ r0, add_footstep
LDI r17, 0x7804
LOAD r19, r17
LDI r17, 0x7806
LOAD r18, r17
CMP r19, r18
JZ r0, skip_footstep

add_footstep:
; Read prev position (tile we're leaving)
LDI r17, 0x7805
LOAD r20, r17           ; r20 = prev_player_x
LDI r17, 0x7806
LOAD r21, r17           ; r21 = prev_player_y

; Add to state table: state[state_head] = (x, y, path_color)
LDI r17, 0x7808
LOAD r22, r17           ; r22 = state_head
LDI r23, 0x9000         ; state table base
MOV r24, r22
LDI r25, 3
MUL r24, r25            ; r24 = head * 3
ADD r23, r24            ; r23 = &state[head][0]

STORE r23, r20          ; world_x
ADD r23, r7
STORE r23, r21          ; world_y
ADD r23, r7
LDI r25, 0xC2A280       ; path/dirt color
STORE r23, r25

; Advance head (circular, 64 entries)
ADD r22, r7
LDI r25, 64
MOV r26, r22
SUB r26, r25
JNZ r26, no_wrap
LDI r22, 0
no_wrap:
LDI r17, 0x7808
STORE r17, r22

; Update state_count (cap at 64)
LDI r17, 0x7807
LOAD r22, r17
ADD r22, r7
LDI r25, 64
MOV r26, r22
SUB r26, r25
JNZ r26, no_cap
LDI r22, 64
no_cap:
LDI r17, 0x7807
STORE r17, r22

skip_footstep:

; ===== Update Creatures (every 8th frame) =====
; Throttle: only move creatures when frame_counter & 7 == 0
LOAD r22, r13           ; r22 = frame_counter
LDI r17, 7
AND r22, r17
JNZ r22, skip_creatures

; --- Creature 0 at 0x7000 ---
LDI r23, 0x7000         ; creature 0 base
CALL update_creature

; --- Creature 1 at 0x7004 ---
LDI r23, 0x7004         ; creature 1 base
CALL update_creature

; --- Creature 2 at 0x7008 ---
LDI r23, 0x7008         ; creature 2 base
CALL update_creature

skip_creatures:

; --- Clear screen to black ---
LDI r17, 0
FILL r17

; ===== Render Viewport (terrain) =====
LOAD r22, r13           ; r22 = frame_counter
LDI r1, 0               ; ty = 0

render_y:
  LDI r2, 0             ; tx = 0

  render_x:
    ; World coordinates
    MOV r3, r14
    ADD r3, r2           ; r3 = world_x
    MOV r4, r15
    ADD r4, r1           ; r4 = world_y

    ; ---- Coarse hash for contiguous biomes ----
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
    MUL r5, r18          ; mix

    LDI r18, 28
    SHR r5, r18          ; r5 = biome_type (0..15)

    ; ---- Fine hash for structure placement ----
    MOV r6, r3
    LDI r18, 374761393
    MUL r6, r18
    MOV r21, r4
    LDI r18, 668265263
    MUL r21, r18
    XOR r6, r21

    LDI r18, 0xFF
    MOV r21, r6
    AND r21, r18
    LDI r18, 42
    CMP r21, r18
    JNZ r0, no_struct

    LDI r18, 4
    CMP r5, r18
    BLT r0, struct_water
    LDI r18, 9
    CMP r5, r18
    BLT r0, struct_land
    LDI r18, 12
    CMP r5, r18
    BLT r0, struct_mountain
    JMP struct_snow

struct_water:
    LDI r17, 0x0066CC
    JMP do_rect
struct_land:
    LDI r17, 0x884422
    JMP do_rect
struct_mountain:
    LDI r17, 0xBBBBCC
    JMP do_rect
struct_snow:
    LDI r17, 0xAABBEE
    JMP do_rect

no_struct:
    ; ---- Biome -> Color ----
    LDI r18, 3
    CMP r5, r18
    BLT r0, color_water

    LDI r18, 4
    CMP r5, r18
    BLT r0, color_beach

    LDI r18, 7
    CMP r5, r18
    BLT r0, color_grass

    LDI r18, 9
    CMP r5, r18
    BLT r0, color_forest

    LDI r18, 12
    CMP r5, r18
    BLT r0, color_mountain

    JMP color_snow

color_water:
    LDI r18, 1
    CMP r5, r18
    BLT r0, water_deep
    LDI r18, 2
    CMP r5, r18
    JZ r0, water_shallow
    LDI r17, 0x000088
    JMP water_animate
water_deep:
    LDI r17, 0x000044
    JMP water_animate
water_shallow:
    LDI r17, 0x0000BB

water_animate:
    MOV r21, r22
    ADD r21, r3
    ADD r21, r4
    LDI r18, 0x1F
    AND r21, r18
    ADD r17, r21
    JMP do_rect

color_beach:
    LDI r17, 0xC2B280
    JMP do_rect

color_grass:
    LDI r18, 5
    CMP r5, r18
    BLT r0, grass_light
    LDI r18, 6
    CMP r5, r18
    JZ r0, grass_dark
    LDI r17, 0x33AA22
    JMP do_rect
grass_light:
    LDI r17, 0x55BB33
    JMP do_rect
grass_dark:
    LDI r17, 0x228811
    JMP do_rect

color_forest:
    LDI r18, 8
    CMP r5, r18
    JZ r0, forest_dense
    LDI r17, 0x116600
    JMP do_rect
forest_dense:
    LDI r17, 0x0A4400
    JMP do_rect

color_mountain:
    LDI r18, 10
    CMP r5, r18
    BLT r0, mt_low
    LDI r18, 11
    CMP r5, r18
    JZ r0, mt_tall
    LDI r17, 0x888888
    JMP do_rect
mt_low:
    LDI r17, 0x667766
    JMP do_rect
mt_tall:
    LDI r17, 0x999999
    JMP do_rect

color_snow:
    LDI r18, 14
    CMP r5, r18
    BLT r0, snow_light
    LDI r18, 15
    CMP r5, r18
    JZ r0, snow_peak
    LDI r17, 0xDDEEFF
    JMP do_rect
snow_light:
    LDI r17, 0xCCCCEE
    JMP do_rect
snow_peak:
    LDI r17, 0xFFFFFF
    JMP do_rect

    ; ---- Draw tile ----
do_rect:
    MOV r3, r2
    MUL r3, r9           ; r3 = tx * 4
    MOV r4, r1
    MUL r4, r9           ; r4 = ty * 4
    RECTF r3, r4, r9, r9, r17

    ; ---- Next tile ----
    ADD r2, r7
    MOV r18, r2
    SUB r18, r8
    JZ r18, next_row
    JMP render_x

next_row:
    ADD r1, r7
    MOV r18, r1
    SUB r18, r8
    JZ r18, render_state
    JMP render_y

; ===== Render State Overlay =====
render_state:
LDI r17, 0x7807
LOAD r20, r17           ; r20 = state_count
LDI r19, 0              ; entry index
LDI r23, 0x9000         ; state table base

state_loop:
  CMP r19, r20
  JNZ r0, state_check
  JMP state_done

state_check:
  MOV r24, r19
  LDI r25, 3
  MUL r24, r25
  MOV r26, r23
  ADD r26, r24

  LOAD r27, r26          ; world_x
  ADD r26, r7
  LOAD r28, r26          ; world_y
  ADD r26, r7
  LOAD r17, r26          ; color

  ; Check on-screen: camera_x <= world_x < camera_x + 64
  LOAD r3, r11           ; camera_x
  MOV r4, r27
  CMP r4, r3
  BLT r0, state_next

  MOV r4, r3
  ADD r4, r8
  MOV r3, r27
  CMP r3, r4
  BGE r0, state_next

  LOAD r3, r12           ; camera_y
  MOV r4, r28
  CMP r4, r3
  BLT r0, state_next

  MOV r4, r3
  ADD r4, r8
  MOV r3, r28
  CMP r3, r4
  BGE r0, state_next

  ; Compute screen position
  MOV r3, r27
  LOAD r4, r11
  SUB r3, r4
  LDI r25, 4
  MUL r3, r25

  MOV r4, r28
  LOAD r26, r12
  SUB r4, r26
  LDI r25, 4
  MUL r4, r25

  RECTF r3, r4, r9, r9, r17

state_next:
  ADD r19, r7
  JMP state_loop

state_done:

; ===== Render Creatures =====
; Creature 0 (orange-red)
LDI r17, 0x7000
LOAD r3, r17             ; world_x
LDI r17, 0x7001
LOAD r4, r17             ; world_y
LDI r17, 0xFF6633        ; orange-red
CALL draw_creature

; Creature 1 (gold)
LDI r17, 0x7004
LOAD r3, r17
LDI r17, 0x7005
LOAD r4, r17
LDI r17, 0xFFCC00        ; gold
CALL draw_creature

; Creature 2 (purple)
LDI r17, 0x7008
LOAD r3, r17
LDI r17, 0x7009
LOAD r4, r17
LDI r17, 0xCC33FF        ; purple
CALL draw_creature

; ===== Render Player =====
LDI r3, 128
LDI r4, 128
LDI r17, 0xFFFFFF        ; white
RECTF r3, r4, r9, r9, r17

; ===== End Frame =====
FRAME
JMP main_loop


; ===== Subroutine: update_creature =====
; Input: r23 = creature base address (world_x, world_y, dir_seed)
; Uses: r3-r6, r18-r21
; Clobbers: r0 (CMP), r3-r6, r18-r21
update_creature:
  PUSH r23
  PUSH r7

  LDI r7, 1

  ; Load position
  LOAD r3, r23           ; r3 = world_x
  LDI r18, 1
  ADD r23, r18
  LOAD r4, r23           ; r4 = world_y
  ADD r23, r18
  LOAD r5, r23           ; r5 = dir_seed

  ; Random direction
  RAND r6
  LDI r18, 3
  AND r6, r18            ; r6 = 0..3

  LDI r18, 1
  CMP r6, r18
  JZ r0, uc_down
  LDI r18, 2
  CMP r6, r18
  JZ r0, uc_left
  LDI r18, 3
  CMP r6, r18
  JZ r0, uc_right

uc_up:
  SUB r4, r7
  JMP uc_update
uc_down:
  ADD r4, r7
  JMP uc_update
uc_left:
  SUB r3, r7
  JMP uc_update
uc_right:
  ADD r3, r7

uc_update:
  ; Write back
  ; r23 currently points to dir_seed, need to reset to base
  POP r7
  POP r23
  STORE r23, r3           ; world_x
  LDI r18, 1
  ADD r23, r18
  STORE r23, r4           ; world_y
  ADD r23, r18
  ; Update seed with XOR for variety
  LDI r18, 2654435761     ; golden ratio constant
  XOR r5, r18
  STORE r23, r5           ; dir_seed

  RET


; ===== Subroutine: draw_creature =====
; Input: r3 = world_x, r4 = world_y, r17 = color
; Draws a 4x4 creature tile if on-screen
; Clobbers: r0, r3, r4, r5, r6, r18
draw_creature:
  ; Compute screen position: (world_x - camera_x) * 4
  LOAD r5, r11            ; camera_x
  ; Check: world_x < camera_x? (unsigned wrap = offscreen)
  MOV r6, r3
  SUB r6, r5              ; r6 = world_x - camera_x
  ; If world_x < camera_x, r6 wraps to huge number
  LDI r18, 64
  CMP r6, r18
  BGE r0, dc_done         ; off-screen right or wrapped left

  MOV r3, r6
  LDI r18, 4
  MUL r3, r18             ; pixel x

  LOAD r5, r12            ; camera_y
  MOV r6, r4
  SUB r6, r5
  CMP r6, r18             ; reuse r18=64 (still >= check)
  ; Actually r18 was overwritten to 4. Recompute.
  LDI r18, 64
  CMP r6, r18
  BGE r0, dc_done

  MOV r4, r6
  LDI r18, 4
  MUL r4, r18             ; pixel y

  RECTF r3, r4, r9, r9, r17

dc_done:
  RET
