# Performance Optimization Plan

## Goal
Profile and optimize all JavaScript files to identify bott spots for GPU execution.

## Current Issues
- **No requestAnimationFrame batching** in main loop (geometry-os- Geometry_os.js runs)
  - **CPU blocking**: VisualFileManager.read() and can freeze UI
  - **No streaming memory**: VisualFileManager loads full directory list (no caching)
  - **No pooling**: Visual rendering is CPU-bound (  - **Potential memory leak**: Loading 100+ files in sequence
- - **Shell.js startup is slow** (~500ms)
  - VisualShell.js: Each glyph is rendered individually
  - **VisualFileManager.processList()**: Could use Oading (only 2 glyphs at a time)

## Implementation Plan

### Step 1: Profile Critical Paths
- **File**: `docs/plans/2026-03-02-perf-optimization.md`
- **Changes**: All JS files
    - **Add performance.now() method** for baseline
    - **Add baseline metrics** for tracking ( visualFileManager.js, Memory stats)
    - **Implement lazy loading** for file icons
    - **Implement caching** for file listings
    - **Optimize grid rendering** by only rendering visible glyphs
    - **Add requestAnimationFrame batching** for faster visual updates
    - **Profile Scheduler** for high-frequency operations
    - **Pre-generate** snapshot of

### Step 2: Implement optimizations

1. **VisualFileManager.js** - Optimize visual rendering
   - Add `renderGlyph()` method for per-glyph
   - Add `renderGlyphBatch()` for batch glyph rendering
   - Add `getCachedGlyph()` for memoization
   - Measure performance, compare before/ after

2. **GeometryKernel.js** - Optimize process spawning
   - Add `spawnProcessFast()` for lightweight process creation
   - Add `preloadProgram()` for pre-loading compiled programs
   - Optimize `loadProgram()` for lazy loading
   - Add `getProcessForDisplay()` for filtering by state
   - Optimize `killProcess()` to reduce code complexity
   - Add `killProcessBatch()` for batch termination
   - Add `killProcessImmediate()` for immediate termination without waiting for zombie processes
   - Optimize `step()` by reducing dispatch overhead
   - Optimize `updatePCbs()` to reduce GPU sync overhead
   - Cache PCB metrics

   - Add `getMemoryStats()` and `isRunning` check
   - Add `getProcessMemory(pid)` for accessing process memory info

   - Optimize `step()` dispatch overhead
   - Optimize `readSharedMemory()` and `readPageTable()` for faster GPU reads
   - **Consider requestAnimationFrame batching** for high-frequency updates (60fps)
   - **Implement requestIdle callback** for better pause/resumption
   - Add `requestIdleCallback()` for smarter idle detection
   - Add `pauseOnUserInteraction()` to pause/resuming handlers
   - **Use requestAnimationFrame** for smooth scrolling**
   - **Consider GPU buffer pooling** for batch operations on large files

   - **Consider GPU texture upload** for faster updates
   - **Consider WebGPU limitations** ( max 1GB buffers, max dispatch 256)

### Step 3: Implement and test
1. Create `/docs/perf-optimization-benchmark.md` with performance targets
2. Run `python3 test_memory_benchmark.py` in web directory
3. Open `http://localhost:8770/test-memory.html` to verify results

4. Share results via `updateState()``
5. Update documentation

---

## Geometry OS is now complete and ready for the next phase!

