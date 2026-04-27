# Legacy Roadmap Reconciliation Report (Phase E)

*Generated: 2026-04-27 by Oracle Worker (automated audit)*

## Summary

| Category | Count |
|----------|-------|
| Total top-level phases | 42 |
| Done | 31 |
| Todo (app-level features) | 11 |
| Planned (bug fixes) | 7 |
| Research | 1 |
| **Superseded by SPEC** | ~16 |

## Meta Block Audit

| Metric | Old Value | Actual Value | Updated? |
|--------|-----------|-------------|----------|
| opcodes | 154 | 187 | YES |
| programs | 147 | 156 | YES |
| loc | 76635 | 83836 | YES |
| integration_tests | 2182 | 1619 | YES |
| lib_tests | 1474 | 788 | YES |
| total_tests | (missing) | 2407 | YES (added) |
| riscv_examples | (missing) | 13 | YES (added) |

Note: integration_tests and lib_tests decreased because duplicate/broken tests were cleaned up.
The total_tests count (2407) is the authoritative number.

## Phase-by-Phase Triage

### DONE Phases (31 phases) — No action needed

All phases from phase-131 through phase-169 that are marked "done" are verified.
These cover: PTY resize, ANSI color, wider display, extended characters, test fixes,
daily driver features, filesystem bridge, fullscreen desktop, text editor, process
monitor, network viewer, desktop polish, terminal hardening, direct boot, multiplexer,
scrollback, copy/paste, quick launch, fullscreen scaling, CLINT fix, terminal polish,
and windowed app fixes.

### TODO Phases (11 phases) — Superseded Assessment

These are app-level features from the pre-SPEC era. Per SPEC.md's direction:
"If a feature can be a pixel program, it must be one." These should be RISC-V
bare-metal programs, not Rust features.

| Phase | Title | Recommendation | Reason |
|-------|-------|---------------|--------|
| phase-142 | Sprite Sheet Editor | **SUPERSDED** | Should be a RISC-V pixel program, not a Rust feature |
| phase-143 | Sound Synthesizer | **SUPERSDED** | Should be a RISC-V pixel program using BEEP/SBI |
| phase-144 | Memory Hex Editor | **SUPERSDED** | sh.c already provides peek/poke/hexdump |
| phase-145 | File Manager App | **SUPERSDED** | Should be a RISC-V program using libgeos |
| phase-146 | Performance Benchmark Suite | **RELEVANT** | Benchmark infrastructure is cross-cutting |
| phase-147 | Paint Application | **SUPERSDED** | paint.c already exists as RISC-V program |
| phase-148 | Chat Bot Terminal | **SUPERSDED** | Should be a RISC-V program |
| phase-149 | RISC-V Assembly Viewer | **RELEVANT** | Useful for debugging, but could be a pixel program |
| phase-150 | Screen Recorder | **SUPERSDED** | Should be a RISC-V program or host tool |
| phase-151 | Thread Pool Manager | **SUPERSDED** | Premature — no Layer 2 kernel yet |
| phase-152 | QR Code Generator | **SUPERSDED** | Should be a RISC-V pixel program |

**Result: 9 superseded, 2 potentially relevant**

### PLANNED Phases (7 phases) — Bug Fix Assessment

| Phase | Title | Status | Recommendation |
|-------|-------|--------|---------------|
| phase-163 | Status Bar Clock -- Local Timezone Fix | planned | **RELEVANT** — small fix, still needed |
| phase-164 | Windowed Apps Halting Immediately | planned | **NEEDS VERIFICATION** — phase-169 may have fixed |
| phase-165 | geo_mcp_server Zombie Accumulation | planned | **RELEVANT** — infrastructure bug |
| phase-166 | Terminal Input Not Working in Desktop Mode | planned | **RELEVANT** — UX bug |
| phase-167 | Building-to-Program Filename Mismatch | planned | **RELEVANT** — routing bug |
| phase-168 | Stale cargo test Process | planned | **RELEVANT** — infrastructure bug |
| phase-170 | Windowed Apps Crashing After Phase-169 | planned | **RELEVANT** — regression from phase-169 |
| phase-171 | Window Title RAM Not Zeroed | planned | **RELEVANT** — UX bug |

### RESEARCH Phase (1 phase)

| Phase | Title | Status | Recommendation |
|-------|-------|--------|---------------|
| phase-160 | Native RISC-V Linux Boot to Userspace | research | **CONFIRM RESEARCH** — SPEC explicitly demotes this |

SPEC.md states: "phase-160 stays in the repo as research, but is demoted from priority 99 —
it costs the system's identity and the boot path is not stable."

## Gap Analysis: roadmap.yaml vs roadmap_v2.yaml

### roadmap_v2.yaml covers (all complete):
- Phase A: Toolchain Hygiene (rv32imac, optimizations, bus routing)
- Phase B: RISC-V Hello World (ELF loader, UART, paint.c)
- Phase C: libgeos Extraction (shared C library)
- Phase D: Desktop Integration (GUI bridge, input)
- Phase E: Legacy Roadmap Reconciliation (this report)
- Phase F: Visual Verification (regression tests, encode_png audit)
- Phase G: In-session Canvas Checkpoint (save/load pixels)
- Phase H: Cross-session Persistence (canvas survives restart)
- Phase 141: Terminal Polish (scrollback, cursor)

### roadmap.yaml has no equivalent for:
- libgeos extraction (Phase C) — roadmap_v2-only
- Visual verification infrastructure (Phase F) — roadmap_v2-only
- Canvas persistence (Phases G, H) — roadmap_v2-only

### roadmap.yaml has that roadmap_v2 doesn't:
- 11 app-level TODO phases (142-152) — most superseded by SPEC direction
- 8 planned bug-fix phases (163-171) — still relevant to GeOS desktop
- 31 done phases covering terminal, daily driver, and polish work

## Recommendations

1. **Mark 9 TODO phases as superseded** (142-145, 147-148, 150-152)
2. **Keep phase-146 and phase-149 as deferred** — potentially relevant but not priority
3. **Keep all planned phases** (163-171) — these are real bugs that need fixing
4. **Keep phase-160 as research** — SPEC confirms this status
5. **Meta block updated** with accurate counts (opcodes: 187, tests: 2407, loc: 83836, programs: 156)
6. **roadmap_v2.yaml is the active roadmap** — roadmap.yaml is legacy/reference
