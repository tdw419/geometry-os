; infinite_map.asm -- Infinite scrolling procedural terrain (v10)
;
; Arrow keys / WASD scroll through infinite procedurally generated terrain.
; Diagonal keys (bits 4-7) allow single-key diagonal scrolling.
; Two-level hash: coarse hash determines biome (8x8 tile zones = 32px blocks),
; fine hash places structures (1/256 tiles get a tree/rock/crystal).
; Water tiles animate (shimmer) based on frame counter.
; Day/night tint: camera_x position shifts color warmth -- west is cooler,
; east is warmer. 16 zones, subtle top-nibble adjustments per channel.
; Biome-aware pattern overlay: each biome gets a deterministic texture type
; (horizontal/vertical/center/corner/diagonal/dither/topedge) with animated
; accent color via frame_counter XOR. Makes terrain look like what it represents.
; Pure math -- no stored world data, truly infinite.
;
; v10: BPE/LINEAR color variation. The fine_hash seed is decoded through a
; Pixelpack-style BPE variation table. Two nibbles each index into a 16-entry
; table of pre-computed RGB offsets, and the results are linearly combined
; (ADDed) with the base biome color. This gives 16x16=256 unique per-tile
; color variations (up from 16 with the old G-channel XOR), producing much
; richer visual texture within each biome zone.
;
; Tile size = 4 pixels. Viewport = 64x64 tiles = 256x256 pixels.
; Renders via RECTF + 1 PSET accent per tile. ~410K instructions/frame.
; Player cursor: pulsing white/yellow crosshair at screen center (127,127).
;   4 arms (3px each) with 1px center gap. Pulses every 16 frames.
;
; Memory:
;   RAM[0x7800] = camera_x (tile coordinates)
;   RAM[0x7801] = camera_y (tile coordinates)
;   RAM[0x7802] = frame_counter (increments each frame)
;   RAM[0xFFB]  = key bitmask (host writes each frame)
;   RAM[0x7900-0x791F] = pattern table (32 biome -> pattern type mappings)
;   RAM[0x7A00-0x7A1F] = color table (32 biome -> base color mappings)
;   RAM[0x7B00-0x7B0F] = BPE variation table (16 entries, Pixelpack seed expansion)
;
; Pattern types (2-bit per biome):
;   0=horizontal: water(0-1), beach(2), desert(3-4), snow(19-21) -- ripple/drift
;   1=vertical: mountain(13-14), tundra(15), ruins(23), crystal(24-25) -- ridge/pillar
;   2=center: oasis(5), grass(6-7), swamp(8-9), forest(10-11), mushroom(12),
;             coral(22), biolum(29-30) -- canopy/dot
;   3=corner: lava(16-17), volcanic(18), ash(26), deadlands(27-28), void(31) -- spark
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

; ===== Pattern Table (32 entries at 0x7900-0x791F) =====
; 3-bit pattern per biome: 0=horiz 1=vert 2=center 3=corner 4=diag\ 5=diag/ 6=topedge 7=dither
LDI r20, 0x7900
LDI r17, 0              ; water(0) -> horizontal
STORE r20, r17
ADD r20, r7
STORE r20, r17           ; water(1) -> horizontal
ADD r20, r7
LDI r17, 4
STORE r20, r17           ; beach(2) -> diagonal\ (wave wash)
ADD r20, r7
LDI r17, 0
STORE r20, r17           ; desert(3) -> horizontal
ADD r20, r7
STORE r20, r17           ; desert(4) -> horizontal
ADD r20, r7
LDI r17, 2
STORE r20, r17           ; oasis(5) -> center
ADD r20, r7
LDI r17, 7
STORE r20, r17           ; grass(6) -> dither (grass clumps)
ADD r20, r7
STORE r20, r17           ; grass(7) -> dither
ADD r20, r7
LDI r17, 4
STORE r20, r17           ; swamp(8) -> diagonal\ (murky)
ADD r20, r7
STORE r20, r17           ; swamp(9) -> diagonal\
ADD r20, r7
LDI r17, 5
STORE r20, r17           ; forest(10) -> diagonal/ (canopy)
ADD r20, r7
STORE r20, r17           ; forest(11) -> diagonal/
ADD r20, r7
LDI r17, 2
STORE r20, r17           ; mushroom(12) -> center
ADD r20, r7
LDI r17, 1
STORE r20, r17           ; mountain(13) -> vertical
ADD r20, r7
STORE r20, r17           ; mountain(14) -> vertical
ADD r20, r7
LDI r17, 7
STORE r20, r17           ; tundra(15) -> dither (frost scatter)
ADD r20, r7
LDI r17, 3
STORE r20, r17           ; lava(16) -> corner
ADD r20, r7
STORE r20, r17           ; lava(17) -> corner
ADD r20, r7
STORE r20, r17           ; volcanic(18) -> corner
ADD r20, r7
LDI r17, 6
STORE r20, r17           ; snow(19) -> top edge (drift tops)
ADD r20, r7
STORE r20, r17           ; snow(20) -> top edge
ADD r20, r7
STORE r20, r17           ; snow(21) -> top edge
ADD r20, r7
LDI r17, 5
STORE r20, r17           ; coral(22) -> diagonal/ (branching)
ADD r20, r7
LDI r17, 1
STORE r20, r17           ; ruins(23) -> vertical (pillar)
ADD r20, r7
LDI r17, 4
STORE r20, r17           ; crystal(24) -> diagonal\ (facets)
ADD r20, r7
STORE r20, r17           ; crystal(25) -> diagonal\
ADD r20, r7
LDI r17, 3
STORE r20, r17           ; ash(26) -> corner
ADD r20, r7
LDI r17, 7
STORE r20, r17           ; deadlands(27) -> dither (debris)
ADD r20, r7
STORE r20, r17           ; deadlands(28) -> dither
ADD r20, r7
LDI r17, 2
STORE r20, r17           ; biolum(29) -> center
ADD r20, r7
STORE r20, r17           ; biolum(30) -> center
ADD r20, r7
LDI r17, 3
STORE r20, r17           ; void(31) -> corner

; ===== Color Table (32 entries at 0x7A00-0x7A1F) =====
; Direct biome_type -> base color lookup. Eliminates cascade dispatch.
LDI r20, 0x7A00
LDI r17, 0x000044
STORE r20, r17           ; 0: deep ocean
ADD r20, r7
LDI r17, 0x0000BB
STORE r20, r17           ; 1: shallow water
ADD r20, r7
LDI r17, 0xC2B280
STORE r20, r17           ; 2: beach sand
ADD r20, r7
LDI r17, 0xDDBB44
STORE r20, r17           ; 3: desert sand
ADD r20, r7
LDI r17, 0xCCAA33
STORE r20, r17           ; 4: desert dunes
ADD r20, r7
LDI r17, 0x22AA55
STORE r20, r17           ; 5: oasis
ADD r20, r7
LDI r17, 0x33AA33
STORE r20, r17           ; 6: grass light
ADD r20, r7
LDI r17, 0x228822
STORE r20, r17           ; 7: grass dark
ADD r20, r7
LDI r17, 0x336633
STORE r20, r17           ; 8: swamp light
ADD r20, r7
LDI r17, 0x224422
STORE r20, r17           ; 9: swamp dark
ADD r20, r7
LDI r17, 0x116611
STORE r20, r17           ; 10: forest light
ADD r20, r7
LDI r17, 0x004400
STORE r20, r17           ; 11: forest dark
ADD r20, r7
LDI r17, 0xAA6688
STORE r20, r17           ; 12: mushroom
ADD r20, r7
LDI r17, 0x888899
STORE r20, r17           ; 13: mountain rock
ADD r20, r7
LDI r17, 0x666677
STORE r20, r17           ; 14: mountain snow
ADD r20, r7
LDI r17, 0xAABBCC
STORE r20, r17           ; 15: tundra
ADD r20, r7
LDI r17, 0xFF3300
STORE r20, r17           ; 16: lava flowing
ADD r20, r7
LDI r17, 0x332222
STORE r20, r17           ; 17: lava cooled
ADD r20, r7
LDI r17, 0x442211
STORE r20, r17           ; 18: volcanic
ADD r20, r7
LDI r17, 0xCCCCEE
STORE r20, r17           ; 19: snow light
ADD r20, r7
LDI r17, 0xDDEEFF
STORE r20, r17           ; 20: snow ice
ADD r20, r7
LDI r17, 0xFFFFFF
STORE r20, r17           ; 21: snow peak
ADD r20, r7
LDI r17, 0x3377AA
STORE r20, r17           ; 22: coral
ADD r20, r7
LDI r17, 0x776655
STORE r20, r17           ; 23: ruins
ADD r20, r7
LDI r17, 0x1A3333
STORE r20, r17           ; 24: crystal dark
ADD r20, r7
LDI r17, 0x2A5555
STORE r20, r17           ; 25: crystal dense
ADD r20, r7
LDI r17, 0x444444
STORE r20, r17           ; 26: ash
ADD r20, r7
LDI r17, 0x554433
STORE r20, r17           ; 27: deadlands light
ADD r20, r7
LDI r17, 0x332211
STORE r20, r17           ; 28: deadlands dark
ADD r20, r7
LDI r17, 0x338866
STORE r20, r17           ; 29: biolum light
ADD r20, r7
LDI r17, 0x226644
STORE r20, r17           ; 30: biolum dark
ADD r20, r7
LDI r17, 0x110022
STORE r20, r17           ; 31: void

; ===== BPE Variation Table (16 entries at 0x7B00-0x7B0F) =====
; Pixelpack-style seed expansion: fine_hash nibbles index into this table,
; and results are linearly combined (ADDed) with the base biome color.
; Two nibble lookups give 16x16=256 unique per-tile color variations.
; Entries are packed (R_offset, G_offset, B_offset) small RGB triplets.
LDI r20, 0x7B00
LDI r17, 0x000000    ; 0: neutral
STORE r20, r17
ADD r20, r7
LDI r17, 0x040404    ; 1: all brighten +4
STORE r20, r17
ADD r20, r7
LDI r17, 0x080808    ; 2: all brighten +8
STORE r20, r17
ADD r20, r7
LDI r17, 0x0C0C0C    ; 3: all brighten +12
STORE r20, r17
ADD r20, r7
LDI r17, 0x080008    ; 4: warm shift (R+B)
STORE r20, r17
ADD r20, r7
LDI r17, 0x000808    ; 5: cool shift (G+B)
STORE r20, r17
ADD r20, r7
LDI r17, 0x080800    ; 6: gold shift (R+G)
STORE r20, r17
ADD r20, r7
LDI r17, 0x0C0400    ; 7: red bias
STORE r20, r17
ADD r20, r7
LDI r17, 0x000C04    ; 8: teal bias
STORE r20, r17
ADD r20, r7
LDI r17, 0x040C00    ; 9: green bias
STORE r20, r17
ADD r20, r7
LDI r17, 0xF80404    ; 10: R-dark (wrap subtract 8 from R)
STORE r20, r17
ADD r20, r7
LDI r17, 0x04F804    ; 11: G-dark (wrap subtract 8 from G)
STORE r20, r17
ADD r20, r7
LDI r17, 0x0404F8    ; 12: B-dark (wrap subtract 8 from B)
STORE r20, r17
ADD r20, r7
LDI r17, 0x0C0800    ; 13: warm heavy (R+G bias)
STORE r20, r17
ADD r20, r7
LDI r17, 0x000C08    ; 14: cool heavy (G+B bias)
STORE r20, r17
ADD r20, r7
LDI r17, 0x08040C    ; 15: balanced violet
STORE r20, r17

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
    ; ---- Biome -> Color via lookup table ----
    LDI r17, 0x7A00
    ADD r17, r5           ; r17 = &color_table[biome_type]
    LOAD r17, r17         ; r17 = base color

    ; Water shimmer: biome 0-1 get animated blue channel
    LDI r18, 2
    CMP r5, r18
    BGE r0, no_struct_not_water
    MOV r21, r22          ; frame_counter
    ADD r21, r3           ; + world_x
    ADD r21, r4           ; + world_y
    LDI r18, 0x1F
    AND r21, r18          ; shimmer = 0..31
    ADD r17, r21
no_struct_not_water:
    ; ---- BPE/LINEAR: Pixelpack-style multi-channel color variation ----
    ; Fine hash seed decoded through two nibble lookups into BPE table.
    ; Each lookup selects a pre-computed RGB offset; results ADDed to base.
    ; 16 x 16 = 256 unique per-tile color variations (was 16 with G-XOR).

    ; Nibble 0 (bits 0-3): first BPE pair lookup
    LDI r18, 0x7B00
    MOV r19, r6
    ANDI r19, 0xF
    ADD r18, r19
    LOAD r18, r18
    ADD r17, r18

    ; Nibble 1 (bits 4-7): second BPE pair lookup
    LDI r18, 0x7B00
    MOV r19, r6
    LDI r20, 4
    SHR r19, r20
    ANDI r19, 0xF
    ADD r18, r19
    LOAD r18, r18
    ADD r17, r18

    ; ---- Apply precomputed day/night tint ----
do_rect:
    ADD r17, r23
    ; Use screen position accumulators (no multiply needed)
    RECTF r26, r25, r9, r9, r17  ; fill 4x4 rect with color

    ; ---- Biome-aware pattern overlay (1 accent pixel per tile) ----
    ; Look up pattern type from boot-initialized table
    LDI r18, 0x7900
    ADD r18, r5           ; r18 = pattern_table[biome_type]
    LOAD r18, r18         ; r18 = pattern type (0-3)

    ; Accent color: brighten base + animate via frame_counter
    MOV r19, r17
    LDI r20, 0x181818
    ADD r19, r20          ; brighten
    MOV r20, r22
    LDI r21, 0xF
    AND r20, r21          ; frame_counter & 0xF
    LDI r21, 4
    SHL r20, r21          ; shift into nibble range (0..0xF0)
    ADD r19, r20          ; animated accent color

    ; Dispatch on pattern type (3-bit: 0-7)
    ; Use fine_hash bit 0 for per-tile position variation
    JZ r18, pat_horiz      ; pattern 0 = horizontal accent
    LDI r20, 1
    CMP r18, r20
    JZ r0, pat_vert        ; pattern 1 = vertical accent
    LDI r20, 2
    CMP r18, r20
    JZ r0, pat_center      ; pattern 2 = center dot
    LDI r20, 3
    CMP r18, r20
    JZ r0, pat_corner      ; pattern 3 = corner spark
    LDI r20, 4
    CMP r18, r20
    JZ r0, pat_diag_bl     ; pattern 4 = diagonal backslash
    LDI r20, 5
    CMP r18, r20
    JZ r0, pat_diag_fw     ; pattern 5 = diagonal forward slash
    LDI r20, 6
    CMP r18, r20
    JZ r0, pat_topedge     ; pattern 6 = top edge
    JMP pat_dither          ; pattern 7 = dither scatter

pat_horiz:
    ; Horizontal accent: pixel at (screen_x + 1 or 2, screen_y + 2)
    MOV r20, r25
    LDI r21, 2
    ADD r20, r21          ; y = screen_y + 2 (middle row)
    MOV r18, r26
    LDI r21, 1
    ADD r18, r21          ; default col = screen_x + 1
    MOV r21, r6
    LDI r17, 1
    AND r21, r17          ; fine_hash & 1
    JZ r21, pat_h_draw
    ADD r18, r7           ; col = screen_x + 2
pat_h_draw:
    PSET r18, r20, r19
    JMP next_tile

pat_vert:
    ; Vertical accent: pixel at (screen_x + 2, screen_y + 1 or 2)
    MOV r18, r26
    LDI r21, 2
    ADD r18, r21          ; x = screen_x + 2 (middle col)
    MOV r21, r6
    LDI r17, 1
    AND r21, r17          ; fine_hash & 1
    JZ r21, pat_v_hi
    MOV r20, r25
    LDI r21, 1
    ADD r20, r21          ; y = screen_y + 1
    JMP pat_v_draw
pat_v_hi:
    MOV r20, r25
    LDI r21, 2
    ADD r20, r21          ; y = screen_y + 2
pat_v_draw:
    PSET r18, r20, r19
    JMP next_tile

pat_center:
    ; Center dot: pixel at (screen_x + 1 or 2, screen_y + 1 or 2)
    MOV r18, r26
    LDI r21, 1
    ADD r18, r21          ; x = screen_x + 1
    MOV r20, r25
    ADD r20, r21          ; y = screen_y + 1
    MOV r21, r6
    LDI r17, 1
    AND r21, r17          ; fine_hash & 1
    JZ r21, pat_c_draw
    ADD r18, r7           ; x = screen_x + 2
    ADD r20, r7           ; y = screen_y + 2
pat_c_draw:
    PSET r18, r20, r19
    JMP next_tile

pat_corner:
    ; Corner spark: pixel at (screen_x + 1 or 2, screen_y + 0 or 3)
    MOV r18, r26
    MOV r20, r25
    MOV r21, r6
    LDI r17, 1
    AND r21, r17          ; fine_hash & 1
    JZ r21, pat_cr_tl
    LDI r21, 2
    ADD r18, r21          ; x = screen_x + 2
    LDI r21, 3
    ADD r20, r21          ; y = screen_y + 3
    JMP pat_cr_draw
pat_cr_tl:
    LDI r21, 1
    ADD r18, r21          ; x = screen_x + 1
    ; y = screen_y + 0 (already set)
pat_cr_draw:
    PSET r18, r20, r19
    JMP next_tile

pat_diag_bl:
    ; Diagonal backslash: pixel at (1,1) or (2,2)
    MOV r18, r26
    MOV r20, r25
    LDI r21, 1
    ADD r18, r21          ; x = screen_x + 1
    ADD r20, r21          ; y = screen_y + 1
    MOV r21, r6
    LDI r17, 1
    AND r21, r17          ; fine_hash & 1
    JZ r21, pat_dbl_draw
    ADD r18, r7           ; x = screen_x + 2
    ADD r20, r7           ; y = screen_y + 2
pat_dbl_draw:
    PSET r18, r20, r19
    JMP next_tile

pat_diag_fw:
    ; Diagonal forward slash: pixel at (3,1) or (1,3)
    MOV r18, r26
    MOV r20, r25
    MOV r21, r6
    LDI r17, 1
    AND r21, r17          ; fine_hash & 1
    JZ r21, pat_dfw_lo
    LDI r21, 1
    ADD r18, r21          ; x = screen_x + 1
    LDI r21, 3
    ADD r20, r21          ; y = screen_y + 3
    JMP pat_dfw_draw
pat_dfw_lo:
    LDI r21, 3
    ADD r18, r21          ; x = screen_x + 3
    LDI r21, 1
    ADD r20, r21          ; y = screen_y + 1
pat_dfw_draw:
    PSET r18, r20, r19
    JMP next_tile

pat_topedge:
    ; Top edge: pixel at (1,0) or (2,0)
    MOV r18, r26
    MOV r20, r25          ; y = screen_y + 0
    LDI r21, 1
    ADD r18, r21          ; x = screen_x + 1
    MOV r21, r6
    LDI r17, 1
    AND r21, r17          ; fine_hash & 1
    JZ r21, pat_te_draw
    ADD r18, r7           ; x = screen_x + 2
pat_te_draw:
    PSET r18, r20, r19
    JMP next_tile

pat_dither:
    ; Dither scatter: pixel at one of 4 positions based on fine_hash bits 0-1
    MOV r18, r26
    MOV r20, r25
    MOV r21, r6
    LDI r17, 3
    AND r21, r17          ; fine_hash & 3 = position index (0-3)
    JZ r21, pat_di_a
    LDI r17, 1
    CMP r21, r17
    JZ r0, pat_di_b
    LDI r17, 2
    CMP r21, r17
    JZ r0, pat_di_c
    JMP pat_di_d
pat_di_a:
    LDI r21, 1
    ADD r18, r21          ; (1, 1)
    ADD r20, r21
    JMP pat_di_draw
pat_di_b:
    LDI r21, 3
    ADD r18, r21          ; (3, 1)
    LDI r21, 1
    ADD r20, r21
    JMP pat_di_draw
pat_di_c:
    LDI r21, 1
    ADD r18, r21          ; (1, 3)
    LDI r21, 3
    ADD r20, r21
    JMP pat_di_draw
pat_di_d:
    LDI r21, 3
    ADD r18, r21          ; (3, 3)
    ADD r20, r21
pat_di_draw:
    PSET r18, r20, r19
    JMP next_tile

    ; ---- Next tile ----
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

; ===== Player Cursor (crosshair at screen center) =====
; The camera IS the player position; the viewport is always centered on it.
; Draw a pulsing crosshair at pixel (127,127) -- center of the 256x256 screen.
; Four arms radiate from center with a 1px gap so terrain shows through.
; Color pulses between white and yellow based on frame_counter bit 4.
LOAD r17, r13           ; r17 = frame_counter
LDI r18, 16
AND r17, r18            ; frame_counter & 16 -> toggles every 16 frames
JZ r17, cursor_white
LDI r17, 0xFFFF00       ; yellow pulse
JMP cursor_arms
cursor_white:
LDI r17, 0xFFFFFF       ; white
cursor_arms:
LDI r18, 1              ; thin dimension (1px)
LDI r19, 3              ; arm length (3px)
; Top arm: (127, 124) 1x3
LDI r3, 127
LDI r4, 124
RECTF r3, r4, r18, r19, r17
; Bottom arm: (127, 128) 1x3
LDI r4, 128
RECTF r3, r4, r18, r19, r17
; Left arm: (124, 127) 3x1
LDI r3, 124
LDI r4, 127
RECTF r3, r4, r19, r18, r17
; Right arm: (128, 127) 3x1
LDI r3, 128
RECTF r3, r4, r19, r18, r17

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
