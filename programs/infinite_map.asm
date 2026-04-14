; infinite_map.asm -- Infinite scrolling procedural terrain
;
; Arrow keys or WASD scroll through an infinite procedurally generated world.
; Every (x,y) coordinate deterministically produces a tile via a hash function.
; The world is mathematically infinite -- no stored data, no boundaries.
;
; Tile size = 4 pixels. Viewport = 64x64 tiles = 256x256 pixels.
; Renders via RECTF (one opcode per tile, Rust fills the pixels).
;
; Memory:
;   RAM[0x7800] = camera_x
;   RAM[0x7801] = camera_y
;   RAM[0xFFB]  = key bitmask (host writes each frame)

; ===== Constants =====
LDI r7, 1               ; constant 1
LDI r8, 64              ; TILES per axis
LDI r9, 4               ; TILE_SIZE pixels
LDI r10, 0xFFB          ; key bitmask port
LDI r11, 0x7800         ; camera_x address
LDI r12, 0x7801         ; camera_y address
LDI r13, 0x7802         ; scratch address

; ===== Main Loop =====
main_loop:

; --- Read camera position ---
LOAD r14, r11           ; r14 = camera_x
LOAD r15, r12           ; r15 = camera_y

; --- Read key bitmask ---
LOAD r16, r10           ; r16 = key bitmask

; --- Process Up (bit 0) ---
MOV r17, r16
LDI r18, 1
AND r17, r18
JZ r17, no_up
SUB r15, r7             ; camera_y--
no_up:

; --- Process Down (bit 1) ---
MOV r17, r16
LDI r18, 2
AND r17, r18
JZ r17, no_down
ADD r15, r7             ; camera_y++
no_down:

; --- Process Left (bit 2) ---
MOV r17, r16
LDI r18, 4
AND r17, r18
JZ r17, no_left
SUB r14, r7             ; camera_x--
no_left:

; --- Process Right (bit 3) ---
MOV r17, r16
LDI r18, 8
AND r17, r18
JZ r17, no_right
ADD r14, r7             ; camera_x++
no_right:

; --- Store updated camera ---
STORE r11, r14
STORE r12, r15

; --- Clear screen to black ---
LDI r17, 0
FILL r17

; ===== Render Viewport =====
; r14 = camera_x, r15 = camera_y
; 64x64 tile loop: ty=0..63, tx=0..63
; Per tile: hash(world_x, world_y) -> terrain type -> color -> RECTF

LDI r1, 0               ; ty = 0

render_y:
  LDI r2, 0             ; tx = 0

  render_x:
    ; World coordinates
    MOV r3, r14
    ADD r3, r2           ; r3 = world_x = camera_x + tx
    MOV r4, r15
    ADD r4, r1           ; r4 = world_y = camera_y + ty

    ; ---- Hash: (world_x * 99001) XOR (world_y * 79007), then >> 28 ----
    MOV r5, r3
    LDI r18, 99001
    MUL r5, r18          ; r5 = world_x * 99001

    MOV r6, r4
    LDI r18, 79007
    MUL r6, r18          ; r6 = world_y * 79007

    XOR r5, r6           ; r5 = hash

    ; Extract top 4 bits: terrain type 0..15
    LDI r18, 28
    SHR r5, r18          ; r5 = terrain_type (0..15)

    ; ---- Terrain -> Color ----
    ; Cascading comparisons. r0 is set by CMP; BLT/BGE/JZ check r0.

    ; Is it water? (types 0-2)
    LDI r18, 3
    CMP r5, r18
    BLT r0, color_water

    ; Beach? (type 3)
    LDI r18, 4
    CMP r5, r18
    BLT r0, color_beach

    ; Grass? (types 4-6)
    LDI r18, 7
    CMP r5, r18
    BLT r0, color_grass

    ; Forest? (types 7-8)
    LDI r18, 9
    CMP r5, r18
    BLT r0, color_forest

    ; Hills/mountain? (types 9-11)
    LDI r18, 12
    CMP r5, r18
    BLT r0, color_mountain

    ; Snow/ice (types 12-15)
    JMP color_snow

    ; ---- Water subtypes ----
color_water:
    LDI r18, 1
    CMP r5, r18
    BLT r0, water_deep
    LDI r18, 2
    CMP r5, r18
    JZ r0, water_shallow
    LDI r17, 0x000088    ; mid water
    JMP do_rect
water_deep:
    LDI r17, 0x000044    ; deep ocean
    JMP do_rect
water_shallow:
    LDI r17, 0x0000BB    ; shallow water
    JMP do_rect

color_beach:
    LDI r17, 0xC2B280    ; sand
    JMP do_rect

    ; ---- Grass subtypes ----
color_grass:
    LDI r18, 5
    CMP r5, r18
    BLT r0, grass_light
    LDI r18, 6
    CMP r5, r18
    JZ r0, grass_dark
    LDI r17, 0x33AA22    ; medium grass
    JMP do_rect
grass_light:
    LDI r17, 0x55BB33    ; light grass
    JMP do_rect
grass_dark:
    LDI r17, 0x228811    ; dark grass
    JMP do_rect

    ; ---- Forest subtypes ----
color_forest:
    LDI r18, 8
    CMP r5, r18
    JZ r0, forest_dense
    LDI r17, 0x116600    ; forest
    JMP do_rect
forest_dense:
    LDI r17, 0x0A4400    ; dense forest
    JMP do_rect

    ; ---- Mountain subtypes ----
color_mountain:
    LDI r18, 10
    CMP r5, r18
    BLT r0, mt_low
    LDI r18, 11
    CMP r5, r18
    JZ r0, mt_tall
    LDI r17, 0x888888    ; medium mountain
    JMP do_rect
mt_low:
    LDI r17, 0x667766    ; foothills
    JMP do_rect
mt_tall:
    LDI r17, 0x999999    ; tall mountain
    JMP do_rect

    ; ---- Snow subtypes ----
color_snow:
    LDI r18, 14
    CMP r5, r18
    BLT r0, snow_light
    LDI r18, 15
    CMP r5, r18
    JZ r0, snow_peak
    LDI r17, 0xDDEEFF    ; ice
    JMP do_rect
snow_light:
    LDI r17, 0xCCCCEE    ; snow
    JMP do_rect
snow_peak:
    LDI r17, 0xFFFFFF    ; peak
    JMP do_rect

    ; ---- Draw tile ----
do_rect:
    ; Screen position: (tx * 4, ty * 4)
    MOV r3, r2
    MUL r3, r9           ; r3 = tx * TILE_SIZE = tx * 4
    MOV r4, r1
    MUL r4, r9           ; r4 = ty * TILE_SIZE = ty * 4
    RECTF r3, r4, r9, r9, r17  ; fill 4x4 rect with color

    ; ---- Next tile ----
    ADD r2, r7           ; tx++
    MOV r18, r2
    SUB r18, r8          ; tx - 64
    JZ r18, next_row
    JMP render_x

next_row:
    ADD r1, r7           ; ty++
    MOV r18, r1
    SUB r18, r8          ; ty - 64
    JZ r18, frame_end
    JMP render_y

frame_end:
    FRAME
    JMP main_loop
