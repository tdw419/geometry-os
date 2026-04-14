# Infinite Map -- Design Document

## Overview

A fully procedural infinite scrolling terrain map, implemented entirely in Geometry OS assembly. Arrow keys / WASD scroll through terrain that is generated on-the-fly from a deterministic hash function. No Rust changes needed -- pure "pixels driving pixels."

## Status: WORKING (v3)

- Assembled size: 528 words
- Frame budget: 225,486 / 1,000,000 instructions (22.5% utilization)
- All 65,536 screen pixels rendered per frame
- Diagonal movement works (simultaneous keys)
- Water tiles animate with diagonal wave pattern
- 1011 tests passing

## Architecture

### Render Target: SCREEN (not canvas)

The viewport renders to the 256x256 screen buffer (address 0x10000+) via RECTF opcode. The canvas is reserved for source text editing.

### Tile Size: 4x4 pixels

64x64 tiles cover the full 256x256 screen. At 4 pixels per tile, the world has clear tile boundaries that give it a retro feel while keeping the instruction count manageable.

### World Model: Pure Function (No Storage)

The world is a mathematical function with two levels:

**Coarse hash (biome zones):**
```
biome(cx, cy) = ((cx * 99001) XOR (cy * 79007)) >> 28
where cx = world_x >> 3, cy = world_y >> 3
```
This creates contiguous 8x8 tile biome zones (32x32 pixel blocks). Adjacent tiles within the same zone get the same biome type.

**Fine hash (structure placement):**
```
fine(x, y) = (x * 374761393) XOR (y * 668265263)
structure if fine(x,y) & 0xFF == 0x2A  (1 in 256 tiles, ~16 per screen)
```

**Water animation:**
```
shimmer = (frame_counter + world_x + world_y) & 0x1F
water_color = base_color + shimmer
```
Creates a diagonal wave pattern that moves across water tiles each frame.

### Memory Layout

```
RAM[0x7800] = camera_x (u32)
RAM[0x7801] = camera_y (u32)
RAM[0x7802] = frame_counter (increments each frame)
RAM[0xFFB]  = key bitmask (host writes: bit0=up, bit1=down, bit2=left, bit3=right)
```

### Biome Distribution

```
Types 0-2:   Water (deep/mid/shallow blues, ANIMATED)
Type 3:      Beach (sand)
Types 4-6:   Grass (light/medium/dark greens)
Types 7-8:   Forest (greens)
Type 9:      Hills (gray-green)
Types 10-11: Mountain (grays)
Types 12-15: Snow/ice/peak (whites)
```

### Structure Types (1/256 tiles)

```
On water biomes:    Wave crest (bright blue 0x0066CC)
On land biomes:     Tree trunk / hut (brown 0x884422)
On mountain biomes: Snow patch (pale 0xBBBBCC)
On snow biomes:     Ice crystal (blue-white 0xAABBEE)
```

### Register Allocation

```
r1   = tile row (ty, 0..63)
r2   = tile column (tx, 0..63)
r3,r4 = scratch (world coords, then screen coords)
r5   = biome type (0-15), then fine hash
r6   = hash temp
r7   = constant 1
r8   = constant 64 (grid size)
r9   = constant 4 (tile size)
r10  = key bitmask port (0xFFB)
r11  = camera_x addr (0x7800)
r12  = camera_y addr (0x7801)
r13  = frame_counter addr (0x7802)
r14  = camera_x value
r15  = camera_y value
r16  = key bitmask value
r17  = current tile color
r18  = scratch / comparison value
r19  = coarse hash temp
r20  = coarse hash temp
r21  = fine hash / structure check
r22  = frame_counter value (loaded once per frame)
```

## Performance Analysis

Per frame:
- Frame counter increment: 3 instructions
- Input processing: ~30 instructions
- Screen clear (FILL): 1 instruction
- Render loop (64x64 = 4096 tiles):
  - Per tile: coarse hash (10 ops) + fine hash (8 ops) + structure check (6 ops)
    + biome color lookup (5-15 ops) + water animation (4 ops for water)
    + RECTF (1 op) + loop (5 ops) = ~55 ops
  - Total: 4096 * 55 = ~225K instructions
- Frame yield: 1 instruction

**Total: ~225K instructions per frame (22.5% of 1M budget)**

Leaves ~775K instructions (77.5%) for future features.

## Corrections from Initial Design

The initial design document had several wrong assumptions:

1. **"No XOR opcode" -- WRONG.** XOR exists as opcode 0x26. OR exists as 0x25.
2. **STORE/LOAD syntax** is `STORE addr_reg, val_reg` and `LOAD val_reg, addr_reg`.
3. **CMP/Branch pattern** is `CMP rA, rB; BLT r0, label` -- branches always reference r0.
4. **FILL** takes 1 register arg, not an immediate.
5. **RECTF** takes 5 register args (x, y, w, h, color).

## Version History

### v1 (398 words, 127K steps)
- Per-tile independent hash, no biome smoothing
- 16 terrain types with sub-type colors
- Arrow/WASD scrolling

### v2 (495 words, 197K steps)
- Coarse hash (>> 3) for contiguous biome zones
- Fine hash for deterministic structure placement (1/256 tiles)
- Trees, wave crests, snow patches, ice crystals

### v3 (528 words, 225K steps)
- Frame counter at RAM[0x7802]
- Water tiles animate with diagonal wave pattern
- ~16K pixels change per frame due to water animation

## Future Enhancements

### v4: Smooth Scrolling
- Sub-tile camera position (camera_x/y as fixed-point)
- Offset rendering: first/last column rendered partially
- Requires pixel-level STORE instead of RECTF for edge tiles

### v5: Biome-Aware Structures
- Trees only on grass/forest (not beach/snow)
- Rocks only on mountain/hills
- Houses only on grass (rare)
- Fish jumping animation on water

### v6: Minimap
- Top-right corner shows 32x32 downsampled view of nearby terrain
- Uses STORE to write single pixels in a reserved screen area
- ~1024 STORE instructions = negligible cost

### v7: ASMSELF Region Evolution
- Detect when camera crosses distance thresholds from origin
- Use ASMSELF to rewrite hash constants for new "regions"
- Volcanic region, crystal caverns, ancient ruins -- different terrain code

### v8: Sound
- Different tone per biome when entering a new zone
- Use the BEEP opcode (if available) or TEXT-to-audio
