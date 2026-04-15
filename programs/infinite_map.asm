; infinite_map.asm -- Infinite scrolling procedural terrain (v4)
;
; Arrow keys / WASD scroll through infinite procedurally generated terrain.
; Two-level hash: coarse hash determines biome (8x8 tile zones = 32px blocks),
; fine hash places structures (1/256 tiles get a tree/rock/crystal).
; Water tiles animate (shimmer) based on frame counter.
; Pure math -- no stored world data, truly infinite.
;
; Tile size = 4 pixels. Viewport = 64x64 tiles = 256x256 pixels.
; Renders via RECTF. ~210K instructions/frame (21% of 1M budget).
;
; Memory:
;   RAM[0x7800] = camera_x (tile coordinates)
;   RAM[0x7801] = camera_y (tile coordinates)
;   RAM[0x7802] = frame_counter (increments each frame)
;   RAM[0xFFB]  = key bitmask (host writes each frame)
;
; Biome distribution (8 biomes, types 0-15):
;   water(0-1), beach(2), desert(3-4), grass(5-6),
;   swamp(7), forest(8-9), mountain(10-11), lava(12), snow(13-15)

; ===== Constants =====
LDI r7, 1               ; constant 1
LDI r8, 64              ; TILES per axis
LDI r9, 4               ; TILE_SIZE pixels
LDI r10, 0xFFB          ; key bitmask port
LDI r11, 0x7800         ; camera_x address
LDI r12, 0x7801         ; camera_y address
LDI r13, 0x7802         ; frame_counter address

; ===== Main Loop =====
main_loop:

; --- Increment frame counter ---
LOAD r17, r13
ADD r17, r7
STORE r13, r17          ; frame_counter++

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

; --- Clear screen to black ---
LDI r17, 0
FILL r17

; ===== Render Viewport =====
; r14 = camera_x, r15 = camera_y
; r22 = frame_counter (loaded once)
; 64x64 tile loop: ty=0..63, tx=0..63
; Per tile: coarse hash -> biome, fine hash -> structure check, color -> RECTF

LOAD r22, r13           ; r22 = frame_counter (load once for whole frame)
LDI r1, 0               ; ty = 0

render_y:
  LDI r2, 0             ; tx = 0

  render_x:
    ; World coordinates
    MOV r3, r14
    ADD r3, r2           ; r3 = world_x = camera_x + tx
    MOV r4, r15
    ADD r4, r1           ; r4 = world_y = camera_y + ty

    ; ---- Coarse hash for contiguous biomes ----
    ; Zone size = 8 tiles (>> 3) = 32x32 pixel biome patches
    MOV r5, r3
    LDI r18, 3
    SHR r5, r18          ; r5 = world_x >> 3 (coarse_x)
    LDI r18, 99001
    MUL r5, r18          ; r5 = coarse_x * 99001

    MOV r6, r4
    LDI r18, 3
    SHR r6, r18          ; r6 = world_y >> 3 (coarse_y)
    LDI r18, 79007
    MUL r6, r18          ; r6 = coarse_y * 79007

    XOR r5, r6           ; r5 = coarse_hash

    ; Mix: multiply by a large prime to spread bits into upper positions
    LDI r18, 1103515245
    MUL r5, r18          ; r5 = coarse_hash * mixing_prime

    ; Extract top 4 bits: biome type 0..15
    LDI r18, 28
    SHR r5, r18          ; r5 = biome_type (0..15)

    ; ---- Fine hash for structure placement ----
    MOV r6, r3
    LDI r18, 374761393
    MUL r6, r18          ; r6 = world_x * big_prime
    MOV r21, r4
    LDI r18, 668265263
    MUL r21, r18         ; r21 = world_y * big_prime
    XOR r6, r21          ; r6 = fine_hash

    ; Structure if fine_hash & 0xFF == 0x2A (1/256 tiles, ~16 per screen)
    LDI r18, 0xFF
    MOV r21, r6
    AND r21, r18
    LDI r18, 42
    CMP r21, r18
    JNZ r0, no_struct

    ; Override with structure color based on biome
    ; water(0-1)->wave, beach(2)->hut, desert(3-4)->cactus,
    ; grass(5-6)->hut, swamp(7)->lily, forest(8-9)->hut,
    ; mountain(10-11)->snow patch, lava(12)->ember, snow(13-15)->crystal
    LDI r18, 2
    CMP r5, r18
    BLT r0, struct_water
    LDI r18, 3
    CMP r5, r18
    BLT r0, struct_land        ; beach gets hut
    LDI r18, 5
    CMP r5, r18
    BLT r0, struct_desert      ; 3-4 desert cactus
    LDI r18, 7
    CMP r5, r18
    BLT r0, struct_land        ; 5-6 grass hut
    LDI r18, 8
    CMP r5, r18
    BLT r0, struct_swamp       ; 7 swamp lily
    LDI r18, 10
    CMP r5, r18
    BLT r0, struct_land        ; 8-9 forest hut
    LDI r18, 12
    CMP r5, r18
    BLT r0, struct_mountain    ; 10-11
    LDI r18, 13
    CMP r5, r18
    BLT r0, struct_lava        ; 12
    JMP struct_snow            ; 13-15

struct_water:
    LDI r17, 0x0066CC    ; wave crest (bright blue)
    JMP do_rect
struct_desert:
    LDI r17, 0x228800    ; cactus (green)
    JMP do_rect
struct_land:
    LDI r17, 0x884422    ; tree trunk / hut (brown)
    JMP do_rect
struct_swamp:
    LDI r17, 0x44BB44    ; lily pad (bright green)
    JMP do_rect
struct_mountain:
    LDI r17, 0xBBBBCC    ; snow patch (pale)
    JMP do_rect
struct_lava:
    LDI r17, 0xFF8800    ; ember (orange)
    JMP do_rect
struct_snow:
    LDI r17, 0xAABBEE    ; ice crystal (blue-white)
    JMP do_rect

no_struct:
    ; ---- Biome -> Color (using r5 = biome_type 0..15) ----
    ; Cascading comparisons. r0 set by CMP; BLT/BGE/JZ check r0.
    ; water(0-1), beach(2), desert(3-4), grass(5-6),
    ; swamp(7), forest(8-9), mountain(10-11), lava(12), snow(13-15)

    ; Is it water? (types 0-1)
    LDI r18, 2
    CMP r5, r18
    BLT r0, color_water

    ; Beach? (type 2)
    LDI r18, 3
    CMP r5, r18
    BLT r0, color_beach

    ; Desert? (types 3-4)
    LDI r18, 5
    CMP r5, r18
    BLT r0, color_desert

    ; Grass? (types 5-6)
    LDI r18, 7
    CMP r5, r18
    BLT r0, color_grass

    ; Swamp? (type 7)
    LDI r18, 8
    CMP r5, r18
    BLT r0, color_swamp

    ; Forest? (types 8-9)
    LDI r18, 10
    CMP r5, r18
    BLT r0, color_forest

    ; Hills/mountain? (types 10-11)
    LDI r18, 12
    CMP r5, r18
    BLT r0, color_mountain

    ; Lava? (type 12)
    LDI r18, 13
    CMP r5, r18
    BLT r0, color_lava

    ; Snow/ice (types 13-15)
    JMP color_snow

    ; ---- Water subtypes (animated with frame counter) ----
color_water:
    LDI r18, 1
    CMP r5, r18
    BLT r0, water_deep
    ; type 1 = shallow
    LDI r17, 0x0000BB    ; shallow water
    JMP water_animate
water_deep:
    LDI r17, 0x000044    ; deep ocean

water_animate:
    ; Diagonal wave: shimmer = (frame_counter + world_x + world_y) & 0x1F
    ; Creates a moving wave pattern across water tiles
    MOV r21, r22         ; r21 = frame_counter
    ADD r21, r3          ; + world_x
    ADD r21, r4          ; + world_y
    LDI r18, 0x1F
    AND r21, r18         ; r21 = 0..31
    ADD r17, r21         ; shimmer the blue channel
    JMP do_rect

color_beach:
    LDI r17, 0xC2B280    ; sand
    JMP do_rect

    ; ---- Desert subtypes ----
color_desert:
    LDI r18, 4
    CMP r5, r18
    JZ r0, desert_dunes
    LDI r17, 0xDDBB44    ; sand
    JMP do_rect
desert_dunes:
    LDI r17, 0xCCAA33    ; dunes
    JMP do_rect

    ; ---- Grass subtypes ----
color_grass:
    LDI r18, 6
    CMP r5, r18
    JZ r0, grass_dark
    LDI r17, 0x55BB33    ; light grass
    JMP do_rect
grass_dark:
    LDI r17, 0x228811    ; dark grass
    JMP do_rect

    ; ---- Swamp ----
color_swamp:
    LDI r17, 0x445522    ; dark green-brown
    JMP do_rect

    ; ---- Forest subtypes ----
color_forest:
    LDI r18, 9
    CMP r5, r18
    JZ r0, forest_dense
    LDI r17, 0x116600    ; forest
    JMP do_rect
forest_dense:
    LDI r17, 0x0A4400    ; dense forest
    JMP do_rect

    ; ---- Mountain subtypes ----
color_mountain:
    LDI r18, 11
    CMP r5, r18
    JZ r0, mt_tall
    LDI r17, 0x667766    ; foothills
    JMP do_rect
mt_tall:
    LDI r17, 0x999999    ; tall mountain
    JMP do_rect

    ; ---- Lava ----
color_lava:
    LDI r17, 0xFF3300    ; red-orange lava
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

; ===== Minimap Overlay (16x16, top-right corner) =====
; Shows biome overview: samples every 4th tile in a 64x64 area centered on camera.
; Screen coords: x=240..255, y=0..15

LDI r1, 0               ; my = 0
mm_y:
  LDI r2, 0             ; mx = 0
  mm_x:
    ; World tile: camera_x + mx*4, camera_y + my*4
    MOV r3, r2
    LDI r18, 4
    MUL r3, r18          ; r3 = mx * 4
    ADD r3, r14          ; r3 = world_x

    MOV r4, r1
    LDI r18, 4
    MUL r4, r18          ; r4 = my * 4
    ADD r4, r15          ; r4 = world_y

    ; Coarse hash for biome
    MOV r5, r3
    LDI r18, 3
    SHR r5, r18          ; r5 = world_x >> 3
    LDI r18, 99001
    MUL r5, r18

    MOV r6, r4
    LDI r18, 3
    SHR r6, r18          ; r6 = world_y >> 3
    LDI r18, 79007
    MUL r6, r18

    XOR r5, r6
    LDI r18, 1103515245
    MUL r5, r18
    LDI r18, 28
    SHR r5, r18          ; biome 0..15

    ; 6-category color map (dimmed for minimap)
    LDI r18, 2
    CMP r5, r18
    BLT r0, mm_water          ; 0-1 water
    LDI r18, 3
    CMP r5, r18
    BLT r0, mm_shore          ; 2 beach
    LDI r18, 5
    CMP r5, r18
    BLT r0, mm_desert         ; 3-4 desert
    LDI r18, 8
    CMP r5, r18
    BLT r0, mm_green          ; 5-7 grass/swamp
    LDI r18, 10
    CMP r5, r18
    BLT r0, mm_forest         ; 8-9 forest
    LDI r18, 13
    CMP r5, r18
    BLT r0, mm_gray           ; 10-12 mountain/lava
    JMP mm_white              ; 13-15 snow

mm_water:
    LDI r17, 0x000055    ; dim blue
    JMP mm_draw
mm_shore:
    LDI r17, 0x554422    ; dim sand
    JMP mm_draw
mm_desert:
    LDI r17, 0x665522    ; dim desert yellow
    JMP mm_draw
mm_green:
    LDI r17, 0x225500    ; dim green
    JMP mm_draw
mm_forest:
    LDI r17, 0x113300    ; dim dark green
    JMP mm_draw
mm_gray:
    LDI r17, 0x553311    ; dim brown-gray (mountain/lava)
    JMP mm_draw
mm_white:
    LDI r17, 0x8888AA    ; dim white-blue (snow)
    JMP mm_draw

mm_draw:
    ; Screen pos: x = 240 + mx, y = my
    MOV r3, r2
    LDI r18, 240
    ADD r3, r18
    PSET r3, r1, r17

    ; mx++
    ADD r2, r7
    LDI r18, 16
    MOV r19, r2
    SUB r19, r18
    JZ r19, mm_next_row
    JMP mm_x

mm_next_row:
    ; my++
    ADD r1, r7
    LDI r18, 16
    MOV r19, r1
    SUB r19, r18
    JZ r19, mm_border
    JMP mm_y

; --- Border (1px frame) ---
mm_border:
LDI r17, 0xAAAAAA       ; border gray
LDI r18, 1              ; thin dimension
LDI r19, 16             ; long dimension

; Top: (240,0) 16x1
LDI r3, 240
LDI r4, 0
RECTF r3, r4, r19, r18, r17

; Bottom: (240,15) 16x1
LDI r4, 15
RECTF r3, r4, r19, r18, r17

; Left: (240,0) 1x16
LDI r4, 0
RECTF r3, r4, r18, r19, r17

; Right: (255,0) 1x16
LDI r3, 255
RECTF r3, r4, r18, r19, r17

; --- Player dot (white, center) ---
LDI r3, 248             ; 240 + 8
LDI r4, 8
LDI r17, 0xFFFFFF
PSET r3, r4, r17

    FRAME
    JMP main_loop
