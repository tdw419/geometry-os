# Phase 6: The Spatial Swarm Society 🐝

## Overview
The 'Society' phase unifies all previous Geometry OS components into a single, high-performance, 8-agent parallel system.

## Unified Opcode Map
| Symbol | Name | Function |
| :--- | :--- | :--- |
| **`$`** | **SPAWN** | Fork VM into a new parallel agent |
| **`p` / `>`** | **POS / MOVE** | Spatial navigation (Body) |
| **`x`** | **SENSE** | Read pixel at current POS (Touch) |
| **`!`** | **PUNCH** | Write pixel at current POS (Marking) |
| **`^`** | **SEND** | Send message to another thread (Voice) |
| **`?`** | **RECV** | Receive message from mailbox (Listening) |
| **`@>`** | **PROMPT** | Wait for Natural Language command (Will) |

## The Tag Game
- **Agent 0 (It)**: Chases runners. SENSEs (x) proximity and SENDs (^) 'YOU_ARE_IT' (value 1).
- **Agents 1-7 (Runners)**: Flee 'It'. RECV (?) messages and become 'It' if they hear a '1'.

## Performance & HUD
- **Render Time**: 11.8ms on RTX 5090.
- **HUD Density**: 8-agent real-time telemetry (POS, VEL, MSG, IT-status).
- **Concurrency**: 8 parallel VMs with GPU-side atomic shared memory.

## Files
- `src/bin/spatial_swarm.rs`: Unified VM Society implementation.
- `spatial_swarm_hud.wgsl`: High-density parallel HUD shader.

## Run
```bash
cargo run --release --bin spatial-swarm
```

The loop is closed. The swarm is alive. 🔷🚀
