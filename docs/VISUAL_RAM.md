# Visual RAM - Stateful Memory in SPIR-V

## Overview
Visual RAM provides stateful memory operations for Geometry OS visual programs.
It enables reading and writing to a persistent GPU buffer during execution.

## Opcodes

| Opcode | Name | Description |
|--------|------|-------------|
| 204 (0xCC) | GEO_VISUAL_LOAD | Load float from visual RAM at index |
| 205 (0xCD) | GEO_VISUAL_STORE | Store float to visual RAM at index |

## Usage

### Storage Format
- Buffer binding: 5
- Type: storage buffer of f32 values
- Size: Configurable (default 1024 floats = 4KB)

### Example Program
```
// Store 42.0 at index 0
GEO_VISUAL_STORE(0, 42.0)

// Load from index 0 and add 8.0
result = GEO_VISUAL_LOAD(0) + 8.0  // result = 50.0
```

## Test Verification
Run `web/test_visual_ram.html` in a WebGPU-compatible browser.
Expected output: 50.0

## Implementation Files
- `geometry_os_spirv/scripts/emit_spirv.py` - Opcode definitions
- `web/executor.wgsl` - GPU execution with visual_ram buffer
- `web/SpirvRunner.js` - WebGPU buffer management
- `tests/test_visual_ram.py` - Test program generator
