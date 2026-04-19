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
;   0x5540..0x555F  text strings
;   0x5560..0x5563  temp: cx_i, cy_i, cx_j, cy_j

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
  LDI r30, 0x8000
  CALL init_tiles
  CALL init_text
  CALL generate_dungeon
  CALL render

; ── Main Game Loop ───────────────────────────────────────────

game_loop:
  LDI r4, STATE
  LOAD r1, r4
  LDI r9, 1
  CMP r1, r9
  JZ r0, descend_screen
  IKEY r7
  JZ r7, idle
  LDI r6, 87
  CMP r7, r6
  JZ r0, try_up
  LDI r6, 119
  CMP r7, r6
  JZ r0, try_up
  LDI r6, 83
  CMP r7, r6
  JZ r0, try_down
  LDI r6, 115
  CMP r7, r6
  JZ r0, try_down
  LDI r6, 65
  CMP r7, r6
  JZ r0, try_left
  LDI r6, 97
  CMP r7, r6
  JZ r0, try_left
  LDI r6, 68
  CMP r7, r6
  JZ r0, try_right
  LDI r6, 100
  CMP r7, r6
  JZ r0, try_right
  LDI r6, 82
  CMP r7, r6
  JZ r0, restart
  LDI r6, 114
  CMP r7, r6
  JZ r0, restart
  JMP idle

; ── Movement ─────────────────────────────────────────────────

try_up:
  LDI r4, P_X
  LOAD r2, r4
  LDI r4, P_Y
  LOAD r1, r4
  LDI r9, 1
  SUB r1, r9
  CALL get_tile
  LDI r9, TILE_WALL
  CMP r1, r9
  JZ r0, idle
  LDI r4, P_Y
  LOAD r1, r4
  LDI r9, 1
  SUB r1, r9
  STORE r4, r1
  CALL check_stairs
  JMP do_move

try_down:
  LDI r4, P_X
  LOAD r2, r4
  LDI r4, P_Y
  LOAD r1, r4
  LDI r9, 1
  ADD r1, r9
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
  SUB r1, r9
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
  ADD r1, r9
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
  LDI r5, 220
  LDI r6, 25
  BEEP r5, r6

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
  STORE r4, r1
  JMP restart

; ─────────────────────────────────────────────────────────────
; SUBROUTINES
; ─────────────────────────────────────────────────────────────

; ── init_tiles ───────────────────────────────────────────────

init_tiles:
  PUSH r31
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
  LDI r10, 0
it_wl:
  LDI r4, TILE_BASE
  LDI r9, 64
  ADD r4, r9
  ADD r4, r10
  LDI r1, 0
  ADD r1, r10
  LDI r2, 0
  ADD r2, r10
  LDI r9, 8
  LDI r3, 0
  ADD r3, r1
  DIV r3, r9
  LDI r9, 8
  MOD r2, r9
  XOR r2, r3
  LDI r9, 1
  AND r2, r9
  JZ r2, it_wd
  LDI r1, 0x5A7AAA
  JMP it_ws
it_wd:
  LDI r1, 0x3A5A7A
it_ws:
  STORE r4, r1
  LDI r9, 1
  ADD r10, r9
  LDI r6, 64
  CMP r10, r6
  BLT r0, it_wl
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

; ── init_text ────────────────────────────────────────────────

init_text:
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

generate_dungeon:
  PUSH r31
  ; Fill map with walls
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
  ; Place rooms
  LDI r1, 0
  LDI r4, ROOM_COUNT
  STORE r4, r1
  LDI r25, 0
gd_rl:
  LDI r4, ROOM_COUNT
  LOAD r1, r4
  LDI r9, 7
  CMP r1, r9
  BGE r0, gd_rd
  LDI r9, 60
  CMP r25, r9
  BGE r0, gd_rd
  LDI r9, 1
  ADD r25, r9
  RAND r20
  LDI r9, 6
  MOD r20, r9
  LDI r9, 4
  ADD r20, r9
  RAND r21
  LDI r9, 5
  MOD r21, r9
  LDI r9, 3
  ADD r21, r9
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
  CALL check_room_overlap
  JNZ r1, gd_rl
  CALL carve_room
  LDI r4, ROOM_COUNT
  LOAD r1, r4
  LDI r9, 4
  MUL r1, r9
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 0
  ADD r9, r22
  STORE r4, r9
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  LDI r9, 0
  ADD r9, r23
  STORE r4, r9
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 2
  ADD r4, r9
  LDI r9, 0
  ADD r9, r20
  STORE r4, r9
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 3
  ADD r4, r9
  LDI r9, 0
  ADD r9, r21
  STORE r4, r9
  LDI r4, ROOM_COUNT
  LOAD r1, r4
  LDI r9, 1
  ADD r1, r9
  STORE r4, r1
  JMP gd_rl
gd_rd:
  ; Connect rooms
  LDI r4, ROOM_COUNT
  LOAD r24, r4
  LDI r9, 2
  CMP r24, r9
  BLT r0, gd_cd
  LDI r25, 0
gd_cl:
  LDI r9, 1
  LDI r26, 0
  ADD r26, r24
  SUB r26, r9
  CMP r25, r26
  BGE r0, gd_cd
  ; Center of room i
  MOV r1, r25
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
  LDI r4, 0x5560
  LDI r9, 0
  ADD r9, r20
  STORE r4, r9
  LDI r4, 0x5561
  LDI r9, 0
  ADD r9, r21
  STORE r4, r9
  ; Center of room i+1
  MOV r1, r25
  LDI r9, 1
  ADD r1, r9
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
  LDI r4, 0x5562
  LDI r9, 0
  ADD r9, r20
  STORE r4, r9
  LDI r4, 0x5563
  LDI r9, 0
  ADD r9, r21
  STORE r4, r9
  ; Horizontal corridor
  LDI r4, 0x5560
  LOAD r20, r4
  LDI r4, 0x5562
  LOAD r22, r4
  LDI r4, 0x5561
  LOAD r21, r4
  CALL carve_h_corridor
  ; Vertical corridor
  LDI r4, 0x5561
  LOAD r20, r4
  LDI r4, 0x5563
  LOAD r22, r4
  LDI r4, 0x5562
  LOAD r21, r4
  CALL carve_v_corridor
  LDI r9, 1
  ADD r25, r9
  JMP gd_cl
gd_cd:
  ; Place stairs in last room
  LDI r4, ROOM_COUNT
  LOAD r1, r4
  JZ r1, gd_nr
  LDI r9, 1
  SUB r1, r9
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
  LDI r4, MAP_BASE
  LDI r9, MAP_W
  MUL r9, r21
  ADD r4, r9
  ADD r4, r20
  LDI r1, TILE_STAIR
  STORE r4, r1
gd_nr:
  ; Place player in first room
  LDI r4, ROOM_COUNT
  LOAD r1, r4
  JZ r1, gd_fb
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
  JMP gd_dn
gd_fb:
  LDI r1, 16
  LDI r4, P_X
  STORE r4, r1
  LDI r1, 16
  LDI r4, P_Y
  STORE r4, r1
gd_dn:
  LDI r1, 0
  LDI r4, STATE
  STORE r4, r1
  POP r31
  RET

; ── check_room_overlap ──────────────────────────────────────
; Input: r22=x, r23=y, r20=w, r21=h
; Output: r1=0 no overlap, r1=1 overlap

check_room_overlap:
  LDI r4, ROOM_COUNT
  LOAD r10, r4
  LDI r1, 0
  JZ r10, cro_ok
  LDI r11, 0
cro_lp:
  MOV r9, r10
  CMP r11, r9
  BGE r0, cro_ok
  MOV r1, r11
  LDI r9, 4
  MUL r1, r9
  LDI r4, ROOM_BASE
  ADD r4, r1
  LOAD r12, r4
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  LOAD r13, r4
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 2
  ADD r4, r9
  LOAD r14, r4
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 3
  ADD r4, r9
  LOAD r15, r4
  LDI r1, 0
  ADD r1, r22
  LDI r9, 0
  ADD r9, r12
  LDI r16, 0
  ADD r16, r14
  LDI r5, 1
  ADD r16, r5
  ADD r9, r16
  CMP r1, r9
  BGE r0, cro_nx
  LDI r1, 0
  ADD r1, r22
  LDI r9, 0
  ADD r9, r20
  LDI r5, 1
  ADD r9, r5
  ADD r1, r9
  LDI r9, 0
  ADD r9, r12
  CMP r9, r1
  BGE r0, cro_nx
  LDI r1, 0
  ADD r1, r23
  LDI r9, 0
  ADD r9, r13
  LDI r16, 0
  ADD r16, r15
  LDI r5, 1
  ADD r16, r5
  ADD r9, r16
  CMP r1, r9
  BGE r0, cro_nx
  LDI r1, 0
  ADD r1, r23
  LDI r9, 0
  ADD r9, r21
  LDI r5, 1
  ADD r9, r5
  ADD r1, r9
  LDI r9, 0
  ADD r9, r13
  CMP r9, r1
  BGE r0, cro_nx
  LDI r1, 1
  RET
cro_nx:
  LDI r9, 1
  ADD r11, r9
  JMP cro_lp
cro_ok:
  LDI r1, 0
  RET

; ── carve_room ──────────────────────────────────────────────
; Input: r22=x, r23=y, r20=w, r21=h

carve_room:
  LDI r10, 0
cr_y:
  LDI r11, 0
cr_x:
  LDI r4, MAP_BASE
  LDI r9, MAP_W
  MUL r9, r23
  ADD r4, r9
  LDI r9, MAP_W
  MUL r9, r10
  ADD r4, r9
  LDI r9, 0
  ADD r9, r22
  ADD r4, r9
  ADD r4, r11
  LDI r1, TILE_FLOOR
  STORE r4, r1
  LDI r9, 1
  ADD r11, r9
  CMP r11, r20
  BLT r0, cr_x
  LDI r9, 1
  ADD r10, r9
  CMP r10, r21
  BLT r0, cr_y
  RET

; ── carve_h_corridor ────────────────────────────────────────
; Input: r20=from_x, r22=to_x, r21=y

carve_h_corridor:
ch_lp:
  LDI r4, MAP_BASE
  LDI r9, MAP_W
  MUL r9, r21
  ADD r4, r9
  ADD r4, r20
  LDI r1, TILE_FLOOR
  STORE r4, r1
  CMP r20, r22
  JZ r0, ch_dn
  BLT r0, ch_rt
  LDI r9, 1
  SUB r20, r9
  JMP ch_lp
ch_rt:
  LDI r9, 1
  ADD r20, r9
  JMP ch_lp
ch_dn:
  RET

; ── carve_v_corridor ────────────────────────────────────────
; Input: r20=from_y, r22=to_y, r21=x

carve_v_corridor:
cv_lp:
  LDI r4, MAP_BASE
  LDI r9, MAP_W
  MUL r9, r20
  ADD r4, r9
  ADD r4, r21
  LDI r1, TILE_FLOOR
  STORE r4, r1
  CMP r20, r22
  JZ r0, cv_dn
  BLT r0, cv_d2
  LDI r9, 1
  SUB r20, r9
  JMP cv_lp
cv_d2:
  LDI r9, 1
  ADD r20, r9
  JMP cv_lp
cv_dn:
  RET

; ── get_tile ─────────────────────────────────────────────────
; Input: r2=x, r1=y  Output: r1 = map[y*32+x]

get_tile:
  LDI r9, MAP_W
  MUL r1, r9
  LDI r4, MAP_BASE
  ADD r4, r1
  ADD r4, r2
  LOAD r1, r4
  RET

; ── check_stairs ────────────────────────────────────────────

check_stairs:
  LDI r4, P_X
  LOAD r1, r4
  LDI r4, STAIRS_X
  LOAD r2, r4
  CMP r1, r2
  JNZ r0, cs_dn
  LDI r4, P_Y
  LOAD r1, r4
  LDI r4, STAIRS_Y
  LOAD r2, r4
  CMP r1, r2
  JNZ r0, cs_dn
  LDI r1, 1
  LDI r4, STATE
  STORE r4, r1
  LDI r5, 660
  LDI r6, 150
  BEEP r5, r6
cs_dn:
  RET

; ── render ──────────────────────────────────────────────────

render:
  LDI r1, 0
  LDI r2, 0
  LDI r3, MAP_BASE
  LDI r4, TILE_BASE
  LDI r5, MAP_W
  LDI r6, MAP_H
  LDI r7, TILE_SZ
  LDI r8, TILE_SZ
  TILEMAP r1, r2, r3, r4, r5, r6, r7, r8
  LDI r4, P_X
  LOAD r1, r4
  LDI r9, TILE_SZ
  MUL r1, r9
  LDI r9, 1
  ADD r1, r9
  LDI r4, P_Y
  LOAD r2, r4
  LDI r9, TILE_SZ
  MUL r2, r9
  LDI r9, 1
  ADD r2, r9
  LDI r22, 6
  LDI r23, 6
  LDI r24, 0x00FF88
  RECTF r1, r2, r22, r23, r24
  RET
