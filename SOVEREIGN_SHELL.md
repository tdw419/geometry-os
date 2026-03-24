# Sovereign Shell - Natural Language Control for Geometry OS

## Overview

The Sovereign Shell enables natural language control of the Geometry OS VM. Users type commands in plain English, and the system automatically converts them to VM opcodes, executes them, and displays results in a visual HUD.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                 SOVEREIGN SHELL ARCHITECTURE                │
├─────────────────────────────────────────────────────────────┤
│  Row 0-399:    Agent execution space                        │
│  Row 400-449:  HUD zone (registers, messages)               │
│  Row 450-479:  INPUT ZONE (user types here)                 │
│  Row 475-479:  PATCH STATUS (success/fail display)          │
├─────────────────────────────────────────────────────────────┤
│  FLOW:                                                      │
│  1. User types command in input zone                        │
│  2. Agent hits @> (PROMPT opcode)                           │
│  3. Vision reads input zone                                 │
│  4. LLM generates opcode patch                              │
│  5. Host injects patch into agent                           │
│  6. HUD shows PATCH_SUCCESS                                 │
│  7. Agent resumes with new opcodes                          │
└─────────────────────────────────────────────────────────────┘
```

## Components

### 1. Text Input Region (Rows 450-479)

- 5x7 bitmap font rendering in WGSL
- Keyboard input from host (injected into framebuffer)
- Visual cursor blink (frame-based animation)
- Green prompt indicator (`>`)

### 2. PROMPT Opcode (`@>`)

New opcode that:
- Halts the current VM thread
- Triggers Vision model to read the text input region
- Vision extracts user intent (natural language command)
- LLM generates opcode patch

### 3. Live Patch Pattern

- LLM returns patched opcodes as text
- Host injects patch into agent's instruction stream
- "Patch-and-Copy" logic from existing code
- Patch applied atomically

### 4. Visual Verification

After patch, HUD displays:
- **PATCH_SUCCESS** (green) - Code validated and executed
- **PATCH_FAIL** (red) - Code validation failed
- **READY** (gray) - Waiting for input

### 5. HUD Display

**Row 400-449:**
- "SOVEREIGN SHELL" header
- Register values (A-J)
- IP (instruction pointer)
- SP (stack pointer)
- Execution result

**Row 450-474:**
- Input zone with prompt
- User's typed text
- Blinking cursor

**Row 475-479:**
- Patch status indicator
- Color-coded feedback

## Usage

### Interactive Mode

```bash
cd ~/zion/projects/ascii_world/gpu
cargo run --bin sovereign-shell
```

### Test Mode

```bash
cargo run --bin sovereign-shell -- --test
```

### Example Commands

| Natural Language | Generated Opcodes | Result |
|------------------|-------------------|--------|
| `add 5 and 3` | `5 3 + @` | 8 |
| `multiply 4 by 7` | `4 7 * @` | 28 |
| `subtract 10 from 20` | `20 10 - @` | 10 |
| `divide 100 by 5` | `100 5 / @` | 20 |
| `store 42 in register A` | `42 a @` | A=42 |

## Models

- **Vision Model**: `qwen/qwen3-vl-8b` - Reads input zone and extracts text
- **Text Model**: `tinyllama-1.1b-chat-v1.0` - Generates opcodes from natural language

Both models run locally via LM Studio on port 1234.

## Files

```
sovereign_shell_hud.wgsl    - WGSL shader with input zone, HUD, and status display
src/bin/sovereign_shell.rs  - Main shell runner with VM, LLM integration
SOVEREIGN_SHELL.md          - This documentation
```

## VM Opcodes

| Opcode | Description |
|--------|-------------|
| `N` | Push number N onto stack |
| `+` | Add: pop 2, push sum |
| `-` | Subtract: pop 2, push difference |
| `*` | Multiply: pop 2, push product |
| `/` | Divide: pop 2, push quotient |
| `.` | Print/pop top of stack |
| `a-z` | Store top of stack in register |
| `A-Z` | Load register onto stack |
| `@` | Halt execution |
| `@>` | PROMPT: halt and wait for natural language input |

## Performance Targets

- Full loop (input → vision → patch → execute): <2 seconds
- Vision reading: <1 second
- LLM generation: <1 second
- HUD rendering: <50ms (GPU-accelerated)

## Future Enhancements

1. **Loop constructs**: "count from 1 to 5" → `1 5 loop @`
2. **Conditional execution**: "if A > 0 then..."
3. **Multi-threaded agents**: Spawn parallel agents for complex tasks
4. **Self-modifying code**: Agents can patch their own code
5. **Visual feedback**: Execution traces in the framebuffer

## Troubleshooting

### Vision not reading input

- Ensure LM Studio is running on port 1234
- Check that `qwen/qwen3-vl-8b` model is loaded
- Verify output image exists at `output/sovereign_shell.png`

### LLM generating invalid code

- Check LM Studio console for errors
- Ensure `tinyllama-1.1b-chat-v1.0` model is loaded
- Try simpler commands first

### HUD not rendering

- Verify GPU drivers are installed
- Check wgpu compatibility
- Run with `RUST_LOG=debug` for detailed logs

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                     USER INPUT                              │
│                   "add 5 and 3"                             │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                   INPUT ZONE (GPU)                          │
│  Row 450-479: Text rendering, cursor blink                  │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│              VISION MODEL (qwen3-vl-8b)                     │
│  Reads framebuffer → extracts "add 5 and 3"                 │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│              TEXT LLM (tinyllama)                           │
│  "add 5 and 3" → "5 3 + @"                                  │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                   PATCH ENGINE                              │
│  Validates code, injects into VM                            │
│  Status: PATCH_SUCCESS (green) or PATCH_FAIL (red)          │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                    VM EXECUTOR                              │
│  Executes: 5 3 + @                                          │
│  Result: 8                                                  │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                    HUD UPDATE                               │
│  Display result in register view                            │
│  Execution result shown                                     │
└─────────────────────────────────────────────────────────────┘
```

## Credits

Built as part of the Geometry OS project - exploring GPU-native computation and visual proprioception.

- WGSL HUD rendering based on `gpu_native_hud.wgsl`
- VM execution adapted from `agent_messaging.rs`
- Vision integration from `visual_register_hud.rs`
