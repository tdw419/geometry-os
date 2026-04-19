; roguelike.asm -- Procedural Dungeon Crawler
;
; Controls: WASD to move, R to regenerate dungeon
; Goal: find the golden stairs to descend deeper
;
; Screen: 256x256, dungeon 32x32 tiles at 8px each
; Uses TILEMAP opcode for efficient whole-screen rendering
;
; Algorithm: Random room placement + L-shaped corridors
;   1. Fill 32x32 grid with walls
;   2. Place up to 7 random rooms (4-9 wide, 3-7 tall)
;   3. Connect room centers with L-shaped corridors
;   4. Place stairs in last room, player in first room
;
; Tile types (in map): 0=void, 1=floor, 2=wall, 3=stairs
; Tile 0 is skipped by TILEMAP (transparent)
;
; Memory layout:
;   0x5000..0x53FF  map[1024]        tile indices (32x32)
;   0x5400..0x547F  tile_data[192]   pixel data (3 tiles x 8x8)
;   0x5500..0x551F  rooms[32]        room data (8 rooms x 4 words)
;   0x5520          room_count
;   0x5530          player_x
;   0x5531          player_y
;   0x5532          stairs_x
;   0x5533          stairs_y
;   0x5534          dungeon_level
;   0x5535          game_state (0=play, 1=descend)
;   0x5540..0x555F  text strings for HUD
;   0x5560..0x5561  temp: corridor start cx, cy

#define MAP_BASE   0x5000
#define TILE_BASE  0x5400
#define ROOM_BASE  0x5500
#define ROOM_COUNT 0x5520
#define P_X        0x5530
#define P_Y        0x5531
#define STAIRS_X   0x5532
#define STAIRS_Y   0x5533
#define DLEVEL     0x5534
#define STATE      0x5535
#define TILE_FLOOR 1
#define TILE_WALL  2
#define TILE_STAIR 3
#define MAP_W      32
#define MAP_H      32
#define TILE_SZ    8

; ── Entry Point ──────────────────────────────────────────────

restart:
  LDI r30, 0x8000       ; stack pointer
  CALL init_tiles        ; set up tile pixel data
  CALL init_text         ; store HUD text strings
  CALL generate_dungeon  ; procedural dungeon generation
  CALL render            ; draw initial frame

; ── Main Game Loop ───────────────────────────────────────────

game_loop:
  ; Check game state
  LDI r4, STATE
  LOAD r1, r4
  LDI r9, 1
  CMP r1, r9
  JZ r0, descend_screen  ; found stairs

  ; Read keyboard
  IKEY r7
  JZ r7, idle           ; no key, just frame

  ; W = up
  LDI r6, 87
  CMP r7, r6
  JZ r0, try_up
  LDI r6, 119
  CMP r7, r6
  JZ r0, try_up

  ; S = down
  LDI r6, 83
  CMP r7, r6
  JZ r0, try_down
  LDI r6, 115
  CMP r7, r6
  JZ r0, try_down

  ; A = left
  LDI r6, 65
  CMP r7, r6
  JZ r0, try_left
  LDI r6, 97
  CMP r7, r6
  JZ r0, try_left

  ; D = right
  LDI r6, 68
  CMP r7, r6
  JZ r0, try_right
  LDI r6, 100
  CMP r7, r6
  JZ r0, try_right

  ; R = restart same level
  LDI r6, 82
  CMP r7, r6
  JZ r0, restart
  LDI r6, 114
  CMP r7, r6
  JZ r0, restart

  JMP idle

; ── Movement with map-based collision ────────────────────────

try_up:
  LDI r4, P_X
  LOAD r2, r4            ; r2 = px
  LDI r4, P_Y
  LOAD r1, r4
  LDI r9, 1
  SUB r1, r9             ; r1 = py - 1
  CALL get_tile           ; r1 = map[py-1][px]
  LDI r9, TILE_WALL
  CMP r1, r9
  JZ r0, idle            ; wall blocks movement
  LDI r4, P_Y
  LOAD r1, r4
  LDI r9, 1
  SUB r1, r9
  STORE r4, r1           ; py--
  CALL check_stairs
  JMP do_move

try_down:
  LDI r4, P_X
  LOAD r2, r4
  LDI r4, P_Y
  LOAD r1, r4
  LDI r9, 1
  ADD r1, r9             ; r1 = py + 1
  CALL get_tile
  LDI r9, TILE_WALL
  CMP r1, r9
  JZ r0, idle
  LDI r4, P_Y
  LOAD r1, r4
  LDI r9, 1
  ADD r1, r9
  STORE r4, r1           ; py++
  CALL check_stairs
  JMP do_move

try_left:
  LDI r4, P_X
  LOAD r1, r4
  LDI r9, 1
  SUB r1, r9             ; r1 = px - 1
  LDI r4, P_Y
  LOAD r2, r4
  CALL get_tile
  LDI r9, TILE_WALL
  CMP r1, r9
  JZ r0, idle
  LDI r4, P_X
  LOAD r1, r4
  LDI r9, 1
  SUB r1, r9
  STORE r4, r1           ; px--
  CALL check_stairs
  JMP do_move

try_right:
  LDI r4, P_X
  LOAD r1, r4
  LDI r9, 1
  ADD r1, r9             ; r1 = px + 1
  LDI r4, P_Y
  LOAD r2, r4
  CALL get_tile
  LDI r9, TILE_WALL
  CMP r1, r9
  JZ r0, idle
  LDI r4, P_X
  LOAD r1, r4
  LDI r9, 1
  ADD r1, r9
  STORE r4, r1           ; px++
  CALL check_stairs
  JMP do_move

do_move:
  CALL render
  LDI r5, 220
  LDI r6, 25
  BEEP r5, r6            ; step sound

idle:
  FRAME
  JMP game_loop

; ── Descend Screen ───────────────────────────────────────────

descend_screen:
  LDI r1, 0x001a00
  FILL r1
  LDI r10, 0x5540
  LDI r11, 50
  LDI r12, 110
  TEXT r11, r12, r10
  LDI r10, 0x5550
  LDI r11, 50
  LDI r12, 150
  TEXT r11, r12, r10
  FRAME
  IKEY r7
  JZ r7, descend_screen
  LDI r4, DLEVEL
  LOAD r1, r4
  LDI r9, 1
  ADD r1, r9
  STORE r4, r1           ; level++
  JMP restart

; ─────────────────────────────────────────────────────────────
; SUBROUTINES
; ─────────────────────────────────────────────────────────────

; ── init_tiles -- fill tile pixel data ───────────────────────
; Tile 1 (floor): 0x2A2A4E dark purple-gray
; Tile 2 (wall):  checkerboard 0x3A5A7A / 0x5A7AAA
; Tile 3 (stairs): 0xD4A017 gold

init_tiles:
  PUSH r31

  ; Floor tile (64 pixels at TILE_BASE+0)
  LDI r10, 0
it_fl:
  LDI r4, TILE_BASE
  ADD r4, r10
  LDI r1, 0x2A2A4E
  STORE r4, r1
  LDI r9, 1
  ADD r10, r9
  LDI r6, 64
  CMP r10, r6
  BLT r0, it_fl

  ; Wall tile with checkerboard (64 pixels at TILE_BASE+64)
  LDI r10, 0
it_wl:
  LDI r4, TILE_BASE
  LDI r9, 64
  ADD r4, r9
  ADD r4, r10
  ; Checkerboard: (row XOR col) & 1
  LDI r1, 0
  ADD r1, r10         ; r1 = idx
  LDI r2, 0
  ADD r2, r10         ; r2 = idx copy
  LDI r9, 8
  LDI r3, 0
  ADD r3, r1
  DIV r3, r9          ; r3 = row
  LDI r9, 8
  MOD r2, r9          ; r2 = col
  XOR r2, r3          ; r2 = row XOR col
  LDI r9, 1
  AND r2, r9          ; r2 = 0 or 1
  JZ r2, it_wl_dk
  LDI r1, 0x5A7AAA    ; lighter
  JMP it_wl_st
it_wl_dk:
  LDI r1, 0x3A5A7A    ; darker
it_wl_st:
  STORE r4, r1
  LDI r9, 1
  ADD r10, r9
  LDI r6, 64
  CMP r10, r6
  BLT r0, it_wl

  ; Stairs tile (64 pixels at TILE_BASE+128)
  LDI r10, 0
it_st:
  LDI r4, TILE_BASE
  LDI r9, 128
  ADD r4, r9
  ADD r4, r10
  LDI r1, 0xD4A017
  STORE r4, r1
  LDI r9, 1
  ADD r10, r9
  LDI r6, 64
  CMP r10, r6
  BLT r0, it_st

  POP r31
  RET

; ── init_text -- store HUD text strings ──────────────────────

init_text:
  ; "DESCENDED!" at 0x5540 (D=68, E=69, S=83, C=67, N=78, D=68, E=69, D=68, !=33)
  LDI r4, 0x5540
  LDI r1, 68
  STORE r4, r1
  LDI r4, 0x5541
  LDI r1, 69
  STORE r4, r1
  LDI r4, 0x5542
  LDI r1, 83
  STORE r4, r1
  LDI r4, 0x5543
  LDI r1, 67
  STORE r4, r1
  LDI r4, 0x5544
  LDI r1, 69
  STORE r4, r1
  LDI r4, 0x5545
  LDI r1, 78
  STORE r4, r1
  LDI r4, 0x5546
  LDI r1, 68
  STORE r4, r1
  LDI r4, 0x5547
  LDI r1, 69
  STORE r4, r1
  LDI r4, 0x5548
  LDI r1, 68
  STORE r4, r1
  LDI r4, 0x5549
  LDI r1, 33
  STORE r4, r1
  LDI r4, 0x554A
  LDI r1, 0
  STORE r4, r1
  ; "PRESS R" at 0x5550 (P=80, R=82, E=69, S=83, S=83, space=32, R=82)
  LDI r4, 0x5550
  LDI r1, 80
  STORE r4, r1
  LDI r4, 0x5551
  LDI r1, 82
  STORE r4, r1
  LDI r4, 0x5552
  LDI r1, 69
  STORE r4, r1
  LDI r4, 0x5553
  LDI r1, 83
  STORE r4, r1
  LDI r4, 0x5554
  LDI r1, 83
  STORE r4, r1
  LDI r4, 0x5555
  LDI r1, 32
  STORE r4, r1
  LDI r4, 0x5556
  LDI r1, 82
  STORE r4, r1
  LDI r4, 0x5557
  LDI r1, 0
  STORE r4, r1
  RET

; ── generate_dungeon ────────────────────────────────────────
; Procedural dungeon with rooms and corridors
; Clobbers: r10-r27

generate_dungeon:
  PUSH r31

  ; Step 1: Fill map with walls
  LDI r10, 0
gd_fy:
  LDI r11, 0
gd_fx:
  LDI r4, MAP_BASE
  LDI r9, MAP_W
  MUL r9, r10
  ADD r4, r9
  ADD r4, r11
  LDI r1, TILE_WALL
  STORE r4, r1
  LDI r9, 1
  ADD r11, r9
  LDI r6, MAP_W
  CMP r11, r6
  BLT r0, gd_fx
  LDI r9, 1
  ADD r10, r9
  LDI r6, MAP_H
  CMP r10, r6
  BLT r0, gd_fy

  ; Step 2: Place rooms (target 7, max 60 attempts)
  LDI r1, 0
  LDI r4, ROOM_COUNT
  STORE r4, r1
  LDI r25, 0            ; attempts

gd_rl:
  LDI r4, ROOM_COUNT
  LOAD r1, r4
  LDI r9, 7
  CMP r1, r9
  BGE r0, gd_rd        ; enough rooms
  LDI r9, 60
  CMP r25, r9
  BGE r0, gd_rd        ; max attempts
  LDI r9, 1
  ADD r25, r9

  ; Random width 4-9
  RAND r20
  LDI r9, 6
  MOD r20, r9
  LDI r9, 4
  ADD r20, r9

  ; Random height 3-7
  RAND r21
  LDI r9, 5
  MOD r21, r9
  LDI r9, 3
  ADD r21, r9

  ; Random x: 1 to MAP_W-width-2
  RAND r22
  LDI r9, MAP_W
  LDI r26, 0
  ADD r26, r20
  SUB r9, r26
  LDI r26, 2
  SUB r9, r26
  LDI r26, 1
  ADD r9, r26
  MOD r22, r9
  LDI r9, 1
  ADD r22, r9

  ; Random y: 1 to MAP_H-height-2
  RAND r23
  LDI r9, MAP_H
  LDI r26, 0
  ADD r26, r21
  SUB r9, r26
  LDI r26, 2
  SUB r9, r26
  LDI r26, 1
  ADD r9, r26
  MOD r23, r9
  LDI r9, 1
  ADD r23, r9

  ; Check overlap (r20-r23 = w,h,x,y)
  CALL check_room_overlap
  JNZ r1, gd_rl        ; overlap, retry

  ; Carve room
  CALL carve_room

  ; Store room data
  LDI r4, ROOM_COUNT
  LOAD r1, r4
  LDI r9, 4
  MUL r1, r9            ; offset = idx * 4
  ; room[idx].x
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 0
  ADD r9, r22
  STORE r4, r9
  ; room[idx].y
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  LDI r9, 0
  ADD r9, r23
  STORE r4, r9
  ; room[idx].w
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 2
  ADD r4, r9
  LDI r9, 0
  ADD r9, r20
  STORE r4, r9
  ; room[idx].h
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 3
  ADD r4, r9
  LDI r9, 0
  ADD r9, r21
  STORE r4, r9

  ; room_count++
  LDI r4, ROOM_COUNT
  LOAD r1, r4
  LDI r9, 1
  ADD r1, r9
  STORE r4, r1

  JMP gd_rl

gd_rd:

  ; Step 3: Connect rooms with L-shaped corridors
  LDI r4, ROOM_COUNT
  LOAD r24, r4           ; total rooms
  LDI r9, 2
  CMP r24, r9
  BLT r0, gd_cd         ; need >= 2 rooms

  LDI r25, 0             ; i = 0
gd_cl:
  LDI r9, 1
  LDI r26, 0
  ADD r26, r24
  SUB r26, r9            ; r26 = rooms - 1
  CMP r25, r26
  BGE r0, gd_cd         ; i >= rooms-1

  ; Compute center of room i
  LDI r1, r25
  LDI r9, 4
  MUL r1, r9
  LDI r4, ROOM_BASE
  ADD r4, r1
  LOAD r20, r4           ; rx
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  LOAD r21, r4           ; ry
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 2
  ADD r4, r9
  LOAD r26, r4           ; rw
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 3
  ADD r4, r9
  LOAD r27, r4           ; rh
  LDI r9, 2
  SHR r26, r9
  ADD r20, r26           ; cx_i
  LDI r9, 2
  SHR r27, r9
  ADD r21, r27           ; cy_i

  ; Save center of room i to temp RAM
  LDI r4, 0x5560
  LDI r9, 0
  ADD r9, r20
  STORE r4, r9           ; 0x5560 = cx_i
  LDI r4, 0x5561
  LDI r9, 0
  ADD r9, r21
  STORE r4, r9           ; 0x5561 = cy_i

  ; Compute center of room i+1
  LDI r1, r25
  LDI r9, 1
  ADD r1, r9
  LDI r9, 4
  MUL r1, r9
  LDI r4, ROOM_BASE
  ADD r4, r1
  LOAD r22, r4           ; rx
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  LOAD r23, r4           ; ry
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 2
  ADD r4, r9
  LOAD r26, r4           ; rw
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 3
  ADD r4, r9
  LOAD r27, r4           ; rh
  LDI r9, 2
  SHR r26, r9
  ADD r22, r26           ; cx_i1
  LDI r9, 2
  SHR r27, r9
  ADD r23, r27           ; cy_i1

  ; Carve horizontal corridor at cy_i from cx_i to cx_i1
  LDI r4, 0x5560
  LOAD r20, r4           ; r20 = from_x = cx_i
  LDI r4, 0x5561
  LOAD r21, r4           ; r21 = y = cy_i
  ; r22 = to_x = cx_i1 (already set)
  CALL carve_h_corridor

  ; Carve vertical corridor at cx_i1 from cy_i to cy_i1
  LDI r20, 0
  ADD r20, r21           ; r20 = from_y = cy_i (but wait, r21 was cy_i)
  ; Hmm, r21 was overwritten? No - r21 = cy_i from LOAD r4, 0x5561
  ; But then carve_h_corridor might clobber r21. Let me check.
  ; carve_h_corridor uses r10 and r14 only. r21 is safe.
  LDI r4, 0x5561
  LOAD r20, r4           ; r20 = from_y = cy_i
  LDI r21, 0
  ADD r21, r22           ; r21 = x = cx_i1
  LDI r22, 0
  ; Need cy_i1 in r22. It was in r23 but that might be clobbered too.
  ; carve_h_corridor only clobbers r10, r14. r22, r23 are safe.
  ; But wait, I did ADD r21, r22 above which changed r21 = cx_i1. And r22 still = cx_i1.
  ; I need cy_i1. Let me reload it.

  ; Reload cy_i1 by recomputing room i+1 center y
  LDI r1, r25
  LDI r9, 1
  ADD r1, r9
  LDI r9, 4
  MUL r1, r9
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  LOAD r23, r4           ; ry
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 3
  ADD r4, r9
  LOAD r27, r4           ; rh
  LDI r9, 2
  SHR r27, r9
  ADD r23, r27           ; r23 = cy_i1

  ; Now set up for carve_v_corridor
  LDI r4, 0x5561
  LOAD r20, r4           ; r20 = from_y = cy_i
  LDI r21, 0
  ADD r21, r22           ; r21 = x = cx_i1 (r22 still = cx_i1 from above)
  LDI r22, 0
  ADD r22, r23           ; r22 = to_y = cy_i1
  CALL carve_v_corridor

  LDI r9, 1
  ADD r25, r9
  JMP gd_cl

gd_cd:

  ; Step 4: Place stairs in last room center
  LDI r4, ROOM_COUNT
  LOAD r1, r4
  JZ r1, gd_nr          ; no rooms

  LDI r9, 1
  SUB r1, r9             ; last room idx
  LDI r9, 4
  MUL r1, r9
  LDI r4, ROOM_BASE
  ADD r4, r1
  LOAD r20, r4
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  LOAD r21, r4
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 2
  ADD r4, r9
  LOAD r26, r4
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 3
  ADD r4, r9
  LOAD r27, r4
  LDI r9, 2
  SHR r26, r9
  ADD r20, r26
  LDI r9, 2
  SHR r27, r9
  ADD r21, r27
  LDI r4, STAIRS_X
  STORE r4, r20
  LDI r4, STAIRS_Y
  STORE r4, r21
  ; Set stairs tile in map
  LDI r4, MAP_BASE
  LDI r9, MAP_W
  MUL r9, r21
  ADD r4, r9
  ADD r4, r20
  LDI r1, TILE_STAIR
  STORE r4, r1

gd_nr:

  ; Step 5: Place player in first room center
  LDI r4, ROOM_COUNT
  LOAD r1, r4
  JZ r1, gd_fallback

  LDI r4, ROOM_BASE
  LOAD r20, r4
  LDI r4, ROOM_BASE
  LDI r9, 1
  ADD r4, r9
  LOAD r21, r4
  LDI r4, ROOM_BASE
  LDI r9, 2
  ADD r4, r9
  LOAD r26, r4
  LDI r4, ROOM_BASE
  LDI r9, 3
  ADD r4, r9
  LOAD r27, r4
  LDI r9, 2
  SHR r26, r9
  ADD r20, r26
  LDI r9, 2
  SHR r27, r9
  ADD r21, r27
  LDI r4, P_X
  STORE r4, r20
  LDI r4, P_Y
  STORE r4, r21
  JMP gd_done

gd_fallback:
  LDI r1, 16
  LDI r4, P_X
  STORE r4, r1
  LDI r1, 16
  LDI r4, P_Y
  STORE r4, r1

gd_done:
  LDI r1, 0
  LDI r4, STATE
  STORE r4, r1           ; game_state = play
  POP r31
  RET

; ── check_room_overlap ──────────────────────────────────────
; Input: r22=x, r23=y, r20=w, r21=h (new room)
; Output: r1=0 (no overlap) or r1=1 (overlap)
; Uses AABB test with 1-tile border

check_room_overlap:
  LDI r4, ROOM_COUNT
  LOAD r10, r4           ; num rooms
  LDI r1, 0
  JZ r10, cro_ok         ; no rooms yet

  LDI r11, 0             ; loop index
cro_loop:
  LDI r9, r10
  CMP r11, r9
  BGE r0, cro_ok

  ; Load existing room
  LDI r1, r11
  LDI r9, 4
  MUL r1, r9
  LDI r4, ROOM_BASE
  ADD r4, r1
  LOAD r12, r4           ; ex = room[i].x
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  LOAD r13, r4           ; ey = room[i].y
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 2
  ADD r4, r9
  LOAD r14, r4           ; ew = room[i].w
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 3
  ADD r4, r9
  LOAD r15, r4           ; eh = room[i].h

  ; Test 1: new_x < ex + ew + 1
  LDI r1, 0
  ADD r1, r22
  LDI r9, 0
  ADD r9, r14
  LDI r16, 1
  ADD r9, r16
  ADD r9, r12            ; r9 = ex + ew + 1
  CMP r1, r9
  BGE r0, cro_next       ; no overlap possible

  ; Test 2: new_x + new_w + 1 > ex
  LDI r1, 0
  ADD r1, r22
  LDI r9, 0
  ADD r9, r20
  LDI r16, 1
  ADD r9, r16
  ADD r1, r9             ; r1 = new_x + new_w + 1
  LDI r9, 0
  ADD r9, r12            ; r9 = ex
  CMP r1, r9
  BLE r0, cro_next

  ; Test 3: new_y < ey + eh + 1
  LDI r1, 0
  ADD r1, r23
  LDI r9, 0
  ADD r9, r15
  LDI r16, 1
  ADD r9, r16
  ADD r9, r13            ; r9 = ey + eh + 1
  CMP r1, r9
  BGE r0, cro_next

  ; Test 4: new_y + new_h + 1 > ey
  LDI r1, 0
  ADD r1, r23
  LDI r9, 0
  ADD r9, r21
  LDI r16, 1
  ADD r9, r16
  ADD r1, r9             ; r1 = new_y + new_h + 1
  LDI r9, 0
  ADD r9, r13            ; r9 = ey
  CMP r1, r9
  BLE r0, cro_next

  ; All 4 tests passed = overlap!
  LDI r1, 1
  RET

cro_next:
  LDI r9, 1
  ADD r11, r9
  JMP cro_loop

cro_ok:
  LDI r1, 0
  RET

; ── carve_room ──────────────────────────────────────────────
; Input: r22=x, r23=y, r20=w, r21=h
; Sets map tiles to TILE_FLOOR

carve_room:
  LDI r10, 0             ; dy
cr_y:
  LDI r11, 0             ; dx
cr_x:
  LDI r4, MAP_BASE
  LDI r9, MAP_W
  LUL r9, r10            ; r9 = dy * 32
  ; Oops, there is no MUL with two registers. Let me fix this.
  ; Actually, MUL r9, r10 does r9 = r9 * r10, not r9 = r10 * r10.
  ; Wait, how does MUL work? Let me check.
  RET
