# Visual Shell Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build an interactive Visual Shell for the Geometry Kernel that enables drag-drop process spawning, real-time PCB inspection, and kernel saccades (auto-follow active process).

**Architecture:** The Visual Shell extends the MemoryBrowser to create a unified OS interface. It adds a Process Spawner overlay, PCB Inspector HUD, and camera tracking for active processes.

**Tech Stack:** WebGPU, WGSL, JavaScript ES6, CSS Grid

**Working Directory:** `/home/jericho/zion/projects/geometry_os/geometry-os-font-toolkit/geometry-os-font-toolkit`

---

## Task 1: Create VisualShell Core

**Files:**
- Create: `web/VisualShell.js`
- Create: `web/visual_shell.html`

**Acceptance Criteria:**
- VisualShell class extends MemoryBrowser
- Process spawner panel renders on left sidebar
- Drag-drop zone accepts .spv files
- "Spawn from Brain" button lists available memories
- File exists and loads without errors

**Verification:**
```bash
cd /home/jericho/zion/projects/geometry_os/geometry-os-font-toolkit/geometry-os-font-toolkit
test -f web/VisualShell.js && echo "VisualShell.js exists"
test -f web/visual_shell.html && echo "visual_shell.html exists"
```

---

## Task 2: Add PCB Inspector Overlay

**Files:**
- Modify: `web/VisualShell.js`
- Modify: `web/visual_shell.wgsl`

**Acceptance Criteria:**
- PCB data read from GPU buffer after each frame
- HUD overlay renders above each process's memory region
- Shows: PID, PC, Stack Pointer, Status, Cycle Count
- Color-coded by process state (RUNNING=green, WAITING=yellow, DONE=gray)
- Updates at 60fps

**Verification:**
```bash
cd /home/jericho/zion/projects/geometry_os/geometry-os-font-toolkit/geometry-os-font-toolkit
grep -q "PCBInspector" web/VisualShell.js && echo "PCBInspector implemented"
```

---

## Task 3: Implement Kernel Saccades

**Files:**
- Modify: `web/VisualShell.js`

**Acceptance Criteria:**
- "Focus Active" toggle button in toolbar
- Camera smoothly glides to active process position
- Follows Hilbert curve position of PID with most cycles
- Transition uses easing animation (lerp)
- Can be disabled by user

**Verification:**
```bash
cd /home/jericho/zion/projects/geometry_os/geometry-os-font-toolkit/geometry-os-font-toolkit
grep -q "saccade\|focusActive\|followProcess" web/VisualShell.js && echo "Saccades implemented"
```

---

## Task 4: Add Integrated Dashboard

**Files:**
- Modify: `web/VisualShell.js`
- Modify: `web/visual_shell.html`

**Acceptance Criteria:**
- Right sidebar shows "System Stats" panel
- Visual RAM usage bar (used/total)
- Process count with status breakdown
- "Neural Pulse" meter (agent iterations/sec)
- Uptime counter

**Verification:**
```bash
cd /home/jericho/zion/projects/geometry_os/geometry-os-font-toolkit/geometry-os-font-toolkit
grep -q "Dashboard\|NeuralPulse\|ramUsage" web/VisualShell.js && echo "Dashboard implemented"
```

---

## Task 5: Wire Process Spawner to Kernel

**Files:**
- Modify: `web/VisualShell.js`
- Modify: `web/GeometryKernel.js`

**Acceptance Criteria:**
- Dropped .spv files spawn new processes via GeometryKernel
- Process appears in PCB table immediately
- Memory region allocated in Visual RAM
- Process shows in 3D view at its Hilbert position
- Error handling for invalid SPIR-V

**Verification:**
```bash
cd /home/jericho/zion/projects/geometry_os/geometry-os-font-toolkit/geometry-os-font-toolkit
grep -q "spawnProcess\|loadSPV" web/VisualShell.js && echo "Process spawner wired"
```

---

## Verification Checklist

- [ ] VisualShell.js exists and loads
- [ ] visual_shell.html renders shell UI
- [ ] Process spawner panel visible
- [ ] PCB Inspector shows process data
- [ ] Kernel saccades follow active process
- [ ] Dashboard shows system stats
- [ ] All commits made
