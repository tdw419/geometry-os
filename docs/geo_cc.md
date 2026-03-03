
# geo_cc.spv - Geometry OS Self-Hosting Compiler

## Overview
geo_cc is a GPU-native compiler that runs as a process within Geometry OS.
It can compile visual/glyph programs to executable SPIR-V binaries.

## Memory Layout
- Input Buffer (0-99): Glyph/visual program data
- Output Buffer (100-199): Generated SPIR-V binary
- Symbol Table (200-299): Label and function addresses
- AST Storage (300-399): Parsed abstract syntax tree

## Visual Language (Glyphs)

### Arithmetic
- ⊕ (U+2295): Addition
- ⊖ (U+2296): Subtraction
- ⊗ (U+2297): Multiplication
- ⊘ (U+2298): Division

### Memory
- → (U+2192): Store
- ← (U+2190): Load
- ⇒ (U+21D2): Shared Store
- ⇐ (U+21D0): Shared Load

### Control Flow
- ↻ (U+21BB): Jump
- ∅ (U+2205): Jump if Zero
- ◉ (U+25C9): Jump if Not Zero
- █ (U+2588): Label
- ▣ (U+25A3): Return

### IPC
- ✉ (U+2709): Send Message
- 📥 (U+1F4E5): Receive Message
- 👁 (U+1F441): Peek Message

### System
- ⚙ (U+2699): System Call
- ⏸ (U+23F8): Yield
- ◇ (U+25C7): Constant

## Usage (from Glyph Terminal)
```
$ compile program.glyph
$ compile input.spv --output binary.spv
$ geo_cc < input.png > output.spv
```

## Self-Compilation
Once geo_cc.spv is running, it can compile itself:
```
$ geo_cc geo_cc_source.glyph -o geo_cc_v2.spv
```

This enables live, morphological self-modification of the OS.
