# Agent Messaging Protocol - SEND/RECV Opcodes

## Overview

The Agent Messaging system enables inter-thread communication in the Geometry OS using mailbox-based message passing. Threads can send and receive values asynchronously through dedicated mailbox slots.

## Architecture

### Thread Layout
- **Thread 0**: Rows 0-99, mailbox at rows 90-99
- **Thread 1**: Rows 100-199, mailbox at rows 190-199
- **Thread N**: Rows N*100 to N*100+99, mailbox at N*100+90 to N*100+99
- **Maximum Threads**: 8
- **Mailbox Size**: 10 slots per thread

### Shared Memory
- Global mailbox array accessible by all threads
- Atomic operations for thread-safe message passing
- Non-blocking SEND and RECV operations

## Opcodes

### SEND Opcode (`!`)

**Format**: `target_thread row_offset value !`

**Stack Operation** (bottom to top):
1. Push `target_thread` (thread ID to send to)
2. Push `row_offset` (reserved for future use)
3. Push `value` (value to send)
4. Execute `!`

**Behavior**:
- Non-blocking: returns immediately
- Finds first empty slot in target thread's mailbox
- Sets "message waiting" flag on target thread
- Returns `true` on success, `false` if mailbox full

**Example**:
```
42 a          # Store 42 in register A
1 90 A !      # Send A (42) to Thread 1
```

### RECV Opcode (`?`)

**Format**: `?`

**Stack Operation**:
- Pushes received value onto stack (or 0 if mailbox empty)

**Behavior**:
- Checks mailbox for messages
- Returns first available message
- Clears "message waiting" flag if no more messages
- Non-blocking: returns 0 if mailbox empty

**Example**:
```
? b           # Receive into register B
```

## Example Program

```
# Thread 0: Store 42, spawn Thread 1, send value
42 a $ 1 0 42 ! @ ?

# Thread 1: Receives value (runs after spawn)
```

**Execution Flow**:
1. Thread 0 stores 42 in register A
2. Thread 0 spawns Thread 1 (clones state)
3. Thread 0 sends 42 to Thread 1's mailbox[0]
4. Thread 0 halts (@)
5. Thread 1 receives 42 from mailbox

## Visual HUD

The HUD displays for each active thread:
- **Thread ID** (color-coded)
- **Registers A, B, C** (first three)
- **IP** (instruction pointer)
- **SP** (stack pointer)
- **SEND** (last value sent)
- **RECV** (last value received)
- **MSG!** indicator (if messages waiting)
- **Mailbox** contents (first 5 slots)

## Implementation Details

### Rust Side (agent_messaging.rs)
- `SharedMemory` struct with atomic mailboxes
- `send()` method uses compare-and-swap for atomic insertion
- `recv()` method atomically reads and clears slots
- Message waiting flags for quick polling

### Shader Side (agent_messaging_hud.wgsl)
- `ThreadState` struct includes mailbox array
- `render_thread_hud()` draws messaging status
- Color coding: green=send, orange=receive, red=message waiting

## Running

```bash
cd ~/zion/projects/ascii_world/gpu
cargo run --release --bin agent-messaging
```

**Output**: `output/agent_messaging.png`

## Thread Safety

All mailbox operations use atomic operations:
- `AtomicU32` for mailbox slots
- `compare_exchange` for SEND (lock-free insertion)
- ` Ordering::SeqCst` for strong consistency

## Future Enhancements

1. **Blocking RECV**: Wait for message if mailbox empty
2. **Broadcast**: Send to all threads
3. **Channel Types**: Different priority channels
4. **Message Types**: Tagged messages with metadata

## Verification

Use vision model to verify:
- Thread 0 shows `SEND:42`
- Thread 1 shows `RECV:42`
- Mailbox displays show correct values
- Message waiting indicators function correctly
