# Geometry OS

## What This Is

A GPU-native operating system where geometric fonts are executable programs. The kernel boots directly from font-encoded SPIR-V, runs on Vulkan, and provides a visual shell for loading and executing additional font-programs. This is a performance-focused compute paradigm shift - visual symbols that compute on the GPU.

## Core Value

**Self-hosting kernel on Vulkan.** The OS must boot from a font, present a visual shell, and load/run other font-programs entirely on GPU. If this works, everything else follows.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Kernel boots from font-encoded SPIR-V on Vulkan
- [ ] Visual shell renders and accepts input
- [ ] Font-program loader can load and execute additional fonts
- [ ] Basic memory/process management for font-programs
- [ ] IPC between font-programs

### Out of Scope

- Security/users — Single-user, no permissions model for v1
- Networking — No network protocols or distributed compute
- Persistence — No file system or persistent storage
- Hardware drivers — GPU-only, no other hardware abstraction

## Context

**Existing codebase:**
- Font-to-SPIRV compilation is working (core capability validated)
- VisualCompiler.js, SpirvRunner.js for web execution
- kernel.wgsl - GPU kernel implementation
- Visual shell prototype exists (VisualIDE.js, VisualShell.js)
- IPC and shared memory demos present

**Technical foundation:**
- SPIR-V as the execution format
- WGSL shaders for GPU compute
- Python tooling for compilation pipeline
- Web-based visualization and debugging

## Constraints

- **Timeline:** Weeks (fast iteration, ship v1 quickly)
- **Team:** Solo/small (optimize for velocity over completeness)
- **Platform:** Vulkan direct (not WebGPU) for maximum GPU control
- **Scope:** Boot + shell + loader only for v1

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Vulkan over WebGPU | Maximum GPU control, direct SPIR-V execution | — Pending |
| Bootstrapping priority | Self-hosting kernel proves the paradigm | — Pending |
| Defer persistence | RAM-only is simpler, proves core first | — Pending |
| Defer networking | Not needed for kernel self-host proof | — Pending |

---
*Last updated: 2026-03-02 after initialization*
