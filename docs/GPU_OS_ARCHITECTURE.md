# GPU OS Architecture Plan

## Vision

Build a Geometry OS-based operating system that runs directly on the GPU using morphological fonts as the instruction encoding medium. The OS will use the existing visual compilation pipeline (PNG glyphs -> Hilbert reordering -> SPIR-V bytecode) to create a functional computing environment.

## Current State (What We Have)

### Existing Infrastructure

| Component | Status | Description |
|-----------|--------|-------------|
| Visual Grid | ✅ Complete | 16x16 interactive grid for program design |
| Hilbert Reorder | ✅ Complete | Spatial reordering for hardware-level efficiency |
| SPIR-V Emitter | ✅ Complete | Direct binary emission of SPIR-V bytecode |
| WebGPU Executor | ✅ Working | Stack-based VM running on GPU via WGSL interpreter |
| Font Atlas | ✅ Complete | Semantic RGB glyphs with instruction encoding |
| Sound System | ✅ Complete | Audio output via Web Audio API |
| Visual IDE | ✅ Complete | Web-based program editor |

### Current Instruction Set (GeoASM)

| G Channel | Symbol | SPIR-V Op | Description |
|-----------|--------|-----------|-------------|
| 0x6A | + | OpFAdd | Float addition |
| 0x6B | - | OpFSub | Float subtraction |
| 0x6C | * | OpFMul | Float multiplication |
| 0x6D | / | OpFDiv | Float division |
| 0x10 | > | OpFOrdGreaterThan | Comparison |
| 0x11 | < | OpFOrdLessThan | Comparison |
| 0x70 | sin | OpExtInst (Sin) | Trigonometric sine |
| 0x71 | cos | OpExtInst (Cos) | Trigonometric cosine |
| < 0x80 | 0-255 | OpConstant | Push constant value |

---

## Phase 1: Extended Instruction Set

### Goal
Transform the current calculator into a general-purpose computation system.

### New Instructions Needed

#### Memory Operations
| G Channel | Symbol | Description |
|-----------|--------|-------------|
| 0x80 | load | Load from memory address |
| 0x81 | store | Store to memory address |
| 0x82 | alloc | Allocate memory block |
| 0x83 | free | Free memory block |
| 0x84 | memcpy | Copy memory block |

#### Control Flow
| G Channel | Symbol | Description |
|-----------|--------|-------------|
| 0x90 | jmp | Unconditional jump |
| 0x91 | jz | Jump if zero |
| 0x92 | jnz | Jump if not zero |
| 0x93 | call | Function call |
| 0x94 | ret | Function return |
| 0x95 | loop | Loop start |
| 0x96 | endloop | Loop end |

#### Logical Operations
| G Channel | Symbol | Description |
|-----------|--------|-------------|
| 0xA0 | and | Bitwise AND |
| 0xA1 | or | Bitwise OR |
| 0xA2 | xor | Bitwise XOR |
| 0xA3 | not | Bitwise NOT |
| 0xA4 | shl | Shift left |
| 0xA5 | shr | Shift right |

#### Comparison Operations
| G Channel | Symbol | Description |
|-----------|--------|-------------|
| 0xB0 | eq | Equal |
| 0xB1 | neq | Not equal |
| 0xB2 | gte | Greater than or equal |
| 0xB3 | lte | Less than or equal |

### Implementation Files
- Extend `geometry_os_spirv/scripts/emit_spirv.py` with new opcodes
- Update `geometry_os_spirv/scripts/visual_to_spirv.py` with new instruction handlers
- Update `web/executor.wgsl` with new WGSL operations

---

## Phase 2: GPU Memory Management

### Goal
Create a memory management system that runs on the GPU.

### Components

#### Memory Layout
```
GPU Buffer Layout (256MB default):
┌────────────────────────────────────────┐
│ 0x00000000 - 0x000FFFFF: Kernel Space  │  1MB
├────────────────────────────────────────┤
│ 0x00100000 - 0x01FFFFFF: User Space    │ 31MB
├────────────────────────────────────────┤
│ 0x02000000 - 0x02FFFFFF: Shared Memory │ 16MB
├────────────────────────────────────────┤
│ 0x03000000 - 0x03FFFFFF: I/O Buffers   │ 16MB
├────────────────────────────────────────┤
│ 0x04000000 - 0x0FFFFFFF: File Storage  │ 192MB
└────────────────────────────────────────┘
```

#### Page Table System
- 4KB pages (1024 float4 values per page)
- Page table stored in GPU buffer
- Virtual-to-physical mapping via compute shader

#### Memory Protection
- Ring levels (0=kernel, 3=user)
- Read/Write/Execute permissions per page
- Fault handling via GPU atomics

### Implementation Files
- `core/gpu_memory.py` - Memory management Python interface
- `web/GPUMemoryManager.js` - WebGPU buffer management
- `shaders/memory.wgsl` - Memory operations compute shader

---

## Phase 3: Process Model

### Goal
Enable multiple programs to run concurrently on the GPU.

### Process Control Block (PCB)
```wgsl
struct PCB {
    pid: u32,
    state: u32,        // 0=ready, 1=running, 2=blocked, 3=terminated
    pc: u32,           // Program counter (glyph index)
    sp: u32,           // Stack pointer
    memory_base: u32,  // Base address of process memory
    memory_size: u32,  // Size of process memory
    priority: u32,
    ticks: u32,        // CPU ticks consumed
}
```

### Scheduler
- Round-robin with priority boost
- Time quantum: 1000 GPU cycles
- Context switch via compute shader dispatch

### Process States
```
         ┌──────────┐
         │  READY   │◄─────────────────┐
         └────┬─────┘                  │
              │ schedule               │
              ▼                        │
         ┌──────────┐                  │
         │ RUNNING  │─── block ────────┤
         └────┬─────┘                  │
              │                        │
              ├──── exit ──► TERMINATED│
              │                        │
              └──── yield ─────────────┘
```

### Implementation Files
- `core/process.py` - Process management
- `web/ProcessScheduler.js` - GPU-based scheduler
- `shaders/scheduler.wgsl` - Scheduler compute shader

---

## Phase 4: I/O Subsystem

### Goal
Connect the GPU OS to real hardware I/O.

### Framebuffer (Already Working)
- Direct GPU rendering via WebGPU
- Glyph-based text rendering
- GeometryFont.js handles display

### Input System
```javascript
// Input event queue in GPU buffer
struct InputEvent {
    type: u32,     // 1=keydown, 2=keyup, 3=mousedown, 4=mouseup, 5=mousemove
    keycode: u32,  // Key code or button
    x: f32,        // Mouse X
    y: f32,        // Mouse Y
    timestamp: u32,
}
```

### Audio System (Already Working)
- SoundSystem.js uses Web Audio API
- Synthesized tones and samples
- Ready for system sounds

### Storage Interface
- GPU buffer acting as disk
- Block-based read/write (4KB blocks)
- Simple file system on top

### Implementation Files
- `web/InputHandler.js` - Input event routing
- `web/StorageInterface.js` - GPU-based storage
- `shaders/io.wgsl` - I/O operations

---

## Phase 5: System Calls

### Goal
Provide a controlled interface between user programs and kernel services.

### Syscall Mechanism
1. Program writes syscall number to designated buffer
2. GPU atomic signals kernel
3. Kernel shader processes request
4. Result written back to program memory

### Syscall Table

| Number | Name | Arguments | Description |
|--------|------|-----------|-------------|
| 0x01 | read | fd, buf, count | Read from file |
| 0x02 | write | fd, buf, count | Write to file |
| 0x03 | open | path, flags | Open file |
| 0x04 | close | fd | Close file |
| 0x10 | alloc | size | Allocate memory |
| 0x11 | free | ptr | Free memory |
| 0x20 | spawn | path | Create process |
| 0x21 | exit | code | Terminate process |
| 0x22 | wait | pid | Wait for process |
| 0x30 | draw | buf, x, y, w, h | Draw to screen |
| 0x31 | play | freq, dur | Play sound |

### Implementation Files
- `core/syscall.py` - Syscall definitions
- `web/SyscallHandler.js` - Syscall dispatch
- `shaders/syscall.wgsl` - Kernel syscall handler

---

## Phase 6: Shell and Userspace

### Goal
Create a usable interface for the OS.

### Shell (Glyph-based Command Interpreter)
```
Commands are typed as Geometry OS glyphs:
- Each command is a sequence of glyphs
- Glyphs are rendered in the terminal area
- Enter executes the command
```

### Built-in Commands

| Command | Glyph | Description |
|---------|-------|-------------|
| ls | ▣ | List files |
| cd | ◈ | Change directory |
| cat | ◆ | Display file |
| run | ▶ | Execute program |
| edit | ✎ | Edit file |
| help | ? | Show help |
| clear | ✕ | Clear screen |
| ps | ☰ | List processes |
| kill | ✖ | Terminate process |

### File Browser
- Visual grid of file glyphs
- Click to select, double-click to open
- Drag-and-drop support

### Process Manager
- Real-time process list
- CPU/memory usage display
- Kill/renice operations

### Text Editor
- Uses GeometryFont for rendering
- Syntax highlighting for GeoASM
- Save/Load to GPU storage

### Implementation Files
- `web/Shell.js` - Command interpreter
- `web/FileBrowser.js` - File browser UI
- `web/ProcessManager.js` - Process management UI
- `web/Editor.js` - Text editor

---

## Implementation Roadmap

### Week 1: Extended Instruction Set
- [ ] Add memory opcodes to emit_spirv.py
- [ ] Add control flow opcodes
- [ ] Add logical/comparison opcodes
- [ ] Update executor.wgsl
- [ ] Test with visual programs

### Week 2: Memory Management
- [ ] Design GPU buffer layout
- [ ] Implement GPUMemoryManager.js
- [ ] Create memory.wgsl compute shader
- [ ] Test allocation/deallocation

### Week 3: Process Model
- [ ] Design PCB structure
- [ ] Implement ProcessScheduler.js
- [ ] Create scheduler.wgsl
- [ ] Test context switching

### Week 4: I/O System
- [ ] Implement InputHandler.js
- [ ] Implement StorageInterface.js
- [ ] Create io.wgsl
- [ ] Test input and storage

### Week 5: System Calls
- [ ] Define syscall table
- [ ] Implement SyscallHandler.js
- [ ] Create syscall.wgsl
- [ ] Test syscalls from programs

### Week 6: Shell and Userspace
- [ ] Implement Shell.js
- [ ] Implement FileBrowser.js
- [ ] Implement ProcessManager.js
- [ ] Implement Editor.js
- [ ] End-to-end testing

---

## Technical Notes

### WebGPU Limitations
- No true kernel mode - all code runs at same privilege
- Workaround: Use buffer isolation and validation
- Maximum buffer size: typically 1-2GB
- Compute shaders have maximum dispatch size

### Performance Considerations
- Minimize CPU-GPU sync points
- Use async buffer mapping
- Batch operations where possible
- Profile with WebGPU timestamps

### Security Model
- Process isolation via buffer bounds
- Syscall validation in kernel shader
- No direct memory access between processes
- Input sanitization at I/O boundary

---

## Success Criteria

1. **Boots**: System initializes and displays shell prompt
2. **Runs Programs**: Can load and execute GeoASM programs
3. **Multitasks**: Multiple programs run concurrently
4. **I/O Works**: Keyboard input and display output functional
5. **Persists**: Files can be saved and loaded
6. **Recovers**: Graceful handling of program crashes

---

*Document Version: 1.0*
*Created: 2026-03-02*
*Status: Planning Phase*
