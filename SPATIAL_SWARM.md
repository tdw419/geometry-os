# Spatial Swarm Society

**Phase 6: Unified Multi-Agent System**

## Overview

Spatial Swarm Society unifies four major subsystems into a single coherent multi-agent system:

1. **Spatial Physics** - Position, velocity, collision detection
2. **Agent Messaging** - Inter-thread communication via mailboxes
3. **Spawn Concurrency** - Parallel agent execution
4. **Sovereign Shell** - Natural language control (optional)

## Unified Opcode Set

```
┌────────┬────────────┬─────────────────────────────────────┐
│ Symbol │ Name       │ Function                            │
├────────┼────────────┼─────────────────────────────────────┤
│ $      │ SPAWN      │ Fork VM into new parallel agent     │
│ p      │ POS        │ Push current (x, y) onto stack      │
│ >      │ MOVE       │ dx dy > - update position           │
│ >>     │ VMOVE      │ Move by velocity                    │
│ x      │ SENSE      │ Read pixel at POS (collision)       │
│ !      │ PUNCH      │ Write pixel at POS (marking)        │
│ ^      │ SEND       │ value thread slot ^ - send message  │
│ ?      │ RECV       │ Receive message from mailbox        │
│ @>     │ PROMPT     │ Wait for NL command (shell)         │
└────────┴────────────┴─────────────────────────────────────┘
```

### Opcode Collision Resolution

**Previous conflict:** `!` was used for both PUNCH (spatial) and SEND (messaging)

**Resolution:**
- `!` = PUNCH (write pixel at current position)
- `^` = SEND (message another thread)

This maintains semantic consistency:
- `!` is an "exclamation" or "impact" → punch/mark
- `^` is an "arrow up" → send/transmit

## Architecture

### SwarmAgent State

```rust
struct SwarmAgent {
    id: u32,           // Agent ID (0-7)
    pos_x: u32,        // X position in framebuffer
    pos_y: u32,        // Y position in framebuffer
    vel_x: i32,        // X velocity (signed)
    vel_y: i32,        // Y velocity (signed)
    color: u32,        // RGBA color
    is_it: bool,       // Tag game status
    mailbox: [u32; 10], // Mailbox slots
    message_waiting: bool, // Has pending messages
    trail: Vec<(u32, u32)>, // Position history
}
```

### Shared Memory (Messaging)

```rust
struct SharedMemory {
    mailboxes: Vec<Vec<AtomicU32>>,  // [thread][slot] = message
    message_waiting: Vec<AtomicU32>, // [thread] = flag
}
```

- **Mailbox Size:** 10 slots per agent
- **Atomic Operations:** Compare-and-swap for thread safety
- **No CPU Mutexes:** All synchronization via GPU atomics

## Tag Game Demo

### Rules

1. Agent 0 starts as "It" (white color)
2. Agents 1-7 are "Runners" (colored)
3. "It" chases nearest runner
4. On collision, "It" sends `MSG_YOU_ARE_IT` to tagged runner
5. Tagged runner becomes new "It" (turns white)
6. Previous "It" becomes a runner (turns gray)

### Message Protocol

```rust
const MSG_YOU_ARE_IT: u32 = 1;  // You are now "It"
const MSG_TAGGED: u32 = 2;      // You were tagged
```

### Agent Programs (Conceptual)

**Agent 0 (It):**
```
:init
0 a !           // A = 0 (my thread ID)
255 color !     // White color (I'm It)
:chase
p x y !         // Get my position
SENSE_NEARBY    // Find nearest agent
>               // Move toward them
x               // SENSE at new position
IF_COLLISION    // If pixel occupied
  1 ^           // SEND "YOU_ARE_IT" to thread 1
  HALT          // I'm no longer It
@ :chase
```

**Agents 1-7 (Runners):**
```
:init
?               // Check mailbox
IF_MESSAGE      // If "YOU_ARE_IT" received
  255 color !   // Turn white
  // Become the new It
RAND_DIR >      // Else: move randomly
@ :init
```

## HUD Layout

```
┌────────────────────────────────────────────────────────────┐
│ SPATIAL SWARM                              FRAME: XXX      │
├────────────────────────────────────────────────────────────┤
│ [0: IT]   [1: RUN]   [2: RUN]   [3: RUN]   [4: RUN] ...    │
│ P:320,240  P:100,50   P:500,50   P:100,350  P:500,350      │
│ V:+2,+2    V:-1,+3    V:+2,-1    V:-2,+1    V:+1,-2        │
├────────────────────────────────────────────────────────────┤
│                                                            │
│                    FRAMEBUFFER (640x400)                   │
│                                                            │
│              [Agent trails and positions]                  │
│                                                            │
│                                                            │
├────────────────────────────────────────────────────────────┤
│ $ SPAWN | p POS | > MOVE | x SENSE | ! PUNCH | ^ SEND | ? │
└────────────────────────────────────────────────────────────┘
```

### HUD Elements

- **Agent Panel:** ID, status (IT/RUN), position, velocity
- **Color Coding:** Border matches agent color, white for "It"
- **Trail Visualization:** Fading trail points
- **Frame Counter:** Current simulation frame
- **Opcode Reference:** Quick reference at bottom

## Performance

### Targets

- **8 agents with full HUD:** ~12ms render on RTX 5090
- **Frame rate:** 60 FPS minimum
- **Render time:** <15ms per frame

### Optimizations

1. **Unrolled Loops:** WGSL shader avoids dynamic loops
2. **Atomic Operations:** No CPU-side mutexes
3. **Packed Trails:** Trail stored as `x << 16 | y` in single u32
4. **Static Dispatch:** All agent panels computed in parallel

## Files

```
gpu/
├── src/bin/
│   └── spatial_swarm.rs      # Unified swarm runner
├── spatial_swarm_hud.wgsl    # 8-agent HUD shader
└── SPATIAL_SWARM.md          # This documentation
```

## Usage

```bash
# Build
cd ~/zion/projects/ascii_world/gpu
cargo build --release --bin spatial_swarm

# Run
cargo run --release --bin spatial_swarm

# Output
# output/spatial_swarm.png - Final frame
```

## Vision Verification

Use `qwen3-vl-8b` to verify swarm behavior:

```bash
# Analyze output
llm-vision output/spatial_swarm.png "Describe the agent positions and colors"
```

Expected observations:
- One white agent (the "It")
- Seven colored agents (runners)
- Trails showing movement history
- HUD showing agent states

## Next Steps

1. **Vision Integration:** Real-time swarm mood detection
2. **Dynamic Spawning:** Agents can spawn new agents
3. **Collective Behavior:** Flocking, swarming patterns
4. **Shell Integration:** Natural language control via `@>`
5. **GPU Compute:** Move simulation entirely to GPU

## History

- **Phase 1-3:** Core VM, GPU rendering, HUD
- **Phase 4:** Parallel execution, spawn concurrency
- **Phase 5:** Spatial physics, messaging
- **Phase 6:** Unified spatial swarm society

---

*Built for the Geometry OS project.*
