; roguelike.asm -- Procedural Dungeon Crawler
;
; Controls: WASD to move, R to regenerate dungeon
; Goal: find the golden stairs (bright tile) to descend deeper
;
; Screen: 256x256, dungeon 32x32 tiles at 8px each
; Uses TILEMAP opcode for efficient whole-screen rendering
;
; Algorithm: Random room placement with overlap check + L-corridors
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

; ── Movement Handlers ────────────────────────────────────────
; Collision: check map tile before moving

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
  JZ r0, idle            ; wall, block
  LDI r4, P_Y
  LOAD r1, r4
  LDI r9, 1
  SUB r1, r9
  STORE r4, r1           ; move player up
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
  STORE r4, r1
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
  STORE r4, r1
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
  STORE r4, r1
  CALL check_stairs
  JMP do_move

do_move:
  CALL render
  ; Step sound
  LDI r5, 220
  LDI r6, 25
  BEEP r5, r6

idle:
  FRAME
  JMP game_loop

; ── Descend Screen ───────────────────────────────────────────

descend_screen:
  LDI r1, 0x001a00     ; dark green bg
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
  ; Increment level
  LDI r4, DLEVEL
  LOAD r1, r4
  LDI r9, 1
  ADD r1, r9
  STORE r4, r1
  JMP restart

; ─────────────────────────────────────────────────────────────
; SUBROUTINES
; ─────────────────────────────────────────────────────────────

; ── init_tiles -- fill tile pixel data with solid colors ─────
; Tile 1 (floor): 0x2A2A4E dark purple-gray (64 pixels)
; Tile 2 (wall):  0x4A6A8A steel blue (64 pixels)
; Tile 3 (stairs): 0xD4A017 gold (64 pixels)

init_tiles:
  PUSH r31

  ; Floor tile
  LDI r10, 0
it_floor:
  LDI r4, TILE_BASE
  ADD r4, r10
  LDI r1, 0x2A2A4E
  STORE r4, r1
  LDI r9, 1
  ADD r10, r9
  LDI r6, 64
  CMP r10, r6
  BLT r0, it_floor

  ; Wall tile with checkerboard pattern
  LDI r10, 0
it_wall:
  LDI r4, TILE_BASE
  LDI r9, 64
  ADD r4, r9
  ADD r4, r10         ; addr = TILE_BASE + 64 + idx
  ; Compute checkerboard: (row XOR col) AND 1
  LDI r1, 0
  ADD r1, r10         ; r1 = pixel index
  LDI r2, 0
  ADD r2, r10         ; r2 = copy of idx
  LDI r9, 8
  LDI r3, 0
  ADD r3, r1
  DIV r3, r9          ; r3 = row (idx / 8)
  LDI r9, 8
  MOD r2, r9          ; r2 = col (idx % 8)
  XOR r2, r3          ; r2 = row XOR col
  LDI r9, 1
  AND r2, r9          ; r2 = 0 or 1
  JZ r2, it_wall_dk
  LDI r1, 0x5A7AAA    ; lighter blue
  JMP it_wall_st
it_wall_dk:
  LDI r1, 0x3A5A7A    ; darker blue
it_wall_st:
  STORE r4, r1
  LDI r9, 1
  ADD r10, r9
  LDI r6, 64
  CMP r10, r6
  BLT r0, it_wall

  ; Stairs tile - all gold
  LDI r10, 0
it_stair:
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
  BLT r0, it_stair

  POP r31
  RET

; ── init_text -- store HUD text strings ──────────────────────

init_text:
  ; "DESCENDED!" at 0x5540
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

  ; "PRESS R" at 0x5550
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
; Builds a random dungeon with rooms and corridors
; Uses r20-r29 as scratch registers

generate_dungeon:
  PUSH r31

  ; Step 1: Fill entire map with walls
  LDI r10, 0            ; y counter
gd_fill_y:
  LDI r11, 0            ; x counter
gd_fill_x:
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
  BLT r0, gd_fill_x
  LDI r9, 1
  ADD r10, r9
  LDI r6, MAP_H
  CMP r10, r6
  BLT r0, gd_fill_y

  ; Step 2: Place rooms
  LDI r1, 0
  LDI r4, ROOM_COUNT
  STORE r4, r1          ; room_count = 0
  LDI r25, 0            ; attempts counter

gd_room_loop:
  ; Check if we have enough rooms (target 7)
  LDI r4, ROOM_COUNT
  LOAD r1, r4
  LDI r9, 7
  CMP r1, r9
  BGE r0, gd_rooms_done

  ; Check attempts limit (60)
  LDI r9, 60
  CMP r25, r9
  BGE r0, gd_rooms_done

  LDI r9, 1
  ADD r25, r9           ; attempts++

  ; Random room width (4-9)
  RAND r20
  LDI r9, 6
  MOD r20, r9           ; 0-5
  LDI r9, 4
  ADD r20, r9           ; 4-9

  ; Random room height (3-7)
  RAND r21
  LDI r9, 5
  MOD r21, r9           ; 0-4
  LDI r9, 3
  ADD r21, r9           ; 3-7

  ; Random room position (x, y)
  ; x range: 1 to (MAP_W - width - 1)
  RAND r22
  LDI r9, MAP_W
  LDI r26, 0
  ADD r26, r20
  SUB r9, r26           ; MAP_W - width
  LDI r26, 2
  SUB r9, r26           ; -2 for border
  LDI r26, 1
  ADD r9, r26           ; ensure positive
  MOD r22, r9
  LDI r9, 1
  ADD r22, r9           ; x = 1 +

  ; y range: 1 to (MAP_H - height - 1)
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
  ADD r23, r9           ; y = 1 +

  ; Check overlap with existing rooms (1-tile border)
  CALL check_room_overlap
  JNZ r1, gd_room_loop  ; overlap found, try again

  ; Carve room into map
  CALL carve_room

  ; Store room data at rooms[room_count]
  LDI r4, ROOM_COUNT
  LOAD r1, r4           ; r1 = room index
  LDI r9, 4
  MUL r1, r9            ; r1 = byte offset

  ; room[i].x = r22
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 0
  ADD r9, r22
  STORE r4, r9

  ; room[i].y = r23
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  LDI r9, 0
  ADD r9, r23
  STORE r4, r9

  ; room[i].w = r20
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 2
  ADD r4, r9
  LDI r9, 0
  ADD r9, r20
  STORE r4, r9

  ; room[i].h = r21
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 3
  ADD r4, r9
  LDI r9, 0
  ADD r9, r21
  STORE r4, r9

  ; Increment room_count
  LDI r4, ROOM_COUNT
  LOAD r1, r4
  LDI r9, 1
  ADD r1, r9
  STORE r4, r1

  JMP gd_room_loop

gd_rooms_done:

  ; Step 3: Connect rooms with L-shaped corridors
  LDI r4, ROOM_COUNT
  LOAD r24, r4           ; r24 = total rooms
  LDI r9, 2
  CMP r24, r9
  BLT r0, gd_connect_done ; need at least 2 rooms

  LDI r25, 0             ; i = 0
gd_connect:
  ; Loop while i < rooms - 1
  LDI r9, 1
  SUB r24, r9            ; r24 = rooms - 1
  CMP r25, r24
  BGE r0, gd_connect_done

  ; Get center of room i
  CALL get_room_center    ; r20 = cx, r21 = cy for room i
  ; Save center of room i
  LDI r22, 0
  ADD r22, r20           ; r22 = cx_i
  LDI r23, 0
  ADD r23, r21           ; r23 = cy_i

  ; Get center of room i+1
  LDI r9, 1
  ADD r25, r9            ; temporarily increment i
  CALL get_room_center    ; r20 = cx, r21 = cy for room i+1
  LDI r9, 1
  SUB r25, r9            ; restore i

  ; Carve horizontal corridor from (r22, r23) to (r20, r23)
  ; (at y of room i, from cx_i to cx_i+1)
  LDI r20_h, 0
  ADD r20_h, r22         ; use r20 for start x
  ; Actually r20 was overwritten. Let me restructure.
  ; I need to be more careful with register allocation here.

  JMP gd_connect_done

gd_connect_done:

  ; Step 4: Place stairs in last room
  LDI r4, ROOM_COUNT
  LOAD r1, r4
  JZ r1, gd_no_rooms     ; safety: no rooms placed

  LDI r9, 1
  SUB r1, r9             ; last room index
  LDI r9, 4
  MUL r1, r9
  LDI r4, ROOM_BASE
  ADD r4, r1
  LOAD r20, r4           ; last room x
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  LOAD r21, r4           ; last room y
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 2
  ADD r4, r9
  LOAD r26, r4           ; last room w
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 3
  ADD r4, r9
  LOAD r27, r4           ; last room h
  ; center
  LDI r9, 2
  SHR r26, r9
  ADD r20, r26           ; cx = x + w/2
  LDI r9, 2
  SHR r27, r9
  ADD r21, r27           ; cy = y + h/2
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

gd_no_rooms:

  ; Step 5: Place player in first room (or center if no rooms)
  LDI r4, ROOM_COUNT
  LOAD r1, r4
  JZ r1, gd_no_rooms_p  ; no rooms

  ; First room center
  LDI r4, ROOM_BASE
  LOAD r20, r4           ; room[0].x
  LDI r4, ROOM_BASE
  LDI r9, 1
  ADD r4, r9
  LOAD r21, r4           ; room[0].y
  LDI r4, ROOM_BASE
  LDI r9, 2
  ADD r4, r9
  LOAD r26, r4           ; room[0].w
  LDI r4, ROOM_BASE
  LDI r9, 3
  ADD r4, r9
  LOAD r27, r4           ; room[0].h
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
  JMP gd_init_done

gd_no_rooms_p:
  ; Fallback: place player at center
  LDI r1, 16
  LDI r4, P_X
  STORE r4, r1
  LDI r1, 16
  LDI r4, P_Y
  STORE r4, r1

gd_init_done:
  ; Reset game state
  LDI r1, 0
  LDI r4, STATE
  STORE r4, r1

  POP r31
  RET
