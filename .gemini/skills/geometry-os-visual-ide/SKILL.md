---
name: geometry-os-visual-ide
description: Provides instructions and components for the Geometry OS Visual IDE, an interactive, grid-based environment for drawing visual programs using morphological glyphs. Use when the user wants to build, extend, or use a visual programming interface.
---

# Geometry OS Visual IDE

The **Visual IDE** is an interactive environment for drawing visual programs directly on a grid. These programs are composed of morphological glyphs that carry semantic data (G=opcode, B=operand) and follow a Hilbert-linear execution flow.

## Core Component: `web/VisualIDE.js`

This component provides a grid-based canvas editor.

### Key Features:
- **Interactive Drawing**: Place instruction and constant glyphs on a 16x16 grid.
- **Visual Feedback**: Real-time rendering of the visual program using `GeometryFont.js`.
- **Hilbert-Linear Execution**: Programs are mapped to a 1D sequence using the Hilbert curve for GPU execution.

## Workflow: Building a Visual Program

1.  **Initialize IDE**: Load the `VisualIDE` class and initialize it with a canvas.
2.  **Select Glyphs**: Choose from a palette of instruction glyphs (e.g., `+`, `-`, `*`) or constant glyphs.
3.  **Draw on Grid**: Click and drag on the canvas to place glyphs.
4.  **Execute**: Compile the visual grid into a SPIR-V binary and run it on the GPU via `SpirvRunner.js`.

## Extending the IDE

### Adding New Instructions
To add new instructions to the IDE:
1. Update `core/atlas_gen.py` to register the new instruction glyph with a unique G-channel opcode.
2. Update `web/assets/glyph_info.json` (by regenerating the atlas).
3. The IDE will automatically make the new glyph available if it is added to the palette.

### Implementing Compilation in the Browser
Currently, compilation from PNG to SPIR-V happens via the Python `visual_to_spirv.py` script. For a full in-browser experience:
- Port the `HilbertCurve` logic from `core/hilbert_util.py` to JavaScript.
- Implement a `VisualCompiler.js` that iterates through the IDE's grid in Hilbert order and generates a `Uint32Array` containing the SPIR-V words.
