; roguelike.asm -- Procedural Dungeon Crawler with Combat, Enemies, Items
; Controls: WASD to move, U to undo, R to regenerate dungeon
; Goal: find stairs, fight enemies, collect items, descend deeper
;
; Memory layout:
;   0x5000..0x5FFF  map[4096]
;   0x6000..0x613F  tile_data[320] (5 tiles x 8x8)
;   0x6200..0x622F  rooms[48] (12 rooms x 4 words)
;   0x6230          room_count
;   0x6240..0x624C  player state
;   0x6300..0x637F  enemies[128] (32 x 4 words)
;   0x6380          enemy_count
;   0x6400..0x643F  items[64] (16 x 4 words)
;   0x6440          item_count
;   0x6500..0x68FF  viewport[1024]
;   0x6900..0x690F  temp vars
;   0x6A00..0x6AFF  text strings

#define MAP_BASE     0x5000
#define TILE_BASE    0x6000
#define ROOM_BASE    0x6200
#define ROOM_COUNT   0x6230
#define P_X          0x6240
#define P_Y          0x6241
#define P_HP         0x6242
#define P_MAXHP      0x6243
#define P_ATK        0x6244
#define STAIRS_X     0x6245
#define STAIRS_Y     0x6246
#define DLEVEL       0x6247
#define STATE        0x6248
#define CAM_X        0x6249
#define CAM_Y        0x624A
#define KILLS        0x624B
#define MSG_TIMER    0x624C
#define UNDO_SLOT    0x624D
#define ENEMY_BASE   0x6300
#define ENEMY_COUNT  0x6380
#define ITEM_BASE    0x6400
#define ITEM_COUNT   0x6440
#define VP_BASE      0x6500
#define TEMP_BASE    0x6900
#define TILE_FLOOR   1
#define TILE_WALL    2
#define TILE_STAIR   3
#define TILE_ENEMY   4
#define TILE_ITEM    5
#define MAP_W        64
#define MAP_H        64
#define VP_W         32
#define VP_H         32
#define TILE_SZ      8
#define MAX_ENEMIES  32
#define MAX_ITEMS    16

restart:
  ; Clear undo history on restart
  LDI r1, 3
  FORK r1
  ; Reset undo slot to 0
  LDI r1, 0
  LDI r4, UNDO_SLOT
  STORE r4, r1
  LDI r30, 0x8000
  CALL init_tiles
  CALL init_text
  LDI r1, 10
  LDI r4, P_HP
  STORE r4, r1
  LDI r1, 10
  LDI r4, P_MAXHP
  STORE r4, r1
  LDI r1, 1
  LDI r4, P_ATK
  STORE r4, r1
  LDI r1, 0
  LDI r4, KILLS
  STORE r4, r1
  LDI r1, 0
  LDI r4, MSG_TIMER
  STORE r4, r1
  CALL generate_dungeon
  CALL spawn_enemies
  CALL spawn_items
  CALL render

game_loop:
  LDI r4, STATE
  LOAD r1, r4
  LDI r9, 2
  CMP r1, r9
  JZ r0, death_screen
  LDI r4, STATE
  LOAD r1, r4
  LDI r9, 1
  CMP r1, r9
  JZ r0, descend_screen
  LDI r4, MSG_TIMER
  LOAD r1, r4
  JZ r1, gl_input
  LDI r9, 1
  SUB r1, r9
  LDI r4, MSG_TIMER
  STORE r4, r1
gl_input:
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
  LDI r6, 85
  CMP r7, r6
  JZ r0, try_undo
  LDI r6, 117
  CMP r7, r6
  JZ r0, try_undo
  JMP idle

try_up:
  CALL save_undo
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
  CALL check_enemy_at_player
  CALL check_item_at_player
  CALL check_stairs
  CALL enemy_turn
  JMP do_move

try_down:
  CALL save_undo
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
  CALL check_enemy_at_player
  CALL check_item_at_player
  CALL check_stairs
  CALL enemy_turn
  JMP do_move

try_left:
  CALL save_undo
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
  LDI r4, P_X
  LOAD r1, r4
  LDI r9, 1
  SUB r1, r9
  STORE r4, r1
  CALL check_enemy_at_player
  CALL check_item_at_player
  CALL check_stairs
  CALL enemy_turn
  JMP do_move

try_right:
  CALL save_undo
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
  LDI r4, P_X
  LOAD r1, r4
  LDI r9, 1
  ADD r1, r9
  STORE r4, r1
  CALL check_enemy_at_player
  CALL check_item_at_player
  CALL check_stairs
  CALL enemy_turn
  JMP do_move

do_move:
  CALL render
  LDI r5, 220
  LDI r6, 25
  BEEP r5, r6

idle:
  FRAME
  JMP game_loop

try_undo:
  ; Check if we have any undo snapshots (FORK mode 2 = list count)
  LDI r1, 2
  FORK r1
  ; r0 = count of saved snapshots
  JZ r0, idle
  ; Decrement undo slot to get the slot to restore
  LDI r4, UNDO_SLOT
  LOAD r1, r4
  LDI r9, 1
  SUB r1, r9
  ; Clamp to 0 (underflow check)
  LDI r9, 0xFFFFFFFF
  CMP r1, r9
  JNZ r0, undo_do_restore
  LDI r1, 0
undo_do_restore:
  ; r1 = slot index. FORK mode 1 restores from slot in r1.
  LDI r4, 1
  FORK r4
  ; r0 = 0 on success, 0xFFFFFFFF on error
  LDI r9, 0xFFFFFFFF
  CMP r0, r9
  JZ r0, idle
  ; Update undo slot to the restored position
  LDI r4, UNDO_SLOT
  STORE r4, r1
  ; Flash screen blue briefly for undo feedback
  LDI r1, 0x0044AA
  FILL r1
  CALL render
  FRAME
  JMP game_loop

descend_screen:
  LDI r1, 0x001a00
  FILL r1
  LDI r10, 0x6A10
  LDI r11, 50
  LDI r12, 100
  TEXT r11, r12, r10
  LDI r10, 0x6A30
  LDI r11, 50
  LDI r12, 140
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

death_screen:
  LDI r1, 0x1a0000
  FILL r1
  LDI r10, 0x6A50
  LDI r11, 60
  LDI r12, 100
  TEXT r11, r12, r10
  LDI r10, 0x6A70
  LDI r11, 50
  LDI r12, 150
  TEXT r11, r12, r10
  LDI r10, 0x6A90
  LDI r11, 50
  LDI r12, 170
  TEXT r11, r12, r10
  LDI r10, 0x6AB0
  LDI r11, 30
  LDI r12, 210
  TEXT r11, r12, r10
  FRAME
  IKEY r7
  JZ r7, death_screen
  JMP restart


; ─────────────────────────────────────────────────────────────
; SUBROUTINES
; ─────────────────────────────────────────────────────────────

; ── save_undo ───────────────────────────────────────────────
; Save a FORK snapshot before each player move.
; Uses FORK mode 0 (save). Stores slot index in UNDO_SLOT.

save_undo:
  PUSH r31
  ; FORK mode 0 = save snapshot to next slot
  LDI r1, 0
  FORK r1
  ; r0 = slot index on success, 0xFFFFFFFF on error (max snapshots reached)
  ; Save the slot index so try_undo knows where to restore from
  LDI r4, UNDO_SLOT
  STORE r4, r0
  POP r31
  RET

; ── init_tiles ───────────────────────────────────────────────

init_tiles:
  PUSH r31
  PUSH r10
  ; Floor tile (tile 1): dark brownish pixels
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
  ; Wall tile (tile 2): checkerboard dark gray
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
  ; Stairs tile (tile 3): golden pixels
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
  ; Enemy tile (tile 4): red with dark border
  LDI r10, 0
it_en:
  LDI r4, TILE_BASE
  LDI r9, 192
  ADD r4, r9
  ADD r4, r10
  LDI r1, 0
  ADD r1, r10
  LDI r9, 8
  MOD r1, r9
  LDI r2, 0
  ADD r2, r10
  LDI r9, 64
  DIV r2, r9
  LDI r9, 8
  MOD r2, r9
  JZ r1, it_en_b
  LDI r9, 7
  CMP r1, r9
  JZ r0, it_en_b
  JZ r2, it_en_b
  LDI r9, 7
  CMP r2, r9
  JZ r0, it_en_b
  LDI r1, 0xCC2222
  JMP it_en_s
it_en_b:
  LDI r1, 0x881111
it_en_s:
  STORE r4, r1
  LDI r9, 1
  ADD r10, r9
  LDI r6, 64
  CMP r10, r6
  BLT r0, it_en
  ; Item tile (tile 5): green with border
  LDI r10, 0
it_it:
  LDI r4, TILE_BASE
  LDI r9, 256
  ADD r4, r9
  ADD r4, r10
  LDI r1, 0
  ADD r1, r10
  LDI r9, 8
  MOD r1, r9
  LDI r2, 0
  ADD r2, r10
  LDI r9, 64
  DIV r2, r9
  LDI r9, 8
  MOD r2, r9
  JZ r1, it_it_b
  LDI r9, 7
  CMP r1, r9
  JZ r0, it_it_b
  JZ r2, it_it_b
  LDI r9, 7
  CMP r2, r9
  JZ r0, it_it_b
  LDI r1, 0x22AA22
  JMP it_it_s
it_it_b:
  LDI r1, 0x116611
it_it_s:
  STORE r4, r1
  LDI r9, 1
  ADD r10, r9
  LDI r6, 64
  CMP r10, r6
  BLT r0, it_it
  POP r10
  POP r31
  RET

; ── init_text ────────────────────────────────────────────────

init_text:
  PUSH r31
  ; "@" at 0x6A00
  LDI r4, 0x6A00
  LDI r1, 64
  STORE r4, r1
  LDI r4, 0x6A01
  LDI r1, 0
  STORE r4, r1
  ; "HP:" at 0x6A02
  LDI r4, 0x6A02
  LDI r1, 72
  STORE r4, r1
  LDI r4, 0x6A03
  LDI r1, 80
  STORE r4, r1
  LDI r4, 0x6A04
  LDI r1, 58
  STORE r4, r1
  LDI r4, 0x6A05
  LDI r1, 0
  STORE r4, r1
  ; "ATK:" at 0x6A06
  LDI r4, 0x6A06
  LDI r1, 65
  STORE r4, r1
  LDI r4, 0x6A07
  LDI r1, 84
  STORE r4, r1
  LDI r4, 0x6A08
  LDI r1, 75
  STORE r4, r1
  LDI r4, 0x6A09
  LDI r1, 58
  STORE r4, r1
  LDI r4, 0x6A0A
  LDI r1, 0
  STORE r4, r1
  ; "LV:" at 0x6A0B
  LDI r4, 0x6A0B
  LDI r1, 76
  STORE r4, r1
  LDI r4, 0x6A0C
  LDI r1, 86
  STORE r4, r1
  LDI r4, 0x6A0D
  LDI r1, 58
  STORE r4, r1
  LDI r4, 0x6A0E
  LDI r1, 0
  STORE r4, r1
  ; "DESCENDED!" at 0x6A10
  LDI r4, 0x6A10
  LDI r1, 68
  STORE r4, r1
  LDI r4, 0x6A11
  LDI r1, 69
  STORE r4, r1
  LDI r4, 0x6A12
  LDI r1, 83
  STORE r4, r1
  LDI r4, 0x6A13
  LDI r1, 67
  STORE r4, r1
  LDI r4, 0x6A14
  LDI r1, 69
  STORE r4, r1
  LDI r4, 0x6A15
  LDI r1, 78
  STORE r4, r1
  LDI r4, 0x6A16
  LDI r1, 68
  STORE r4, r1
  LDI r4, 0x6A17
  LDI r1, 69
  STORE r4, r1
  LDI r4, 0x6A18
  LDI r1, 68
  STORE r4, r1
  LDI r4, 0x6A19
  LDI r1, 33
  STORE r4, r1
  LDI r4, 0x6A1A
  LDI r1, 0
  STORE r4, r1
  ; "PRESS R" at 0x6A30
  LDI r4, 0x6A30
  LDI r1, 80
  STORE r4, r1
  LDI r4, 0x6A31
  LDI r1, 82
  STORE r4, r1
  LDI r4, 0x6A32
  LDI r1, 69
  STORE r4, r1
  LDI r4, 0x6A33
  LDI r1, 83
  STORE r4, r1
  LDI r4, 0x6A34
  LDI r1, 83
  STORE r4, r1
  LDI r4, 0x6A35
  LDI r1, 32
  STORE r4, r1
  LDI r4, 0x6A36
  LDI r1, 82
  STORE r4, r1
  LDI r4, 0x6A37
  LDI r1, 0
  STORE r4, r1
  ; "GAME OVER" at 0x6A50
  LDI r4, 0x6A50
  LDI r1, 71
  STORE r4, r1
  LDI r4, 0x6A51
  LDI r1, 65
  STORE r4, r1
  LDI r4, 0x6A52
  LDI r1, 77
  STORE r4, r1
  LDI r4, 0x6A53
  LDI r1, 69
  STORE r4, r1
  LDI r4, 0x6A54
  LDI r1, 32
  STORE r4, r1
  LDI r4, 0x6A55
  LDI r1, 79
  STORE r4, r1
  LDI r4, 0x6A56
  LDI r1, 86
  STORE r4, r1
  LDI r4, 0x6A57
  LDI r1, 69
  STORE r4, r1
  LDI r4, 0x6A58
  LDI r1, 82
  STORE r4, r1
  LDI r4, 0x6A59
  LDI r1, 0
  STORE r4, r1
  ; "KILLS:" at 0x6A70
  LDI r4, 0x6A70
  LDI r1, 75
  STORE r4, r1
  LDI r4, 0x6A71
  LDI r1, 73
  STORE r4, r1
  LDI r4, 0x6A72
  LDI r1, 76
  STORE r4, r1
  LDI r4, 0x6A73
  LDI r1, 76
  STORE r4, r1
  LDI r4, 0x6A74
  LDI r1, 83
  STORE r4, r1
  LDI r4, 0x6A75
  LDI r1, 58
  STORE r4, r1
  LDI r4, 0x6A76
  LDI r1, 0
  STORE r4, r1
  ; "LEVEL:" at 0x6A90
  LDI r4, 0x6A90
  LDI r1, 76
  STORE r4, r1
  LDI r4, 0x6A91
  LDI r1, 69
  STORE r4, r1
  LDI r4, 0x6A92
  LDI r1, 86
  STORE r4, r1
  LDI r4, 0x6A93
  LDI r1, 69
  STORE r4, r1
  LDI r4, 0x6A94
  LDI r1, 76
  STORE r4, r1
  LDI r4, 0x6A95
  LDI r1, 58
  STORE r4, r1
  LDI r4, 0x6A96
  LDI r1, 0
  STORE r4, r1
  ; "PRESS R TO RETRY" at 0x6AB0
  LDI r4, 0x6AB0
  LDI r1, 80
  STORE r4, r1
  LDI r4, 0x6AB1
  LDI r1, 82
  STORE r4, r1
  LDI r4, 0x6AB2
  LDI r1, 69
  STORE r4, r1
  LDI r4, 0x6AB3
  LDI r1, 83
  STORE r4, r1
  LDI r4, 0x6AB4
  LDI r1, 83
  STORE r4, r1
  LDI r4, 0x6AB5
  LDI r1, 32
  STORE r4, r1
  LDI r4, 0x6AB6
  LDI r1, 82
  STORE r4, r1
  LDI r4, 0x6AB7
  LDI r1, 32
  STORE r4, r1
  LDI r4, 0x6AB8
  LDI r1, 84
  STORE r4, r1
  LDI r4, 0x6AB9
  LDI r1, 79
  STORE r4, r1
  LDI r4, 0x6ABA
  LDI r1, 32
  STORE r4, r1
  LDI r4, 0x6ABB
  LDI r1, 82
  STORE r4, r1
  LDI r4, 0x6ABC
  LDI r1, 69
  STORE r4, r1
  LDI r4, 0x6ABD
  LDI r1, 84
  STORE r4, r1
  LDI r4, 0x6ABE
  LDI r1, 82
  STORE r4, r1
  LDI r4, 0x6ABF
  LDI r1, 89
  STORE r4, r1
  LDI r4, 0x6AC0
  LDI r1, 0
  STORE r4, r1
  POP r31
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
  ; Place rooms (up to 12)
  LDI r1, 0
  LDI r4, ROOM_COUNT
  STORE r4, r1
  LDI r25, 0
gd_rl:
  LDI r4, ROOM_COUNT
  LOAD r1, r4
  LDI r9, 12
  CMP r1, r9
  BGE r0, gd_rd
  LDI r9, 100
  CMP r25, r9
  BGE r0, gd_rd
  LDI r9, 1
  ADD r25, r9
  RAND r20
  LDI r9, 8
  MOD r20, r9
  LDI r9, 5
  ADD r20, r9
  RAND r21
  LDI r9, 6
  MOD r21, r9
  LDI r9, 4
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
  ; Connect rooms with L-shaped corridors
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
  LDI r4, TEMP_BASE
  LDI r9, 0
  ADD r9, r20
  STORE r4, r9
  LDI r4, TEMP_BASE
  LDI r9, 1
  ADD r4, r9
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
  LDI r4, TEMP_BASE
  LDI r9, 2
  ADD r4, r9
  LDI r9, 0
  ADD r9, r20
  STORE r4, r9
  LDI r4, TEMP_BASE
  LDI r9, 3
  ADD r4, r9
  LDI r9, 0
  ADD r9, r21
  STORE r4, r9
  ; Horizontal corridor
  LDI r4, TEMP_BASE
  LOAD r20, r4
  LDI r4, TEMP_BASE
  LDI r9, 2
  ADD r4, r9
  LOAD r22, r4
  LDI r4, TEMP_BASE
  LDI r9, 1
  ADD r4, r9
  LOAD r21, r4
  CALL carve_h_corridor
  ; Vertical corridor
  LDI r4, TEMP_BASE
  LDI r9, 1
  ADD r4, r9
  LOAD r20, r4
  LDI r4, TEMP_BASE
  LDI r9, 3
  ADD r4, r9
  LOAD r22, r4
  LDI r4, TEMP_BASE
  LDI r9, 2
  ADD r4, r9
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
  LDI r1, 32
  LDI r4, P_X
  STORE r4, r1
  LDI r1, 32
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
; Input: r2=x, r1=y  Output: r1 = map[y*64+x]

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

; ── check_enemy_at_player ────────────────────────────────────
; Check if player position matches any enemy. If so, do combat.

check_enemy_at_player:
  PUSH r31
  LDI r4, ENEMY_COUNT
  LOAD r10, r4
  JZ r10, cep_done
  LDI r11, 0
cep_lp:
  MOV r9, r10
  CMP r11, r9
  BGE r0, cep_done
  ; Load enemy x,y
  MOV r1, r11
  LDI r9, 4
  MUL r1, r9
  LDI r4, ENEMY_BASE
  ADD r4, r1
  LOAD r12, r4
  LDI r4, ENEMY_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  LOAD r13, r4
  ; Compare with player position
  LDI r4, P_X
  LOAD r20, r4
  LDI r4, P_Y
  LOAD r21, r4
  CMP r12, r20
  JNZ r0, cep_nx
  CMP r13, r21
  JNZ r0, cep_nx
  ; Combat! Player attacks enemy
  LDI r4, P_ATK
  LOAD r20, r4
  ; Subtract player ATK from enemy HP
  LDI r4, ENEMY_BASE
  MOV r1, r11
  LDI r9, 4
  MUL r1, r9
  ADD r4, r1
  LDI r9, 2
  ADD r4, r9
  LOAD r21, r4
  SUB r21, r20
  ADD r4, r1
  LDI r9, 2
  ADD r4, r9
  STORE r4, r21
  ; Check if enemy died
  JZ r21, cep_kill
  ; Enemy alive - enemy attacks back
  LDI r4, ENEMY_BASE
  ADD r4, r1
  LDI r9, 3
  ADD r4, r9
  LOAD r20, r4
  ; Enemy damage by type: 0=1, 1=1, 2=2
  LDI r21, 1
  CMP r20, r21
  BLT r0, cep_edmg
  LDI r21, 2
  CMP r20, r21
  BLT r0, cep_edmg
  LDI r21, 2
  CMP r20, r21
  JNZ r0, cep_edmg2
  ; Type 2 = skeleton, 2 damage
  LDI r21, 2
  JMP cep_edmg
cep_edmg2:
  LDI r21, 2
cep_edmg:
  ; Subtract enemy damage from player HP
  LDI r4, P_HP
  LOAD r20, r4
  SUB r20, r21
  LDI r4, P_HP
  STORE r4, r20
  LDI r5, 150
  LDI r6, 200
  BEEP r5, r6
  LDI r1, 30
  LDI r4, MSG_TIMER
  STORE r4, r1
  ; Check if player died
  LDI r4, P_HP
  LOAD r1, r4
  LDI r9, 0
  CMP r9, r1
  BLT r0, cep_done
  ; Player dead
  LDI r1, 2
  LDI r4, STATE
  STORE r4, r1
  JMP cep_done
cep_kill:
  ; Remove enemy: shift remaining enemies down
  MOV r20, r11
cep_shift:
  MOV r9, r10
  LDI r21, 1
  SUB r9, r21
  CMP r20, r9
  BGE r0, cep_shift_done
  ; Copy enemy[i+1] to enemy[i]
  MOV r1, r20
  LDI r9, 4
  MUL r1, r9
  MOV r14, r1
  MOV r1, r20
  LDI r9, 1
  ADD r1, r9
  LDI r9, 4
  MUL r1, r9
  MOV r15, r1
  ; Copy 4 words
  LDI r4, ENEMY_BASE
  ADD r4, r15
  LOAD r21, r4
  LDI r4, ENEMY_BASE
  ADD r4, r14
  STORE r4, r21
  LDI r4, ENEMY_BASE
  ADD r4, r15
  LDI r9, 1
  ADD r4, r9
  LOAD r21, r4
  LDI r4, ENEMY_BASE
  ADD r4, r14
  LDI r9, 1
  ADD r4, r9
  STORE r4, r21
  LDI r4, ENEMY_BASE
  ADD r4, r15
  LDI r9, 2
  ADD r4, r9
  LOAD r21, r4
  LDI r4, ENEMY_BASE
  ADD r4, r14
  LDI r9, 2
  ADD r4, r9
  STORE r4, r21
  LDI r4, ENEMY_BASE
  ADD r4, r15
  LDI r9, 3
  ADD r4, r9
  LOAD r21, r4
  LDI r4, ENEMY_BASE
  ADD r4, r14
  LDI r9, 3
  ADD r4, r9
  STORE r4, r21
  LDI r9, 1
  ADD r20, r9
  JMP cep_shift
cep_shift_done:
  LDI r9, 1
  SUB r10, r9
  LDI r4, ENEMY_COUNT
  STORE r4, r10
  ; Increment kills
  LDI r4, KILLS
  LOAD r1, r4
  LDI r9, 1
  ADD r1, r9
  STORE r4, r1
  LDI r5, 440
  LDI r6, 100
  BEEP r5, r6
  LDI r1, 30
  LDI r4, MSG_TIMER
  STORE r4, r1
  JMP cep_done
cep_nx:
  LDI r9, 1
  ADD r11, r9
  JMP cep_lp
cep_done:
  POP r31
  RET

; ── check_item_at_player ────────────────────────────────────

check_item_at_player:
  PUSH r31
  LDI r4, ITEM_COUNT
  LOAD r10, r4
  JZ r10, cip_done
  LDI r11, 0
cip_lp:
  MOV r9, r10
  CMP r11, r9
  BGE r0, cip_done
  ; Load item x,y
  MOV r1, r11
  LDI r9, 4
  MUL r1, r9
  LDI r4, ITEM_BASE
  ADD r4, r1
  LOAD r12, r4
  LDI r4, ITEM_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  LOAD r13, r4
  ; Compare with player position
  LDI r4, P_X
  LOAD r20, r4
  LDI r4, P_Y
  LOAD r21, r4
  CMP r12, r20
  JNZ r0, cip_nx
  CMP r13, r21
  JNZ r0, cip_nx
  ; Pickup item! Get type and value
  LDI r4, ITEM_BASE
  ADD r4, r1
  LDI r9, 2
  ADD r4, r9
  LOAD r20, r4
  LDI r4, ITEM_BASE
  ADD r4, r1
  LDI r9, 3
  ADD r4, r9
  LOAD r21, r4
  ; Type 0 = health potion
  JZ r20, cip_heal
  ; Type 1 = attack scroll
  LDI r9, 1
  CMP r20, r9
  JNZ r0, cip_other
  ; Attack scroll: +1 ATK
  LDI r4, P_ATK
  LOAD r1, r4
  ADD r1, r21
  STORE r4, r1
  LDI r5, 660
  LDI r6, 120
  BEEP r5, r6
  JMP cip_remove
cip_heal:
  ; Health potion: add value to HP, cap at max
  LDI r4, P_HP
  LOAD r1, r4
  ADD r1, r21
  LDI r4, P_MAXHP
  LOAD r9, r4
  CMP r1, r9
  BLT r0, cip_heal_ok
  MOV r1, r9
cip_heal_ok:
  LDI r4, P_HP
  STORE r4, r1
  LDI r5, 880
  LDI r6, 80
  BEEP r5, r6
cip_other:
cip_remove:
  LDI r1, 30
  LDI r4, MSG_TIMER
  STORE r4, r1
  ; Remove item: shift remaining down
  MOV r20, r11
cip_shift:
  MOV r9, r10
  LDI r21, 1
  SUB r9, r21
  CMP r20, r9
  BGE r0, cip_shift_done
  MOV r1, r20
  LDI r9, 4
  MUL r1, r9
  MOV r14, r1
  MOV r1, r20
  LDI r9, 1
  ADD r1, r9
  LDI r9, 4
  MUL r1, r9
  MOV r15, r1
  LDI r4, ITEM_BASE
  ADD r4, r15
  LOAD r21, r4
  LDI r4, ITEM_BASE
  ADD r4, r14
  STORE r4, r21
  LDI r4, ITEM_BASE
  ADD r4, r15
  LDI r9, 1
  ADD r4, r9
  LOAD r21, r4
  LDI r4, ITEM_BASE
  ADD r4, r14
  LDI r9, 1
  ADD r4, r9
  STORE r4, r21
  LDI r4, ITEM_BASE
  ADD r4, r15
  LDI r9, 2
  ADD r4, r9
  LOAD r21, r4
  LDI r4, ITEM_BASE
  ADD r4, r14
  LDI r9, 2
  ADD r4, r9
  STORE r4, r21
  LDI r4, ITEM_BASE
  ADD r4, r15
  LDI r9, 3
  ADD r4, r9
  LOAD r21, r4
  LDI r4, ITEM_BASE
  ADD r4, r14
  LDI r9, 3
  ADD r4, r9
  STORE r4, r21
  LDI r9, 1
  ADD r20, r9
  JMP cip_shift
cip_shift_done:
  LDI r9, 1
  SUB r10, r9
  LDI r4, ITEM_COUNT
  STORE r4, r10
  JMP cip_done
cip_nx:
  LDI r9, 1
  ADD r11, r9
  JMP cip_lp
cip_done:
  POP r31
  RET

; ── enemy_turn ──────────────────────────────────────────────
; Simple AI: enemies within 8 tiles chase player, others wander

enemy_turn:
  PUSH r31
  LDI r4, ENEMY_COUNT
  LOAD r10, r4
  JZ r10, et_done
  LDI r4, P_X
  LOAD r20, r4
  LDI r4, P_Y
  LOAD r21, r4
  LDI r11, 0
et_lp:
  MOV r9, r10
  CMP r11, r9
  BGE r0, et_done
  ; Load enemy x,y
  MOV r1, r11
  LDI r9, 4
  MUL r1, r9
  LDI r4, ENEMY_BASE
  ADD r4, r1
  LOAD r12, r4
  LDI r4, ENEMY_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  LOAD r13, r4
  ; Calculate Manhattan distance
  MOV r22, r12
  SUB r22, r20
  ; Absolute value
  LDI r9, 0
  CMP r22, r9
  BGE r0, et_absy
  LDI r9, 0
  SUB r22, r9
  SUB r9, r22
  MOV r22, r9
et_absy:
  MOV r23, r13
  SUB r23, r21
  LDI r9, 0
  CMP r23, r9
  BGE r0, et_dist
  LDI r9, 0
  SUB r23, r9
  SUB r9, r23
  MOV r23, r9
et_dist:
  ADD r22, r23
  ; r22 = Manhattan distance
  ; Decide: chase (dist<=8 and random<128) or wander (random<32)
  RAND r24
  LDI r9, 8
  CMP r9, r22
  BLT r0, et_wander
  LDI r9, 128
  CMP r24, r9
  BGE r0, et_nx
  JMP et_chase
et_wander:
  LDI r9, 32
  CMP r24, r9
  BGE r0, et_nx
  ; Random direction
  RAND r24
  LDI r9, 4
  MOD r24, r9
  JZ r24, et_wu
  LDI r9, 1
  CMP r24, r9
  JZ r0, et_wd
  LDI r9, 2
  CMP r24, r9
  JZ r0, et_wl
  JMP et_wr
et_wu:
  LDI r9, 1
  SUB r13, r9
  JMP et_trymove
et_wd:
  LDI r9, 1
  ADD r13, r9
  JMP et_trymove
et_wl:
  LDI r9, 1
  SUB r12, r9
  JMP et_trymove
et_wr:
  LDI r9, 1
  ADD r12, r9
  JMP et_trymove
et_chase:
  ; Move toward player on axis with greater distance
  MOV r22, r12
  SUB r22, r20
  LDI r9, 0
  CMP r22, r9
  BGE r0, et_cxpos
  ; Player is to the right, move right
  LDI r9, 1
  ADD r12, r9
  JMP et_trymove
et_cxpos:
  ; Player is to the left, move left
  LDI r9, 1
  SUB r12, r9
  JMP et_trymove
et_trymove:
  ; Check bounds
  LDI r9, 0
  CMP r12, r9
  BLT r0, et_nx
  LDI r9, MAP_W
  LDI r14, 1
  SUB r9, r14
  CMP r9, r12
  BLT r0, et_nx
  LDI r9, 0
  CMP r13, r9
  BLT r0, et_nx
  LDI r9, MAP_H
  LDI r14, 1
  SUB r9, r14
  CMP r9, r13
  BLT r0, et_nx
  ; Check wall at new position
  MOV r2, r12
  MOV r1, r13
  CALL get_tile
  LDI r9, TILE_WALL
  CMP r1, r9
  JZ r0, et_nx
  ; Check no other enemy at new position
  CALL check_enemy_pos
  JNZ r1, et_nx
  ; Move enemy
  MOV r1, r11
  LDI r9, 4
  MUL r1, r9
  LDI r4, ENEMY_BASE
  ADD r4, r1
  STORE r4, r12
  LDI r4, ENEMY_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  STORE r4, r13
  ; Check if moved onto player
  LDI r4, P_X
  LOAD r20, r4
  LDI r4, P_Y
  LOAD r21, r4
  CMP r12, r20
  JNZ r0, et_nx
  CMP r13, r21
  JNZ r0, et_nx
  ; Enemy attacks player
  MOV r1, r11
  LDI r9, 4
  MUL r1, r9
  LDI r4, ENEMY_BASE
  ADD r4, r1
  LDI r9, 3
  ADD r4, r9
  LOAD r20, r4
  ; Damage by type
  LDI r21, 1
  CMP r20, r21
  BLT r0, et_edmg
  LDI r21, 2
  CMP r20, r21
  BLT r0, et_edmg
  LDI r21, 2
  CMP r20, r21
  JNZ r0, et_edmg2
  LDI r21, 2
  JMP et_edmg
et_edmg2:
  LDI r21, 2
et_edmg:
  LDI r4, P_HP
  LOAD r20, r4
  SUB r20, r21
  STORE r4, r20
  LDI r5, 150
  LDI r6, 200
  BEEP r5, r6
  LDI r1, 30
  LDI r4, MSG_TIMER
  STORE r4, r1
  ; Check player death
  LDI r4, P_HP
  LOAD r1, r4
  LDI r9, 0
  CMP r9, r1
  BLT r0, et_nx
  LDI r1, 2
  LDI r4, STATE
  STORE r4, r1
et_nx:
  LDI r9, 1
  ADD r11, r9
  ; Reload enemy count in case state changed
  LDI r4, ENEMY_COUNT
  LOAD r10, r4
  JMP et_lp
et_done:
  POP r31
  RET

; ── check_enemy_pos ─────────────────────────────────────────
; Check if any enemy is at position r12=x, r13=y (excluding self)
; Returns r1=1 if occupied, r1=0 if free
; Input: r11=self index, r12=x, r13=y

check_enemy_pos:
  PUSH r31
  LDI r4, ENEMY_COUNT
  LOAD r10, r4
  LDI r14, 0
cep2_lp:
  MOV r9, r10
  CMP r14, r9
  BGE r0, cep2_ok
  CMP r14, r11
  JZ r0, cep2_nx
  ; Load enemy pos
  MOV r1, r14
  LDI r9, 4
  MUL r1, r9
  LDI r4, ENEMY_BASE
  ADD r4, r1
  LOAD r15, r4
  LDI r4, ENEMY_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  LOAD r16, r4
  CMP r15, r12
  JNZ r0, cep2_nx
  CMP r16, r13
  JNZ r0, cep2_nx
  LDI r1, 1
  POP r31
  RET
cep2_nx:
  LDI r9, 1
  ADD r14, r9
  JMP cep2_lp
cep2_ok:
  LDI r1, 0
  POP r31
  RET

; ── spawn_enemies ────────────────────────────────────────────

spawn_enemies:
  PUSH r31
  LDI r4, ROOM_COUNT
  LOAD r10, r4
  ; Skip room 0 (player start room)
  LDI r9, 1
  CMP r9, r10
  BGE r0, se_done
  LDI r11, 1
se_rl:
  CMP r11, r10
  BGE r0, se_done
  ; Check enemy count cap
  LDI r4, ENEMY_COUNT
  LOAD r20, r4
  LDI r9, MAX_ENEMIES
  CMP r20, r9
  BGE r0, se_done
  ; Number of enemies for this room: 1-3
  RAND r20
  LDI r9, 3
  MOD r20, r9
  LDI r9, 1
  ADD r20, r9
se_el:
  ; Check cap again
  LDI r4, ENEMY_COUNT
  LOAD r21, r4
  LDI r9, MAX_ENEMIES
  CMP r21, r9
  BGE r0, se_next
  JZ r20, se_next
  ; Get room bounds
  MOV r1, r11
  LDI r9, 4
  MUL r1, r9
  LDI r4, ROOM_BASE
  ADD r4, r1
  LOAD r22, r4
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  LOAD r23, r4
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 2
  ADD r4, r9
  LOAD r24, r4
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 3
  ADD r4, r9
  LOAD r25, r4
  ; Random position within room (1 inset)
  RAND r12
  ADD r12, r22
  LDI r9, 1
  ADD r22, r9
  LDI r9, 1
  SUB r24, r9
  SUB r9, r22
  ADD r9, r22
  CMP r9, r22
  MOV r9, r22
  CMP r12, r9
  BLT r0, se_ex
  MOV r12, r22
se_ex:
  ; Clamp: rand % (w-2) + x + 1
  MOV r14, r24
  LDI r9, 1
  SUB r14, r9
  SUB r14, r22
  LDI r9, 1
  SUB r14, r9
  JZ r14, se_ex_skip
  RAND r12
  LDI r9, 0
  ADD r9, r14
  MOD r12, r9
  LDI r9, 1
  ADD r12, r9
  ADD r12, r22
se_ex_skip:
  RAND r13
  ADD r13, r23
  LDI r9, 1
  ADD r23, r9
  LDI r9, 1
  SUB r25, r9
  MOV r14, r25
  LDI r9, 1
  SUB r14, r9
  SUB r14, r23
  LDI r9, 1
  SUB r14, r9
  JZ r14, se_ey_skip
  RAND r13
  LDI r9, 0
  ADD r9, r14
  MOD r13, r9
  LDI r9, 1
  ADD r13, r9
  ADD r13, r23
se_ey_skip:
  ; Determine enemy type based on dungeon level
  LDI r4, DLEVEL
  LOAD r26, r4
  LDI r9, 3
  CMP r26, r9
  BLT r0, se_type_rat
  LDI r9, 6
  CMP r26, r9
  BLT r0, se_type_mid
  ; Level 6+: any type
  RAND r26
  LDI r9, 3
  MOD r26, r9
  JMP se_type_set
se_type_mid:
  ; Level 3-5: rat or goblin
  RAND r26
  LDI r9, 2
  MOD r26, r9
  JMP se_type_set
se_type_rat:
  LDI r26, 0
se_type_set:
  ; Set HP based on type
  JZ r26, se_hp_rat
  LDI r9, 1
  CMP r26, r9
  JNZ r0, se_hp_skel
  ; Goblin: hp=2
  LDI r27, 2
  JMP se_store
se_hp_rat:
  LDI r27, 1
  JMP se_store
se_hp_skel:
  LDI r27, 3
se_store:
  ; Store enemy
  LDI r4, ENEMY_COUNT
  LOAD r1, r4
  LDI r9, 4
  MUL r1, r9
  LDI r4, ENEMY_BASE
  ADD r4, r1
  STORE r4, r12
  LDI r4, ENEMY_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  STORE r4, r13
  LDI r4, ENEMY_BASE
  ADD r4, r1
  LDI r9, 2
  ADD r4, r9
  STORE r4, r27
  LDI r4, ENEMY_BASE
  ADD r4, r1
  LDI r9, 3
  ADD r4, r9
  STORE r4, r26
  LDI r4, ENEMY_COUNT
  LOAD r1, r4
  LDI r9, 1
  ADD r1, r9
  STORE r4, r1
  LDI r9, 1
  SUB r20, r9
  JMP se_el
se_next:
  LDI r9, 1
  ADD r11, r9
  JMP se_rl
se_done:
  POP r31
  RET

; ── spawn_items ──────────────────────────────────────────────

spawn_items:
  PUSH r31
  LDI r4, ROOM_COUNT
  LOAD r10, r4
  ; Start from room 1, every other room
  LDI r11, 1
si_rl:
  CMP r11, r10
  BGE r0, si_done
  ; Check cap
  LDI r4, ITEM_COUNT
  LOAD r20, r4
  LDI r9, MAX_ITEMS
  CMP r20, r9
  BGE r0, si_done
  ; Only spawn in every other room (odd rooms)
  MOV r1, r11
  LDI r9, 2
  MOD r1, r9
  JNZ r1, si_next
  ; Get room bounds
  MOV r1, r11
  LDI r9, 4
  MUL r1, r9
  LDI r4, ROOM_BASE
  ADD r4, r1
  LOAD r22, r4
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  LOAD r23, r4
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 2
  ADD r4, r9
  LOAD r24, r4
  LDI r4, ROOM_BASE
  ADD r4, r1
  LDI r9, 3
  ADD r4, r9
  LOAD r25, r4
  ; Random position in room center area
  MOV r14, r24
  LDI r9, 1
  SUB r14, r9
  SUB r14, r22
  JZ r14, si_skip
  RAND r12
  LDI r9, 0
  ADD r9, r14
  MOD r12, r9
  LDI r9, 1
  ADD r12, r9
  ADD r12, r22
  MOV r14, r25
  LDI r9, 1
  SUB r14, r9
  SUB r14, r23
  JZ r14, si_skip
  RAND r13
  LDI r9, 0
  ADD r9, r14
  MOD r13, r9
  LDI r9, 1
  ADD r13, r9
  ADD r13, r23
  ; Item type: 70% health potion, 30% attack scroll
  RAND r20
  LDI r9, 10
  MOD r20, r9
  LDI r9, 7
  CMP r20, r9
  BGE r0, si_atk
  ; Health potion
  LDI r20, 0
  LDI r21, 3
  JMP si_store
si_atk:
  ; Attack scroll
  LDI r20, 1
  LDI r21, 1
si_store:
  LDI r4, ITEM_COUNT
  LOAD r1, r4
  LDI r9, 4
  MUL r1, r9
  LDI r4, ITEM_BASE
  ADD r4, r1
  STORE r4, r12
  LDI r4, ITEM_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  STORE r4, r13
  LDI r4, ITEM_BASE
  ADD r4, r1
  LDI r9, 2
  ADD r4, r9
  STORE r4, r20
  LDI r4, ITEM_BASE
  ADD r4, r1
  LDI r9, 3
  ADD r4, r9
  STORE r4, r21
  LDI r4, ITEM_COUNT
  LOAD r1, r4
  LDI r9, 1
  ADD r1, r9
  STORE r4, r1
  JMP si_next
si_skip:
si_next:
  LDI r9, 1
  ADD r11, r9
  JMP si_rl
si_done:
  POP r31
  RET

; ── compute_camera ──────────────────────────────────────────

compute_camera:
  LDI r4, P_X
  LOAD r1, r4
  LDI r9, 16
  SUB r1, r9
  LDI r4, CAM_X
  STORE r4, r1
  LDI r4, CAM_X
  LOAD r1, r4
  LDI r9, 0
  CMP r1, r9
  BGE r0, cam_x_hi
  LDI r1, 0
  LDI r4, CAM_X
  STORE r4, r1
cam_x_hi:
  LDI r4, CAM_X
  LOAD r1, r4
  LDI r9, 32
  CMP r1, r9
  BLT r0, cam_y_start
  LDI r1, 32
  LDI r4, CAM_X
  STORE r4, r1
cam_y_start:
  LDI r4, P_Y
  LOAD r1, r4
  LDI r9, 16
  SUB r1, r9
  LDI r4, CAM_Y
  STORE r4, r1
  LDI r4, CAM_Y
  LOAD r1, r4
  LDI r9, 0
  CMP r1, r9
  BGE r0, cam_y_hi
  LDI r1, 0
  LDI r4, CAM_Y
  STORE r4, r1
cam_y_hi:
  LDI r4, CAM_Y
  LOAD r1, r4
  LDI r9, 32
  CMP r1, r9
  BLT r0, cam_done
  LDI r1, 32
  LDI r4, CAM_Y
  STORE r4, r1
cam_done:
  RET

; ── copy_viewport ──────────────────────────────────────────
; Copy 32x32 from 64x64 map, overlay enemies and items

copy_viewport:
  PUSH r31
  PUSH r10
  PUSH r11
  LDI r10, 0
cv_outer:
  LDI r4, CAM_Y
  LOAD r12, r4
  ADD r12, r10
  LDI r9, MAP_W
  MUL r12, r9
  LDI r4, CAM_X
  LOAD r1, r4
  ADD r12, r1
  LDI r1, MAP_BASE
  ADD r12, r1
  LDI r13, VP_BASE
  MOV r1, r10
  LDI r9, VP_W
  MUL r1, r9
  ADD r13, r1
  LDI r11, 0
cv_inner:
  MOV r4, r12
  ADD r4, r11
  LOAD r1, r4
  MOV r4, r13
  ADD r4, r11
  STORE r4, r1
  LDI r9, 1
  ADD r11, r9
  LDI r6, VP_W
  CMP r11, r6
  BLT r0, cv_inner
  LDI r9, 1
  ADD r10, r9
  LDI r6, VP_H
  CMP r10, r6
  BLT r0, cv_outer
  ; Overlay enemies
  LDI r4, ENEMY_COUNT
  LOAD r10, r4
  JZ r10, cv_items
  LDI r11, 0
cv_en_lp:
  MOV r9, r10
  CMP r11, r9
  BGE r0, cv_items
  MOV r1, r11
  LDI r9, 4
  MUL r1, r9
  LDI r4, ENEMY_BASE
  ADD r4, r1
  LOAD r12, r4
  LDI r4, ENEMY_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  LOAD r13, r4
  LDI r4, CAM_X
  LOAD r20, r4
  LDI r4, CAM_Y
  LOAD r21, r4
  CMP r12, r20
  BLT r0, cv_en_nx
  MOV r9, r20
  LDI r14, VP_W
  ADD r9, r14
  CMP r12, r9
  BGE r0, cv_en_nx
  CMP r13, r21
  BLT r0, cv_en_nx
  MOV r9, r21
  LDI r14, VP_H
  ADD r9, r14
  CMP r13, r9
  BGE r0, cv_en_nx
  SUB r12, r20
  SUB r13, r21
  MOV r1, r13
  LDI r9, VP_W
  MUL r1, r9
  ADD r1, r12
  LDI r4, VP_BASE
  ADD r4, r1
  LDI r1, TILE_ENEMY
  STORE r4, r1
cv_en_nx:
  LDI r9, 1
  ADD r11, r9
  JMP cv_en_lp
cv_items:
  ; Overlay items
  LDI r4, ITEM_COUNT
  LOAD r10, r4
  JZ r10, cv_done
  LDI r11, 0
cv_it_lp:
  MOV r9, r10
  CMP r11, r9
  BGE r0, cv_done
  MOV r1, r11
  LDI r9, 4
  MUL r1, r9
  LDI r4, ITEM_BASE
  ADD r4, r1
  LOAD r12, r4
  LDI r4, ITEM_BASE
  ADD r4, r1
  LDI r9, 1
  ADD r4, r9
  LOAD r13, r4
  LDI r4, CAM_X
  LOAD r20, r4
  LDI r4, CAM_Y
  LOAD r21, r4
  CMP r12, r20
  BLT r0, cv_it_nx
  MOV r9, r20
  LDI r14, VP_W
  ADD r9, r14
  CMP r12, r9
  BGE r0, cv_it_nx
  CMP r13, r21
  BLT r0, cv_it_nx
  MOV r9, r21
  LDI r14, VP_H
  ADD r9, r14
  CMP r13, r9
  BGE r0, cv_it_nx
  SUB r12, r20
  SUB r13, r21
  MOV r1, r13
  LDI r9, VP_W
  MUL r1, r9
  ADD r1, r12
  LDI r4, VP_BASE
  ADD r4, r1
  LDI r1, TILE_ITEM
  STORE r4, r1
cv_it_nx:
  LDI r9, 1
  ADD r11, r9
  JMP cv_it_lp
cv_done:
  POP r11
  POP r10
  POP r31
  RET

; ── draw_number ─────────────────────────────────────────────
; Input: r1=number, r2=x screen pos, r3=y screen pos
; Writes number as ASCII to TEMP_BASE+6, null-terminated, then TEXT

draw_number:
  PUSH r31
  PUSH r1
  PUSH r9
  PUSH r10
  PUSH r4
  ; Handle zero
  JZ r1, dn_zero
  ; Extract digits into TEMP_BASE (reverse order)
  LDI r10, 0
dn_div:
  JZ r1, dn_rev
  LDI r9, 10
  LDI r4, TEMP_BASE
  ADD r4, r10
  LDI r5, 0
  ADD r5, r1
  LDI r6, 10
  MOD r5, r6
  LDI r6, 48
  ADD r5, r6
  STORE r4, r5
  LDI r6, 10
  DIV r1, r6
  LDI r9, 1
  ADD r10, r9
  LDI r9, 5
  CMP r10, r9
  BGE r0, dn_rev
  JMP dn_div
dn_rev:
  ; Reverse into TEMP_BASE+6
  LDI r11, 0
dn_revlp:
  LDI r9, 1
  SUB r10, r9
  BLT r0, dn_null
  LDI r4, TEMP_BASE
  ADD r4, r10
  LOAD r12, r4
  LDI r4, TEMP_BASE
  LDI r9, 6
  ADD r4, r9
  ADD r4, r11
  STORE r4, r12
  LDI r9, 1
  ADD r11, r9
  JMP dn_revlp
  JMP dn_null
dn_zero:
  LDI r4, TEMP_BASE
  LDI r9, 6
  ADD r4, r9
  LDI r1, 48
  STORE r4, r1
  LDI r11, 1
dn_null:
  ; Null-terminate
  LDI r4, TEMP_BASE
  LDI r9, 6
  ADD r4, r9
  ADD r4, r11
  LDI r1, 0
  STORE r4, r1
  ; Draw with TEXT
  ; TEXT needs registers: TEXT x_reg, y_reg, addr_reg
  ; r2 has x, r3 has y, addr = TEMP_BASE+6
  LDI r10, TEMP_BASE
  LDI r9, 6
  ADD r10, r9
  TEXT r2, r3, r10
  POP r4
  POP r10
  POP r9
  POP r1
  POP r31
  RET

; ── render ──────────────────────────────────────────────────

render:
  PUSH r31
  PUSH r10
  CALL compute_camera
  CALL copy_viewport
  ; Clear screen to black
  LDI r1, 0
  FILL r1
  ; Render 32x32 viewport using TILEMAP at (0,8) - leave top 8px for HUD
  LDI r1, 0
  LDI r2, 8
  LDI r3, VP_BASE
  LDI r4, TILE_BASE
  LDI r5, VP_W
  LDI r6, VP_H
  LDI r7, TILE_SZ
  LDI r8, TILE_SZ
  TILEMAP r1, r2, r3, r4, r5, r6, r7, r8
  ; Draw player "@" relative to viewport
  LDI r4, P_X
  LOAD r1, r4
  LDI r4, CAM_X
  LOAD r2, r4
  SUB r1, r2
  LDI r9, TILE_SZ
  MUL r1, r9
  LDI r4, P_Y
  LOAD r2, r4
  LDI r4, CAM_Y
  LOAD r3, r4
  SUB r2, r3
  LDI r9, TILE_SZ
  MUL r2, r9
  ; Add 8px offset for HUD bar
  LDI r9, 8
  ADD r2, r9
  LDI r3, 0x6A00
  TEXT r1, r2, r3
  ; Draw HUD
  CALL draw_hud
  POP r10
  POP r31
  RET

; ── draw_hud ────────────────────────────────────────────────

draw_hud:
  PUSH r31
  ; "HP:" at (2, 0)
  LDI r10, 2
  LDI r11, 0
  LDI r12, 0x6A02
  TEXT r10, r11, r12
  ; HP value
  LDI r4, P_HP
  LOAD r1, r4
  LDI r2, 32
  LDI r3, 0
  CALL draw_number
  ; "/"
  LDI r4, 0x690C
  LDI r1, 47
  STORE r4, r1
  LDI r4, 0x690D
  LDI r1, 0
  STORE r4, r1
  LDI r10, 0x690C
  LDI r11, 56
  LDI r12, 0
  TEXT r11, r12, r10
  ; MAXHP value
  LDI r4, P_MAXHP
  LOAD r1, r4
  LDI r2, 64
  LDI r3, 0
  CALL draw_number
  ; "ATK:" at (100, 0)
  LDI r10, 100
  LDI r11, 0
  LDI r12, 0x6A06
  TEXT r10, r11, r12
  ; ATK value
  LDI r4, P_ATK
  LOAD r1, r4
  LDI r2, 134
  LDI r3, 0
  CALL draw_number
  ; "LV:" at (160, 0)
  LDI r10, 160
  LDI r11, 0
  LDI r12, 0x6A0B
  TEXT r10, r11, r12
  ; Level value
  LDI r4, DLEVEL
  LOAD r1, r4
  LDI r2, 186
  LDI r3, 0
  CALL draw_number
  POP r31
  RET
