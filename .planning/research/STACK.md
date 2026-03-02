# Stack Research: GPU-Native Operating System with Font-Based Execution

**Domain:** GPU-native OS, font-based compute, Vulkan SPIR-V execution
**Researched:** 2026-03-02
**Confidence:** MEDIUM (highly specialized domain with no standard stack exists; recommendations based on ecosystem survey and architectural fit)

---

## Executive Summary

There is no "standard stack" for GPU-native operating systems with font-based execution. This is a novel architecture. The recommended stack combines:

1. **WebGPU (browser)** + **wgpu-native (desktop)** for cross-platform GPU compute
2. **WGSL** for kernel shaders with **SPIR-V** as intermediate representation
3. **Python + fonttools + Pillow + NumPy** for font encoding and asset generation
4. **Vitest + Vite** for web-based testing and development

The key insight: use **WGSL as the primary kernel language** (not raw SPIR-V) because WebGPU tooling is mature in 2025, and naga can cross-compile to SPIR-V for native execution.

---

## Recommended Stack

### Core GPU Runtime

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **WebGPU API** | Browser API (Chrome 113+, Safari 17.4+) | Browser GPU compute | Production-ready as of 2025; native compute shader support; all major desktop browsers |
| **wgpu-native** | 24.x (2025) | Native GPU backend | Rust-based WebGPU implementation; compiles to Vulkan/Metal/DX12; same codebase for web and native |
| **WGSL** | WebGPU Shading Language | Kernel/OS shader language | Rust-like syntax; strongly typed; naga can compile to SPIR-V/GLSL/HLSL/MSL |

### SPIR-V Toolchain

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **naga** | wgpu-naga 24.x | Shader cross-compiler | Compiles WGSL to SPIR-V; included in wgpu; better DX than glslang for this use case |
| **spirv-opt** | SPIRV-Tools 1.4.304+ | SPIR-V optimization | Strip debug, unroll loops, optimize size for font-embedded programs |
| **spirv-val** | SPIRV-Tools 1.4.304+ | SPIR-V validation | Catch errors before GPU execution |
| **spirv-dis** | SPIRV-Tools 1.4.304+ | Disassembly for debugging | Understand compiled bytecode |

**Note:** Use **naga for WGSL-to-SPIR-V** compilation, not glslangValidator. Naga is purpose-built for WebGPU and handles WGSL natively.

### Font System

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **fonttools** | 4.55+ | TTF/OTF generation | Industry-standard Python library; full OpenType spec support; active development |
| **Pillow** | 11.x | Font atlas rendering | Mature image processing; converts glyph bitmaps to PNG atlases |
| **NumPy** | 2.x | Pixel array manipulation | Efficient array operations for glyph bitmap processing |
| **Hilbert curve** | Custom implementation | Spatial locality encoding | Preserves 2D locality in 1D sequence for efficient GPU memory access |

### Web Frontend

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **Vite** | 6.x | Build tool | Native ES modules; fast HMR; excellent WebGPU support |
| **Vitest** | 3.x+ | Testing framework | Vite-native; browser mode for WebGPU tests; Jest-compatible API |
| **Vanilla JS/TypeScript** | ES2024+ | Application code | No framework overhead; direct WebGPU API access |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| **RenderDoc** | GPU debugging | Capture and inspect WebGPU/Vulkan frames |
| **Chrome DevTools** | WebGPU inspection | Memory usage, pipeline stats |
| **spirv-dis** | SPIR-V disassembly | Understand compiled bytecode structure |

---

## Installation

```bash
# === Python (Font Generation) ===
pip install fonttools>=4.55.0 Pillow>=11.0.0 numpy>=2.0.0

# Optional: fonttools extras
pip install fonttools[ufo,woff,interpolatable]

# === Node.js (Web Frontend) ===
npm create vite@latest geometry-os -- --template vanilla-ts
cd geometry-os
npm install

# Dev dependencies
npm install -D vitest @vitest/browser playwright

# === SPIR-V Tools (Native Compilation) ===
# Linux (Ubuntu/Debian)
sudo apt install spirv-tools spirv-headers

# macOS
brew install spirv-tools

# Or via Vulkan SDK (includes all tools)
# Download from: https://www.lunarg.com/vulkan-sdk/
# Current SDK: 1.4.309.0 (March 2025)

# === wgpu-native (Optional, for desktop builds) ===
# Download prebuilt binaries from:
# https://github.com/gfx-rs/wgpu/releases
```

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| **Raw SPIR-V generation** | Complex, error-prone, no type safety | WGSL + naga compiler |
| **OpenGL/WebGL** | No compute shaders (WebGL), deprecated API | WebGPU |
| **CUDA/OpenCL** | Platform-specific, not web-compatible | WebGPU (cross-platform) |
| **glslangValidator for WGSL** | GLSL-focused, WGSL support is secondary | naga (WGSL-native) |
| **TrueType Instructions** | Legacy bytecode, limited expressiveness | SPIR-V compute kernels |
| **Framework-heavy frontend** | React/Vue add overhead for GPU code | Vanilla JS/TypeScript |
| **CPU-side interpreters** | Defeats GPU-native purpose | GPU kernel execution |

---

## Stack Patterns by Variant

### Variant A: Browser-Only (Current Project)

**Use when:** Targeting web deployment only, leveraging existing WebGPU browser support.

**Stack:**
- WebGPU API (browser)
- WGSL kernels (compiled in-browser)
- Vanilla JS frontend
- No SPIR-V toolchain needed (browsers handle internally)

**Rationale:** Simplest deployment; browsers handle shader compilation internally.

### Variant B: Desktop Native

**Use when:** Need maximum performance, desktop distribution, or browser limitations are blocking.

**Stack:**
- wgpu-native (Rust)
- WGSL kernels compiled via naga to SPIR-V
- Native window via winit
- Optional: Tauri for webview-based UI

**Rationale:** Same shader code, better performance, access to Vulkan features not in WebGPU.

### Variant C: Hybrid (Recommended for Geometry OS)

**Use when:** Want web demo + native performance option.

**Stack:**
- Shared WGSL kernels
- WebGPU for browser deployment
- wgpu-native for desktop builds
- Build flag switches backend at compile time

**Rationale:** Maximum flexibility; web for accessibility, native for power users.

---

## Version Compatibility Matrix

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| wgpu-native 24.x | Vulkan 1.3+, Metal 2.0+, DX12 | Requires Vulkan SDK 1.3+ for SPIR-V tools |
| naga (wgpu) | SPIR-V 1.5, GLSL 4.60, HLSL SM 6.0 | Cross-compiles between all formats |
| fonttools 4.55+ | Python 3.9+ | NumPy 2.x compatible |
| Vitest 3.x | Vite 6.x, Node 20+ | Browser mode requires Playwright |
| WebGPU | Chrome 113+, Safari 17.4+, Firefox 141+ | Firefox has 4 bind group limit |

---

## Architecture-Specific Recommendations

### Font-to-SPIR-V Pipeline

```
[Geometry Glyphs (PNG)]
    |
    v  (Hilbert curve traversal)
[1D Byte Sequence]
    |
    v  (Python compiler)
[SPIR-V Binary .spv]
    |
    v  (spirv-opt --strip-debug)
[Optimized SPIR-V]
    |
    v  (WebGPU createShaderModule)
[GPU Execution]
```

**Key decisions:**
1. Use **Hilbert curve** for spatial locality (2D glyph grid -> 1D sequence)
2. Encode opcodes in glyph **Green channel** (G < 128 = constant, G >= 128 = instruction)
3. Encode operands in **Blue channel** (immediate values)
4. Reserve **Red channel** for visual appearance only

### Kernel Architecture

The existing `kernel.wgsl` implements:
- Multi-process scheduler (round-robin)
- Memory-mapped I/O (RAM[0..1023] shared)
- IPC via mailboxes (RAM[PID * 32])
- Syscall interface (RAM[100..105])

**This is correct.** Keep this architecture. The stack above supports it.

---

## WebGPU Limitations to Work Around

| Limitation | Impact | Workaround |
|------------|--------|------------|
| No synchronous readback | Cannot read GPU results immediately | Use async `mapAsync()` pattern |
| 16 texture limit | Limited glyph atlas layers | Use texture arrays or bindless (if supported) |
| No async compute | Compute/graphics share queue | Accept limitation for MVP |
| No wave intrinsics | No subgroup operations | Use workgroup shared memory |
| Firefox 4 bind groups | Limited descriptor sets | Target Chrome/Safari for MVP |

---

## Sources

**HIGH Confidence (Official/Verified):**
- [Vulkan SDK 1.4.309.0](https://www.lunarg.com/vulkan-sdk/) - LunarG official release
- [SPIRV-Tools GitHub](https://github.com/KhronosGroup/SPIRV-Tools) - Khronos Group
- [wgpu GitHub](https://github.com/gfx-rs/wgpu) - gfx-rs project
- [fonttools GitHub](https://github.com/fonttools/fonttools) - fonttools project
- [Vitest Official Docs](https://vitest.dev/) - vitest.dev

**MEDIUM Confidence (WebSearch verified):**
- WebGPU browser support status (Chrome 113+, Safari 17.4+, Firefox 141+) - Multiple sources
- spirv-opt optimization flags - CSDN/Linux From Scratch guides
- Hilbert curve GPU applications - CVPR 2024, arXiv 2025 papers
- WebGPU limitations list - Multiple blog posts, browser docs

**LOW Confidence (Needs validation):**
- iOS WebGPU support timeline (early 2026) - Single source
- Specific performance benchmarks - Blog posts, not official

---

## Confidence Assessment

| Area | Level | Reason |
|------|-------|--------|
| GPU Runtime (WebGPU/wgpu) | HIGH | Official docs, production-ready since 2025 |
| SPIR-V Toolchain | HIGH | Khronos official tools, stable for years |
| Font System (fonttools) | HIGH | Industry standard, active development |
| WebGPU Limitations | MEDIUM | Some browser-specific, need real testing |
| Novel Architecture Fit | MEDIUM | No prior art for font-based SPIR-V OS |

---

## Open Questions

1. **SPIR-V in font files:** Can we embed .spv binaries in OpenType `glyf` tables, or need custom table? (Needs experimentation)
2. **Font atlas compression:** Is PNG atlas sufficient, or need ASTC/BC7 for GPU upload? (Profile first)
3. **Process isolation:** WebGPU has no process isolation - how to safely run untrusted font-programs? (Security research needed)
4. **Desktop vs Browser:** Should MVP target browser-only or hybrid from start? (Recommend: browser-first)

---

*Stack research for: GPU-native OS with font-based execution*
*Researched: 2026-03-02*
*Researcher: GSD Project Researcher Agent*
