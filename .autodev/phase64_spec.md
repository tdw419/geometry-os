# phase-64: MIN/MAX + CLAMP Opcodes + Screensaver Demo

**Goal:** Add value clamping opcodes and a screensaver demo program

## Deliverables

- **MIN opcode (0x89)** -- MIN rd, rs -- rd = min(rd, rs)
- **MAX opcode (0x8A)** -- MAX rd, rs -- rd = max(rd, rs)
- **CLAMP opcode (0x8B)** -- CLAMP rd, min_reg, max_reg -- rd = clamp(rd, min, max)
- **MIN/MAX/CLAMP assembler + disassembler entries** --  
- **MIN/MAX/CLAMP tests** -- Test edge cases: equal values, negative, overflow
- **screensaver.asm** -- Multi-effect screensaver with bouncing logos, starfield, plasma cycling. Auto-starts after N seconds of no input.

## Context

This is for Geometry OS, a pixel-art VM with 113 opcodes.
Source files: src/vm/mod.rs (VM execution), src/vm/disasm.rs (disassembler),
src/asm/system_ops.rs (assembler). Opcode numbers: MIN=0x89, MAX=0x8A, CLAMP=0x8B.
Each opcode needs: VM handler, assembler entry, disassembler entry, and tests.