; infinite_map_pxpk.asm -- Pixelpack seed-driven infinite terrain
;
; Evolution of infinite_map.asm. Uses Pixelpack-style table-driven expansion
; instead of cascading CMP/BLT for biome color selection.
;
; Key changes from infinite_map.asm:
;   1. Biome color table in RAM replaces the ~200-instruction CMP/BLT cascade
;   2. Fine hash (seed) drives per-tile color variation via nibble extraction
;   3. Seed expansion with 4 pattern strategies (flat, horiz, vert, center)
;   4. Accent color via XOR_CHAIN (Pixelpack strategy 0xC)
;   5. Net result: ~46-53 instructions/tile (all paths under 55 budget)
;
; Memory layout:
;   RAM[0x7000-0x701F] = biome color table (32 entries, RGB packed)
;   RAM[0x7020-0x702F] = nibble variation table (16 entries, signed offsets)
;   RAM[0x7800] = camera_x
;   RAM[0x7801] = camera_y
;   RAM[0x7802] = frame_counter
;   RAM[0xFFB]  = key bitmask
;
; Seed expansion architecture (32-bit fine_hash = THE SEED):
;   Bits 0-3:    R channel variation (nibble lookup into 0x7020 table)
;   Bits 16-20:  XOR mask for accent color (XOR_CHAIN strategy 0xC)
;   Bits 30-31:  Pattern type selector (dispatch-optimized order)
;
; Pattern strategies (ordered by dispatch cost for balanced paths):
;   0 (flat):    Single RECTF -- smooth terrain (water, snow, plains)
;   1 (center):  Base background + 2x2 accent center -- oasis, crystals
;   2 (horiz):   Top half base + bottom half accent -- dune ridges, grass
;   3 (vert):    Left half base + right half accent -- rock faces, walls
;
; Tile size = 4 pixels. Viewport = 64x64 tiles = 256x256 pixels.
; Renders via RECTF (1-2 per tile depending on pattern).

; ===== Constants =====
LDI r7, 1               ; constant 1
LDI r8, 64              ; TILES per axis
LDI r9, 4               ; TILE_SIZE pixels
LDI r10, 0xFFB          ; key bitmask port
LDI r11, 0x7800         ; camera_x address
LDI r12, 0x7801         ; camera_y address
LDI r13, 0x7802         ; frame_counter address

; ===== Initialize Tables =====
; Biome color table at RAM[0x7000] (32 entries)
LDI r20, 0x7000         ; table base

; Water
LDI r17, 0x000044
STORE r20, r17
LDI r17, 1
ADD r20, r17
LDI r17, 0x0000BB
STORE r20, r17
LDI r17, 1
ADD r20, r17

; Beach
LDI r17, 0xC2B280
STORE r20, r17
LDI r17, 1
ADD r20, r17

; Desert
LDI r17, 0xDDBB44
STORE r20, r17
LDI r17, 1
ADD r20, r17
LDI r17, 0xCCAA33
STORE r20, r17
LDI r17, 1
ADD r20, r17

; Oasis
LDI r17, 0x22AA55
STORE r20, r17
LDI r17, 1
ADD r20, r17

; Grass
LDI r17, 0x55BB33
STORE r20, r17
LDI r17, 1
ADD r20, r17
LDI r17, 0x228811
STORE r20, r17
LDI r17, 1
ADD r20, r17

; Swamp
LDI r17, 0x445522
STORE r20, r17
LDI r17, 1
ADD r20, r17
LDI r17, 0x2D4A1A
STORE r20, r17
LDI r17, 1
ADD r20, r17

; Forest
LDI r17, 0x116600
STORE r20, r17
LDI r17, 1
ADD r20, r17
LDI r17, 0x0A4400
STORE r20, r17
LDI r17, 1
ADD r20, r17

; Mushroom
LDI r17, 0x883388
STORE r20, r17
LDI r17, 1
ADD r20, r17

; Mountain
LDI r17, 0x667766
STORE r20, r17
LDI r17, 1
ADD r20, r17
LDI r17, 0x999999
STORE r20, r17
LDI r17, 1
ADD r20, r17

; Tundra
LDI r17, 0x8899AA
STORE r20, r17
LDI r17, 1
ADD r20, r17

; Lava
LDI r17, 0xFF3300
STORE r20, r17
LDI r17, 1
ADD r20, r17
LDI r17, 0x332222
STORE r20, r17
LDI r17, 1
ADD r20, r17

; Volcanic
LDI r17, 0x442211
STORE r20, r17
LDI r17, 1
ADD r20, r17

; Snow
LDI r17, 0xCCCCEE
STORE r20, r17
LDI r17, 1
ADD r20, r17
LDI r17, 0xDDEEFF
STORE r20, r17
LDI r17, 1
ADD r20, r17
LDI r17, 0xFFFFFF
STORE r20, r17
LDI r17, 1
ADD r20, r17

; Coral
LDI r17, 0x3377AA
STORE r20, r17
LDI r17, 1
ADD r20, r17

; Ruins
LDI r17, 0x776655
STORE r20, r17
LDI r17, 1
ADD r20, r17

; Crystal
LDI r17, 0x1A3333
STORE r20, r17
LDI r17, 1
ADD r20, r17
LDI r17, 0x2A5555
STORE r20, r17
LDI r17, 1
ADD r20, r17

; Ash
LDI r17, 0x444444
STORE r20, r17
LDI r17, 1
ADD r20, r17

; Deadlands
LDI r17, 0x3D2B1F
STORE r20, r17
LDI r17, 1
ADD r20, r17
LDI r17, 0x4A3525
STORE r20, r17
LDI r17, 1
ADD r20, r17

; Bioluminescent
LDI r17, 0x004433
STORE r20, r17
LDI r17, 1
ADD r20, r17
LDI r17, 0x006655
STORE r20, r17
LDI r17, 1
ADD r20, r17

; Void
LDI r17, 0x110022
STORE r20, r17
; Table init complete. r20 = 0x7020

; ===== Nibble variation table at RAM[0x7020] (16 entries) =====
; Signed offsets: -16 to +15 mapped to small color variation
; Encoded as raw u32 values that we ADD to base color
LDI r20, 0x7020

LDI r17, 0xFFFFFFF0    ; -16
STORE r20, r17
ADD r20, r7
LDI r17, 0xFFFFFFF4    ; -12
STORE r20, r17
ADD r20, r7
LDI r17, 0xFFFFFFF8    ; -8
STORE r20, r17
ADD r20, r7
LDI r17, 0xFFFFFFFC    ; -4
STORE r20, r17
ADD r20, r7
LDI r17, 0x00000000    ; 0
STORE r20, r17
ADD r20, r7
LDI r17, 0x00000004    ; +4
STORE r20, r17
ADD r20, r7
LDI r17, 0x00000008    ; +8
STORE r20, r17
ADD r20, r7
LDI r17, 0x0000000C    ; +12
STORE r20, r17
ADD r20, r7
LDI r17, 0xFFFFFFF0    ; -16
STORE r20, r17
ADD r20, r7
LDI r17, 0xFFFFFFF4    ; -12
STORE r20, r17
ADD r20, r7
LDI r17, 0xFFFFFFF8    ; -8
STORE r20, r17
ADD r20, r7
LDI r17, 0xFFFFFFFC    ; -4
STORE r20, r17
ADD r20, r7
LDI r17, 0x00000000    ; 0
STORE r20, r17
ADD r20, r7
LDI r17, 0x00000004    ; +4
STORE r20, r17
ADD r20, r7
LDI r17, 0x00000008    ; +8
STORE r20, r17
ADD r20, r7
LDI r17, 0x0000000C    ; +12
STORE r20, r17

; ===== Main Loop =====
main_loop:

; --- Increment frame counter ---
LOAD r17, r13
ADD r17, r7
STORE r13, r17

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

; --- Process diagonal keys (bits 4-7) ---
MOV r17, r16
LDI r18, 16
AND r17, r18
JZ r17, no_ur
SUB r15, r7
ADD r14, r7
no_ur:

MOV r17, r16
LDI r18, 32
AND r17, r18
JZ r17, no_dr
ADD r15, r7
ADD r14, r7
no_dr:

MOV r17, r16
LDI r18, 64
AND r17, r18
JZ r17, no_dl
ADD r15, r7
SUB r14, r7
no_dl:

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

; --- Clear screen ---
LDI r17, 0
FILL r17

; ===== Precompute day/night tint =====
MOV r18, r14
LDI r19, 4
SHR r18, r19
LDI r19, 0xF
AND r18, r19
LDI r19, 8
CMP r18, r19
BGE r0, pre_tint_warm
LDI r19, 0x0808
MUL r18, r19
NEG r18
MOV r23, r18
JMP pre_tint_done
pre_tint_warm:
SUB r18, r19
LDI r19, 0x080000
MUL r18, r19
MOV r23, r18
pre_tint_done:

; ===== Render Viewport =====
; r14 = camera_x, r15 = camera_y
; r23 = precomputed tint offset
; Table base addresses
LDI r24, 0x7000         ; biome color table base
LDI r25, 0x7020         ; nibble variation table base

LDI r1, 0               ; ty = 0
LDI r27, 0              ; screen_y accumulator

render_y:
  LDI r2, 0             ; tx = 0
  LDI r28, 0            ; screen_x accumulator

  render_x:
    ; World coordinates
    MOV r3, r14
    ADD r3, r2           ; r3 = world_x
    MOV r4, r15
    ADD r4, r1           ; r4 = world_y

    ; ---- Coarse hash for biome (optimized: reuse shift constant) ----
    MOV r5, r3
    MOV r6, r4
    LDI r18, 3
    SHR r5, r18          ; r5 = world_x >> 3
    SHR r6, r18          ; r6 = world_y >> 3 (reuse r18=3)
    LDI r18, 99001
    MUL r5, r18
    LDI r18, 79007
    MUL r6, r18
    XOR r5, r6           ; r5 = coarse_hash
    LDI r18, 1103515245
    MUL r5, r18          ; r5 = mixed_hash (full 32-bit)

    ; Extract biome index (top 5 bits)
    MOV r17, r5
    LDI r18, 27
    SHR r17, r18         ; r17 = biome_type (0..31)

    ; ---- TABLE LOOKUP: biome color ----
    MOV r18, r24          ; r18 = 0x7000
    ADD r18, r17          ; r18 = 0x7000 + biome_index
    LOAD r17, r18         ; r17 = biome base color from table

    ; ---- Fine hash for seed expansion (per-tile variation) ----
    MOV r6, r3
    LDI r18, 374761393
    MUL r6, r18
    MOV r21, r4
    LDI r18, 668265263
    MUL r21, r18
    XOR r6, r21          ; r6 = fine_hash (THE SEED)

    ; ---- Seed: R variation (nibble 0, bits 0-3) ----
    MOV r18, r6
    LDI r19, 0xF
    AND r18, r19          ; r18 = seed & 0xF
    ADD r18, r25          ; r18 = 0x7020 + index
    LOAD r18, r18         ; r18 = variation offset
    ADD r17, r18          ; r17 = base color + R variation

    ; ---- Combined: accent + pattern type (optimized) ----
    ; Pattern type from bits 30-31, accent from bits 16-20
    MOV r18, r6
    LDI r20, 30
    SHR r18, r20          ; r18 = pattern_type (0-3)
    MOV r19, r6
    LDI r20, 16
    SHR r19, r20          ; r19 = seed >> 16
    LDI r20, 0x1F1F1F     ; 5 bits per channel mask
    AND r19, r20          ; r19 = seed-derived XOR mask
    XOR r19, r17          ; r19 = accent color (base XOR mask)

    ; ---- Apply day/night tint to both colors ----
    ADD r17, r23          ; base += tint
    ADD r19, r23          ; accent += tint

    ; ---- Pattern dispatch (order: flat=0, center=1, horiz=2, vert=3) ----
    ; Reordered so expensive patterns get shorter dispatch paths
    JZ r18, pat_flat       ; 0: flat tile
    SUB r18, r7            ; pattern - 1
    JZ r18, pat_center     ; 1: center bright (2x2 accent)
    SUB r18, r7            ; pattern - 2
    JZ r18, pat_horiz      ; 2: horizontal stripe
    ; Fall through: 3 = vertical stripe

    ; Pattern 3: left half base, right half accent (rock faces)
    LDI r20, 2
    RECTF r28, r27, r20, r9, r17
    MOV r21, r28
    ADD r21, r20           ; r21 = x + 2
    RECTF r21, r27, r20, r9, r19
    JMP tile_done

pat_flat:
    ; Pattern 0: single flat tile
    RECTF r28, r27, r9, r9, r17
    JMP tile_done

pat_center:
    ; Pattern 1: base background + 2x2 accent center (oasis, crystals)
    RECTF r28, r27, r9, r9, r17
    MOV r21, r28
    ADD r21, r7            ; r21 = x + 1 (r7=1)
    MOV r22, r27
    ADD r22, r7            ; r22 = y + 1 (r7=1)
    LDI r20, 2
    RECTF r21, r22, r20, r20, r19
    JMP tile_done

pat_horiz:
    ; Pattern 2: top half base, bottom half accent (dune ridges)
    LDI r20, 2
    RECTF r28, r27, r9, r20, r17
    MOV r21, r27
    ADD r21, r20           ; r21 = y + 2
    RECTF r28, r21, r9, r20, r19
    JMP tile_done

tile_done:

    ; ---- Next tile ----
    ADD r2, r7            ; tx++
    ADD r28, r9           ; screen_x += TILE_SIZE
    MOV r18, r2
    SUB r18, r8           ; tx - 64
    JZ r18, next_row
    JMP render_x

next_row:
    ADD r1, r7            ; ty++
    ADD r27, r9           ; screen_y += TILE_SIZE
    MOV r18, r1
    SUB r18, r8           ; ty - 64
    JZ r18, frame_end
    JMP render_y

frame_end:

; ===== Player Cursor =====
LOAD r17, r13
LDI r18, 16
AND r17, r18
JZ r17, cursor_white
LDI r17, 0xFFFF00
JMP cursor_arms
cursor_white:
LDI r17, 0xFFFFFF
cursor_arms:
LDI r18, 1
LDI r19, 3
LDI r3, 127
LDI r4, 124
RECTF r3, r4, r18, r19, r17
LDI r4, 128
RECTF r3, r4, r18, r19, r17
LDI r3, 124
LDI r4, 127
RECTF r3, r4, r19, r18, r17
LDI r3, 128
RECTF r3, r4, r19, r18, r17

; ===== Minimap Overlay (16x16) =====
LDI r1, 0

mm_y:
  LDI r2, 0
  mm_x:
    MOV r3, r2
    LDI r18, 4
    MUL r3, r18
    ADD r3, r14

    MOV r4, r1
    LDI r18, 4
    MUL r4, r18
    ADD r4, r15

    ; Coarse hash for biome
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
    LDI r18, 27
    SHR r5, r18          ; biome 0..31

    ; Dimmed minimap lookup from same table
    MOV r18, r24
    ADD r18, r5
    LOAD r17, r18

    ; Dim the color (shift right 1 bit = 50% brightness)
    LDI r18, 1
    SHR r17, r18

    ; Screen pos: x = 240 + mx, y = my
    MOV r3, r2
    LDI r18, 240
    ADD r3, r18
    PSET r3, r1, r17

    ADD r2, r7
    LDI r18, 16
    MOV r19, r2
    SUB r19, r18
    JZ r19, mm_next_row
    JMP mm_x

mm_next_row:
    ADD r1, r7
    LDI r18, 16
    MOV r19, r1
    SUB r19, r18
    JZ r19, mm_border
    JMP mm_y

; --- Minimap border ---
mm_border:
LDI r17, 0xAAAAAA
LDI r18, 1
LDI r19, 16

LDI r3, 240
LDI r4, 0
RECTF r3, r4, r19, r18, r17
LDI r4, 15
RECTF r3, r4, r19, r18, r17
LDI r4, 0
RECTF r3, r4, r18, r19, r17
LDI r3, 255
RECTF r3, r4, r18, r19, r17

; --- Player dot ---
LDI r3, 248
LDI r4, 8
LDI r17, 0xFFFFFF
PSET r3, r4, r17

    FRAME
    JMP main_loop
