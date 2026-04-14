# Infinite Map -- Design Document

## Overview

A fully procedural infinite scrolling terrain map, implemented entirely in Geometry OS assembly. Arrow keys / WASD scroll through terrain that is generated on-the-fly from a deterministic hash function. No Rust changes needed -- pure "pixels driving pixels."

## Status: WORKING

- Assembled size: 398 words
- Frame budget: 127,328 / 1,000,000 instructions (12.7% utilization)
- All 65,536 screen pixels rendered per frame
- Diagonal movement works (simultaneous keys)
- 1011 tests passing (including new infinite_map test)

## Architecture

### Render Target: SCREEN (not canvas)

The viewport renders to the 256x256 screen buffer (address 0x10000+) via RECTF opcode. The canvas is reserved for source text editing.

### Tile Size: 4x4 pixels

64x64 tiles cover the full 256x256 screen. At 4 pixels per tile, the world has clear tile boundaries that give it a retro feel while keeping the instruction count manageable.

### World Model: Pure Function (No Storage)

The world is a mathematical function: `terrain_type(x, y) = ((x * 99001) XOR (y * 79007)) >> 28`

This maps every (x,y) coordinate to a deterministic value 0-15. No world data is stored in RAM. The world is truly infinite -- coordinates can range from 0 to ~4 billion before u32 wrapping.

### Memory Layout

```
RAM[0x7800] = camera_x (u32)
RAM[0x7801] = camera_y (u32)
RAM[0xFFB]  = key bitmask (host writes: bit0=up, bit1=down, bit2=left, bit3=right)
```

### Biome Distribution

```
Types 0-2:   Water (deep/mid/shallow blues)
Type 3:      Beach (sand)
Types 4-6:   Grass (light/medium/dark greens)
Types 7-8:   Forest (greens)
Type 9:      Hills (gray-green)
Types 10-11: Mountain (grays)
Types 12-15: Snow/ice/peak (whites)
```

### Register Allocation

```
r1   = tile row (ty, 0..63)
r2   = tile column (tx, 0..63)
r3,r4 = scratch (world coords, screen coords)
r5,r6 = hash computation
r7   = constant 1
r8   = constant 64 (grid size)
r9   = constant 4 (tile size)
r10  = key bitmask port (0xFFB)
r11  = camera_x addr (0x7800)
r12  = camera_y addr (0x7801)
r14  = camera_x value
r15  = camera_y value
r16  = key bitmask value
r17  = current tile color
r18  = scratch / comparison value
```

## Performance Analysis

Per frame:
- Input processing: ~30 instructions
- Screen clear (FILL): 1 instruction
- Render loop (64x64 = 4096 tiles):
  - Per tile: hash (8 ops) + color lookup (5-15 ops) + RECTF (1 op) + loop (5 ops) = ~30 ops
  - Total: 4096 * 30 = ~123K instructions
- Frame yield: 1 instruction

**Total: ~127K instructions per frame (12.7% of 1M budget)**

This leaves ~873K instructions (87.3%) for features like:
- Entities / structures placed by hash
- Animated tiles (water shimmer)
- Minimap overlay
- Camera coordinate display (via TEXT opcode)
- ASMSELF terrain rule evolution

## Corrections from Initial Design

The initial design document had several wrong assumptions:

1. **"No XOR opcode" -- WRONG.** XOR exists as opcode 0x26. OR exists as 0x25. This makes hash functions trivial.
2. **STORE/LOAD syntax** is `STORE addr_reg, val_reg` and `LOAD val_reg, addr_reg` (load puts value first).
3. **CMP/Branch pattern** is `CMP rA, rB; BLT r0, label` -- branches always reference r0.
4. **FILL** takes 1 register arg, not an immediate.
5. **RECTF** takes 5 register args (x, y, w, h, color).

## Self-Modification Opportunities

### ASMSELF for Region Transitions

The program could detect when the camera crosses biome boundaries (e.g., enters a "volcanic region" at distance >500 from origin) and use ASMSELF to rewrite its own hash constants. Different world regions would literally run different terrain generation code.

### Where NOT to use ASMSELF

The per-frame render loop should never use ASMSELF -- the compilation overhead (~1000 instructions) would be wasted every frame. ASMSELF is for structural changes on boundary transitions.

## FORMULA: Not Useful

FORMULA operates on canvas_buffer indices (0-4095). The infinite map renders to screen buffer (0x10000+). The two address spaces don't overlap. FORMULA would only be useful for a HUD drawn on the canvas while the main render goes to screen.

## Future Enhancements

### v2: Better Terrain
- Two-level hash: `biome = hash(x>>3, y>>3)`, `variation = hash(x, y)` 
- This creates smooth biome regions instead of per-tile noise
- Biome boundaries at 8x8 tile clusters

### v3: Entities
- `entity_present = hash(x*13, y*17) >> 30 == 3` (1 in 4 chance)
- Entity type from another hash: tree, rock, house, chest
- Render as different colored pixels within the tile

### v4: Smooth Scrolling
- Sub-tile camera position (camera_x/y as fixed-point)
- Offset rendering: first/last column rendered partially
- Requires pixel-level STORE instead of RECTF for edge tiles

### v5: Minimap
- Top-right corner shows 32x32 downsampled view of nearby terrain
- Uses STORE to write single pixels in a reserved screen area
- ~1024 STORE instructions = negligible cost
