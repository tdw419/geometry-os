# GPU-Native Visual Compiler Design

## Goal
Implement a self-hosting compiler that runs as a GPU process, compiling visual programs to SPIR-V bytecode.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Geometry OS GPU Memory                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐    Shared     ┌────────────────────────────┐ │
│  │  VisualIDE   │ ───────────▶  │   Input Buffer (64KB)      │ │
│  │  (Frontend)  │    Memory     │   - Pixel grid [0..4095]   │ │
│  └──────────────┘               │   - Grid metadata          │ │
│         │                       └────────────────────────────┘ │
│         │                                  │                   │
│         │                                  ▼                   │
│         │                       ┌────────────────────────────┐ │
│         │                       │   compiler.spv (PID 1)     │ │
│         │                       │   ───────────────────────  │ │
│         │                       │   1. Read input buffer     │ │
│         │                       │   2. Hilbert reordering    │ │
│         │                       │   3. RGB → Opcode mapping  │ │
│         │                       │   4. SPIR-V emission       │ │
│         │                       │   5. Write output buffer   │ │
│         │                       └────────────────────────────┘ │
│         │                                  │                   │
│         │                                  ▼                   │
│         │                       ┌────────────────────────────┐ │
│         └──────────────────────▶│   Output Buffer (64KB)     │ │
│                                 │   - Generated SPIR-V       │ │
│                                 │   - Status/Error codes     │ │
│                                 └────────────────────────────┘ │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Compiler Process (compiler.spv)

### Memory Layout
```
INPUT_BUFFER   @ 0x10000000 (64KB)  - Visual grid input
OUTPUT_BUFFER  @ 0x10010000 (64KB)  - Generated SPIR-V
SYMBOL_TABLE   @ 0x10020000 (16KB)  - Variable/function names
CONSTANT_POOL  @ 0x10024000 (16KB)  - Numeric constants
ERROR_LOG      @ 0x10028000 (4KB)   - Compilation errors
```

### Compilation Pipeline

```
Stage 1: Grid Ingestion
─────────────────────────
for each pixel in input_buffer:
    hilbert_index = hilbert_encode(pixel.x, pixel.y)
    ordered_grid[hilbert_index] = pixel

Stage 2: Tokenization  
─────────────────────────
for each pixel in ordered_grid:
    token = classify_glyph(pixel.rgb)
    tokens.append(token)

Stage 3: Opcode Mapping
─────────────────────────
for each token in tokens:
    opcode = GLYPH_TO_OPCODE[token.g]
    operands = extract_operands(token.rb)
    instructions.append((opcode, operands))

Stage 4: SPIR-V Emission
─────────────────────────
header = emit_spirv_header()
entrypoint = emit_entry_point()
for instruction in instructions:
    emit_instruction(instruction)
footer = emit_footer()
```

## Glyph → Opcode Mapping

| G Channel | Glyph | Opcode | SPIR-V Op |
|-----------|-------|--------|-----------|
| 0x6A (106) | ⊕ | OP_FADD | OpFAdd |
| 0x6B (107) | ⊖ | OP_FSUB | OpFSub |
| 0x6C (108) | ⊗ | OP_FMUL | OpFMul |
| 0x6D (109) | ⊘ | OP_FDIV | OpFDiv |
| 0x10 (16) | → | OP_STORE | OpStore |
| 0x11 (17) | ← | OP_LOAD | OpLoad |
| 0x70 (112) | sin | OP_SIN | GLSL.Sin |
| 0x71 (113) | cos | OP_COS | GLSL.Cos |
| 0x90 (144) | ⤴ | OP_JMP | OpBranch |
| 0x91 (145) | ⤵ | OP_JZ | OpBranchConditional |
| 0x93 (147) | ⚙ | OP_CALL | OpFunctionCall |
| 0x94 (148) | ↩ | OP_RET | OpReturn |

## Syscalls for Compiler

| Syscall | Number | Description |
|---------|--------|-------------|
| SYS_COMPILE | 0x100 | Trigger compilation |
| SYS_GET_STATUS | 0x101 | Get compilation status |
| SYS_GET_OUTPUT | 0x102 | Read generated SPIR-V |
| SYS_GET_ERRORS | 0x103 | Read error log |

## Implementation Files

1. **compiler.wgsl** - GPU compiler shader
2. **CompilerAgent.js** - Host-side compiler management
3. **compiler.spv** - Compiled compiler binary
4. **test-compiler.html** - Test page

## Self-Hosting Verification

```
Step 1: compiler_v1.spv compiled by JavaScript (bootstrap)
Step 2: compiler_v1.spv compiles compiler_v2.spv
Step 3: compiler_v2.spv compiles compiler_v3.spv
Step 4: Diff compiler_v2.spv == compiler_v3.spv → VERIFIED
```

## Benefits

1. **True Self-Hosting**: The OS compiles itself
2. **GPU-Accelerated**: Compilation happens on GPU
3. **Visual Feedback**: Compilation state visible in memory
4. **Bootstrapping**: Can port to new architectures by writing a minimal bootstrap
