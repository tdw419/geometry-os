# LLM Oracle (Opcode 0x9C)

How the Geometry OS VM talks to a local LLM, giving programs spatial awareness
of the procedurally generated world.

---

## Overview

Any Geometry OS program can call the LLM via opcode 0x9C. The Rust host
intercepts the call, enriches the prompt with live map state (player position,
biome, nearby buildings), and routes it through model_choice to Ollama.

Programs that use this: oracle, smart_term, thinking_creature, and any user
program that calls 0x9C.

---

## How It Works

### Opcode Interface

```
0x9C  prompt_reg  response_reg  max_len_reg
```

- `prompt_reg`: register pointing to null-terminated prompt string in RAM
- `response_reg`: register pointing to RAM buffer for response
- `max_len_reg`: register with max response length
- `r0`: set to response length on return (0 = error)

### Call Chain

```
ASM program
  -> 0x9C opcode (vm/mod.rs ~line 1651)
  -> call_llm_external() (~line 2409)
     |
     +-- Primary: model_choice Python subprocess
     |   MODEL_CHOICE_TEMPLATE=ai_daemon
     |   complexity='fast'
     |   System prompt from build_llm_system_prompt()
     |
     +-- Fallback: call_llm_curl() (~line 2549)
         Raw curl to provider.json endpoint
```

### Map-Aware System Prompt

`build_llm_system_prompt()` (~line 2465) reads live VM state from RAM:

| RAM Address | Data |
|-------------|------|
| 0x7808/0x7809 | Player X/Y position |
| 0x7800/0x7801 | Camera X/Y |
| 0x7812 | Zoom level |
| 0x7802 | Frame counter |
| 0x7500-0x757F | Building table (name, position) |
| 0x7580 | Building count |
| 0x7588 | Nearby building flag |

The biome is computed from player position using the same hash as the ASM
terrain generator, so the LLM's biome names match what's rendered on screen.

---

## Ollama Setup

### Requirements

- Ollama installed and running as a systemd user service
- `qwen2.5-coder:14b` model pulled (17GB VRAM on RTX 5090)
- `model_choice` Python package installed (handles routing and fallback)

### Service Configuration

Ollama runs as `~/.config/systemd/user/ollama.service`.

An override keeps the model loaded permanently:

**File:** `~/.config/systemd/user/ollama.service.d/override.conf`
```ini
[Service]
Environment=OLLAMA_KEEP_ALIVE=-1
```

Without this, Ollama evicts the model after 5 minutes of inactivity. The next
0x9C call would block for 10-15 seconds while reloading. With `keep_alive=-1`,
the model stays in VRAM forever.

### Restarting Ollama

```bash
# Restart (model will auto-reload on next request)
systemctl --user restart ollama

# Check status
systemctl --user status ollama

# Verify model is loaded and persistent
ollama ps
# Should show: UNTIL  Forever
```

After restart, the first LLM call will take a few seconds to load the model
back into VRAM. Subsequent calls are fast (~1-3s for short responses).

### If Model Is Missing

```bash
# Pull the model if not installed
ollama pull qwen2.5-coder:14b

# Manually load it into VRAM
curl -s http://localhost:11434/api/generate \
  -d '{"model":"qwen2.5-coder:14b","prompt":"ready","stream":false,"options":{"num_predict":1}}'
```

### VRAM Budget

On RTX 5090 (24GB):

| Component | VRAM |
|-----------|------|
| Ollama (qwen2.5-coder:14b) | ~15-17 GB |
| Geometry OS (rendering) | ~0-1 GB |
| X11 / desktop | ~4 MB |
| **Total** | ~16-18 GB |
| **Free** | ~6-8 GB |

Both coexist fine. If you add another GPU-heavy process and Ollama starts
swapping to CPU, responses will slow dramatically. Check with `nvidia-smi`.

---

## Troubleshooting

### LLM opcode returns empty response

1. Check Ollama is running: `systemctl --user status ollama`
2. Check model is loaded: `ollama ps` (should show `Forever`)
3. Check model_choice works:
   ```bash
   MODEL_CHOICE_TEMPLATE=ai_daemon python3 -c \
     "from model_choice import generate; print(generate('hello', complexity='fast'))"
   ```
4. Check VRAM isn't full: `nvidia-smi`

### Response is slow (>5s)

- Likely GPU contention. Check `nvidia-smi` for other processes eating VRAM.
- The curl fallback path (~line 2549) is slower than model_choice.
- Check if model is on CPU (swap): `ollama ps` shows processor column.

### Geometry OS won't start / crashes

- Not related to Ollama. The 0x9C path is non-blocking -- it runs a subprocess
  and returns 0 on error. The VM keeps running.

### After reboot

Ollama is a user service with `Restart=always`, so it starts automatically.
The model loads on first 0x9C call (or any Ollama request). No manual steps
needed, though the first call after boot will be slow.

---

## Files

| File | Purpose |
|------|---------|
| `src/vm/mod.rs` (~L1651) | 0x9C opcode handler |
| `src/vm/mod.rs` (~L2409) | `call_llm_external()` - model_choice path |
| `src/vm/mod.rs` (~L2465) | `build_llm_system_prompt()` - map state reader |
| `src/vm/mod.rs` (~L2549) | `call_llm_curl()` - fallback curl path |
| `~/.config/systemd/user/ollama.service` | Ollama service |
| `~/.config/systemd/user/ollama.service.d/override.conf` | keep_alive override |
