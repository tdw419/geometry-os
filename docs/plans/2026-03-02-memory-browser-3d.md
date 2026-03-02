# Implementation Plan: 3D Memory Browser

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a high-performance 3D visual browser for the "Visual Open Brain" memory substrate using WebGPU and a "Hybrid (Hilbert + Semantic Gravity)" topography.

**Architecture:**
- **Frontend:** `web/MemoryBrowser.js` (WebGPU controller), `web/memory_browser.html` (UI shell).
- **Shaders:** `web/memory_browser.wgsl` (Compute: Gravity, Render: Particles, Picking: Selection).
- **Data:** `web/assets/memory_substrate.spv` (The SPIR-V encoded memory substrate).
- **Logic:** Hybrid mapping (X/Y from Hilbert ID, Z from Priority, Displacement from Semantic Gravity).

**Tech Stack:**
- WebGPU (Compute & Render)
- WGSL (Shader language)
- JavaScript (ES Modules)
- Geometry OS SPIR-V Substrate

---

## Tasks

### Task 1: Scaffolding and HTML Shell
Create the `web/memory_browser.html` file and basic `web/MemoryBrowser.js` class structure.
- **Files:** `web/memory_browser.html`, `web/MemoryBrowser.js`
- **Goal:** Set up a full-screen WebGPU canvas with a basic UI overlay for memory details.

### Task 2: Substrate Loader and GPU Buffers
Implement the logic to load `memory_substrate.spv` and populate GPU buffers.
- **Files:** `web/MemoryBrowser.js`
- **Goal:** Read the 512-word stride SPIR-V binary, extracting ID, Type, Priority, and the 384-dim Embedding into `GPUBuffer` objects.

### Task 3: WGSL Particle Renderer
Implement the vertex and fragment shaders for rendering memory particles as glyphs.
- **Files:** `web/memory_browser.wgsl`, `web/MemoryBrowser.js`
- **Goal:** Render each memory as a 16x16 glyph from the `universal_font.spv` atlas (via `GeometryFont.js`).

### Task 4: Semantic Gravity Compute Shader
Implement the "Hybrid (Hilbert + Semantic Gravity)" compute pass.
- **Files:** `web/memory_browser.wgsl`
- **Goal:** Apply attraction forces between semantically similar memories (top-K=8) while tethering them to their Hilbert base coordinates.

### Task 5: GPU Picking and Interaction
Implement the selection logic using a hidden ID texture (Picking Buffer).
- **Files:** `web/memory_browser.wgsl`, `web/MemoryBrowser.js`
- **Goal:** Allow clicking a 3D particle to highlight it and display its full memory content in the UI overlay.

---

## Verification Strategy
1. **Visual Test:** Ensure the Hilbert grid is visible and "warped" by semantic clusters.
2. **Performance Test:** Maintain 60 FPS with 1000+ particles.
3. **Interaction Test:** Clicking a particle must correctly identify its ID and fetch the corresponding metadata from the substrate.
