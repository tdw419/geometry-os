# Spatial Physics & Geometry Opcodes

Phase 5 of the Geometry OS introduces **spatial registers** and **geometry opcodes** for position, velocity, and collision detection.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    SPATIAL PHYSICS                          │
├─────────────────────────────────────────────────────────────┤
│  Each thread has independent spatial state:                 │
│                                                             │
│    POS_X (u32) ─── Current X position (0-639)              │
│    POS_Y (u32) ─── Current Y position (0-479)              │
│    VEL_X (i32) ─── X velocity vector (signed)              │
│    VEL_Y (i32) ─── Y velocity vector (signed)              │
│                                                             │
│  Trail: Last N positions for visual effect                  │
│  Collision: Flag set when hitting boundary or pixel         │
└─────────────────────────────────────────────────────────────┘
```

## Spatial Registers

| Register | Type | Description |
|----------|------|-------------|
| `POS_X`  | u32  | Current X position in framebuffer |
| `POS_Y`  | u32  | Current Y position in framebuffer |
| `VEL_X`  | i32  | X velocity vector (can be negative) |
| `VEL_Y`  | i32  | Y velocity vector (can be negative) |

Each thread maintains its own spatial state. Boundary clamping ensures positions stay within valid range (10 to WIDTH-10, 10 to HEIGHT-10).

## Geometry Opcodes

### POS Opcode (`p`)

Push current position onto stack.

```
p           ; Push POS_Y, then POS_X onto stack
            ; Stack: [... POS_Y, POS_X]

p x         ; Set POS_X from stack top
p y         ; Set POS_Y from stack top
```

**Examples:**
```
p . .       ; Print both x and y coordinates
320 p x !   ; Set POS_X = 320
240 p y !   ; Set POS_Y = 240
```

### MOVE Opcode (`>`)

Move by delta values or by current velocity.

```
dx dy >     ; Move by dx, dy (pops from stack)
>>          ; Move by current velocity (VEL_X, VEL_Y)
```

**Boundary Clamping:**
- X: clamped to [10, 630]
- Y: clamped to [10, 470]

**Examples:**
```
10 5 >      ; Move right 10, down 5
>>          ; Move by VEL_X, VEL_Y
-5 3 >      ; Move left 5, down 3
```

### SENSE Opcode (`x`)

Read pixel at current position from output framebuffer.

```
x           ; Returns 0 if black/empty, 1 if occupied
```

This enables **collision detection** - agents can sense their environment.

**Example:**
```
x .         ; Print whether current pixel is occupied
x 1 = :collision  ; Jump to :collision if pixel occupied
```

### PUNCH Opcode (`!`)

Write pixel at current position to output framebuffer.

```
value !     ; Write value at (POS_X, POS_Y)
!           ; Write stack top at current position
```

This enables agents to **draw** on the grid.

**Example:**
```
255 !       ; Punch white pixel
0 !         ; Punch black pixel (erase)
```

## Bouncing Agent Demo

The demo program shows a pixel agent that:
1. Starts at center (320, 240)
2. Moves with velocity (2, 2)
3. Bounces off screen boundaries
4. Leaves a visible trail

```
; Bouncing Pixel Agent
; Demonstrates spatial physics opcodes

:init
320 p x !    ; Set POS_X = 320
240 p y !    ; Set POS_Y = 240
2 v x !      ; VEL_X = 2
2 v y !      ; VEL_Y = 2

:loop
>>           ; Move by velocity
255 !        ; Punch white pixel

; Check boundaries
:check_x
POS_X 630 > :hit_x
POS_X 10 < :hit_x

:check_y
POS_Y 470 > :hit_y
POS_Y 10 < :hit_y

@ :loop

:hit_x
VEL_X -1 * v x !  ; Reverse X velocity
@ :loop

:hit_y
VEL_Y -1 * v y !  ; Reverse Y velocity
@ :loop
```

## HUD Display

The spatial physics HUD shows:

```
┌────────────────────────────────────────────────────────┐
│ SPATIAL PHYSICS                                        │
│                                                        │
│ POS: 320, 240        TRAIL: 50                        │
│ VEL: +2, +2          FRAME: 60                        │
│                     COLLISION                          │
├────────────────────────────────────────────────────────┤
│                                                        │
│  ┌──────────────────────────────────────────────┐     │
│  │                                              │     │
│  │      ·  ·  ·  ·  ·  ○ →                     │     │
│  │    trail         agent + velocity           │     │
│  │                                              │     │
│  └──────────────────────────────────────────────┘     │
│                                                        │
├────────────────────────────────────────────────────────┤
│ p POS  > MOVE  x SENSE  ! PUNCH                       │
└────────────────────────────────────────────────────────┘
```

### HUD Elements

- **POS**: Current (x, y) position in white
- **VEL**: Velocity (dx, dy) in green
- **TRAIL**: Number of trail points
- **FRAME**: Current frame number
- **COLLISION**: Red indicator when collision detected
- **Agent**: White 3x3 pixel (red on collision)
- **Velocity Vector**: Green line showing direction
- **Trail**: Cyan dots fading with age

## Implementation Details

### Boundary Clamping

```rust
fn clamp_position(&mut self) {
    self.spatial.pos_x = self.spatial.pos_x.clamp(10, WIDTH - 10);
    self.spatial.pos_y = self.spatial.pos_y.clamp(10, HEIGHT - 10);
}
```

### Trail Management

```rust
// Add to trail
self.spatial.trail.push((self.spatial.pos_x, self.spatial.pos_y));
if self.spatial.trail.len() > TRAIL_LENGTH {
    self.spatial.trail.remove(0);
}
```

### Collision Detection

```rust
// SENSE opcode
let pixel = output_buffer[(pos_y * WIDTH + pos_x) as usize];
let occupied = if pixel > 0 { 1 } else { 0 };
stack.push(occupied);
collision = occupied > 0;
```

## Performance

- **Target**: 60 FPS
- **Render Time**: ~1-5ms on RTX 5090
- **Trail Length**: 50 points (configurable)
- **Max Agents**: 8 simultaneous

## Files

| File | Purpose |
|------|---------|
| `src/bin/spatial_physics.rs` | Physics runner with VM |
| `spatial_physics_hud.wgsl` | GPU shader for HUD |
| `SPATIAL_PHYSICS.md` | This documentation |

## Running the Demo

```bash
cd ~/zion/projects/ascii_world/gpu
cargo run --bin spatial-physics --release
```

Output: `output/spatial_physics.png`

## Success Criteria

- [x] Agent starts at center (320, 240)
- [x] Moves with velocity (2, 2)
- [x] Bounces off screen boundaries
- [x] Leaves visible trail
- [x] HUD shows POS and VEL
- [x] Full demo runs at 60 FPS

## Next Steps

1. **Multi-Agent Collision**: Agents sensing each other
2. **Pixel Traps**: Detect and escape stuck states
3. **Vision Debug**: Use qwen3-vl-8b to verify physics
4. **Interactive Control**: Natural language commands for agents

## License

Part of the Geometry OS project.
