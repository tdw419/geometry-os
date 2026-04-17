; infinite_map.asm -- Infinite scrolling procedural terrain (v5)
;
; Arrow keys / WASD scroll through infinite procedurally generated terrain.
; Diagonal keys (bits 4-7) allow single-key diagonal scrolling.
; Two-level hash: coarse hash determines biome (8x8 tile zones = 32px blocks),
; fine hash places structures (1/256 tiles get a tree/rock/crystal).
; Water tiles animate (shimmer) based on frame counter.
; Day/night tint: camera_x position shifts color warmth -- west is cooler,
; east is warmer. 16 zones, subtle top-nibble adjustments per channel.
; Pure math -- no stored world data, truly infinite.
;
; Tile size = 4 pixels. Viewport = 64x64 tiles = 256x256 pixels.
; Renders via RECTF. ~322K instructions/frame (32% of 1M budget).
; Optimized: day/night tint precomputed once per frame (was per-tile ~40K savings),
; screen position via incrementing accumulators (was MUL per-tile ~8K savings),
; viewport (64x64 tiles) exactly fills 256x256 screen -- no off-screen tiles,
;   so bounds checks removed (was 4 dead ops/tile = 16K savings).
;
; Memory:
;   RAM[0x7800] = camera_x (tile coordinates)
;   RAM[0x7801] = camera_y (tile coordinates)
;   RAM[0x7802] = frame_counter (increments each frame)
;   RAM[0xFFB]  = key bitmask (host writes each frame)
;
; Biome distribution (21 biomes, types 0-31):
;   water(0-1), beach(2), desert(3-4), oasis(5), grass(6-7),
;   swamp(8-9), forest(10-11), mushroom(12), mountain(13-14),
;   tundra(15), lava(16-17), volcanic(18), snow(19-21),
;   coral(22), ruins(23), crystal(24-25), ash(26),
;   deadlands(27-28), bioluminescent(29-30), void(31)

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

; --- Process Up+Right diagonal (bit 4) ---
MOV r17, r16
LDI r18, 16
AND r17, r18
JZ r17, no_ur
SUB r15, r7
ADD r14, r7
no_ur:

; --- Process Down+Right diagonal (bit 5) ---
MOV r17, r16
LDI r18, 32
AND r17, r18
JZ r17, no_dr
ADD r15, r7
ADD r14, r7
no_dr:

; --- Process Down+Left diagonal (bit 6) ---
MOV r17, r16
LDI r18, 64
AND r17, r18
JZ r17, no_dl
ADD r15, r7
SUB r14, r7
no_dl:

; --- Process Up+Left diagonal (bit 7) ---
MOV r17, r16
LDI r18, 128
AND r17, r18
JZ r17, no_ul
SUB r15, r7
SUB r14, r7
no_ul:

; --- Store updated camera ---
STORE r11, r14
STORE r12, r15

; --- Clear screen to black ---
LDI r17, 0
FILL r17

; ===== Precompute day/night tint (once per frame) =====
; Tint depends only on camera_x, which is constant within a frame.
; zone = (camera_x >> 4) & 0xF  ->  16 zones across the world
; West  (zone 0-7): negate zone*0x0808 so ADD performs subtraction
; East  (zone 8-15): (zone-8)*0x080000, ADD boosts red
MOV r18, r14
LDI r19, 4
SHR r18, r19           ; camera_x >> 4
LDI r19, 0xF
AND r18, r19           ; zone = 0..15
LDI r19, 8
CMP r18, r19
BGE r0, pre_tint_warm  ; zone >= 8 -> east
LDI r19, 0x0808
MUL r18, r19
NEG r18                ; negate: ADD will subtract (cool/west tint)
MOV r23, r18           ; r23 = tint offset (sign-encoded)
JMP pre_tint_done
pre_tint_warm:
SUB r18, r19           ; zone - 8
LDI r19, 0x080000
MUL r18, r19
MOV r23, r18           ; r23 = tint offset (positive, warm/east)
pre_tint_done:

; ===== Render Viewport =====
; r14 = camera_x, r15 = camera_y
; r22 = frame_counter (loaded once)
; r23 = precomputed tint offset (sign-encoded: negative=west, positive=east)
; r25 = screen_y accumulator, r26 = screen_x accumulator
; 64x64 tile loop: ty=0..63, tx=0..63
; Per tile: coarse hash -> biome, fine hash -> structure check, color -> RECTF

LOAD r22, r13           ; r22 = frame_counter (load once for whole frame)
LDI r1, 0               ; ty = 0
LDI r25, 0              ; screen_y = 0 (accumulator, replaces ty*4 multiply)

render_y:
  LDI r2, 0             ; tx = 0
  LDI r26, 0            ; screen_x = 0 (accumulator, replaces tx*4 multiply)

  render_x:
    ; All 64x64 tiles are on-screen (64*4=256 = screen size), no bounds check needed.

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

    ; Extract top 5 bits: biome type 0..31
    LDI r18, 27
    SHR r5, r18          ; r5 = biome_type (0..31)

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
    ; oasis(5)->palm, grass(6-7)->hut, swamp(8-9)->lily,
    ; forest(10-11)->hut, mushroom(12)->cap, mountain(13-14)->snow patch,
    ; tundra(15)->frost, lava(16-17)->ember, volcanic(18)->vent,
    ; snow(19-21)->crystal, coral(22)->anemone, ruins(23)->pillar,
    ; crystal(24-25)->cluster, ash(26)->geyser, deadlands(27-28)->bone,
    ; bioluminescent(29-30)->spore, void(31)->spark
    LDI r18, 2
    CMP r5, r18
    BLT r0, struct_water       ; 0-1 water
    LDI r18, 3
    CMP r5, r18
    BLT r0, struct_land        ; 2 beach hut
    LDI r18, 5
    CMP r5, r18
    BLT r0, struct_desert      ; 3-4 desert cactus
    LDI r18, 6
    CMP r5, r18
    BLT r0, struct_oasis       ; 5 oasis palm
    LDI r18, 8
    CMP r5, r18
    BLT r0, struct_land        ; 6-7 grass hut
    LDI r18, 10
    CMP r5, r18
    BLT r0, struct_swamp       ; 8-9 swamp lily
    LDI r18, 12
    CMP r5, r18
    BLT r0, struct_land        ; 10-11 forest hut
    LDI r18, 13
    CMP r5, r18
    BLT r0, struct_mushroom    ; 12 mushroom cap
    LDI r18, 15
    CMP r5, r18
    BLT r0, struct_mountain    ; 13-14
    LDI r18, 16
    CMP r5, r18
    BLT r0, struct_tundra      ; 15 tundra frost
    LDI r18, 18
    CMP r5, r18
    BLT r0, struct_lava        ; 16-17 lava ember
    LDI r18, 19
    CMP r5, r18
    BLT r0, struct_volcanic    ; 18 volcanic vent
    LDI r18, 22
    CMP r5, r18
    BLT r0, struct_snow        ; 19-21 snow crystal
    LDI r18, 23
    CMP r5, r18
    BLT r0, struct_coral       ; 22 coral anemone
    LDI r18, 24
    CMP r5, r18
    BLT r0, struct_ruins       ; 23 ruins pillar
    LDI r18, 26
    CMP r5, r18
    BLT r0, struct_crystal     ; 24-25 crystal cluster
    LDI r18, 27
    CMP r5, r18
    BLT r0, struct_ash         ; 26 ash geyser
    LDI r18, 29
    CMP r5, r18
    BLT r0, struct_dead        ; 27-28 deadlands bone
    LDI r18, 31
    CMP r5, r18
    BLT r0, struct_biolum      ; 29-30 bioluminescent spore
    JMP struct_void            ; 31 void spark

struct_water:
    LDI r17, 0x0066CC    ; wave crest (bright blue)
    JMP do_rect
struct_desert:
    LDI r17, 0x228800    ; cactus (green)
    JMP do_rect
struct_oasis:
    LDI r17, 0x33CC33    ; palm frond (bright green)
    JMP do_rect
struct_land:
    LDI r17, 0x884422    ; tree trunk / hut (brown)
    ; -- tree sway: shimmer green every 4th frame, offset by world_x
    MOV r20, r22
    ADD r20, r3           ; frame_counter + world_x
    LDI r18, 3
    AND r20, r18          ; & 3 -> 0..3
    JNZ r20, struct_land_done
    LDI r18, 0x001100    ; brighter foliage flicker
    ADD r17, r18
struct_land_done:
    JMP do_rect
struct_swamp:
    LDI r17, 0x44BB44    ; lily pad (bright green)
    JMP do_rect
struct_mushroom:
    LDI r17, 0xBB22BB    ; mushroom cap (purple-red)
    JMP do_rect
struct_mountain:
    LDI r17, 0xBBBBCC    ; snow patch (pale)
    JMP do_rect
struct_tundra:
    LDI r17, 0xCCDDFF    ; frost crystal (pale blue)
    JMP do_rect
struct_lava:
    LDI r17, 0xFF8800    ; ember (orange)
    ; -- ember pulse: blue flicker based on frame + world_y
    MOV r20, r22
    ADD r20, r4           ; frame_counter + world_y
    LDI r18, 7
    AND r20, r18          ; & 7 -> 0..7
    ADD r17, r20          ; subtle blue channel flicker
    JMP do_rect
struct_volcanic:
    LDI r17, 0xFFDD00    ; fire vent (yellow-orange)
    JMP do_rect
struct_snow:
    LDI r17, 0xAABBEE    ; ice crystal (blue-white)
    JMP do_rect
struct_coral:
    LDI r17, 0xFF77AA    ; anemone (pink)
    JMP do_rect
struct_ruins:
    LDI r17, 0x998877    ; stone pillar (weathered gray)
    JMP do_rect
struct_crystal:
    LDI r17, 0x22DDCC    ; crystal cluster (bright teal)
    ; -- crystal sparkle: XOR flicker per frame, shifted into hue variation
    MOV r20, r22
    XOR r20, r3           ; frame_counter ^ world_x
    LDI r18, 0xF
    AND r20, r18          ; & 0xF -> 0..15
    LDI r18, 4
    SHL r20, r18          ; shift left 4 -> 0..0xF0
    ADD r17, r20
    JMP do_rect
struct_ash:
    LDI r17, 0x666655    ; ash geyser (dark grey-green)
    JMP do_rect
struct_dead:
    LDI r17, 0xBBAA99    ; bleached bone (pale tan)
    JMP do_rect
struct_biolum:
    LDI r17, 0x00FFAA    ; glowing spore (bright cyan-green)
    ; -- spore glow cycle: slow 4-step green pulse via frame >> 2
    MOV r20, r22
    LDI r18, 2
    SHR r20, r18          ; frame_counter >> 2 (slow cycle)
    LDI r18, 3
    AND r20, r18          ; & 3 -> 0..3
    LDI r18, 0x002200    ; green pulse unit
    MUL r20, r18          ; 0, 0x002200, 0x004400, or 0x006600
    ADD r17, r20
    JMP do_rect
struct_void:
    LDI r17, 0x440088    ; void spark (deep purple)
    JMP do_rect

no_struct:
    ; ---- Biome -> Color (using r5 = biome_type 0..31) ----
    ; Cascading comparisons. r0 set by CMP; BLT/BGE/JZ check r0.
    ; water(0-1), beach(2), desert(3-4), oasis(5), grass(6-7),
    ; swamp(8-9), forest(10-11), mushroom(12), mountain(13-14),
    ; tundra(15), lava(16-17), volcanic(18), snow(19-21),
    ; coral(22), ruins(23), crystal(24-25), ash(26),
    ; deadlands(27-28), bioluminescent(29-30), void(31)

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

    ; Oasis? (type 5)
    LDI r18, 6
    CMP r5, r18
    BLT r0, color_oasis

    ; Grass? (types 6-7)
    LDI r18, 8
    CMP r5, r18
    BLT r0, color_grass

    ; Swamp? (types 8-9)
    LDI r18, 10
    CMP r5, r18
    BLT r0, color_swamp

    ; Forest? (types 10-11)
    LDI r18, 12
    CMP r5, r18
    BLT r0, color_forest

    ; Mushroom? (type 12)
    LDI r18, 13
    CMP r5, r18
    BLT r0, color_mushroom

    ; Mountain? (types 13-14)
    LDI r18, 15
    CMP r5, r18
    BLT r0, color_mountain

    ; Tundra? (type 15)
    LDI r18, 16
    CMP r5, r18
    BLT r0, color_tundra

    ; Lava? (types 16-17)
    LDI r18, 18
    CMP r5, r18
    BLT r0, color_lava

    ; Volcanic? (type 18)
    LDI r18, 19
    CMP r5, r18
    BLT r0, color_volcanic

    ; Snow? (types 19-21)
    LDI r18, 22
    CMP r5, r18
    BLT r0, color_snow

    ; Coral? (type 22)
    LDI r18, 23
    CMP r5, r18
    BLT r0, color_coral

    ; Ruins? (type 23)
    LDI r18, 24
    CMP r5, r18
    BLT r0, color_ruins

    ; Crystal Caverns? (types 24-25)
    LDI r18, 26
    CMP r5, r18
    BLT r0, color_crystal

    ; Ash Wastes? (type 26)
    LDI r18, 27
    CMP r5, r18
    BLT r0, color_ash

    ; Deadlands? (types 27-28)
    LDI r18, 29
    CMP r5, r18
    BLT r0, color_dead

    ; Bioluminescent? (types 29-30)
    LDI r18, 31
    CMP r5, r18
    BLT r0, color_biolum

    ; Void? (type 31) -> render as dark abyss
    JMP color_void

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

    ; ---- Oasis ----
color_oasis:
    LDI r17, 0x22AA55    ; lush green pool
    JMP do_rect

    ; ---- Grass subtypes ----
color_grass:
    LDI r18, 7
    CMP r5, r18
    JZ r0, grass_dark
    LDI r17, 0x55BB33    ; light grass
    JMP do_rect
grass_dark:
    LDI r17, 0x228811    ; dark grass
    JMP do_rect

    ; ---- Swamp subtypes ----
color_swamp:
    LDI r18, 9
    CMP r5, r18
    JZ r0, swamp_mangrove
    LDI r17, 0x445522    ; murky green-brown
    JMP do_rect
swamp_mangrove:
    LDI r17, 0x2D4A1A    ; dark mangrove
    JMP do_rect

    ; ---- Forest subtypes ----
color_forest:
    LDI r18, 11
    CMP r5, r18
    JZ r0, forest_dense
    LDI r17, 0x116600    ; forest
    JMP do_rect
forest_dense:
    LDI r17, 0x0A4400    ; dense forest
    JMP do_rect

    ; ---- Mushroom grove ----
color_mushroom:
    LDI r17, 0x883388    ; purple fungal ground
    JMP do_rect

    ; ---- Mountain subtypes ----
color_mountain:
    LDI r18, 14
    CMP r5, r18
    JZ r0, mt_tall
    LDI r17, 0x667766    ; foothills
    JMP do_rect
mt_tall:
    LDI r17, 0x999999    ; tall mountain
    JMP do_rect

    ; ---- Tundra ----
color_tundra:
    LDI r17, 0x8899AA    ; cold rocky gray-blue
    JMP do_rect

    ; ---- Lava subtypes ----
color_lava:
    LDI r18, 17
    CMP r5, r18
    JZ r0, lava_cooled
    LDI r17, 0xFF3300    ; red-orange flowing lava
    JMP do_rect
lava_cooled:
    LDI r17, 0x332222    ; cooled basalt (dark)
    JMP do_rect

    ; ---- Volcanic wasteland ----
color_volcanic:
    LDI r17, 0x442211    ; scorched earth (dark brown-red)
    JMP do_rect

    ; ---- Snow subtypes ----
color_snow:
    LDI r18, 20
    CMP r5, r18
    BLT r0, snow_light
    LDI r18, 21
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

    ; ---- Coral reef ----
color_coral:
    LDI r17, 0x3377AA    ; shallow turquoise water
    JMP do_rect

    ; ---- Ruins ----
color_ruins:
    LDI r17, 0x776655    ; weathered stone
    JMP do_rect

    ; ---- Crystal Cavern subtypes ----
color_crystal:
    LDI r18, 25
    CMP r5, r18
    JZ r0, crystal_dense
    LDI r17, 0x1A3333    ; dark teal cavern
    JMP do_rect
crystal_dense:
    LDI r17, 0x2A5555    ; lighter teal crystal
    JMP do_rect

    ; ---- Ash Wastes ----
color_ash:
    LDI r17, 0x444444    ; dark grey volcanic ash
    JMP do_rect

    ; ---- Deadlands subtypes ----
color_dead:
    LDI r18, 28
    CMP r5, r18
    JZ r0, dead_cracked
    LDI r17, 0x3D2B1F    ; dark cracked earth
    JMP do_rect
dead_cracked:
    LDI r17, 0x4A3525    ; dry barren brown
    JMP do_rect

    ; ---- Bioluminescent subtypes ----
color_biolum:
    LDI r18, 30
    CMP r5, r18
    JZ r0, biolum_glow
    LDI r17, 0x004433    ; dark fungal cavern
    JMP do_rect
biolum_glow:
    LDI r17, 0x006655    ; brighter glowing cavern
    JMP do_rect

    ; ---- Void ----
color_void:
    LDI r17, 0x110022    ; near-black deep purple abyss
    JMP do_rect

    ; ---- Draw tile ----
do_rect:
    ; ---- Apply precomputed day/night tint (1 instruction vs ~15 before) ----
    ADD r17, r23
    ; Use screen position accumulators (no multiply needed)
    RECTF r26, r25, r9, r9, r17  ; fill 4x4 rect with color

    ; ---- Next tile (shared by on-screen and off-screen paths) ----
next_tile:
    ADD r2, r7           ; tx++
    ADD r26, r9          ; screen_x += TILE_SIZE
    MOV r18, r2
    SUB r18, r8          ; tx - 64
    JZ r18, next_row
    JMP render_x

next_row:
    ADD r1, r7           ; ty++
    ADD r25, r9          ; screen_y += TILE_SIZE
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
    LDI r18, 27
    SHR r5, r18          ; biome 0..31

    ; 10-category color map (dimmed for minimap)
    LDI r18, 2
    CMP r5, r18
    BLT r0, mm_water          ; 0-1 water
    LDI r18, 3
    CMP r5, r18
    BLT r0, mm_shore          ; 2 beach
    LDI r18, 5
    CMP r5, r18
    BLT r0, mm_desert         ; 3-4 desert
    LDI r18, 6
    CMP r5, r18
    BLT r0, mm_oasis          ; 5 oasis
    LDI r18, 8
    CMP r5, r18
    BLT r0, mm_green          ; 6-7 grass
    LDI r18, 10
    CMP r5, r18
    BLT r0, mm_swamp          ; 8-9 swamp
    LDI r18, 12
    CMP r5, r18
    BLT r0, mm_forest         ; 10-11 forest
    LDI r18, 13
    CMP r5, r18
    BLT r0, mm_mushroom       ; 12 mushroom
    LDI r18, 15
    CMP r5, r18
    BLT r0, mm_gray           ; 13-14 mountain
    LDI r18, 16
    CMP r5, r18
    BLT r0, mm_tundra         ; 15 tundra
    LDI r18, 18
    CMP r5, r18
    BLT r0, mm_lava           ; 16-17 lava
    LDI r18, 19
    CMP r5, r18
    BLT r0, mm_volcanic       ; 18 volcanic
    LDI r18, 22
    CMP r5, r18
    BLT r0, mm_white          ; 19-21 snow
    LDI r18, 23
    CMP r5, r18
    BLT r0, mm_coral          ; 22 coral
    LDI r18, 24
    CMP r5, r18
    BLT r0, mm_ruins          ; 23 ruins
    LDI r18, 26
    CMP r5, r18
    BLT r0, mm_crystal        ; 24-25 crystal
    LDI r18, 27
    CMP r5, r18
    BLT r0, mm_ash            ; 26 ash
    LDI r18, 29
    CMP r5, r18
    BLT r0, mm_dead           ; 27-28 deadlands
    LDI r18, 31
    CMP r5, r18
    BLT r0, mm_biolum         ; 29-30 bioluminescent
    JMP mm_void               ; 31 void

mm_water:
    LDI r17, 0x000055    ; dim blue
    JMP mm_draw
mm_shore:
    LDI r17, 0x554422    ; dim sand
    JMP mm_draw
mm_desert:
    LDI r17, 0x665522    ; dim desert yellow
    JMP mm_draw
mm_oasis:
    LDI r17, 0x225533    ; dim oasis green
    JMP mm_draw
mm_green:
    LDI r17, 0x225500    ; dim green
    JMP mm_draw
mm_swamp:
    LDI r17, 0x1A2200    ; dim murky green
    JMP mm_draw
mm_forest:
    LDI r17, 0x113300    ; dim dark green
    JMP mm_draw
mm_mushroom:
    LDI r17, 0x441144    ; dim purple
    JMP mm_draw
mm_gray:
    LDI r17, 0x444444    ; dim gray (mountain)
    JMP mm_draw
mm_tundra:
    LDI r17, 0x445566    ; dim cold blue-gray
    JMP mm_draw
mm_lava:
    LDI r17, 0x551100    ; dim red-orange (lava)
    JMP mm_draw
mm_volcanic:
    LDI r17, 0x331100    ; dim dark red-brown (volcanic)
    JMP mm_draw
mm_white:
    LDI r17, 0x8888AA    ; dim white-blue (snow)
    JMP mm_draw
mm_coral:
    LDI r17, 0x224466    ; dim turquoise (coral)
    JMP mm_draw
mm_ruins:
    LDI r17, 0x443322    ; dim brown-gray (ruins)
    JMP mm_draw
mm_crystal:
    LDI r17, 0x113333    ; dim teal (crystal caverns)
    JMP mm_draw
mm_ash:
    LDI r17, 0x222222    ; dim grey (ash wastes)
    JMP mm_draw
mm_dead:
    LDI r17, 0x1A1008    ; dim brown (deadlands)
    JMP mm_draw
mm_biolum:
    LDI r17, 0x003322    ; dim cyan-green (bioluminescent)
    JMP mm_draw
mm_void:
    LDI r17, 0x0A0011    ; dim deep purple (void)
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
