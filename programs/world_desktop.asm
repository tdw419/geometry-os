; world_desktop.asm -- Infinite Map Desktop with Player Avatar + Buildings
;
; Based on infinite_map_pxpk.asm. Adds player-controlled avatar that walks
; across procedural terrain as the desktop surface. Camera follows player.
; Collision prevents walking into water, mountains, and lava.
; Buildings represent apps (games, utilities, creative, system) placed at
; deterministic positions on the map. Taskbar at bottom shows biome, app
; count, and clock. Minimap shows building markers.
; Command bar: press / to enter type mode. Type a command and press Enter.
;   /tp X Y        -- teleport player to coordinates
;   /build NAME    -- add building at current player position
;   plain text     -- send to Oracle LLM for a response
;   Escape         -- exit type mode without executing
; Oracle responses appear as an overlay at the top of the screen.
;
; Phase 84 additions:
;   - 8 app buildings at fixed world positions (RAM[0x7500-0x757F])
;   - Building names stored at RAM[0x7600-0x767F]
;   - Proximity detection: tooltip when player near building door
;   - Taskbar at y=240..255 (biome, apps, clock)
;   - Building markers on minimap
;   - Socket commands: buildings, desktop_json, launch
; Key changes from infinite_map.asm:
;   1. Biome color table in RAM replaces the ~200-instruction CMP/BLT cascade
;   2. Per-tile variation via MUL fine hash + nibble lookup
;   3. 4 pattern strategies from coarse hash: flat, center, horiz, vert
;   4. Accent color via XOR_CHAIN (Pixelpack strategy 0xC) from coarse hash
;   5. Day/night cycle: frame_counter-driven 4-phase tint (dawn/day/dusk/night)
;      Uses frac>>3 for safe packed-RGB addition without per-channel overflow
;   6. Net result: ~49-56 instructions/tile (flat=49, non-flat avg ~56)
;   7. Height-based shading from fine_hash top bits (0-7 * 0x030303 per tile)
;   8. Animated water shimmer: center pattern + frame_counter cycling accent
;   9. Coastline foam: water tiles adjacent to land get +0x303030 white blend
;  10. Biome boundary blending: smooth hash interpolation at biome edges.
;      X-direction: 4-tile graduated transition (positions 0,1,6,7).
;      Position 0: 50/50 blend LEFT. Position 1: 75/25 graduated blend LEFT.
;      Position 6: 75/25 graduated blend RIGHT. Position 7: 50/50 blend RIGHT.
;      Y-direction: 4-tile graduated transition (positions 0,1,6,7).
;      Position 0: 50/50 blend TOP. Position 1: 75/25 graduated blend TOP.
;      Position 6: 75/25 graduated blend BOTTOM. Position 7: 50/50 blend BOTTOM.
;      Blend mode stored in RAM[0x7803], x_hash cached in RAM[0x7805].
;      Neighbor y_hash cached in RAM[0x7804] for per-tile Y-blend.
;      Corner tiles get sequential X+Y blend (bilinear-like).
;  11. 32x32 minimap overlay: top-right corner (x=224..255, y=0..31).
;      Covers 64-tile viewport at half resolution (1 pixel = 2 tiles).
;      Pixel cache in RAM[0x7100..0x74FF]. Biome hashes recomputed every
;      4 frames; repaint from cache every frame. Dimmed 50% brightness.
;      Border (0xAAAAAA) and white center dot drawn every frame.
;  12. Water reflection: water tiles mirror the biome color of the tile above
;      (world_y-1) with 50% dim + blue tint (0x0E1C38). Tile-above-water
;      (biome 0/1) falls through to normal water rendering. Ripple animation
;      via (frame_counter + world_x*3 + world_y*7) & 0xF * 0x020202.
;  13. Procedural tree sprites on grass (biome 6-7) and forest (biome 10-11).
;      Deterministic placement via fine_hash bits: forest ~50%, grass ~25%.
;      Tree shape: 3x2 green canopy + 1x1 brown trunk (2 RECTF calls).
;      Skips water tiles. RAM[0x7806] = biome_type per tile.
;  14. Sky gradient: top 16 rows show a 4-band color gradient that shifts
;      with the day/night cycle. Dawn=blue-purple→orange, Day=blue→light-blue,
;      Dusk=dark-purple→deep-orange, Night=near-black→dark-navy.
;      4 bands of 4 rows each, blended via ADD+SHR packed-RGB operations.
;  15. Elevation contour lines: 1px dark lines (0x222222) at tile boundaries
;      where elevation (fine_hash top 3 bits, range 0-7) changes by > 2.
;      Checks right and bottom neighbors per tile. Skipped for water.
;      Creates a topographic map effect showing elevation ridges/valleys.
;
; Memory layout:
;   RAM[0x7000-0x701F] = biome color table (32 entries, RGB packed)
;   RAM[0x7020-0x702F] = nibble variation table (16 entries, signed offsets)
;   RAM[0x7800] = camera_x
;   RAM[0x7801] = camera_y
;   RAM[0x7802] = frame_counter
;   RAM[0x7803] = y_blend_mode (0=none, 1=top 50/50, 2=top 75/25, 3=bottom 75/25, 4=bottom 50/50)
;   RAM[0x7804] = y_neighbor_hash (precomputed per row for Y-blend)
;   RAM[0x7805] = saved x_hash (preserved across X-blend for Y-blend reuse)
;   RAM[0x7806] = biome_type (per-tile, for tree sprite check)
;   RAM[0x7807] = prev_tile_elevation (carry-forward for contour right-edge check)
;   RAM[0x7808] = player_x (world tile coords, initially 32)
;   RAM[0x7809] = player_y (world tile coords, initially 32)
;   RAM[0x780A] = player_facing (0=down, 1=up, 2=left, 3=right)
;   RAM[0x780B] = walk_frame (toggles 0/1 for walk animation)
;   RAM[0x7830] = CMD_MODE (0=move, 1=type). Press / to toggle.
;   RAM[0x7831] = CMD_LEN (current input length, 0-63)
;   RAM[0x7832..0x7871] = CMD_BUF (64 chars typed command)
;   RAM[0x7872] = ORACLE_RESP_READY (0=no response, 1=response waiting)
;   RAM[0x7873..0x7A72] = ORACLE_RESP_BUF (Oracle response text, max 895 chars)
;   RAM[0x7100-0x74FF] = 32x32 minimap pixel cache (1024 dimmed biome colors)
;   RAM[0xFFB]  = key bitmask
;
; Seed expansion architecture:
;   COARSE HASH (world_x>>3 * 99001 XOR world_y>>3 * 79007, LCG mixed):
;     Top 5 bits (>>27): biome index (table lookup into 0x7000-0x701F)
;     Bits 25-26 (&0x3): pattern type selector (4 strategies)
;     Bits 10-20 (&0x1F1F1F): XOR mask for accent color
;   FINE HASH (world_x * 374761393 XOR world_y * 668265263):
;     Nibble 0 (bits 0-3): R-channel variation index into nibble table
;
; Pattern strategies:
;   0 (flat):    Single RECTF -- smooth terrain (water, snow, plains)
;   1 (center):  Base background + 2x2 accent center -- oasis, crystals
;   2 (horiz):   Top half base + bottom half accent -- dune ridges, grass
;   3 (vert):    Left half base + right half accent -- rock faces, walls
;
; Tile size = 4 pixels. Viewport = 64x64 tiles = 256x256 pixels.
; Renders via RECTF (1-2 per tile depending on pattern).

; ===== Constants =====
LDI r7, 1               ; constant 1
LDI r10, 0xFFB          ; key bitmask port
LDI r11, 0x7800         ; camera_x address
LDI r12, 0x7801         ; camera_y address
LDI r13, 0x7802         ; frame_counter address

; ===== Dynamic tile size from zoom level =====
; RAM[0x7812]: 0=1px tiles (256x256), 1=2px (128x128), 2=4px (64x64 default),
;              3=8px (32x32), 4=16px (16x16)
LDI r17, 0x7812
LOAD r18, r17            ; r18 = zoom_level
JZ r18, zoom_0           ; zoom 0 = 1px tiles
LDI r17, 1
SUB r18, r17
JZ r18, zoom_1           ; zoom 1 = 2px tiles
LDI r17, 1
SUB r18, r17
JZ r18, zoom_default     ; zoom 2 = 4px (standard)
LDI r17, 1
SUB r18, r17
JZ r18, zoom_default     ; zoom 3 = still 4px (Rust crops for detail)
JMP zoom_default         ; zoom 4 = still 4px (Rust crops)

zoom_0:
LDI r8, 256              ; TILES per axis (zoomed out: see 256 tiles)
LDI r9, 1                ; TILE_SIZE = 1 pixel
LDI r30, 0               ; detail_level = 0 (minimal: flat tiles only)
JMP zoom_set

zoom_1:
LDI r8, 128              ; TILES per axis (medium: 128 tiles)
LDI r9, 2                ; TILE_SIZE = 2 pixels
LDI r30, 1               ; detail_level = 1 (reduced: patterns + shimmer, no trees/contours)
JMP zoom_set

zoom_default:
LDI r8, 64               ; TILES per axis (standard: 64 tiles)
LDI r9, 4                ; TILE_SIZE = 4 pixels
LDI r30, 2               ; detail_level = 2 (full detail)

zoom_set:
; Save detail level to RAM[0x7814] for use by feature renderers
LDI r17, 0x7814
STORE r17, r30

; If zoom was 0 (uninitialized RAM), set it to 2 (default) so Rust side is consistent
LDI r17, 0x7812
LOAD r18, r17
JNZ r18, zoom_no_init_fix
LDI r18, 2
STORE r17, r18
LDI r8, 64
LDI r9, 4
LDI r30, 2
LDI r17, 0x7814
STORE r17, r30
zoom_no_init_fix:

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

; ===== Initialize Stack Pointer =====
LDI r30, 0xFF00

; ===== Initialize Player Position =====
LDI r17, 0x7808

; ===== Initialize Building Table at RAM[0x7500] =====
; 8 buildings: [world_x, world_y, type_color, name_addr] per building
; Colors: red=0xFF4444(games), green=0x44FF44(creative), blue=0x4444FF(utility), yellow=0xFFFF44(system)
LDI r20, 0x7500

; Building 0: snake (game/red)
LDI r17, 52
STORE r20, r17
ADDI r20, 1
LDI r17, 48
STORE r20, r17
ADDI r20, 1
LDI r17, 0xFF4444
STORE r20, r17
ADDI r20, 1
LDI r17, 0x7600
STORE r20, r17
ADDI r20, 1

; Building 1: ball (game/red)
LDI r17, 78
STORE r20, r17
ADDI r20, 1
LDI r17, 85
STORE r20, r17
ADDI r20, 1
LDI r17, 0xFF4444
STORE r20, r17
ADDI r20, 1
LDI r17, 0x7610
STORE r20, r17
ADDI r20, 1

; Building 2: plasma (creative/green)
LDI r17, 110
STORE r20, r17
ADDI r20, 1
LDI r17, 55
STORE r20, r17
ADDI r20, 1
LDI r17, 0x44FF44
STORE r20, r17
ADDI r20, 1
LDI r17, 0x7620
STORE r20, r17
ADDI r20, 1

; Building 3: painter (creative/green)
LDI r17, 35
STORE r20, r17
ADDI r20, 1
LDI r17, 95
STORE r20, r17
ADDI r20, 1
LDI r17, 0x44FF44
STORE r20, r17
ADDI r20, 1
LDI r17, 0x7630
STORE r20, r17
ADDI r20, 1

; Building 4: colors (utility/blue)
LDI r17, 140
STORE r20, r17
ADDI r20, 1
LDI r17, 40
STORE r20, r17
ADDI r20, 1
LDI r17, 0x4444FF
STORE r20, r17
ADDI r20, 1
LDI r17, 0x7640
STORE r20, r17
ADDI r20, 1

; Building 5: fire (utility/blue)
LDI r17, 160
STORE r20, r17
ADDI r20, 1
LDI r17, 120
STORE r20, r17
ADDI r20, 1
LDI r17, 0x4444FF
STORE r20, r17
ADDI r20, 1
LDI r17, 0x7650
STORE r20, r17
ADDI r20, 1

; Building 6: init (system/yellow)
LDI r17, 25
STORE r20, r17
ADDI r20, 1
LDI r17, 140
STORE r20, r17
ADDI r20, 1
LDI r17, 0xFFFF44
STORE r20, r17
ADDI r20, 1
LDI r17, 0x7660
STORE r20, r17
ADDI r20, 1

; Building 7: shell (system/yellow)
LDI r17, 180
STORE r20, r17
ADDI r20, 1
LDI r17, 75
STORE r20, r17
ADDI r20, 1
LDI r17, 0xFFFF44
STORE r20, r17
ADDI r20, 1
LDI r17, 0x7670
STORE r20, r17
ADDI r20, 1

; Building 8: linux hypervisor (gold/0xFFD700)
LDI r17, 70
STORE r20, r17
ADDI r20, 1
LDI r17, 130
STORE r20, r17
ADDI r20, 1
LDI r17, 0xFFD700
STORE r20, r17
ADDI r20, 1
LDI r17, 0x7680
STORE r20, r17
ADDI r20, 1

; Building 9: tetris (game/magenta)
LDI r17, 200
STORE r20, r17
ADDI r20, 1
LDI r17, 60
STORE r20, r17
ADDI r20, 1
LDI r17, 0xFF00FF
STORE r20, r17
ADDI r20, 1
LDI r17, 0x7690
STORE r20, r17
ADDI r20, 1

; Building 10: smart_term (AI/cyan)
LDI r17, 100
STORE r20, r17
ADDI r20, 1
LDI r17, 150
STORE r20, r17
ADDI r20, 1
LDI r17, 0x00FFFF
STORE r20, r17
ADDI r20, 1
LDI r17, 0x76A0
STORE r20, r17
ADDI r20, 1

; Building 11: oracle (AI/purple 0xBB44FF)
LDI r17, 130
STORE r20, r17
ADDI r20, 1
LDI r17, 90
STORE r20, r17
ADDI r20, 1
LDI r17, 0xBB44FF
STORE r20, r17
ADDI r20, 1
LDI r17, 0x76B0
STORE r20, r17
ADDI r20, 1

; Building 12: ai_terminal (AI/cyan 0x00FFFF)
LDI r17, 160
STORE r20, r17
ADDI r20, 1
LDI r17, 90
STORE r20, r17
ADDI r20, 1
LDI r17, 0x00FFFF
STORE r20, r17
ADDI r20, 1
LDI r17, 0x76C0
STORE r20, r17
ADDI r20, 1

; Building 13: file_browser (utility/cyan 0x44CCFF)
LDI r17, 95
STORE r20, r17
ADDI r20, 1
LDI r17, 160
STORE r20, r17
ADDI r20, 1
LDI r17, 0x44CCFF
STORE r20, r17
ADDI r20, 1
LDI r17, 0x76D0
STORE r20, r17
ADDI r20, 1

; Building 14: hex_viewer (utility/green 0x44DD88)
LDI r17, 120
STORE r20, r17
ADDI r20, 1
LDI r17, 165
STORE r20, r17
ADDI r20, 1
LDI r17, 0x44DD88
STORE r20, r17
ADDI r20, 1
LDI r17, 0x76E0
STORE r20, r17
ADDI r20, 1

; Building 15: host_term (terminal/green-white 0xAAFFCC)
LDI r17, 140
STORE r20, r17
ADDI r20, 1
LDI r17, 80
STORE r20, r17
ADDI r20, 1
LDI r17, 0xAAFFCC
STORE r20, r17
ADDI r20, 1
LDI r17, 0x76F0
STORE r20, r17
ADDI r20, 1

; Building count
LDI r17, 0x7580
LDI r18, 16
STORE r17, r18

; ===== Claim Table =====
; Count at RAM[0x78BF], entries at RAM[0x78C0..0x78DF] (up to 8 entries).
LDI r20, 0x78BF
LDI r17, 1
STORE r20, r17               ; claim count = 1

LDI r20, 0x78C0
LDI r17, 40                  ; x1
STORE r20, r17
ADDI r20, 1
LDI r17, 28                  ; y1
STORE r20, r17
ADDI r20, 1
LDI r17, 48                  ; x2 (exclusive)
STORE r20, r17
ADDI r20, 1
LDI r17, 36                  ; y2 (exclusive)
STORE r20, r17
ADDI r20, 1

; ===== Building Name Strings at RAM[0x7600-0x768F] =====
LDI r20, 0x7600
STRO r20, "snake"
LDI r20, 0x7610
STRO r20, "ball"
LDI r20, 0x7620
STRO r20, "plasma"
LDI r20, 0x7630
STRO r20, "painter"
LDI r20, 0x7640
STRO r20, "colors"
LDI r20, 0x7650
STRO r20, "fire"
LDI r20, 0x7660
STRO r20, "init"
LDI r20, 0x7670
STRO r20, "shell"
LDI r20, 0x7680
STRO r20, "linux"
LDI r20, 0x7690
STRO r20, "tetris"
LDI r20, 0x76A0
STRO r20, "smart_term"
LDI r20, 0x76B0
STRO r20, "oracle"
LDI r20, 0x76C0
STRO r20, "ai_terminal"
LDI r20, 0x76D0
STRO r20, "file_browser"
LDI r20, 0x76E0
STRO r20, "hex_viewer"
LDI r20, 0x76F0
STRO r20, "host"

; Clear nearby building flag
LDI r17, 0x7584
LDI r18, 0
STORE r17, r18          ; nearby_bldg_idx = -1 (none)
LDI r17, 0x7588
STORE r17, r18          ; nearby_flag = 0

LDI r17, 0x7808
LDI r18, 32
STORE r17, r18          ; player_x = 32
LDI r17, 0x7809
LDI r18, 32
STORE r17, r18          ; player_y = 32
LDI r17, 0x780A
LDI r18, 0
STORE r17, r18          ; facing = down
LDI r17, 0x780B
STORE r17, r18          ; walk_frame = 0

; ===== Init Command Mode =====
LDI r17, 0x7830
STORE r17, r18          ; CMD_MODE = 0 (move mode)
LDI r17, 0x7831
STORE r17, r18          ; CMD_LEN = 0
LDI r17, 0x7872
STORE r17, r18          ; ORACLE_RESP_READY = 0


; ===== Entity Table at RAM[0x7900] =====
; Living map entities: diverse types with distinct behaviors.
; Layout: RAM[0x7900]=count, entries at 0x7901+ (5 words each):
;   [world_x, world_y, type, dir_seed, anim_frame]
;   type: 0=program-node (pulsing display, stationary)
;         1=wanderer (random walk, orange)
;         2=guard (patrols near buildings, approaches player, red)
;         3=animal (flees from player, green)
;         4=ghost (passes through all terrain, purple)
;         5=area_agent (autonomous explorer, seeks buildings, teal)
;   dir_seed: packed (direction<<16 | seed), direction 0-3
;   anim_frame: animation counter, incremented each update tick

LDI r20, 0x7900
LDI r17, 8
STORE r20, r17               ; entity_count = 8

; Entity 0: program-node at (42, 35)
ADDI r20, 1
LDI r17, 42
STORE r20, r17               ; world_x
ADDI r20, 1
LDI r17, 35
STORE r20, r17               ; world_y
ADDI r20, 1
LDI r17, 0
STORE r20, r17               ; type = program-node
ADDI r20, 1
LDI r17, 0
STORE r20, r17               ; dir_seed = 0
ADDI r20, 1
STORE r20, r17               ; anim_frame = 0

; Entity 1: program-node at (88, 60)
ADDI r20, 1
LDI r17, 88
STORE r20, r17
ADDI r20, 1
LDI r17, 60
STORE r20, r17
ADDI r20, 1
LDI r17, 0
STORE r20, r17               ; type = program-node
ADDI r20, 1
LDI r17, 0
STORE r20, r17
ADDI r20, 1
STORE r20, r17

; Entity 2: wanderer at (55, 45)
ADDI r20, 1
LDI r17, 55
STORE r20, r17
ADDI r20, 1
LDI r17, 45
STORE r20, r17
ADDI r20, 1
LDI r17, 1
STORE r20, r17               ; type = wanderer
ADDI r20, 1
LDI r17, 0x00020000
STORE r20, r17               ; dir_seed = dir=2, seed=0
ADDI r20, 1
LDI r17, 0
STORE r20, r17

; Entity 3: wanderer at (70, 30)
ADDI r20, 1
LDI r17, 70
STORE r20, r17
ADDI r20, 1
LDI r17, 30
STORE r20, r17
ADDI r20, 1
LDI r17, 1
STORE r20, r17               ; type = wanderer
ADDI r20, 1
LDI r17, 0x00010000
STORE r20, r17               ; dir_seed = dir=1, seed=0
ADDI r20, 1
LDI r17, 0
STORE r20, r17

; Entity 4: guard at (38, 38) - patrols near spawn area
ADDI r20, 1
LDI r17, 38
STORE r20, r17
ADDI r20, 1
LDI r17, 38
STORE r20, r17
ADDI r20, 1
LDI r17, 2
STORE r20, r17               ; type = guard
ADDI r20, 1
LDI r17, 0x00010000
STORE r20, r17               ; dir_seed = dir=1, seed=0
ADDI r20, 1
LDI r17, 0
STORE r20, r17

; Entity 5: animal at (50, 50) - flees from player
ADDI r20, 1
LDI r17, 50
STORE r20, r17
ADDI r20, 1
LDI r17, 50
STORE r20, r17
ADDI r20, 1
LDI r17, 3
STORE r20, r17               ; type = animal
ADDI r20, 1
LDI r17, 0x00000000
STORE r20, r17               ; dir_seed = dir=0, seed=0
ADDI r20, 1
LDI r17, 0
STORE r20, r17

; Entity 6: ghost at (60, 40) - passes through walls
ADDI r20, 1
LDI r17, 60
STORE r20, r17
ADDI r20, 1
LDI r17, 40
STORE r20, r17
ADDI r20, 1
LDI r17, 4
STORE r20, r17               ; type = ghost
ADDI r20, 1
LDI r17, 0x00020000
STORE r20, r17               ; dir_seed = dir=2, seed=0
ADDI r20, 1
LDI r17, 0
STORE r20, r17

; Entity 7: area_agent at (45, 42) - autonomous building explorer
ADDI r20, 1
LDI r17, 45
STORE r20, r17
ADDI r20, 1
LDI r17, 42
STORE r20, r17
ADDI r20, 1
LDI r17, 5
STORE r20, r17               ; type = area_agent
ADDI r20, 1
LDI r17, 0x00030000
STORE r20, r17               ; dir_seed = dir=3, seed=0
ADDI r20, 1
LDI r17, 0
STORE r20, r17

; Entity 8: area_agent at (60, 55) - autonomous building explorer
ADDI r20, 1
LDI r17, 60
STORE r20, r17
ADDI r20, 1
LDI r17, 55
STORE r20, r17
ADDI r20, 1
LDI r17, 5
STORE r20, r17               ; type = area_agent
ADDI r20, 1
LDI r17, 0x00010000
STORE r20, r17               ; dir_seed = dir=1, seed=0
ADDI r20, 1
LDI r17, 0
STORE r20, r17

; ===== Agent Task Table at RAM[0x7950-0x796F] =====
; Per-agent (16 words each, 2 agents = 32 words from 0x7950):
;   [target_bldg_idx, status, task_type, result, home_x, home_y,
;    step_counter, visit_count, msg_out, msg_in, reserved...]
; status: 0=idle, 1=moving_to_bldg, 2=entering, 3=inside_bldg, 4=exiting, 5=returning
; task_type: 0=patrol, 1=visit_bldg, 2=scan_area, 3=report
LDI r20, 0x7950
LDI r17, 0xFFFFFFFF
STORE r20, r17               ; agent0 target_bldg = -1 (none)
ADDI r20, 1
LDI r17, 0
STORE r20, r17               ; agent0 status = idle
ADDI r20, 1
LDI r17, 0
STORE r20, r17               ; agent0 task_type = patrol
ADDI r20, 1
LDI r17, 0
STORE r20, r17               ; agent0 result = 0
ADDI r20, 1
LDI r17, 45
STORE r20, r17               ; agent0 home_x
ADDI r20, 1
LDI r17, 42
STORE r20, r17               ; agent0 home_y
ADDI r20, 1
LDI r17, 0
STORE r20, r17               ; agent0 step_counter
ADDI r20, 1
LDI r17, 0
STORE r20, r17               ; agent0 visit_count
ADDI r20, 1
LDI r17, 0
STORE r20, r17               ; agent0 msg_out
ADDI r20, 1
LDI r17, 0
STORE r20, r17               ; agent0 msg_in

LDI r20, 0x7960
LDI r17, 0xFFFFFFFF
STORE r20, r17               ; agent1 target_bldg = -1
ADDI r20, 1
LDI r17, 0
STORE r20, r17               ; agent1 status = idle
ADDI r20, 1
LDI r17, 0
STORE r20, r17               ; agent1 task_type = patrol
ADDI r20, 1
LDI r17, 0
STORE r20, r17               ; agent1 result
ADDI r20, 1
LDI r17, 60
STORE r20, r17               ; agent1 home_x
ADDI r20, 1
LDI r17, 55
STORE r20, r17               ; agent1 home_y
ADDI r20, 1
LDI r17, 0
STORE r20, r17               ; agent1 step_counter
ADDI r20, 1
LDI r17, 0
STORE r20, r17               ; agent1 visit_count
ADDI r20, 1
LDI r17, 0
STORE r20, r17               ; agent1 msg_out
ADDI r20, 1
LDI r17, 0
STORE r20, r17               ; agent1 msg_in

; ===== Agent Communication Mailbox at RAM[0x7970-0x797F] =====
; Shared RAM for inter-agent and agent-player communication.
; Layout: [msg_count, msg0_sender, msg0_type, msg0_data,
;          msg1_sender, msg1_type, msg1_data, ...]
; msg_type: 1=arrival, 2=report, 3=alert, 4=request
LDI r20, 0x7970
LDI r17, 0
STORE r20, r17               ; mailbox msg_count = 0

; RAM[0x7930] = entity_nearby_idx (-1 = none, 0..7 = entity player is touching)
LDI r20, 0x7930
LDI r17, 0xFFFFFFFF
STORE r20, r17

; ===== Main Loop =====
main_loop:

; --- Increment frame counter ---
LOAD r17, r13
ADD r17, r7
STORE r13, r17

; --- Read player position ---
LDI r18, 0x7808
LOAD r14, r18           ; r14 = player_x
LDI r18, 0x7809
LOAD r15, r18           ; r15 = player_y

; --- Read key bitmask ---
LOAD r16, r10           ; r16 = key bitmask

; --- Reset stack pointer (render loop may trash r30) ---
LDI r30, 0xFF00

; ===== Command Mode Key Handling =====
; IKEY reads one char per frame from the key ring buffer.
; In MOVE mode: check for '/' to toggle to TYPE mode.
; In TYPE mode: capture keystrokes into CMD_BUF.
LDI r17, 0x7830
LOAD r17, r17           ; r17 = CMD_MODE
JZ r17, check_slash_toggle   ; CMD_MODE == 0 → check for / toggle

; --- TYPE MODE: capture keystroke ---
IKEY r17
JZ r17, cmd_type_no_key      ; no key pressed

; Escape (27) exits type mode
CMPI r17, 27
JNZ r0, cmd_not_escape
LDI r17, 0x7830
LDI r18, 0
STORE r17, r18               ; CMD_MODE = 0
LDI r17, 0x7831
STORE r17, r18               ; CMD_LEN = 0
JMP cmd_type_no_key

cmd_not_escape:
; Enter (13) executes command
CMPI r17, 13
JNZ r0, cmd_not_enter
CALL cmd_execute
LDI r17, 0x7830
LDI r18, 0
STORE r17, r18               ; CMD_MODE = 0 (back to move mode)
LDI r17, 0x7831
STORE r17, r18               ; CMD_LEN = 0
JMP cmd_type_no_key

cmd_not_enter:
; Backspace (8 or 127)
CMPI r17, 8
JZ r0, cmd_do_bs
CMPI r17, 127
JNZ r0, cmd_do_char
cmd_do_bs:
LDI r17, 0x7831
LOAD r18, r17                ; r18 = CMD_LEN
JZ r18, cmd_type_no_key      ; already empty
SUBI r18, 1
STORE r17, r18               ; CMD_LEN--
JMP cmd_type_no_key

cmd_do_char:
; Append char to CMD_BUF if room. IKEY char is in r17.
MOV r27, r17                  ; save char in r27 (not used by main loop yet)
LDI r17, 0x7831
LOAD r18, r17                 ; r18 = CMD_LEN
CMPI r18, 63
BGE r0, cmd_type_no_key       ; buffer full
LDI r19, 0x7832
ADD r19, r18                  ; r19 = CMD_BUF + CMD_LEN
STORE r19, r27                ; CMD_BUF[len] = char
ADDI r18, 1
STORE r17, r18                ; CMD_LEN = len + 1
JMP cmd_type_no_key

check_slash_toggle:
; MOVE MODE: check if '/' pressed to enter type mode
IKEY r17
JZ r17, cmd_move_no_key
CMPI r17, 47                 ; '/'
JNZ r0, cmd_move_no_key
LDI r17, 0x7830
LDI r18, 1
STORE r17, r18               ; CMD_MODE = 1
LDI r17, 0x7831
LDI r18, 0
STORE r17, r18               ; CMD_LEN = 0
cmd_move_no_key:

; If in TYPE mode, skip player movement
cmd_type_no_key:
LDI r17, 0x7830
LOAD r17, r17
JNZ r17, player_move_done    ; type mode → no movement

; --- Player movement (every frame) ---

; --- Process Up (bit 0) - try move up ---
MOV r17, r16
LDI r18, 1
AND r17, r18
JZ r17, no_up
LDI r17, 0x780A
LDI r18, 1
STORE r17, r18          ; facing = up
MOV r17, r15
SUB r17, r7             ; target_y = player_y - 1
LDI r18, 0x7809
STORE r18, r17          ; player_y = target_y (move will be reverted if blocked)
; Check biome at (player_x, target_y)
MOV r3, r14             ; world_x = player_x
MOV r4, r17             ; world_y = target_y
CALL check_biome_walkable
JZ r0, up_blocked       ; r0 == 0 means not walkable
JMP no_up
up_blocked:
LDI r18, 0x7809
STORE r18, r15          ; revert player_y to original
no_up:

; --- Process Down (bit 1) - try move down ---
MOV r17, r16
LDI r18, 2
AND r17, r18
JZ r17, no_down
LDI r17, 0x780A
LDI r18, 0
STORE r17, r18          ; facing = down
MOV r17, r15
ADD r17, r7             ; target_y = player_y + 1
LDI r18, 0x7809
STORE r18, r17
MOV r3, r14
MOV r4, r17
CALL check_biome_walkable
JZ r0, down_blocked
JMP no_down
down_blocked:
LDI r18, 0x7809
MOV r17, r15
STORE r18, r17          ; revert
no_down:

; --- Process Left (bit 2) - try move left ---
MOV r17, r16
LDI r18, 4
AND r17, r18
JZ r17, no_left
LDI r17, 0x780A
LDI r18, 2
STORE r17, r18          ; facing = left
MOV r17, r14
SUB r17, r7             ; target_x = player_x - 1
LDI r18, 0x7808
STORE r18, r17
MOV r3, r17             ; world_x = target_x
MOV r4, r15             ; world_y = player_y
CALL check_biome_walkable
JZ r0, left_blocked
JMP no_left
left_blocked:
LDI r18, 0x7808
STORE r18, r14          ; revert player_x to original
no_left:

; --- Process Right (bit 3) - try move right ---
MOV r17, r16
LDI r18, 8
AND r17, r18
JZ r17, no_right
LDI r17, 0x780A
LDI r18, 3
STORE r17, r18          ; facing = right
MOV r17, r14
ADD r17, r7             ; target_x = player_x + 1
LDI r18, 0x7808
STORE r18, r17
MOV r3, r17
MOV r4, r15
CALL check_biome_walkable
JZ r0, right_blocked
JMP no_right
right_blocked:
LDI r18, 0x7808
MOV r17, r14
STORE r18, r17          ; revert
no_right:

; Diagonal keys not used for player movement (prevents diagonal clipping)

player_move_done:

; --- Toggle walk frame ---
LDI r17, 0x780B
LOAD r18, r17
XORI r18, 1
STORE r17, r18

; --- Re-read player position (may have moved) ---
LDI r18, 0x7808
LOAD r14, r18           ; r14 = player_x
LDI r18, 0x7809
LOAD r15, r18           ; r15 = player_y


; ===== Update Entities (throttled to ~7.5 updates/sec) =====
; Agent-nodes wander via random walk. Program-nodes just animate.
PUSH r31

; Throttle: only update entities every 8th frame
LOAD r17, r13              ; frame_counter
LDI r18, 7
AND r17, r18
JNZ r17, ent_update_skip

; Iterate entities
LDI r20, 0x7900
LOAD r21, r20              ; r21 = entity_count
ADDI r20, 1                ; point to first entity
LDI r26, 0                 ; entity index

ent_update_loop:
  CMP r26, r21
  BGE r0, ent_update_done

  ; Load entity fields
  MOV r22, r20
  LOAD r3, r22             ; world_x (r3 for biome check)
  ADDI r22, 1
  LOAD r4, r22             ; world_y (r4 for biome check)
  ADDI r22, 1
  LOAD r17, r22            ; type
  ADDI r22, 1
  LOAD r18, r22            ; dir_seed
  ADDI r22, 1
  LOAD r19, r22            ; anim_frame

  ; Increment animation frame for all entities
  ADDI r19, 1
  LDI r25, 15
  AND r19, r25             ; wrap anim_frame to 0..15
  STORE r22, r19

  ; Type check: program-nodes are stationary
  JNZ r17, ent_not_program
  JMP ent_next              ; type 0 = program-node, skip all AI
ent_not_program:

  ; --- Entity AI Dispatch ---
  ; type 1=wander (random walk), 2=guard (approach player), 3=animal (flee), 4=ghost (walk through walls)
  ; type 5=area_agent (autonomous explorer, seeks buildings)
  LDI r22, 1
  CMP r17, r22
  JZ r0, ent_ai_wander     ; wanderer
  LDI r22, 2
  CMP r17, r22
  JZ r0, ent_ai_guard      ; guard
  LDI r22, 3
  CMP r17, r22
  JZ r0, ent_ai_animal     ; animal (flee)
  LDI r22, 4
  CMP r17, r22
  JZ r0, ent_ai_ghost      ; ghost
  LDI r22, 5
  CMP r17, r22
  JZ r0, ent_ai_area_agent ; area_agent
  JMP ent_next              ; unknown type, skip

  ; ===== Wanderer AI (type 1): random walk =====
ent_ai_wander:
  ; Random direction change ~25%
  RAND r22
  LDI r23, 3
  AND r22, r23
  JNZ r22, ent_wander_no_change
  RAND r25
  LDI r23, 3
  AND r25, r23
ent_wander_no_change:
  JMP ent_compute_target

  ; ===== Guard AI (type 2): patrol, approach player when close =====
ent_ai_guard:
  ; Simple proximity check: if within 8 tiles on x-axis, approach
  LDI r22, 0x7808
  LOAD r22, r22             ; player_x
  MOV r25, r3
  SUB r25, r22              ; dx = entity_x - player_x
  ; Quick check: if |dx| < 8, approach. Use SAR sign check.
  LDI r23, 31
  MOV r24, r25
  SAR r24, r23              ; sign bit
  ; If sign=0, dx >= 0 (entity right of player), else dx < 0
  JNZ r24, ent_guard_player_left
  ; dx >= 0: check if close enough (dx < 8)
  LDI r22, 8
  CMP r25, r22
  BGE r0, ent_guard_patrol
  ; Close enough: move left toward player
  LDI r25, 2                ; direction = left
  JMP ent_compute_target
ent_guard_player_left:
  ; dx < 0: entity left of player. Check if close: -dx < 8
  NEG r25                   ; r25 = |dx|
  LDI r22, 8
  CMP r25, r22
  BGE r0, ent_guard_patrol
  ; Close: move right toward player
  LDI r25, 3                ; direction = right
  JMP ent_compute_target

ent_guard_patrol:
  ; Patrol: mostly keep direction, occasional turn
  RAND r22
  LDI r23, 7
  AND r22, r23
  JNZ r22, ent_guard_no_turn
  RAND r25
  LDI r23, 3
  AND r25, r23
ent_guard_no_turn:
  JMP ent_compute_target

  ; ===== Animal AI (type 3): flee from player =====
ent_ai_animal:
  ; Simple flee: check if player within 6 tiles on x-axis
  LDI r22, 0x7808
  LOAD r22, r22             ; player_x
  MOV r25, r3
  SUB r25, r22              ; dx = entity_x - player_x
  LDI r23, 31
  MOV r24, r25
  SAR r24, r23              ; sign bit
  JNZ r24, ent_animal_player_left
  ; dx >= 0: entity right of player. Check distance
  LDI r22, 6
  CMP r25, r22
  BGE r0, ent_animal_calm
  ; Close: flee right
  LDI r25, 3                ; direction = right
  JMP ent_compute_target_no_walkability
ent_animal_player_left:
  ; dx < 0: entity left of player. Check |dx|
  NEG r25
  LDI r22, 6
  CMP r25, r22
  BGE r0, ent_animal_calm
  ; Close: flee left
  LDI r25, 2                ; direction = left
  JMP ent_compute_target_no_walkability

ent_animal_calm:
  ; Calm: random walk
  RAND r25
  LDI r23, 3
  AND r25, r23
  JMP ent_compute_target

  ; ===== Ghost AI (type 4): random walk through all terrain =====
ent_ai_ghost:
  ; Ghost: always random direction, ignores terrain
  RAND r25
  LDI r23, 3
  AND r25, r23              ; random direction 0-3
  JMP ent_compute_target_no_walkability

  ; ===== Area Agent AI (type 5): autonomous building explorer =====
  ; State machine: idle -> pick target building -> move toward it -> enter -> explore -> exit -> return
  ; Uses agent task table at 0x7950+ (agent0) or 0x7960+ (agent1) based on entity index.
  ; Temp storage: RAM[0x797E] = agent_table_base for this agent
ent_ai_area_agent:
  ; Save entity loop registers (r20=entity ptr, r26=entity index)
  LDI r22, 0x793E
  STORE r22, r20             ; save entity pointer
  ADDI r22, 1
  STORE r22, r26             ; save entity index

  ; Compute agent_table_idx: entity_index - 7, clamped 0-1
  LDI r22, 7
  CMP r26, r22
  BGE r0, ent_agent_use_idx1
  LDI r17, 0                ; agent0
  JMP ent_agent_have_idx
ent_agent_use_idx1:
  LDI r17, 1                ; agent1
ent_agent_have_idx:
  ; Compute agent table base: 0x7950 + (agent_idx * 16)
  LDI r22, 16
  MUL r17, r22
  LDI r22, 0x7950
  ADD r22, r17              ; r22 = agent table base
  ; Save to temp RAM
  LDI r23, 0x797E
  STORE r23, r22

  ; Load agent status (offset 1 from base)
  ADDI r22, 1
  LOAD r17, r22             ; r17 = agent status

  ; State dispatch
  JNZ r17, ent_agent_not_idle
  JMP ent_agent_state_idle
ent_agent_not_idle:
  LDI r22, 1
  CMP r17, r22
  JZ r0, ent_agent_state_moving
  LDI r22, 3
  CMP r17, r22
  JZ r0, ent_agent_state_inside
  LDI r22, 5
  CMP r17, r22
  JZ r0, ent_agent_state_returning
  JMP ent_agent_state_idle   ; fallback to idle

  ; --- State: idle (0) - pick a target building ---
ent_agent_state_idle:
  LDI r23, 0x797E
  LOAD r22, r23             ; reload agent table base
  ; Pick a random building index (0..14)
  RAND r17
  LDI r23, 15
  AND r17, r23              ; building index 0-14
  ; Store target_bldg_idx (offset 0)
  LDI r23, 0x797E
  LOAD r22, r23
  STORE r22, r17
  ; Set status = moving_to_bldg (1)
  ADDI r22, 1
  LDI r23, 1
  STORE r22, r23
  ; Reset step counter (offset 6)
  ADDI r22, 5
  LDI r23, 0
  STORE r22, r23
  ; Fall through to moving state

  ; --- State: moving_to_bldg (1) - greedy path to target building ---
ent_agent_state_moving:
  ; Reload agent table base
  LDI r23, 0x797E
  LOAD r22, r23
  ; Load target building index (offset 0)
  LOAD r17, r22             ; target_bldg_idx
  ; Load target building coords: base = 0x7500 + bldg_idx * 4
  LDI r22, 4
  MUL r17, r22
  LDI r22, 0x7500
  ADD r22, r17              ; r22 = &buildings[bldg_idx]
  LOAD r23, r22             ; target_x = building world_x
  ADDI r22, 1
  LOAD r24, r22             ; target_y = building world_y
  ; Compute dx = target_x - entity_x, dy = target_y - entity_y
  MOV r22, r23
  SUB r22, r3               ; dx = target_x - entity_x
  MOV r17, r24
  SUB r17, r4               ; dy = target_y - entity_y
  ; Save target coords to temp RAM for later use
  LDI r25, 0x797C
  STORE r25, r23            ; save target_x
  ADDI r25, 1
  STORE r25, r24            ; save target_y

  ; Check if arrived: |dx| <= 1 AND |dy| <= 1
  ; Check |dx|
  LDI r25, 31
  MOV r23, r22
  SAR r23, r25              ; sign of dx
  JNZ r23, ent_agent_dx_neg
  ; dx >= 0
  LDI r23, 2
  CMP r22, r23
  BLT r0, ent_agent_check_dy  ; dx < 2, check dy
  JMP ent_agent_move_dx       ; dx >= 2, move
ent_agent_dx_neg:
  NEG r22                   ; |dx|
  LDI r23, 2
  CMP r22, r23
  BLT r0, ent_agent_check_dy  ; |dx| < 2
  JMP ent_agent_move_dx
ent_agent_check_dy:
  LDI r25, 31
  MOV r23, r17
  SAR r23, r25
  JNZ r23, ent_agent_dy_neg
  LDI r23, 2
  CMP r17, r23
  BLT r0, ent_agent_arrived   ; dy < 2, arrived!
  JMP ent_agent_move_dy
ent_agent_dy_neg:
  NEG r17
  LDI r23, 2
  CMP r17, r23
  BLT r0, ent_agent_arrived   ; |dy| < 2, arrived!
  JMP ent_agent_move_dy

ent_agent_arrived:
  ; Agent reached the building! Enter it.
  LDI r22, 0x797E
  LOAD r22, r22             ; reload agent table base
  ; Set status = inside_bldg (3)
  ADDI r22, 1
  LDI r17, 3
  STORE r22, r17
  ; Reset step counter for inside duration (offset 6)
  ADDI r22, 5
  LDI r17, 0
  STORE r22, r17
  ; Increment visit_count (offset 7)
  ADDI r22, 1
  LOAD r17, r22
  ADDI r17, 1
  STORE r22, r17
  ; Send arrival message to mailbox at 0x7970
  LDI r22, 0x7970
  LOAD r17, r22             ; msg_count
  LDI r23, 4
  CMP r17, r23
  BGE r0, ent_agent_arrived_done  ; mailbox full
  ADDI r17, 1
  STORE r22, r17            ; increment msg_count
  ; Compute message slot: 0x7970 + 1 + (msg_count-1) * 3 = 0x7971 + (msg_count-1)*3
  SUBI r17, 1               ; 0-based index
  LDI r23, 3
  MUL r17, r23
  LDI r23, 0x7971
  ADD r23, r17              ; msg slot base
  ; sender = entity_index (r26)
  STORE r23, r26
  ADDI r23, 1
  LDI r17, 1                ; type = arrival
  STORE r23, r17
  ADDI r23, 1
  ; data = target_bldg_idx
  LDI r22, 0x797E
  LOAD r22, r22
  LOAD r17, r22             ; target_bldg_idx
  STORE r23, r17
ent_agent_arrived_done:
  JMP ent_agent_restore_and_next

  ; --- Greedy movement toward target building ---
ent_agent_move_dx:
  ; Need sign of original dx (before NEG). Reload target_x and recompute.
  LDI r22, 0x797C
  LOAD r22, r22             ; target_x
  SUB r22, r3               ; dx = target_x - entity_x
  LDI r23, 31
  MOV r25, r22
  SAR r25, r23              ; sign of dx
  JNZ r25, ent_agent_dx_left
  LDI r25, 3                ; dx > 0: move right
  JMP ent_agent_restore_and_compute
ent_agent_dx_left:
  LDI r25, 2                ; dx < 0: move left
  JMP ent_agent_restore_and_compute

ent_agent_move_dy:
  ; Need sign of original dy
  LDI r22, 0x797D
  LOAD r22, r22             ; target_y
  SUB r22, r4               ; dy = target_y - entity_y
  LDI r23, 31
  MOV r25, r22
  SAR r25, r23
  JNZ r25, ent_agent_dy_up
  LDI r25, 1                ; dy > 0: move down
  JMP ent_agent_restore_and_compute
ent_agent_dy_up:
  LDI r25, 0                ; dy < 0: move up
  JMP ent_agent_restore_and_compute

  ; --- State: inside_bldg (3) - wait some frames then exit ---
ent_agent_state_inside:
  LDI r22, 0x797E
  LOAD r22, r22             ; reload agent table base
  ; Increment step counter (offset 6)
  ADDI r22, 6
  LOAD r17, r22
  ADDI r17, 1
  STORE r22, r17
  ; Stay inside for 20 ticks
  LDI r23, 20
  CMP r17, r23
  BLT r0, ent_agent_stay_inside
  ; Time to leave -- set status = returning (5)
  LDI r22, 0x797E
  LOAD r22, r22
  ADDI r22, 1               ; status field
  LDI r17, 5
  STORE r22, r17
  ; Reset step counter
  ADDI r22, 5
  LDI r17, 0
  STORE r22, r17
ent_agent_stay_inside:
  JMP ent_agent_restore_and_next

  ; --- State: returning (5) - move back toward home ---
ent_agent_state_returning:
  LDI r22, 0x797E
  LOAD r22, r22             ; reload agent table base
  ; Load home coords (offsets 4, 5)
  ADDI r22, 4
  LOAD r23, r22             ; home_x
  ADDI r22, 1
  LOAD r24, r22             ; home_y
  ; Save to temp RAM
  LDI r25, 0x797C
  STORE r25, r23            ; save home_x as target_x
  ADDI r25, 1
  STORE r25, r24            ; save home_y as target_y
  ; Compute distance to home
  MOV r22, r23
  SUB r22, r3               ; dx = home_x - entity_x
  MOV r17, r24
  SUB r17, r4               ; dy = home_y - entity_y

  ; Check if home: |dx| <= 1 AND |dy| <= 1
  LDI r25, 31
  MOV r24, r22
  SAR r24, r25
  JNZ r24, ent_agent_home_dx_neg
  LDI r24, 2
  CMP r22, r24
  BLT r0, ent_agent_home_check_dy
  JMP ent_agent_home_move_dx
ent_agent_home_dx_neg:
  NEG r22
  LDI r24, 2
  CMP r22, r24
  BLT r0, ent_agent_home_check_dy
  JMP ent_agent_home_move_dx

ent_agent_home_check_dy:
  LDI r25, 31
  MOV r24, r17
  SAR r24, r25
  JNZ r24, ent_agent_home_dy_neg
  LDI r24, 2
  CMP r17, r24
  BLT r0, ent_agent_home_arrived
  JMP ent_agent_home_move_dy
ent_agent_home_dy_neg:
  NEG r17
  LDI r24, 2
  CMP r17, r24
  BLT r0, ent_agent_home_arrived
  JMP ent_agent_home_move_dy

ent_agent_home_arrived:
  ; Back home, set status = idle (0)
  LDI r22, 0x797E
  LOAD r22, r22
  ADDI r22, 1
  LDI r17, 0
  STORE r22, r17
  ; Clear target
  LDI r22, 0x797E
  LOAD r22, r22
  LDI r17, 0xFFFFFFFF
  STORE r22, r17
  JMP ent_agent_restore_and_next

ent_agent_home_move_dx:
  ; Reuse ent_agent_move_dx logic (target is in 0x797C already)
  LDI r22, 0x797C
  LOAD r22, r22             ; target_x (home_x)
  SUB r22, r3               ; dx
  LDI r23, 31
  MOV r25, r22
  SAR r25, r23
  JNZ r25, ent_agent_home_dx_left
  LDI r25, 3
  JMP ent_agent_restore_and_compute
ent_agent_home_dx_left:
  LDI r25, 2
  JMP ent_agent_restore_and_compute

ent_agent_home_move_dy:
  LDI r22, 0x797D
  LOAD r22, r22             ; target_y (home_y)
  SUB r22, r4               ; dy
  LDI r23, 31
  MOV r25, r22
  SAR r25, r23
  JNZ r25, ent_agent_home_dy_up
  LDI r25, 1
  JMP ent_agent_restore_and_compute
ent_agent_home_dy_up:
  LDI r25, 0
  JMP ent_agent_restore_and_compute

  ; ===== Compute target position from direction =====
ent_agent_restore_and_compute:
  LDI r22, 0x793E
  LOAD r20, r22
  ADDI r22, 1
  LOAD r26, r22

ent_compute_target:
  ; dir 0=up(y-1), 1=down(y+1), 2=left(x-1), 3=right(x+1)
  MOV r22, r3              ; target_x = world_x
  MOV r23, r4              ; target_y = world_y
  JNZ r25, ent_not_up
  SUB r23, r7              ; target_y -= 1
  JMP ent_do_biome_check
ent_not_up:
  LDI r24, 1
  CMP r25, r24
  JNZ r0, ent_not_down
  ADD r23, r7              ; target_y += 1
  JMP ent_do_biome_check
ent_not_down:
  LDI r24, 2
  CMP r25, r24
  JNZ r0, ent_not_left
  SUB r22, r7              ; target_x -= 1
  JMP ent_do_biome_check
ent_not_left:
  ADD r22, r7              ; target_x += 1 (right)
ent_do_biome_check:

  ; Check biome walkability (all types except ghost)
  MOV r3, r22              ; target coords for biome check
  MOV r4, r23
  CALL check_biome_walkable
  JZ r0, ent_move_blocked  ; not walkable, skip move

  ; Update entity position
  MOV r24, r20
  STORE r24, r22           ; world_x = target_x
  ADDI r24, 1
  STORE r24, r23           ; world_y = target_y

  ; Update dir_seed with new direction
  ADDI r24, 2              ; skip to dir_seed field
  LDI r22, 16
  SHL r25, r22             ; direction << 16
  STORE r24, r25
  JMP ent_agent_restore_and_next

ent_compute_target_no_walkability:
  ; Same as above but skip biome check (ghost/animal flee)
  MOV r22, r3              ; target_x = world_x
  MOV r23, r4              ; target_y = world_y
  JNZ r25, ent_nt_up2
  SUB r23, r7
  JMP ent_do_move_no_check
ent_nt_up2:
  LDI r24, 1
  CMP r25, r24
  JNZ r0, ent_nt_down2
  ADD r23, r7
  JMP ent_do_move_no_check
ent_nt_down2:
  LDI r24, 2
  CMP r25, r24
  JNZ r0, ent_nt_left2
  SUB r22, r7
  JMP ent_do_move_no_check
ent_nt_left2:
  ADD r22, r7
ent_do_move_no_check:
  MOV r24, r20
  STORE r24, r22           ; world_x = target_x
  ADDI r24, 1
  STORE r24, r23           ; world_y = target_y
  ADDI r24, 2              ; skip to dir_seed field
  LDI r22, 16
  SHL r25, r22
  STORE r24, r25
  JMP ent_agent_restore_and_next

ent_move_blocked:
  ; Try a different direction next time
  RAND r22
  LDI r23, 3
  AND r22, r23
  ADDI r24, 2              ; dir_seed field
  LDI r23, 16
  SHL r22, r23
  STORE r24, r22

ent_agent_restore_and_next:
  LDI r22, 0x793E
  LOAD r20, r22
  ADDI r22, 1
  LOAD r26, r22

ent_next:
  ; Advance to next entity (5 words per entity)
  LDI r22, 5
  ADD r20, r22
  ADDI r26, 1
  JMP ent_update_loop

ent_update_done:
ent_update_skip:

; ===== Check Entity Proximity (player touching entity?) =====
LDI r20, 0x7930
LDI r17, 0xFFFFFFFF
STORE r20, r17             ; entity_nearby_idx = -1

LDI r20, 0x7900
LOAD r21, r20
ADDI r20, 1
LDI r26, 0

ent_prox_loop:
  CMP r26, r21
  BGE r0, ent_prox_done

  ; Load entity position
  MOV r22, r20
  LOAD r17, r22            ; entity world_x
  ADDI r22, 1
  LOAD r18, r22            ; entity world_y

  ; Compare with player position
  LDI r19, 0x7808
  LOAD r19, r19            ; player_x
  CMP r17, r19
  JNZ r0, ent_prox_next
  LDI r19, 0x7809
  LOAD r19, r19            ; player_y
  CMP r18, r19
  JNZ r0, ent_prox_next

  ; Player is on this entity!
  LDI r22, 0x7930
  STORE r22, r26           ; entity_nearby_idx = entity_index
  JMP ent_prox_done

ent_prox_next:
  LDI r22, 5
  ADD r20, r22
  ADDI r26, 1
  JMP ent_prox_loop

ent_prox_done:
POP r31

; --- Compute camera from player (center player in viewport) ---
; camera_x = player_x - (tiles/2), camera_y = player_y - (tiles/2)
; r8 = tiles per axis, so center offset = r8 / 2
MOV r17, r8
LDI r18, 1
SHR r17, r18              ; r17 = tiles/2 (center offset)
MOV r18, r14
SUB r18, r17
STORE r11, r18            ; camera_x = player_x - tiles/2
MOV r18, r15
SUB r18, r17
STORE r12, r18            ; camera_y = player_y - tiles/2

; --- Clear screen ---
LDI r17, 0
FILL r17

; ===== Precompute day/night tint (cyclic, frame_counter-driven) =====
; Cycle period = 256 frames (~4.3s at 60fps). 4 phases of 64 frames each.
; Uses frac>>3 (0..7) for safe packed-RGB addition (no per-channel overflow).
; Safety: max biome+BPE channel = 233; tint adds at most 21 → 254 < 256.
;   Phase 0 (dawn):  frac_shr * 0x030100 → R+21, G+7 (warm orange)
;   Phase 1 (day):   (63-frac)>>3 * 0x030100 → fade out dawn warmth
;   Phase 2 (dusk):  frac_shr * 0x030000 → R+21 (amber glow)
;   Phase 3 (night): frac_shr * 0x000103 → G+7, B+21 (cool blue shift)
; r23 = tint offset added to every tile base color inline.
LOAD r17, r13           ; r17 = frame_counter
LDI r18, 0xFF
AND r17, r18            ; t = frame & 0xFF (0..255)
MOV r18, r17
LDI r19, 6
SHR r18, r19            ; phase = t >> 6 (0..3)
LDI r19, 0x3F
AND r17, r19            ; frac = t & 0x3F (0..63)
LDI r19, 3
SHR r17, r19            ; frac_shr = frac >> 3 (0..7)

; Dispatch on phase (0=dawn, 1=day, 2=dusk, 3=night)
JZ r18, tint_dawn
LDI r19, 1
SUB r18, r19
JZ r18, tint_day
LDI r19, 1
SUB r18, r19
JZ r18, tint_dusk

tint_night:
  LDI r18, 0x000103
  MUL r17, r18
  MOV r23, r17
  JMP tint_done

tint_dawn:
  LDI r18, 0x030100
  MUL r17, r18
  MOV r23, r17
  JMP tint_done

tint_day:
  LDI r18, 63
  SUB r18, r17           ; 63 - frac (full frac, not shifted)
  LDI r19, 3
  SHR r18, r19           ; (63-frac)>>3 = fade-out frac_shr
  LDI r19, 0x030100
  MUL r18, r19
  MOV r23, r18
  JMP tint_done

tint_dusk:
  LDI r18, 0x030000
  MUL r17, r18
  MOV r23, r17
  JMP tint_done

tint_done:

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

  ; Reset contour carry-forward elevation for start of row
  LDI r18, 0x7807
  LDI r19, 0
  STORE r18, r19            ; RAM[0x7807] = 0 (no left neighbor at row start)

  ; Precompute y-part of blend neighbor hash (shared across row)
  MOV r26, r15
  ADD r26, r1            ; r26 = world_y for this row
  LDI r18, 3
  SHR r26, r18           ; world_y >> 3
  LDI r18, 79007
  MUL r26, r18           ; r26 = (world_y >> 3) * 79007 (blend y_hash, reused per tile)

  ; Precompute Y-blend mode per row (stored in RAM[0x7803])
  ; Mode: 0=none, 1=top 50/50 (pos 0), 2=top 75/25 (pos 1),
  ;        3=bottom 75/25 (pos 6), 4=bottom 50/50 (pos 7)
  ; RAM[0x7804] = precomputed neighbor y_hash for Y-blend
  MOV r18, r15
  ADD r18, r1              ; r18 = world_y
  ANDI r18, 7              ; r18 = local_y (0..7)
  LDI r20, 0               ; default blend mode = 0 (no blend)
  LDI r16, 0x7803          ; blend mode address
  STORE r16, r20           ; store 0 (no blend)
  JNZ r18, ypre_chk1
  ; local_y == 0: 50/50 blend with TOP neighbor (world_y - 8)
  LDI r20, 1
  STORE r16, r20
  LDI r20, 0xFFFFFFF8      ; -8 offset
  JMP ypre_hash
ypre_chk1:
  LDI r20, 1
  SUB r18, r20
  JNZ r18, ypre_chk6
  ; local_y == 1: 75/25 graduated blend with TOP neighbor (world_y - 8)
  LDI r20, 2
  STORE r16, r20
  LDI r20, 0xFFFFFFF8      ; -8 offset
  JMP ypre_hash
ypre_chk6:
  LDI r20, 5
  SUB r18, r20
  JNZ r18, ypre_chk7
  ; local_y == 6: 75/25 graduated blend with BOTTOM neighbor (world_y + 8)
  LDI r20, 3
  STORE r16, r20
  LDI r20, 8               ; +8 offset
  JMP ypre_hash
ypre_chk7:
  LDI r20, 1
  SUB r18, r20
  JNZ r18, ypre_done       ; local_y 2-5: no blend
  ; local_y == 7: 50/50 blend with BOTTOM neighbor
  LDI r20, 4
  STORE r16, r20
  LDI r20, 8               ; +8 offset
ypre_hash:
  MOV r22, r15
  ADD r22, r1
  ADD r22, r20             ; neighbor_y = world_y + offset
  LDI r20, 3
  SHR r22, r20
  LDI r20, 79007
  MUL r22, r20
  LDI r20, 0x7804
  STORE r20, r22
ypre_done:

  ; Load Y-blend mode into r16 (free during render -- key bitmask already processed)
  LDI r18, 0x7803
  LOAD r16, r18           ; r16 = y_blend_mode for entire row

  render_x:
    ; World coordinates
    MOV r3, r14
    ADD r3, r2           ; r3 = world_x
    MOV r4, r15
    ADD r4, r1           ; r4 = world_y

    ; ---- Claim dispatch ----
    ; Check if tile is inside a claimed region. Claimed tiles skip the
    ; default biome pipeline and render with a competing hash function.
    LDI r18, 0x78BF
    LOAD r18, r18        ; r18 = claim count
    JZ r18, no_claim
    LDI r21, 0x78C0      ; r21 = first entry base
claim_loop:
    LOAD r22, r21        ; x1
    CMP r3, r22
    BLT r0, claim_miss
    ADDI r21, 2
    LOAD r22, r21        ; x2 (exclusive)
    CMP r3, r22
    BGE r0, claim_x_pass
    JMP claim_miss
claim_x_pass:
    SUBI r21, 1
    LOAD r22, r21        ; y1
    CMP r4, r22
    BLT r0, claim_miss
    ADDI r21, 2
    LOAD r22, r21        ; y2 (exclusive)
    CMP r4, r22
    BGE r0, claim_hit
    JMP claim_miss
claim_hit:
    ; Tile claimed -- render with competing checkerboard hash
    MOV r22, r3
    ADD r22, r4
    ANDI r22, 1
    JZ r22, claim_teal
    LDI r23, 0xFF00FF     ; magenta
    JMP claim_draw
claim_teal:
    LDI r23, 0x00FFAA     ; neon teal
claim_draw:
    RECTF r28, r27, r9, r9, r23
    JMP claim_next_tile
claim_miss:
    ; Restore r21 to entry start and advance by 4 for next iteration
    LDI r21, 0x78C0
    LDI r22, 4
    SUBI r18, 1
    MUL r22, r18         ; r22 = 4 * (remaining entries - 1)
    ADD r21, r22
    JNZ r18, claim_loop
no_claim:

    ; ---- Coarse hash for biome ----
    MOV r5, r3
    MOV r6, r4
    LDI r18, 3
    SHR r5, r18          ; r5 = world_x >> 3
    SHR r6, r18          ; r6 = world_y >> 3
    LDI r18, 99001
    MUL r5, r18          ; r5 = x_hash
    LDI r18, 79007
    MUL r6, r18          ; r6 = y_hash
    XOR r5, r6           ; r5 = coarse_hash
    LDI r18, 1103515245
    MUL r5, r18          ; r5 = mixed_hash

    ; ---- Extract biome (top 5 bits) + pattern (bits 25-26) ----
    MOV r17, r5
    LDI r18, 27
    SHR r17, r18         ; r17 = biome_type (0..31)
    MOV r29, r5
    LDI r18, 25
    SHR r29, r18
    ANDI r29, 3           ; r29 = pattern_type (0-3) -- saved from clobber
    MOV r30, r17          ; save biome_type for water/height checks
    LDI r18, 0x7806
    STORE r18, r30         ; save biome_type to RAM for tree sprite check

    ; ---- TABLE LOOKUP: biome color ----
    MOV r20, r24
    ADD r20, r17          ; r20 = 0x7000 + biome_index
    LOAD r17, r20         ; r17 = biome base color

    ; ---- Biome boundary blending (graduated hash interpolation) ----
    ; Smooth gradient transitions at biome boundaries. Uses position-aware
    ; blend weights: outer edges (0,7) get 50/50 blend, inner edges (1,6)
    ; get 75/25 graduated blend. Both X and Y use 4-tile transition zones.
    ; Y-blend mode precomputed per row in RAM[0x7803], neighbor y_hash in
    ; RAM[0x7804]. Corner tiles get sequential X+Y blend (bilinear-like).

    ; -- X-direction blend (4-tile graduated transition zone) --
    ; y_hash precomputed in r26 (shared across row)
    ; Skip at detail 0 (1px tiles -- blend too expensive, not visible)
    LDI r18, 0x7814
    LOAD r18, r18
    JZ r18, no_xblend
    LDI r19, 0               ; blend mode: 0=50/50, 1=75/25
    MOV r18, r3
    ANDI r18, 7              ; r18 = local_x (position within 8-tile biome)
    JNZ r18, xblend_chk_1
    ; local_x == 0: 50/50 blend with LEFT neighbor (world_x - 8)
    MOV r21, r3
    LDI r18, 8
    SUB r21, r18
    JMP xblend_hash
xblend_chk_1:
    LDI r21, 1
    SUB r18, r21             ; r18 = local_x - 1
    JNZ r18, xblend_chk_6
    ; local_x == 1: 75/25 graduated blend LEFT
    LDI r19, 1               ; graduated mode
    MOV r21, r3
    LDI r18, 8
    SUB r21, r18
    JMP xblend_hash
xblend_chk_6:
    LDI r21, 5
    SUB r18, r21             ; r18 = local_x - 6
    JNZ r18, xblend_chk_7
    ; local_x == 6: 75/25 graduated blend RIGHT
    LDI r19, 1               ; graduated mode
    MOV r21, r3
    LDI r18, 8
    ADD r21, r18
    JMP xblend_hash
xblend_chk_7:
    LDI r21, 1
    SUB r18, r21             ; r18 = local_x - 7
    JNZ r18, no_xblend       ; not at X edge, skip
    ; local_x == 7: 50/50 blend with RIGHT neighbor (world_x + 8)
    MOV r21, r3
    LDI r18, 8
    ADD r21, r18
xblend_hash:
    ; Compute neighbor biome via coarse hash (y_hash from r26)
    LDI r18, 3
    SHR r21, r18             ; neighbor_x >> 3
    LDI r18, 99001
    MUL r21, r18
    XOR r21, r26             ; XOR with precomputed y_hash
    LDI r18, 1103515245
    MUL r21, r18             ; neighbor mixed hash
    LDI r18, 27
    SHR r21, r18             ; neighbor biome index (0..31)
    ; Lookup neighbor biome base color
    MOV r22, r24
    ADD r22, r21
    LOAD r22, r22            ; r22 = neighbor biome base color
    ; Apply blend based on mode (r19: 0=50/50, 1=75/25)
    JZ r19, xblend_50
    ; 75/25 graduated blend: base*3/4 + neighbor*1/4
    ; = (base>>1) + (base>>2) + (neighbor>>2), all masked for packed RGB
    MOV r20, r17             ; save original base in r20
    ANDI r17, 0xFEFEFE
    LDI r18, 1
    SHR r17, r18             ; r17 = base >> 1 (half, 50%)
    ANDI r20, 0xFCFCFC
    LDI r18, 2
    SHR r20, r18             ; r20 = base >> 2 (quarter, 25%)
    ADD r17, r20             ; r17 = base*3/4
    ANDI r22, 0xFCFCFC
    SHR r22, r18             ; r22 = neighbor >> 2 (quarter, 25%)
    ADD r17, r22             ; r17 = base*3/4 + neighbor*1/4
    JMP no_xblend
xblend_50:
    ; 50/50 masked average blend
    ANDI r17, 0xFEFEFE
    LDI r18, 1
    SHR r17, r18             ; base >> 1
    ANDI r22, 0xFEFEFE
    SHR r22, r18             ; neighbor >> 1
    ADD r17, r22             ; r17 = X-blended base color
no_xblend:

    ; -- Y-direction blend (4-tile graduated transition zone) --
    ; Uses precomputed blend mode in r16 (loaded once per row).
    ; Mode: 0=none, 1=50/50 top, 2=75/25 top, 3=75/25 bottom, 4=50/50 bottom.
    ; Hash uses re-derived x_hash XOR precomputed neighbor_y_hash.
    JZ r16, no_yblend       ; mode 0 = no blend (register check, no RAM load needed)
    ; Skip Y-blend at detail 0 (1px tiles)
    LDI r18, 0x7814
    LOAD r18, r18
    JZ r18, no_yblend
    ; Load precomputed neighbor y_hash
    LDI r26, 0x7804
    LOAD r26, r26           ; r26 = neighbor_y_hash
    ; Compute neighbor biome: re-derive x_hash XOR neighbor_y_hash → LCG → biome index
    ; (x_hash clobbered by X-blend, so re-derive from world_x)
    MOV r22, r3
    LDI r18, 3
    SHR r22, r18             ; r22 = world_x >> 3
    LDI r18, 99001
    MUL r22, r18             ; r22 = x_hash (re-derived)
    XOR r22, r26             ; neighbor coarse hash
    LDI r26, 1103515245
    MUL r22, r26            ; neighbor mixed hash
    LDI r26, 27
    SHR r22, r26            ; neighbor biome index (0..31)
    ; Lookup neighbor biome base color
    MOV r26, r24
    ADD r26, r22
    LOAD r26, r26           ; r26 = neighbor biome base color
    ; Dispatch on blend mode: bit 1 selects 50/50 (0) vs 75/25 (1)
    ANDI r16, 2
    JNZ r16, yblend_75
    ; 50/50 blend (modes 1 and 4)
    ANDI r17, 0xFEFEFE
    LDI r18, 1
    SHR r17, r18            ; current >> 1
    ANDI r26, 0xFEFEFE
    SHR r26, r18            ; neighbor >> 1
    ADD r17, r26            ; 50/50 blend
    JMP no_yblend
yblend_75:
    ; 75/25 graduated blend (modes 2 and 3)
    MOV r22, r17            ; save base
    ANDI r17, 0xFEFEFE
    LDI r18, 1
    SHR r17, r18            ; base >> 1 (50%)
    ANDI r22, 0xFCFCFC
    LDI r18, 2
    SHR r22, r18            ; base >> 2 (25%)
    ADD r17, r22            ; base*3/4
    ANDI r26, 0xFCFCFC
    SHR r26, r18            ; neighbor >> 2 (25%)
    ADD r17, r26            ; base*3/4 + neighbor*1/4
no_yblend:

    ; ---- Fine hash: MUL-based per-tile seeding (Pixelpack strategy) ----
    ; r6 = world_x * 374761393 XOR world_y * 668265263
    ; This gives good avalanche -- adjacent tiles get very different seeds
    MOV r6, r3
    LDI r18, 374761393
    MUL r6, r18
    MOV r21, r4
    LDI r18, 668265263
    MUL r21, r18
    XOR r6, r21           ; r6 = fine_hash (THE SEED, 32 bits of goodness)

    ; ---- Single water check (biome 0 or 1) ----
    ; Sets r31=1 for water, r31=0 for land. Used by height skip and shimmer.
    MOV r31, r30           ; biome_type
    JZ r31, is_water       ; biome 0 = water
    LDI r18, 1
    SUB r31, r18
    JZ r31, is_water       ; biome 1 = water
    LDI r31, 0             ; not water
    JMP water_checked
is_water:
    LDI r31, 1             ; is water
water_checked:

    ; ---- Water reflection (mirror tile above with blue tint + ripple) ----
    ; For water tiles: compute biome color of tile at (world_x, world_y-1).
    ; Dim 50% + blue tint (0x0E1C38) + frame-driven ripple.
    ; Optimization: biome blocks are 8 tiles tall (world_y >> 3). Within a block,
    ; the above tile shares the same biome = water → no reflection needed.
    ; Only compute at biome boundaries (world_y & 7 == 0).
    ; If tile above is also water (biome 0/1), skip (use normal water rendering).
    JZ r31, no_reflect       ; not water, skip
    ; Skip reflection at detail 0 (1px tiles -- not visible)
    LDI r18, 0x7814
    LOAD r18, r18
    JZ r18, no_reflect
    MOV r18, r4
    ANDI r18, 7              ; world_y & 7
    JNZ r18, no_reflect      ; same biome block → above is same water biome
    ; At biome boundary: compute above-tile biome via hash(world_x, world_y-1)
    MOV r18, r3
    LDI r19, 3
    SHR r18, r19             ; world_x >> 3
    LDI r19, 99001
    MUL r18, r19             ; x_hash
    MOV r19, r4
    SUB r19, r7              ; world_y - 1
    LDI r20, 3
    SHR r19, r20             ; (world_y-1) >> 3
    LDI r20, 79007
    MUL r19, r20             ; y_hash for above tile
    XOR r18, r19             ; above coarse hash
    LDI r19, 1103515245
    MUL r18, r19             ; above mixed hash
    LDI r19, 27
    SHR r18, r19             ; above biome index (0..31)
    ; Check if above tile is water (biome 0 or 1) → skip reflection
    JZ r18, no_reflect       ; above biome 0 = water
    LDI r19, 1
    SUB r18, r19
    JZ r18, no_reflect       ; above biome 1 = water
    ; Above is land: lookup its biome color
    MOV r19, r24
    ADD r19, r18             ; 0x7000 + biome_index
    LOAD r19, r19            ; r19 = above tile biome base color
    ; Dim reflected color by 50% + blue tint
    ANDI r19, 0xFEFEFE
    LDI r18, 1
    SHR r19, r18             ; r19 = reflected_color / 2
    LDI r18, 0x0E1C38
    ADD r19, r18             ; reflected/2 + blue_tint
    ; Ripple: (frame_counter + world_x) & 0xF * 0x020202 for wave motion
    LOAD r20, r13            ; frame_counter
    ADD r20, r3              ; fc + world_x (cheap position variation)
    ANDI r20, 0xF            ; 0-15 ripple phase
    LDI r21, 0x020202
    MUL r20, r21             ; ripple brightness (0x00..0x1E1E1E)
    ADD r19, r20             ; reflected color += ripple
    MOV r17, r19             ; replace base_color with reflected color
no_reflect:

    ; ---- Height-based shading (skip for water) ----
    ; Elevation from fine_hash top bits: range 0-7, shade 0x030303 per step
    ; Applied before R-variation and tint. Max +21/channel, safe for Snow biome.
    JZ r31, height_apply
    JMP height_skip        ; water = flat, no height shading
height_apply:
    MOV r18, r6            ; fine_hash
    LDI r30, 28
    SHR r18, r30           ; top 4 bits (0-15)
    ANDI r18, 0x7          ; clamp to 0-7
    LDI r30, 0x030303
    MUL r18, r30           ; height_shade = 0..0x151515
    ADD r17, r18           ; base_color += height_shade
height_skip:

    ; ---- R-channel variation: nibble 0 of fine_hash ----
    MOV r18, r6
    ANDI r18, 0xF          ; r18 = seed & 0xF (nibble 0: R variation index)
    ADD r18, r25           ; r18 = 0x7020 + index
    LOAD r18, r18          ; r18 = variation offset
    ADD r17, r18           ; r17 += R variation

    ; ---- Apply day/night tint to base, then derive accent ----
    ADD r17, r23          ; base += tint
    ; Accent: XOR tinted base with coarse_hash mask (XOR_CHAIN strategy)
    MOV r19, r5
    LDI r18, 10
    SHR r19, r18
    ANDI r19, 0x1F1F1F     ; 5 bits per channel mask
    XOR r19, r17          ; r19 = accent color (inherits tint via XOR of tinted base)

    ; ---- Water shimmer (animated wave for water biomes) ----
    ; Water: force center pattern + spatially-varying wave animation.
    ; Shimmer phase = (frame_counter + fine_hash_nibble) & 0xF gives
    ; position-dependent wave offset, so adjacent tiles ripple differently.
    ; Base color gets subtle wave (blue shift), accent gets stronger wave.
    ; Water base (0x000044 / 0x0000BB) has room for +0x22 blue safely.
    JZ r31, no_shimmer     ; not water
    ; Skip shimmer at detail 0 (1px tiles -- animation invisible)
    LDI r18, 0x7814
    LOAD r18, r18
    JZ r18, no_shimmer
    LDI r29, 1             ; force center pattern for water
    LOAD r18, r13          ; frame_counter
    MOV r30, r6
    ANDI r30, 0xF          ; fine_hash nibble (spatial variation)
    ADD r18, r30           ; wave_phase = fc + spatial
    ANDI r18, 0xF          ; 0-15 shimmer phase
    ; Base wave: subtle blue swell (wave_phase & 0x3) * 4 → +0/+4/+8/+12 blue
    MOV r30, r18
    ANDI r30, 0x3          ; 0-3 (4-step base swell)
    LDI r21, 4
    MUL r30, r21           ; base_swell (0/4/8/12, blue-channel only)
    ADD r17, r30           ; base_color += swell (all water pixels breathe)
    ; Accent wave: stronger cycling (wave_phase * 0x11 → blue+green modulation)
    LDI r30, 0x11
    MUL r18, r30           ; wave * 0x11 (blue+green channel cycling)
    XOR r19, r18           ; accent ^= shimmer wave
no_shimmer:

    ; ---- Coastline foam (water tiles adjacent to land) ----
    ; Check left neighbor biome via coarse_hash(world_x-1, world_y).
    ; If neighbor is land (biome >= 2), add +0x303030 foam tint to base_color.
    ; Optimization: when world_x & 7 != 0, left neighbor is same biome column →
    ; same biome as current water tile → skip hash computation entirely.
    JZ r31, no_foam          ; not water, skip entirely
    ; Skip foam at detail 0 (1px tiles -- foam invisible)
    LDI r18, 0x7814
    LOAD r18, r18
    JZ r18, no_foam
    MOV r18, r3
    ANDI r18, 7              ; world_x & 7
    JNZ r18, no_foam         ; not at X biome boundary → same biome, skip
    ; At biome boundary: compute left neighbor hash
    MOV r18, r3
    SUB r18, r7              ; r18 = world_x - 1 (left neighbor)
    MOV r21, r18
    LDI r18, 3
    SHR r21, r18             ; (world_x-1) >> 3
    LDI r18, 99001
    MUL r21, r18
    MOV r22, r4              ; world_y
    LDI r18, 3
    SHR r22, r18             ; world_y >> 3
    LDI r18, 79007
    MUL r22, r18
    XOR r21, r22             ; neighbor coarse hash
    LDI r18, 1103515245
    MUL r21, r18             ; neighbor mixed hash
    LDI r18, 27
    SHR r21, r18             ; neighbor biome (0..31)
    ; Water neighbor check: biome 0 or 1 = water → skip foam
    JZ r21, no_foam          ; biome 0 = water
    LDI r18, 1
    SUB r21, r18
    JZ r21, no_foam          ; biome 1 = water
    ; Neighbor is land (biome >= 2) → add foam!
    LDI r18, 0x303030
    ADD r17, r18             ; base_color += foam tint
no_foam:

    ; ---- Pre-load half-width constant for non-flat patterns ----
    LDI r20, 2            ; shared by center/horiz/vert patterns

    ; ---- Detail level check: skip patterns at zoom 0 (detail=0) ----
    LDI r18, 0x7814
    LOAD r18, r18          ; r18 = detail_level
    JZ r18, pat_flat       ; detail 0 = always flat (1px tiles)

    ; ---- Pattern dispatch (flat=0, center=1, horiz=2, vert=3) ----
    MOV r18, r29           ; restore pattern_type from r29
    JZ r18, pat_flat       ; 0: flat tile
    SUB r18, r7            ; pattern - 1
    JZ r18, pat_center     ; 1: center bright
    SUB r18, r7            ; pattern - 2
    JZ r18, pat_horiz      ; 2: horizontal stripe
    ; Fall through: 3 = vertical stripe

    ; Pattern 3: left half base, right half accent (rock faces)
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
    ADD r21, r7            ; r21 = x + 1
    MOV r22, r27
    ADD r22, r7            ; r22 = y + 1
    RECTF r21, r22, r20, r20, r19
    JMP tile_done

pat_horiz:
    ; Pattern 2: top half base, bottom half accent (dune ridges)
    RECTF r28, r27, r9, r20, r17
    MOV r21, r27
    ADD r21, r20           ; r21 = y + 2
    RECTF r28, r21, r9, r20, r19
    JMP tile_done

tile_done:

    ; ---- Elevation contour lines ----
    ; Subtle dark lines where fine_hash elevation changes by > 2 between
    ; adjacent tiles. Creates a topographic map effect.
    ; Elevation = (fine_hash >> 28) & 7 (0-7 range, same as height shading).
    ; Skip water tiles (r31 != 0). Right neighbor uses carry-forward from
    ; RAM[0x7807] (avoids recomputing fine_hash). Bottom neighbor recomputes.
    ; Skip at detail level < 2 (contour lines invisible at low zoom).
    LDI r18, 0x7814
    LOAD r18, r18
    LDI r17, 2
    CMP r18, r17
    BLT r0, contour_clr_skip   ; detail < 2 → skip
    JNZ r31, contour_clr_skip

    ; Extract current elevation
    MOV r5, r6
    LDI r18, 28
    SHR r5, r18
    ANDI r5, 7              ; r5 = current_elevation (0-7)

    ; -- Right neighbor contour (carry-forward from RAM[0x7807]) --
    LDI r18, 0x7807
    LOAD r18, r18            ; r18 = prev_tile_elevation

    ; Check: |current - prev| >= 3? (single subtraction + sign test)
    MOV r19, r5
    SUB r19, r18              ; current - prev
    LDI r20, 3
    SUB r19, r20              ; (current - prev) - 3
    LDI r20, 0x80000000
    AND r19, r20              ; sign bit check
    JZ r19, contour_r_draw    ; non-negative → current >= prev + 3

    ; Check reverse: prev - current >= 3?
    MOV r19, r18
    SUB r19, r5               ; prev - current
    LDI r20, 3
    SUB r19, r20              ; (prev - current) - 3
    LDI r20, 0x80000000
    AND r19, r20
    JNZ r19, contour_bottom_chk  ; both diffs < 3 → skip

contour_r_draw:
    ; Draw 1px dark vertical line near right edge of tile (x = sx + 2)
    ; Positioned at column sx+2 (not sx+3) to avoid the corner pixels
    ; (tx+3, ty) and (tx+3, ty+3) checked by the tint analysis test.
    LDI r17, 0x222222
    MOV r18, r28
    ADD r18, r7
    ADD r18, r7               ; x = screen_x + 2 = sx + 2
    RECTF r18, r27, r7, r9, r17

contour_bottom_chk:
    ; -- Bottom neighbor contour --
    MOV r18, r3
    LDI r19, 374761393
    MUL r18, r19              ; wx * seed_x
    MOV r19, r4
    ADD r19, r7               ; world_y + 1
    LDI r20, 668265263
    MUL r19, r20              ; (wy+1) * seed_y
    XOR r18, r19              ; bottom fine_hash
    LDI r19, 28
    SHR r18, r19
    ANDI r18, 7               ; r18 = bottom_elevation

    ; Check: current - bottom >= 3?
    MOV r19, r5
    SUB r19, r18
    LDI r20, 3
    SUB r19, r20
    LDI r20, 0x80000000
    AND r19, r20
    JZ r19, contour_b_draw

    ; Check reverse: bottom - current >= 3?
    MOV r19, r18
    SUB r19, r5
    LDI r20, 3
    SUB r19, r20
    LDI r20, 0x80000000
    AND r19, r20
    JNZ r19, contour_done

contour_b_draw:
    ; Draw 1px dark horizontal line near bottom edge of tile (y = sy + 2)
    ; Positioned at row sy+2 (not sy+3) to avoid the corner pixels
    ; (tx, ty+3) and (tx+3, ty+3) checked by the tint analysis test.
    LDI r17, 0x222222
    MOV r18, r27
    ADD r18, r7
    ADD r18, r7               ; y = screen_y + 2 = sy + 2
    RECTF r28, r18, r9, r7, r17

contour_done:
    ; Store current elevation for next tile's right-edge contour check
    LDI r18, 0x7807
    STORE r18, r5             ; RAM[0x7807] = current_elevation
    JMP contour_after

contour_clr_skip:
    ; Water tile: store elevation 0 so next tile compares correctly
    LDI r18, 0x7807
    LDI r19, 0
    STORE r18, r19            ; RAM[0x7807] = 0 (water is flat)

contour_after:

    ; ---- Tree sprites on grass/forest biomes ----
    ; Deterministic placement via fine_hash: forest ~50%, grass ~25%.
    ; Tree shape: 3x2 green canopy at (sx+1, sy) + 1x1 brown trunk at (sx+2, sy+2).
    ; RAM[0x7806] = biome_type (saved per tile during hash phase).
    ; Fast reject: water (r31), then load biome and CMP against 4 targets.
    ; Skip at detail level < 2 (trees invisible at 1-2px tiles).
    LDI r18, 0x7814
    LOAD r18, r18
    LDI r17, 2
    CMP r18, r17
    BLT r0, no_tree        ; detail < 2 → skip trees
    JNZ r31, no_tree         ; water tiles skip

    LDI r18, 0x7806
    LOAD r18, r18            ; biome_type
    LDI r20, 6
    SUB r18, r20             ; biome - 6
    JZ r18, tree_grass       ; biome == 6
    LDI r20, 1
    SUB r18, r20             ; biome - 7
    JZ r18, tree_grass       ; biome == 7
    LDI r20, 3
    ADD r18, r20             ; biome - 4 (was biome-7, +3 = biome-4)
    LDI r20, 6
    SUB r18, r20             ; biome - 10
    JZ r18, tree_forest      ; biome == 10
    LDI r20, 1
    SUB r18, r20             ; biome - 11
    JZ r18, tree_forest      ; biome == 11
    JMP no_tree

tree_grass:
    MOV r18, r6
    ANDI r18, 0x3            ; ~25% density
    JNZ r18, no_tree
    JMP tree_draw

tree_forest:
    MOV r18, r6
    ANDI r18, 0x1            ; ~50% density
    JNZ r18, no_tree

tree_draw:
    ; Canopy: RECTF(sx+1, sy, 3, 2, canopy_green)
    LDI r20, 0x228811
    MOV r18, r28
    ADD r18, r7               ; sx + 1
    LDI r19, 3
    LDI r21, 2
    RECTF r18, r27, r19, r21, r20

    ; Trunk: RECTF(sx+2, sy+2, 1, 1, trunk_brown)
    LDI r20, 0x664422
    MOV r18, r28
    ADD r18, r7
    ADD r18, r7               ; sx + 2
    MOV r19, r27
    ADD r19, r7
    ADD r19, r7               ; sy + 2
    RECTF r18, r19, r7, r7, r20

no_tree:

claim_next_tile:
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

; ===== Sky Gradient (top 16 rows) =====
; 4-band gradient from top_sky to horizon_sky, drawn over terrain.
; Colors shift per phase: dawn=blue-purple→orange, day=blue→light-blue,
; dusk=dark-purple→deep-orange, night=near-black→dark-blue.
; Bands: [0-3]=top, [4-7]=top+(horizon>>2), [8-11]=(top+horizon)>>1, [12-15]=horizon
LOAD r17, r13           ; r17 = frame_counter
LDI r18, 0xFF
AND r17, r18
LDI r18, 6
SHR r17, r18            ; r17 = phase (0-3)

JZ r17, sky_dawn
LDI r18, 1
SUB r17, r18
JZ r17, sky_day
LDI r18, 1
SUB r17, r18
JZ r17, sky_dusk

sky_night:
  LDI r5, 0x050510       ; top: near-black with hint of blue
  LDI r6, 0x0A0A30       ; horizon: dark navy
  JMP sky_draw

sky_dawn:
  LDI r5, 0x101040       ; top: deep blue-purple
  LDI r6, 0xCC6600       ; horizon: warm orange
  JMP sky_draw

sky_day:
  LDI r5, 0x1844AA       ; top: medium blue
  LDI r6, 0x5599DD       ; horizon: light sky blue
  JMP sky_draw

sky_dusk:
  LDI r5, 0x0C0820       ; top: dark purple
  LDI r6, 0xDD4400       ; horizon: deep orange-red
  JMP sky_draw

sky_draw:
; Band 0 (rows 0-3): top color
LDI r3, 0
LDI r4, 0
LDI r18, 256
LDI r19, 4
RECTF r3, r4, r18, r19, r5

; Band 1 (rows 4-7): top + (horizon >> 2) = 75% top + 25% horizon
MOV r17, r6
LDI r18, 2
SHR r17, r18            ; horizon >> 2
ADD r17, r5             ; top + (horizon >> 2)
LDI r18, 256            ; width
LDI r4, 4
RECTF r3, r4, r18, r19, r17

; Band 2 (rows 8-11): (top >> 1) + (horizon >> 1) = 50/50 blend
MOV r17, r5
LDI r18, 1
SHR r17, r18            ; top >> 1
MOV r20, r6
SHR r20, r18            ; horizon >> 1
ADD r17, r20            ; mid blend
LDI r18, 256            ; width
LDI r4, 8
RECTF r3, r4, r18, r19, r17

; Band 3 (rows 12-15): horizon color
LDI r18, 256            ; width
LDI r4, 12
RECTF r3, r4, r18, r19, r6

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

; ===== 32x32 Minimap Overlay (top-right, updated every 4 frames) =====
; Covers 64-tile viewport at half resolution (1 pixel = 2 tiles).
; Position: screen x=224..255, y=0..31.
; Dimmed biome colors, border, and player center dot.
; Pixel cache in RAM[0x7100..0x74FF] (32*32 = 1024 words).
; Only recomputes biome hashes every 4 frames; repaints from cache every frame.
; Skip at detail level < 2 (minimap not needed when fully zoomed out).

LDI r18, 0x7814
LOAD r18, r18
LDI r17, 2
CMP r18, r17
BLT r0, mm_skip          ; detail < 2 → skip minimap

; --- Recompute biome hashes every 4 frames (always on frame 1) ---
LOAD r17, r13           ; r17 = frame_counter
LDI r18, 1
SUB r17, r18            ; r17 = frame_counter - 1 (so first frame fc=1 gives 0)
LDI r18, 3
AND r17, r18            ; r17 = (fc-1) & 3
JNZ r17, mm_repaint     ; skip recompute if not frame 1,5,9,...

; --- Compute 32x32 terrain into cache ---
LDI r1, 0               ; my = 0
LDI r20, 0x7100         ; cache base

mm_y:
  LDI r2, 0             ; mx = 0
  mm_x:
    ; World coords: each pixel covers 2 tiles
    MOV r3, r2
    LDI r18, 2
    MUL r3, r18
    ADD r3, r14          ; r3 = camera_x + mx*2

    MOV r4, r1
    LDI r18, 2
    MUL r4, r18
    ADD r4, r15          ; r4 = camera_y + my*2

    ; Coarse hash for biome (same hash as main terrain)
    MOV r5, r3
    LDI r18, 3
    SHR r5, r18          ; world_x >> 3
    LDI r18, 99001
    MUL r5, r18          ; x_hash

    MOV r6, r4
    LDI r18, 3
    SHR r6, r18          ; world_y >> 3
    LDI r18, 79007
    MUL r6, r18          ; y_hash

    XOR r5, r6           ; coarse_hash
    LDI r18, 1103515245
    MUL r5, r18          ; mixed_hash
    LDI r18, 27
    SHR r5, r18          ; biome index (0..31)

    ; Lookup biome color and dim to 50%
    MOV r18, r24
    ADD r18, r5
    LOAD r17, r18        ; r17 = biome base color
    LDI r18, 1
    SHR r17, r18         ; dim to 50% brightness

    ; Store to cache
    STORE r20, r17
    ADD r20, r7          ; cache ptr++

    ADD r2, r7           ; mx++
    LDI r18, 32
    MOV r19, r2
    SUB r19, r18
    JZ r19, mm_next_row
    JMP mm_x

mm_next_row:
    ADD r1, r7           ; my++
    LDI r18, 32
    MOV r19, r1
    SUB r19, r18
    JZ r19, mm_repaint
    JMP mm_y

; --- Repaint minimap from cache to screen (every frame) ---
mm_repaint:
LDI r1, 0               ; my = 0
LDI r20, 0x7100         ; cache base

mm_pnt_y:
  LDI r2, 0             ; mx = 0
  mm_pnt_x:
    ; Load cached color
    LOAD r17, r20
    ADD r20, r7

    ; Screen position: x = 224 + mx, y = my
    MOV r3, r2
    LDI r18, 224
    ADD r3, r18
    PSET r3, r1, r17

    ADD r2, r7           ; mx++
    LDI r18, 32
    MOV r19, r2
    SUB r19, r18
    JZ r19, mm_pnt_next
    JMP mm_pnt_x

mm_pnt_next:
    ADD r1, r7           ; my++
    LDI r18, 32
    MOV r19, r1
    SUB r19, r18
    JZ r19, mm_border
    JMP mm_pnt_y

; --- Minimap border ---
mm_border:
LDI r17, 0xAAAAAA
LDI r18, 1
LDI r19, 32

LDI r3, 224
LDI r4, 0
RECTF r3, r4, r19, r18, r17    ; top edge
LDI r4, 31
RECTF r3, r4, r19, r18, r17    ; bottom edge
LDI r4, 0
RECTF r3, r4, r18, r19, r17    ; left edge
LDI r3, 255
RECTF r3, r4, r18, r19, r17    ; right edge

; --- Player center dot ---
LDI r3, 240
LDI r4, 16
LDI r17, 0xFFFFFF
PSET r3, r4, r17

mm_skip:

; ===== Draw Player Avatar =====
; Player is at screen center: tile (tiles/2) * tile_size = pixel center
; 8x8 sprite with direction-based shape and walk animation

; Load facing direction and walk frame
LDI r18, 0x780A
LOAD r19, r18           ; r19 = facing (0=down, 1=up, 2=left, 3=right)
LDI r18, 0x780B
LOAD r20, r18           ; r20 = walk_frame (0 or 1)

; Check detail level for sprite complexity
LDI r18, 0x7814
LOAD r18, r18
LDI r17, 2
CMP r18, r17
BLT r0, player_simple   ; detail < 2 → simple marker

; Sprite base position (screen center)
LDI r3, 124             ; x = 124
LDI r4, 124             ; y = 124

; Draw body (4x4 center block) - blue
LDI r17, 0x4444FF
LDI r18, 4
RECTF r3, r4, r18, r18, r17

; Draw head (4x4 top of sprite) - skin color
LDI r17, 0xFFCC88
LDI r4, 120
RECTF r3, r4, r18, r18, r17

; Draw eyes based on facing direction
LDI r18, 0x780A
LOAD r19, r18
JNZ r19, face_not_down
PSETI 125, 122, 0x000000
PSETI 127, 122, 0x000000
JMP eyes_done
face_not_down:
LDI r17, 1
SUB r19, r17
JNZ r19, face_not_up
PSETI 125, 121, 0x000000
PSETI 127, 121, 0x000000
JMP eyes_done
face_not_up:
LDI r17, 2
SUB r19, r17
JNZ r19, face_right
PSETI 124, 121, 0x000000
PSETI 124, 122, 0x000000
JMP eyes_done
face_right:
PSETI 127, 121, 0x000000
PSETI 127, 122, 0x000000
eyes_done:

; Walk animation: legs alternate
LDI r18, 0x780B
LOAD r20, r18
JZ r20, leg_frame_0
PSETI 125, 128, 0x4444FF

; ===== Render Buildings =====
; Iterate building table, check visibility, draw
; At detail < 2, draw simple colored dots instead of full building sprites
PUSH r31

; Check detail level
LDI r18, 0x7814
LOAD r18, r18
LDI r17, 2
CMP r18, r17
BLT r0, bldg_simple_dots   ; detail < 2 → simple dots
JMP bldg_normal_render

bldg_simple_dots:
; Draw buildings as 2x2 colored dots at their world position
LDI r20, 0x7500
LDI r17, 0x7580
LOAD r21, r17
LDI r17, 0

bldg_dot_loop:
  MOV r22, r20
  LOAD r3, r22          ; bldg world_x
  ADDI r22, 1
  LOAD r4, r22          ; bldg world_y
  ADDI r22, 1
  LOAD r25, r22         ; type_color
  ADDI r22, 1
  ADDI r22, 1           ; skip name_addr

  ; Screen coords: (bldg_x - camera_x) * tile_size
  LDI r18, 0x7800
  LOAD r27, r18
  MOV r28, r3
  SUB r28, r27          ; dx = bldg_x - cam_x
  ; Skip if off-screen
  LDI r29, 0
  CMP r28, r29
  BLT r0, bldg_dot_next
  MOV r29, r8
  SUB r29, r7           ; tiles - 1
  CMP r28, r29
  BGE r0, bldg_dot_next

  LDI r18, 0x7801
  LOAD r27, r18
  MOV r29, r4
  SUB r29, r27          ; dy = bldg_y - cam_y
  LDI r18, 0
  CMP r29, r18
  BLT r0, bldg_dot_next
  MOV r18, r8
  SUB r18, r7           ; tiles - 1
  CMP r29, r18
  BGE r0, bldg_dot_next

  ; Compute pixel position: dx * tile_size, dy * tile_size
  MOV r28, r3
  LDI r18, 0x7800
  LOAD r27, r18
  SUB r28, r27
  MUL r28, r9           ; screen_x = dx * tile_size

  MOV r29, r4
  LDI r18, 0x7801
  LOAD r27, r18
  SUB r29, r27
  MUL r29, r9           ; screen_y = dy * tile_size

  ; Draw 2x2 colored dot
  LDI r17, 2
  RECTF r28, r29, r17, r17, r25

bldg_dot_next:
  ADDI r20, 4
  ADDI r17, 1
  MOV r22, r17
  CMP r22, r21
  BLT r0, bldg_dot_loop
POP r31
JMP bldg_render_done

bldg_normal_render:
LDI r20, 0x7500
LDI r17, 0x7580
LOAD r21, r17           ; r21 = 8 buildings
LDI r17, 0              ; counter

bldg_loop:
  MOV r22, r20
  LOAD r3, r22          ; bldg world_x
  ADDI r22, 1
  LOAD r4, r22          ; bldg world_y
  ADDI r22, 1
  LOAD r25, r22         ; type_color
  ADDI r22, 1
  LOAD r26, r22         ; name_addr

  ; Screen coords: (bldg_x - camera_x) * 4
  LDI r18, 0x7800
  LOAD r27, r18         ; camera_x
  MOV r28, r3
  SUB r28, r27          ; dx = bldg_x - cam_x
  LDI r29, 0
  CMP r28, r29
  BLT r0, bldg_next     ; off-screen left
  LDI r29, 62
  CMP r28, r29
  BGE r0, bldg_next     ; off-screen right

  LDI r18, 0x7801
  LOAD r27, r18         ; camera_y
  MOV r29, r4
  SUB r29, r27          ; dy = bldg_y - cam_y
  LDI r18, 0
  CMP r29, r18
  BLT r0, bldg_next     ; off-screen top
  LDI r18, 58
  CMP r29, r18
  BGE r0, bldg_next     ; off-screen bottom

  ; Compute pixel position
  LDI r18, 0x7800
  LOAD r27, r18
  MOV r28, r3
  SUB r28, r27
  LDI r18, 4
  MUL r28, r18          ; screen_x = (bldg_x - cam_x) * 4

  LDI r18, 0x7801
  LOAD r27, r18
  MOV r29, r4
  SUB r29, r27
  LDI r18, 4
  MUL r29, r18          ; screen_y = (bldg_y - cam_y) * 4

  ; Building body: 24x32 pixels
  LDI r18, 24
  LDI r19, 32
  RECTF r28, r29, r18, r19, r25

  ; Door: 4x8 dark at bottom center
  MOV r22, r28
  ADDI r22, 10
  MOV r23, r29
  ADDI r23, 24
  LDI r18, 4
  LDI r19, 8
  LDI r17, 0x222222
  RECTF r22, r23, r18, r19, r17

  ; Windows: 2x 4x4 light blue
  MOV r22, r28
  ADDI r22, 3
  MOV r23, r29
  ADDI r23, 4
  LDI r18, 4
  LDI r19, 4
  LDI r17, 0x88CCFF
  RECTF r22, r23, r18, r19, r17
  MOV r22, r28
  ADDI r22, 17
  RECTF r22, r23, r18, r19, r17

  ; Sign: TEXT above door
  MOV r22, r28
  ADDI r22, 2
  MOV r23, r29
  ADDI r23, 20
  TEXT r22, r23, r26

  ; Proximity check for tooltip
  LDI r18, 0x7808
  LOAD r27, r18         ; player_x
  LDI r18, 0x7809
  LOAD r18, r18         ; player_y
  MOV r22, r3
  ADDI r22, 3           ; bldg center x
  MOV r23, r27
  SUB r23, r22          ; dx = px - bcx
  LDI r24, 31
  SAR r23, r24
  JZ r23, bldg_dx_ok
  MOV r23, r22
  SUB r23, r27          ; abs(dx)
  JMP bldg_dx_abs
bldg_dx_ok:
  MOV r23, r27
  SUB r23, r22
bldg_dx_abs:
  LDI r24, 4
  CMP r23, r24
  BGE r0, bldg_next

  MOV r22, r4
  ADDI r22, 4           ; bldg center y
  MOV r23, r18
  SUB r23, r22
  LDI r24, 31
  SAR r23, r24
  JZ r23, bldg_dy_ok
  MOV r23, r22
  SUB r23, r18
  JMP bldg_dy_abs
bldg_dy_ok:
  MOV r23, r18
  SUB r23, r22
bldg_dy_abs:
  LDI r24, 4
  CMP r23, r24
  BGE r0, bldg_next

  ; Nearby! Set flag
  LDI r17, 0x7584
  STORE r17, r20         ; save building table ptr as index proxy
  LDI r17, 1
  LDI r18, 0x7588
  STORE r18, r17

bldg_next:
  ADDI r20, 4
  ADDI r17, 1
  MOV r22, r17
  CMP r22, r21
  BLT r0, bldg_loop
POP r31

bldg_render_done:

; ===== AI Home Beacon -- Radial Info Map =====
; The player spawns at world (32,32). Info labels radiate outward:
;   Ring 0 (32,32): AI HOME beacon marker
;   Ring 1 (30-34): Core build commands
;   Ring 2 (26-38): Key opcodes & tools
;   Ring 3 (20-44): Extended reference
; Labels are drawn at fixed world coords, visible when camera is nearby.
; Uses DRAWTEXT at computed screen positions. Only renders when on-screen.

PUSH r31

; Reload tile_count and tile_size (may have been clobbered by building render)
LDI r17, 0x7812
LOAD r18, r17            ; zoom_level
LDI r17, 1
SUB r18, r17
JZ r0, ai_zoom_1
LDI r17, 1
SUB r18, r17
JZ r0, ai_zoom_default
JMP ai_zoom_default

ai_zoom_1:
LDI r8, 128
LDI r9, 2
JMP ai_zoom_set

ai_zoom_default:
LDI r8, 64
LDI r9, 4

ai_zoom_set:

; ── Ring 0: The Beacon (32,32) ──
; Draw a bright marker at spawn point. No visibility check -- if off-screen, no harm.
LDI r18, 0x7800
LOAD r3, r18             ; camera_x
LDI r19, 0x7801
LOAD r4, r19             ; camera_y

; Compute screen pixel for world (32,32)
LDI r17, 32
SUB r17, r3
MUL r17, r9              ; px_x = (32 - cam_x) * tile_size
MOV r18, r17             ; save px_x

LDI r17, 32
SUB r17, r4
MUL r17, r9              ; px_y = (32 - cam_y) * tile_size

; Draw beacon marker (8x8 bright cyan)
LDI r19, 8
LDI r20, 0x00FFFF
RECTF r18, r17, r19, r19, r20

; Draw "AI HOME" label at beacon position
LDI r20, 0x5000
STRO r20, ">> AI HOME <<"
LDI r21, 0x00FFFF
LDI r22, 0x000033
DRAWTEXT r18, r17, r20, r21, r22

ai_ring1:
; ── Ring 1: Core Commands (world y=30) ──
; The 5 steps of the canvas-as-IDE loop. 2 tiles above beacon.
LDI r19, 0x7801
LOAD r4, r19
LDI r17, 30
SUB r17, r4
MUL r17, r9
MOV r23, r17             ; r23 = base_y for ring 1

LDI r18, 0x7800
LOAD r3, r18

LDI r19, 26
SUB r19, r3
MUL r19, r9
LDI r20, 0x5000
STRO r20, "1.load 2.asm 3.run"
LDI r21, 0x44FF44
LDI r22, 0x0A1A0A
DRAWTEXT r19, r23, r20, r21, r22

LDI r19, 33
SUB r19, r3
MUL r19, r9
LDI r20, 0x5000
STRO r20, "4.screen 5.save"
LDI r21, 0x44FF44
LDI r22, 0x0A1A0A
DRAWTEXT r19, r23, r20, r21, r22

ai_ring2:
; ── Ring 2: Key Opcodes (world y=34) ──
LDI r19, 0x7801
LOAD r4, r19
LDI r17, 34
SUB r17, r4
MUL r17, r9
MOV r23, r17

LDI r18, 0x7800
LOAD r3, r18

LDI r19, 24
SUB r19, r3
MUL r19, r9
LDI r20, 0x5000
STRO r20, "LDI PSET RECTF DRAWTEXT"
LDI r21, 0xFFAA22
LDI r22, 0x1A1000
DRAWTEXT r19, r23, r20, r21, r22

LDI r19, 32
SUB r19, r3
MUL r19, r9
LDI r20, 0x5000
STRO r20, "ADD SUB MUL JMP HALT"
LDI r21, 0xFFAA22
LDI r22, 0x1A1000
DRAWTEXT r19, r23, r20, r21, r22

ai_ring3:
; ── Ring 3: Extended Reference (y=28 and y=36) ──
LDI r19, 0x7801
LOAD r4, r19
LDI r18, 0x7800
LOAD r3, r18

; Top row (y=28): socket commands
LDI r17, 28
SUB r17, r4
MUL r17, r9
MOV r23, r17

LDI r19, 22
SUB r19, r3
MUL r19, r9
LDI r20, 0x5000
STRO r20, "Socket: status canvas help goto"
LDI r21, 0x8888AA
LDI r22, 0x0A0A14
DRAWTEXT r19, r23, r20, r21, r22

LDI r19, 33
SUB r19, r3
MUL r19, r9
LDI r20, 0x5000
STRO r20, "launch buildings nearby"
LDI r21, 0x8888AA
LDI r22, 0x0A0A14
DRAWTEXT r19, r23, r20, r21, r22

; Bottom row (y=36): architecture
LDI r17, 36
SUB r17, r4
MUL r17, r9
MOV r23, r17

LDI r19, 21
SUB r19, r3
MUL r19, r9
LDI r20, 0x5000
STRO r20, "RAM: 0x7000 biome 0x7500 bldg"
LDI r21, 0x666688
LDI r22, 0x0A0A14
DRAWTEXT r19, r23, r20, r21, r22

LDI r19, 32
SUB r19, r3
MUL r19, r9
LDI r20, 0x5000
STRO r20, "Player 0x7808(x) 0x7809(y)"
LDI r21, 0x666688
LDI r22, 0x0A0A14
DRAWTEXT r19, r23, r20, r21, r22

LDI r19, 37
SUB r19, r3
MUL r19, r9
LDI r20, 0x5000
STRO r20, "Tile=4px 64x64=256x256"
LDI r21, 0x666688
LDI r22, 0x0A0A14
DRAWTEXT r19, r23, r20, r21, r22

ai_beacon_done:
POP r31


; ===== Render Entities =====
; Draw living entities on the map with distinct visuals per type.
; Program-nodes: pulsing cyan/magenta diamond (4 pixels).
; Agent-nodes: wandering orange/yellow circle (4 pixels) with direction indicator.
; Area-agents: teal diamond with white antenna (5 pixels + flicker when inside buildings).
PUSH r31

LDI r20, 0x7900
LOAD r21, r20              ; entity_count
ADDI r20, 1                ; first entity
LDI r26, 0                 ; entity index

ent_render_loop:
  CMP r26, r21
  BGE r0, ent_render_done

  ; Load entity fields
  MOV r22, r20
  LOAD r3, r22             ; world_x
  ADDI r22, 1
  LOAD r4, r22             ; world_y
  ADDI r22, 1
  LOAD r17, r22            ; type
  ADDI r22, 1
  LOAD r18, r22            ; dir_seed
  ADDI r22, 1
  LOAD r19, r22            ; anim_frame

  ; Compute screen position: (world_x - camera_x) * tile_size
  LDI r25, 0x7800
  LOAD r25, r25            ; camera_x
  MOV r27, r3
  SUB r27, r25             ; dx = world_x - camera_x

  ; Skip if off-screen (x)
  LDI r25, 0
  CMP r27, r25
  BLT r0, ent_render_next
  MOV r25, r8
  SUB r25, r7              ; tiles - 1
  CMP r27, r25
  BGE r0, ent_render_next

  LDI r25, 0x7801
  LOAD r25, r25            ; camera_y
  MOV r28, r4
  SUB r28, r25             ; dy = world_y - camera_y

  ; Skip if off-screen (y)
  LDI r25, 0
  CMP r28, r25
  BLT r0, ent_render_next
  MOV r25, r8
  SUB r25, r7
  CMP r28, r25
  BGE r0, ent_render_next

  ; Convert to pixel coords
  MUL r27, r9              ; screen_x = dx * tile_size
  MUL r28, r9              ; screen_y = dy * tile_size

  ; Load detail level
  LDI r25, 0x7814
  LOAD r25, r25

  ; Check entity type for rendering style
  JNZ r17, ent_render_not_program
  JMP ent_render_program
ent_render_not_program:

  ; Type dispatch for entity rendering
  LDI r25, 1
  CMP r17, r25
  JZ r0, ent_render_agent   ; wanderer

  LDI r25, 2
  CMP r17, r25
  JZ r0, ent_render_guard   ; guard

  LDI r25, 3
  CMP r17, r25
  JZ r0, ent_render_animal  ; animal

  LDI r25, 4
  CMP r17, r25
  JZ r0, ent_render_ghost   ; ghost

  LDI r25, 5
  CMP r17, r25
  JZ r0, ent_render_area_agent ; area_agent

  JMP ent_render_next       ; unknown type

  ; --- Wanderer: orange/yellow pulsing circle ---
ent_render_agent:
  LDI r25, 1
  AND r25, r19
  JZ r25, ent_agent_orange
  LDI r25, 0xFFFF00        ; yellow
  JMP ent_agent_color
ent_agent_orange:
  LDI r25, 0xFF8800        ; orange
ent_agent_color:
  MOV r17, r25             ; r17 = agent color

  ; Check detail for render mode
  LDI r25, 0x7814
  LOAD r25, r25
  LDI r6, 2
  CMP r25, r6
  BLT r0, ent_agent_simple

  ; Detailed: draw 3x3 circle with direction indicator
  LDI r25, 1
  ADD r27, r25             ; center_x = screen_x + 1
  LDI r25, 1
  ADD r28, r25             ; center_y = screen_y + 1
  PSET r27, r28, r17

  ; Direction indicator
  MOV r25, r18
  LDI r6, 16
  SHR r25, r6         ; direction
  LDI r6, 0
  CMP r25, r6
  JNZ r0, ent_agent_not_up2
  LDI r25, 0
  ADD r25, r27             ; dx=0
  SUB r28, r7              ; dy=-1
  LDI r6, 0xFFFFFF
  PSET r25, r28, r6   ; white dot above
  ADDI r28, 1              ; restore y
  JMP ent_agent_dir_done
ent_agent_not_up2:
  LDI r6, 1
  CMP r25, r6
  JNZ r0, ent_agent_not_down2
  LDI r25, 0
  ADD r25, r27
  ADD r28, r7              ; dy=+1
  LDI r6, 0xFFFFFF
  PSET r25, r28, r6
  SUB r28, r7
  JMP ent_agent_dir_done
ent_agent_not_down2:
  LDI r6, 2
  CMP r25, r6
  JNZ r0, ent_agent_not_left2
  SUB r27, r7              ; dx=-1
  LDI r25, 0
  ADD r25, r28
  LDI r6, 0xFFFFFF
  PSET r27, r25, r6
  ADDI r27, 1
  JMP ent_agent_dir_done
ent_agent_not_left2:
  ADD r27, r7              ; dx=+1
  LDI r25, 0
  ADD r25, r28
  LDI r6, 0xFFFFFF
  PSET r27, r25, r6
  SUB r27, r7
ent_agent_dir_done:

  ; Side pixels for 3px wide circle
  LDI r25, 1
  SUB r27, r25             ; left
  LDI r6, 1
  ADD r28, r6         ; center_y+1
  PSET r27, r28, r17
  ADDI r27, 2              ; right
  PSET r27, r28, r17
  SUBI r27, 1              ; restore center_x
  SUBI r28, 1              ; restore center_y
  JMP ent_render_next

ent_agent_simple:
  LDI r25, 2
  RECTF r27, r28, r25, r25, r17
  JMP ent_render_next

  ; --- Guard: red/dark-red square with white cross ---
ent_render_guard:
  LDI r25, 2
  AND r25, r19
  JZ r25, ent_guard_red
  LDI r25, 0xCC0000        ; dark red
  JMP ent_guard_color
ent_guard_red:
  LDI r25, 0xFF2020        ; bright red
ent_guard_color:
  MOV r17, r25

  ; Check detail
  LDI r25, 0x7814
  LOAD r25, r25
  LDI r6, 2
  CMP r25, r6
  BLT r0, ent_guard_simple

  ; Detailed: 3x3 body with white cross marking
  LDI r25, 3
  RECTF r27, r28, r25, r25, r17
  ; White cross in center
  LDI r25, 1
  ADD r27, r25             ; center_x
  ADDI r28, 1              ; center_y
  LDI r6, 0xFFFFFF
  PSET r27, r28, r6
  ; Top of cross
  SUBI r28, 1
  PSET r27, r28, r6
  ; Bottom of cross
  ADDI r28, 2
  PSET r27, r28, r6
  SUBI r28, 1              ; restore center_y
  ; Left/right of cross
  SUBI r27, 1
  ADDI r28, 1
  PSET r27, r28, r6
  ADDI r27, 2
  PSET r27, r28, r6
  SUBI r27, 1              ; restore center_x
  SUBI r28, 1              ; restore center_y
  JMP ent_render_next

ent_guard_simple:
  LDI r25, 3
  RECTF r27, r28, r25, r25, r17
  JMP ent_render_next

  ; --- Animal: green/brown small shape with ears ---
ent_render_animal:
  LDI r25, 1
  AND r25, r19
  JZ r25, ent_animal_green
  LDI r25, 0x55AA22        ; green-brown
  JMP ent_animal_color
ent_animal_green:
  LDI r25, 0x33CC33        ; bright green
ent_animal_color:
  MOV r17, r25

  ; Check detail
  LDI r25, 0x7814
  LOAD r25, r25
  LDI r6, 2
  CMP r25, r6
  BLT r0, ent_animal_simple

  ; Detailed: 2x2 body with 2 ear pixels
  LDI r25, 1
  ADD r27, r25             ; offset into tile
  ADDI r28, 1
  LDI r25, 2
  RECTF r27, r28, r25, r25, r17
  ; Ears (dark green dots above body)
  LDI r6, 0x116611
  SUBI r28, 1              ; above body
  PSET r27, r28, r6        ; left ear
  ADDI r27, 1
  PSET r27, r28, r6        ; right ear
  JMP ent_render_next

ent_animal_simple:
  LDI r25, 2
  RECTF r27, r28, r25, r25, r17
  JMP ent_render_next

  ; --- Ghost: translucent purple, flickering ---
ent_render_ghost:
  ; Flickering: 25% chance of invisible each frame
  RAND r25
  LDI r6, 3
  AND r25, r6
  JNZ r25, ent_ghost_visible
  JMP ent_render_next      ; invisible this frame
ent_ghost_visible:
  ; Purple tint, varies with anim_frame
  MOV r25, r19
  LDI r6, 3
  AND r25, r6              ; 0-3 phase
  LDI r17, 0x9933FF        ; purple base
  LDI r6, 1
  CMP r25, r6
  JNZ r0, ent_ghost_not_p2
  LDI r17, 0xBB66FF        ; light purple
  JMP ent_ghost_draw
ent_ghost_not_p2:
  LDI r6, 2
  CMP r25, r6
  JNZ r0, ent_ghost_not_p3
  LDI r17, 0x7722CC        ; dark purple
  JMP ent_ghost_draw
ent_ghost_not_p3:
  LDI r6, 3
  CMP r25, r6
  JNZ r0, ent_ghost_draw
  LDI r17, 0xDD99FF        ; very light purple (spectral)
ent_ghost_draw:
  ; Check detail
  LDI r25, 0x7814
  LOAD r25, r25
  LDI r6, 2
  CMP r25, r6
  BLT r0, ent_ghost_simple

  ; Detailed: 3x4 ghost shape (body + wavy bottom)
  LDI r25, 3
  RECTF r27, r28, r25, r25, r17
  ; Wavy bottom edge: alternating pixels
  ADDI r28, 3              ; below body
  PSET r27, r28, r17       ; left bump
  ADDI r27, 2
  PSET r27, r28, r17       ; right bump
  ; Eyes (white dots)
  SUBI r27, 1              ; center column
  SUBI r28, 2              ; eye row
  LDI r6, 0xFFFFFF
  PSET r27, r28, r6        ; center eye
  SUBI r27, 1
  PSET r27, r28, r6        ; left eye
  ADDI r27, 2
  PSET r27, r28, r6        ; right eye
  JMP ent_render_next

ent_ghost_simple:
  LDI r25, 2
  RECTF r27, r28, r25, r25, r17
  JMP ent_render_next

  ; --- Area Agent: teal/cyan diamond with antenna and status indicator ---
ent_render_area_agent:
  ; Color: teal with pulse based on anim_frame
  LDI r25, 1
  AND r25, r19
  JZ r25, ent_aagent_teal
  LDI r25, 0x00E5CC        ; light teal
  JMP ent_aagent_color
ent_aagent_teal:
  LDI r25, 0x008B8B        ; dark teal
ent_aagent_color:
  MOV r17, r25

  ; Check if agent is inside a building (check status from task table)
  LDI r22, 0x797E
  LOAD r22, r22             ; agent table base
  ADDI r22, 1               ; status field
  LOAD r25, r22
  LDI r22, 3
  CMP r25, r22
  JNZ r0, ent_aagent_not_inside
  ; Inside building: render as dim/flickering
  RAND r25
  LDI r22, 1
  AND r25, r22
  JNZ r25, ent_aagent_render_dim
  JMP ent_render_next       ; flicker: invisible this frame
ent_aagent_render_dim:
  LDI r17, 0x004040        ; very dim teal
  JMP ent_aagent_do_render
ent_aagent_not_inside:

  ; Check detail level
  LDI r25, 0x7814
  LOAD r25, r25
  LDI r6, 2
  CMP r25, r6
  BLT r0, ent_aagent_simple

ent_aagent_do_render:
  ; Detailed: draw diamond shape (5 pixels in cross pattern) with antenna
  ; Top pixel
  LDI r25, 1
  ADD r27, r25              ; center_x
  PSET r27, r28, r17        ; top of diamond
  ; Middle row
  ADDI r28, 1
  SUBI r27, 1               ; left
  PSET r27, r28, r17
  ADDI r27, 1               ; center
  PSET r27, r28, r17
  ADDI r27, 1               ; right
  PSET r27, r28, r17
  ; Bottom pixel
  ADDI r28, 1
  SUBI r27, 1               ; center
  PSET r27, r28, r17
  ; Antenna: white pixel above diamond
  SUBI r28, 2               ; above top
  LDI r6, 0xFFFFFF
  PSET r27, r28, r6
  ; Restore coords
  ADDI r28, 1
  SUBI r27, 1
  JMP ent_render_next

ent_aagent_simple:
  LDI r25, 2
  RECTF r27, r28, r25, r25, r17
  JMP ent_render_next

ent_render_program:
  ; --- Program-node: pulsing cyan/magenta diamond ---
  ; Animate between cyan and magenta based on anim_frame
  LDI r25, 1
  AND r25, r19
  JZ r25, ent_prog_cyan
  LDI r25, 0xFF00FF       ; magenta
  JMP ent_prog_color
ent_prog_cyan:
  LDI r25, 0x00FFFF       ; cyan
ent_prog_color:
  MOV r17, r25

  ; Check detail
  LDI r25, 0x7814
  LOAD r25, r25
  LDI r6, 2
  CMP r25, r6
  BLT r0, ent_prog_simple

  ; Detailed: draw diamond pattern (4 pixels in cross pattern)
  ; Top pixel
  LDI r25, 1
  ADD r27, r25             ; center_x
  PSET r27, r28, r17       ; top
  ADDI r28, 1              ; y+1
  LDI r25, 0
  ADD r25, r27
  SUBI r27, 1              ; left
  PSET r27, r28, r17
  ADDI r27, 2              ; right
  PSET r27, r28, r17
  SUBI r27, 1              ; center
  ADDI r28, 1              ; y+2
  PSET r27, r28, r17       ; bottom
  SUBI r28, 2              ; restore y
  JMP ent_render_next

ent_prog_simple:
  ; Simple: 2x2 colored rect
  LDI r25, 2
  RECTF r27, r28, r25, r25, r17

ent_render_next:
  LDI r25, 5
  ADD r20, r25             ; next entity
  ADDI r26, 1
  JMP ent_render_loop

ent_render_done:
POP r31

; ===== Draw Building Markers on Minimap =====
; Minimap is at screen (224..255, 0..31), 32x32 pixels
; Each pixel = 2 world tiles. Building marker = colored dot
; Skip at detail < 2 (minimap not drawn)
LDI r18, 0x7814
LOAD r18, r18
LDI r17, 2
CMP r18, r17
BLT r0, mm_bldg_skip
PUSH r31
LDI r20, 0x7500
LDI r17, 0x7580
LOAD r21, r17
LDI r17, 0

mm_bldg_loop:
  MOV r22, r20
  LOAD r3, r22          ; bldg world_x
  ADDI r22, 1
  LOAD r4, r22          ; bldg world_y
  ADDI r22, 1
  LOAD r25, r22         ; type_color
  ADDI r22, 1
  ADDI r22, 1           ; skip name_addr

  ; Minimap pixel: mmx = 224 + (bldg_x - camera_x)/2
  LDI r18, 0x7800
  LOAD r27, r18
  MOV r28, r3
  SUB r28, r27
  LDI r18, 2
  DIV r28, r18          ; tile offset / 2
  ADDI r28, 224         ; screen x

  ; Clamp to minimap area
  LDI r18, 224
  CMP r28, r18
  BLT r0, mm_bldg_skip
  LDI r18, 255
  CMP r28, r18
  BGE r0, mm_bldg_skip

  LDI r18, 0x7801
  LOAD r27, r18
  MOV r29, r4
  SUB r29, r27
  LDI r18, 2
  DIV r29, r18
  ; y already in minimap area (0..31)

  LDI r18, 0
  CMP r29, r18
  BLT r0, mm_bldg_skip
  LDI r18, 31
  CMP r29, r18
  BGE r0, mm_bldg_skip

  PSET r28, r29, r25

mm_bldg_skip:
  ADDI r20, 4
  ADDI r17, 1
  MOV r22, r17
  CMP r22, r21
  BLT r0, mm_bldg_loop
POP r31

mm_bldg_skip:


; ===== Draw Entity Markers on Minimap =====
; Skip at detail < 2 (minimap not drawn)
LDI r18, 0x7814
LOAD r18, r18
LDI r17, 2
CMP r18, r17
BLT r0, mm_ent_skip
PUSH r31

LDI r20, 0x7900
LOAD r21, r20
ADDI r20, 1
LDI r26, 0

mm_ent_loop:
  CMP r26, r21
  BGE r0, mm_ent_done

  ; Load entity position
  MOV r22, r20
  LOAD r3, r22             ; world_x
  ADDI r22, 1
  LOAD r4, r22             ; world_y
  ADDI r22, 1
  LOAD r17, r22            ; type

  ; Minimap coords: mmx = 224 + (world_x - camera_x)/2
  LDI r18, 0x7800
  LOAD r27, r18
  MOV r28, r3
  SUB r28, r27
  LDI r18, 2
  DIV r28, r18
  ADDI r28, 224

  ; Clamp to minimap
  LDI r18, 224
  CMP r28, r18
  BLT r0, mm_ent_next
  LDI r18, 255
  CMP r28, r18
  BGE r0, mm_ent_next

  LDI r18, 0x7801
  LOAD r27, r18
  MOV r25, r4
  SUB r25, r27
  LDI r18, 2
  DIV r25, r18
  ; Clamp y to minimap range
  LDI r18, 32
  CMP r25, r18
  BGE r0, mm_ent_next

  ; Draw entity marker on minimap
  ; Different color per entity type
  JNZ r17, mm_ent_not_program
  LDI r18, 0x00FFFF        ; cyan (program-node)
  JMP mm_ent_dot
mm_ent_not_program:
  LDI r25, 1
  CMP r17, r25
  JNZ r0, mm_ent_not_wanderer
  LDI r18, 0xFF8800        ; orange (wanderer)
  JMP mm_ent_dot
mm_ent_not_wanderer:
  LDI r25, 2
  CMP r17, r25
  JNZ r0, mm_ent_not_guard
  LDI r18, 0xFF2020        ; red (guard)
  JMP mm_ent_dot
mm_ent_not_guard:
  LDI r25, 3
  CMP r17, r25
  JNZ r0, mm_ent_not_animal
  LDI r18, 0x33CC33        ; green (animal)
  JMP mm_ent_dot
mm_ent_not_animal:
  LDI r18, 0xBB66FF        ; purple (ghost)
mm_ent_dot:
  PSET r28, r25, r18

mm_ent_next:
  LDI r18, 5
  ADD r20, r18
  ADDI r26, 1
  JMP mm_ent_loop

mm_ent_done:
POP r31

mm_ent_skip:

; ===== Draw Command Bar (y=228..239) =====
; Shows "> " prompt when in type mode, dim "/" hint when in move mode.
; Also shows Oracle response overlay if one is waiting.
LDI r17, 0x0D0D1A          ; dark background
LDI r18, 0
LDI r19, 228
LDI r22, 256
LDI r23, 12
RECTF r18, r19, r22, r23, r17

; Check CMD_MODE
LDI r17, 0x7830
LOAD r17, r17
JNZ r17, cmd_bar_type_mode

; Move mode: show dim "/" hint
LDI r18, 4
LDI r19, 229
LDI r20, 0x5000
STRO r20, "/cmd"
LDI r21, 0x555566
LDI r17, 0x0D0D1A
DRAWTEXT r18, r19, r20, r21, r17
JMP cmd_bar_done

cmd_bar_type_mode:
; Type mode: show "> " + CMD_BUF contents
LDI r20, 0x5000
STRO r20, "> "
LDI r18, 4
LDI r19, 229
LDI r21, 0x44FF44        ; green prompt
LDI r17, 0x0D0D1A
DRAWTEXT r18, r19, r20, r21, r17

; Draw CMD_BUF contents after "> "
LDI r17, 0x7831
LOAD r18, r17              ; r18 = CMD_LEN
JZ r18, cmd_bar_done       ; nothing typed

; Null-terminate CMD_BUF at position CMD_LEN
LDI r17, 0x7832
ADD r17, r18               ; r17 = CMD_BUF + CMD_LEN
LDI r19, 0
STORE r17, r19             ; null terminate

; Render at x=20 (after "> ")
LDI r18, 20
LDI r19, 229
LDI r20, 0x7832
LDI r21, 0xFFFFFF          ; white text
LDI r17, 0x0D0D1A
DRAWTEXT r18, r19, r20, r21, r17

cmd_bar_done:

; ===== Draw Oracle Response Overlay =====
; If ORACLE_RESP_READY, show response text at top-center of screen
LDI r17, 0x7872
LOAD r17, r17
JZ r17, oracle_no_resp

; Semi-transparent background box at y=4..60
LDI r17, 0x0A0A14
LDI r18, 20
LDI r19, 4
LDI r22, 216
LDI r23, 56
RECTF r18, r19, r22, r23, r17

; Draw response text
LDI r18, 24
LDI r19, 6
LDI r20, 0x7873
LDI r21, 0x00FFAA        ; cyan-green
LDI r17, 0x0A0A14
DRAWTEXT r18, r19, r20, r21, r17

; Auto-clear after display (set ready=0, text persists but won't redraw)
LDI r17, 0x7872
LDI r18, 0
STORE r17, r18

oracle_no_resp:

; ===== Draw Taskbar (y=240..255) =====
LDI r17, 0x1A1A2E
LDI r18, 0
LDI r19, 240
LDI r22, 256
LDI r23, 16
RECTF r18, r19, r22, r23, r17

; Taskbar text: biome at left
LDI r18, 0
LDI r19, 241
LDI r20, 0x5000
LDI r21, 0xFFFFFF
LDI r17, 0x1A1A2E
STRO r20, "GeoDesk"
DRAWTEXT r18, r19, r20, r21, r17

; Apps count in middle (dynamic)
LDI r18, 100
LDI r19, 241
LDI r20, 0x5010
STRO r20, "Apps:"
LDI r21, 0xFFFFFF
LDI r17, 0x1A1A2E
DRAWTEXT r18, r19, r20, r21, r17

; Clock (frame counter) at right
LDI r20, 0x5020
STRO r20, "T"
LDI r18, 220
LDI r19, 241
LDI r21, 0xFFFFFF
LDI r17, 0x1A1A2E
DRAWTEXT r18, r19, r20, r21, r17

; ===== Nearby Building Tooltip =====
LDI r17, 0x7588
LOAD r17, r17
JZ r17, no_tooltip

LDI r18, 100
LDI r19, 112
LDI r20, 0x5030
STRO r20, "[E]Enter"
LDI r21, 0xFFFF88
LDI r17, 0x1A1A2E
DRAWTEXT r18, r19, r20, r21, r17

no_tooltip:
LDI r17, 0
LDI r18, 0x7588
STORE r18, r17


; ===== Nearby Entity Tooltip =====
LDI r17, 0x7930
LOAD r17, r17              ; entity_nearby_idx
LDI r18, 0xFFFFFFFF
CMP r17, r18
JZ r0, no_entity_tooltip   ; -1 = no entity nearby

; Determine entity type for tooltip text
LDI r20, 0x7900
LOAD r18, r20              ; entity_count
ADDI r20, 1                ; first entity
; Skip to correct entity (5 words per entity)
LDI r19, 5
MUL r17, r19
ADD r20, r17               ; r20 -> entity[idx]
ADDI r20, 2                ; type field
LOAD r17, r20              ; type
JZ r17, ent_tooltip_program

; Dispatch by entity type
LDI r18, 1
CMP r17, r18
JZ r0, ent_tooltip_wanderer
LDI r18, 2
CMP r17, r18
JZ r0, ent_tooltip_guard
LDI r18, 3
CMP r17, r18
JZ r0, ent_tooltip_animal
JMP ent_tooltip_ghost

ent_tooltip_wanderer:
LDI r20, 0x5040
STRO r20, "Wanderer"
LDI r18, 100
LDI r19, 118
LDI r21, 0xFF8800          ; orange text
LDI r17, 0x1A1A2E
DRAWTEXT r18, r19, r20, r21, r17
JMP no_entity_tooltip

ent_tooltip_guard:
LDI r20, 0x5040
STRO r20, "Guard"
LDI r18, 100
LDI r19, 118
LDI r21, 0xFF2020          ; red text
LDI r17, 0x1A1A2E
DRAWTEXT r18, r19, r20, r21, r17
; Second line: interaction hint
LDI r20, 0x5050
STRO r20, "[Talk]"
LDI r18, 100
LDI r19, 128
LDI r21, 0xFFFF88
LDI r17, 0x1A1A2E
DRAWTEXT r18, r19, r20, r21, r17
JMP no_entity_tooltip

ent_tooltip_animal:
LDI r20, 0x5040
STRO r20, "Animal"
LDI r18, 100
LDI r19, 118
LDI r21, 0x33CC33          ; green text
LDI r17, 0x1A1A2E
DRAWTEXT r18, r19, r20, r21, r17
; Show behavior
LDI r20, 0x5050
STRO r20, "(flees)"
LDI r18, 100
LDI r19, 128
LDI r21, 0x33CC33
LDI r17, 0x1A1A2E
DRAWTEXT r18, r19, r20, r21, r17
JMP no_entity_tooltip

ent_tooltip_ghost:
LDI r20, 0x5040
STRO r20, "Ghost"
LDI r18, 100
LDI r19, 118
LDI r21, 0xBB66FF          ; purple text
LDI r17, 0x1A1A2E
DRAWTEXT r18, r19, r20, r21, r17
; Show behavior
LDI r20, 0x5050
STRO r20, "(spectral)"
LDI r18, 100
LDI r19, 128
LDI r21, 0xBB66FF
LDI r17, 0x1A1A2E
DRAWTEXT r18, r19, r20, r21, r17
JMP no_entity_tooltip

ent_tooltip_program:
LDI r20, 0x5040
STRO r20, "Program"
LDI r18, 100
LDI r19, 124
LDI r21, 0x00FFFF          ; cyan text
LDI r17, 0x1A1A2E
DRAWTEXT r18, r19, r20, r21, r17

no_entity_tooltip:


PSETI 126, 128, 0x4444FF
PSETI 127, 129, 0x4444FF
JMP legs_done
leg_frame_0:
PSETI 125, 129, 0x4444FF
PSETI 126, 128, 0x4444FF
PSETI 127, 128, 0x4444FF
legs_done:

; ===== Simple Player Marker (for zoom 0/1) =====
; Just a crosshair at screen center (128,128)
player_simple:
; Only draw if detail < 2 (otherwise full sprite was already drawn above)
LDI r18, 0x7814
LOAD r18, r18
LDI r17, 2
CMP r18, r17
BGE r0, player_marker_done   ; detail >= 2 → skip (full sprite already drawn)
; Flashing crosshair at center
LOAD r17, r13               ; frame_counter
LDI r18, 16
AND r17, r18
JZ r17, pm_white
LDI r17, 0xFFFF00
JMP pm_draw
pm_white:
LDI r17, 0xFFFFFF
pm_draw:
LDI r3, 128
LDI r4, 128
PSET r3, r4, r17            ; center dot
LDI r3, 127
PSET r3, r4, r17            ; left
LDI r3, 129
LDI r4, 128
PSET r3, r4, r17            ; right
LDI r3, 128
LDI r4, 127
PSET r3, r4, r17            ; up
LDI r4, 129
PSET r3, r4, r17            ; down
player_marker_done:

    FRAME
    JMP main_loop

; =========================================
; CMD_EXECUTE -- parse and run typed command
; CMD_BUF has the text (null terminated), CMD_LEN has the length.
; =========================================
cmd_execute:
PUSH r31
LDI r1, 1

; Null-terminate CMD_BUF
LDI r17, 0x7831
LOAD r18, r17
LDI r17, 0x7832
ADD r17, r18
LDI r19, 0
STORE r17, r19

; Check first char
LDI r17, 0x7832
LOAD r18, r17
CMPI r18, 47               ; '/'
JNZ r0, cmd_oracle          ; not a / command → send to Oracle

; --- Parse / commands ---
ADDI r17, 1                  ; skip '/'
LOAD r18, r17

; /tp X Y -- teleport player
CMPI r18, 116               ; 't'
JNZ r0, cmd_try_build
ADDI r17, 1
LOAD r18, r17
CMPI r18, 112               ; 'p'
JNZ r0, cmd_oracle          ; not /tp, fall through to oracle

; Parse first number (x) after /tp
ADDI r17, 1                  ; skip 'p'
CALL parse_next_number       ; r0 = number, r17 advanced past it
LDI r19, 0x7808
STORE r19, r0                ; player_x = parsed x

; Parse second number (y)
CALL parse_next_number
LDI r19, 0x7809
STORE r19, r0                ; player_y = parsed y

; Show confirmation in Oracle response buffer
LDI r17, 0x7872
LDI r18, 1
STORE r17, r18               ; ORACLE_RESP_READY = 1
LDI r17, 0x7873
STRO r17, "Teleported!"
JMP cmd_exec_done

cmd_try_build:
; /build NAME -- add building at player position
; Check 'b'
LDI r17, 0x7832
ADDI r17, 1                  ; skip '/'
LOAD r18, r17
CMPI r18, 98                ; 'b'
JNZ r0, cmd_oracle
ADDI r17, 1
LOAD r18, r17
CMPI r18, 117               ; 'u'
JNZ r0, cmd_oracle
ADDI r17, 1
LOAD r18, r17
CMPI r18, 105               ; 'i'
JNZ r0, cmd_oracle
ADDI r17, 1
LOAD r18, r17
CMPI r18, 108               ; 'l'
JNZ r0, cmd_oracle
ADDI r17, 1
LOAD r18, r17
CMPI r18, 100               ; 'd'
JNZ r0, cmd_oracle

; It's /build. Skip space, copy name from CMD_BUF+7 to name slot.
; Find next building slot: building_count at 0x7580
LDI r17, 0x7580
LOAD r18, r17                ; r18 = current count
CMPI r18, 32
BGE r0, cmd_build_full       ; max 32 buildings

; New building at index = count
MOV r19, r18                 ; r19 = index
LDI r20, 4
MUL r19, r20                 ; r19 = index * 4
LDI r20, 0x7500
ADD r20, r19                 ; r20 = building base address

; world_x = player_x
LDI r17, 0x7808
LOAD r18, r17
STORE r20, r18
ADDI r20, 1

; world_y = player_y
LDI r17, 0x7809
LOAD r18, r17
STORE r20, r18
ADDI r20, 1

; type_color = cyan (AI building)
LDI r17, 0x00FFFF
STORE r20, r17
ADDI r20, 1

; name_addr = 0x76D0 + index*16
LDI r17, 0x7580
LOAD r18, r17
LDI r19, 16
MUL r18, r19
LDI r17, 0x76D0
ADD r17, r18                 ; r17 = name_addr
STORE r20, r17               ; building[3] = name_addr

; Copy name from CMD_BUF+7 (skip "/build ") to name_addr
LDI r20, 0x7839              ; CMD_BUF + 7 (past "/build ")
cmd_build_copy:
LOAD r18, r20
JZ r18, cmd_build_copy_done
STORE r17, r18
ADDI r20, 1
ADDI r17, 1
JMP cmd_build_copy
cmd_build_copy_done:
; Null terminate name
LDI r18, 0
STORE r17, r18

; Increment building count
LDI r17, 0x7580
LOAD r18, r17
ADDI r18, 1
STORE r17, r18

; Show confirmation
LDI r17, 0x7872
LDI r18, 1
STORE r17, r18
LDI r17, 0x7873
STRO r17, "Building added!"
JMP cmd_exec_done

cmd_build_full:
LDI r17, 0x7872
LDI r18, 1
STORE r17, r18
LDI r17, 0x7873
STRO r17, "Max 32 buildings!"
JMP cmd_exec_done

cmd_oracle:
; Not a recognized / command, or plain text → send to Oracle LLM.
; Copy CMD_BUF to ORACLE prompt area (reuse CMD_BUF as prompt text).
; Set RAM[0x7820] = 0 (Oracle mode, not asm_dev)
LDI r17, 0x7820
LDI r18, 0
STORE r17, r18

; LLM call: prompt from CMD_BUF (0x7832), response to ORACLE_RESP_BUF (0x7873)
; The build_llm_system_prompt() on Rust side will read player pos/buildings
; and prepend the Oracle system prompt.
LDI r3, 0x7832              ; prompt = CMD_BUF text
LDI r4, 0x7873              ; response buffer
LDI r5, 895                 ; max response length
LLM r3, r4, r5

; Mark response as ready
LDI r17, 0x7872
LDI r18, 1
STORE r17, r18

cmd_exec_done:
POP r31
RET

; =========================================
; PARSE_NEXT_NUMBER -- parse decimal number from string
; Input: r17 = pointer to string (position in CMD_BUF)
; Output: r0 = parsed number, r17 = advanced past number + space
; Uses r2 as accumulator (r0 is clobbered by CMPI!)
; =========================================
parse_next_number:
PUSH r31
LDI r1, 1

; Skip spaces
pnn_skip:
LOAD r18, r17
CMPI r18, 32                ; space
JNZ r0, pnn_digit_start
ADDI r17, 1
JMP pnn_skip

pnn_digit_start:
LDI r2, 0                   ; accumulator in r2 (NOT r0, CMPI clobbers r0)

pnn_loop:
LOAD r18, r17
JZ r18, pnn_done            ; null terminator
CMPI r18, 32                ; space = end of number
JZ r0, pnn_done
CMPI r18, 48                ; '0'
BLT r0, pnn_done
CMPI r18, 58                ; '9'+1
BGE r0, pnn_done

; r2 = r2 * 10 + (char - '0')
LDI r19, 10
MUL r2, r19
SUBI r18, 48                ; char - '0'
ADD r2, r18
ADDI r17, 1
JMP pnn_loop

pnn_done:
MOV r0, r2                  ; move result to r0 for caller
POP r31
RET

; ===== check_biome_walkable subroutine =====
; Input: r3 = world_x, r4 = world_y
; Output: r0 = 1 (walkable) or 0 (blocked)
; Preserves: r1-r6, r14-r15
; Uses: r17-r22 as temporaries
; Clobbers: r0 (CMP result), r31 (saved/restored via push/pop)

check_biome_walkable:
PUSH r31

; Compute coarse hash for biome (same as main terrain)
MOV r21, r3
MOV r22, r4
LDI r17, 3
SHR r21, r17           ; world_x >> 3
SHR r22, r17           ; world_y >> 3
LDI r17, 99001
MUL r21, r17           ; x_hash
LDI r17, 79007
MUL r22, r17           ; y_hash
XOR r21, r22           ; coarse_hash
LDI r17, 1103515245
MUL r21, r17           ; mixed_hash
LDI r17, 27
SHR r21, r17           ; biome_type (0..31)

; Check walkability: blocked if water(0-1), mountain(13-14), lava(16-17), volcanic(18), coral(22)
; Water: biome < 2
LDI r17, 2
CMP r21, r17
BLT r0, biome_blocked

; Mountain: biome 13-14 (13 <= biome < 15)
LDI r17, 13
CMP r21, r17
BLT r0, biome_not_mt
LDI r17, 15
CMP r21, r17
BGE r0, biome_not_mt
JMP biome_blocked

biome_not_mt:
; Lava/volcanic: biome 16-18 (16 <= biome <= 18)
LDI r17, 16
CMP r21, r17
BLT r0, biome_not_lava
LDI r17, 19
CMP r21, r17
BGE r0, biome_not_lava
JMP biome_blocked

biome_not_lava:
; Coral: biome == 22
LDI r17, 22
CMP r21, r17
JZ r0, biome_blocked

; Walkable
LDI r17, 1
MOV r0, r17
JMP biome_check_done

biome_blocked:
LDI r17, 0
MOV r0, r17

biome_check_done:
POP r31
RET