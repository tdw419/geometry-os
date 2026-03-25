# Sovereign Shell - Development Roadmap

## Vision
A fully autonomous, visually self-represented operating system where natural language controls the silicon via a vision-to-opcode feedback loop.

---

## Phase 1: Input & HUD Zone
**Status:** 🔄 In Progress
**Goal:** Establish input text rendering and status HUD

### Milestones
- [ ] 5x7 WGSL font rendering for input zone
- [ ] Host-to-GPU keyboard injection
- [ ] Visual cursor blink implementation
- [ ] PATCH_STATUS indicator (READY/SUCCESS/FAIL)

### Metrics
- Render time: < 10ms
- Input responsiveness: Instant

---

## Phase 2: Vision Bridge
**Status:** ⏳ Pending
**Goal:** Connect qwen3-vl-8b to the input zone

### Milestones
- [ ] Automated screen capture of input rows
- [ ] Vision extraction with 100% accuracy
- [ ] Latency < 1 second for extraction

--- 

## Phase 3: Natural Language Loop
**Status:** ⏳ Pending
**Goal:** Complete the NL-to-Opcode pipeline

### Milestones
- [ ] PROMPT (@>) opcode integration
- [ ] LLM (tinyllama) code generation
- [ ] Live patching of VM instruction memory
- [ ] 1.0s loop latency achieved

---

## Phase 4: Scaling & Society
**Status:** ⏳ Pending
**Goal:** Multi-agent autonomous swarms

### Milestones
- [ ] Complex commands (e.g., "count from 1 to 5")
- [ ] Parallel agent spawning via NL
- [ ] Self-modifying swarms

--- 

## Current Focus
> Finalize the Sovereign Shell input zone and HUD rendering.
