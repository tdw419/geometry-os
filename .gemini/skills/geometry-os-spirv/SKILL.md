---
name: geometry-os-spirv
description: Compiles visual program patterns from Geometry OS glyphs directly into SPIR-V bytecode for GPU execution. Uses a Stack-based (Postfix) execution model where each glyph carries semantic RGB data (G=opcode, B=operand).
---

# Geometry OS SPIR-V Compilation

## Overview
This skill implements a SPIR-V compilation pipeline for Geometry OS. It enables the conversion of visual program patterns (encoded as semantic RGB glyphs) directly into SPIR-V bytecode for high-performance GPU execution.

## Execution Model: Stack-based (Postfix)
To maintain simplicity and visual locality, Geometry OS programs use a **Stack-based** model:
1. **Hilbert Order**: Glyphs are read in Hilbert curve order (preserving spatial locality).
2. **Push Logic**: If the Green channel (G) < 128, the glyph is a **Constant**. The Blue channel (B) is pushed to the stack as a float.
3. **Instruction Logic**: If G >= 128, the glyph is an **Instruction** (e.g., `+`, `-`, `*`). It pops operands from the stack and pushes the result.

## Workflow: Visual Pattern to SPIR-V

### 1. Identify Semantic Glyphs
- **R (Visual)**: Glyph appearance/structure.
- **G (Opcode)**: SPIR-V instruction opcode (>= 128) or Constant ( < 128).
- **B (Operand)**: Immediate value for Constants.

### 2. Map Opcodes to SPIR-V
| GeoASM Op (G) | Symbol | SPIR-V Instruction | Description |
|---------------|--------|---------------------|-------------|
| `0x6A`        | `+`    | `OpFAdd`            | Float Add |
| `0x6B`        | `-`    | `OpFSub`            | Float Subtract |
| `0x6C`        | `*`    | `OpFMul`            | Float Multiply |

### 3. Compilation
Run the compiler from the project root:
```bash
python3 geometry_os_spirv/scripts/visual_to_spirv.py <input.png> <output.spv>
```

## Resource Usage

- **Scripts**:
  - `geometry_os_spirv/scripts/visual_to_spirv.py`: Main compiler script.
  - `geometry_os_spirv/scripts/emit_spirv.py`: SPIR-V binary builder.

## Example: Simple Addition (10 + 20)
Visual Glyphs (Hilbert Order):
1. `[G:0, B:10]` -> Push 10
2. `[G:0, B:20]` -> Push 20
3. `[G:0x6A, B:0]` -> Add (10 + 20)
Result is left on the GPU stack or returned by the function.
