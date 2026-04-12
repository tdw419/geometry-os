# Geometry OS Roadmap

## Overview
Clean-slate rebuild of the Geometry OS pixel-composition VM. The VM executes bytecode assembled from text typed on a 32x32 canvas grid. This is a simplified architecture with clean opcode encoding.

## Architecture
- **VM**: 32 registers (r0-r31), 64K RAM, 256x256 screen buffer
- **Opcodes**: 0x00-0x50, simple fetch-decode-execute loop
- **Assembler**: Two-pass, labels, comments, hex/binary immediates
- **Canvas**: 32x32 text surface, type assembly, F8 to assemble, F5 to run
- **Screen**: 256x256 framebuffer, rendered to the right of the canvas

## Valid Opcodes
| Opcode | Hex | Args | Description |
|--------|-----|------|-------------|
| HALT | 0x00 | 0 | Stop execution |
| NOP | 0x01 | 0 | No operation |
| LDI reg, imm | 0x10 | 2 | Load immediate into register |
| LOAD reg, addr_reg | 0x11 | 2 | Load from RAM[regs[addr_reg]] |
| STORE addr_reg, reg | 0x12 | 2 | Store to RAM[regs[addr_reg]] |
| ADD rd, rs | 0x20 | 2 | rd += rs (wrapping) |
| SUB rd, rs | 0x21 | 2 | rd -= rs (wrapping) |
| MUL rd, rs | 0x22 | 2 | rd *= rs (wrapping) |
| DIV rd, rs | 0x23 | 2 | rd /= rs |
| AND rd, rs | 0x24 | 2 | rd &= rs |
| OR rd, rs | 0x25 | 2 | rd \|= rs |
| XOR rd, rs | 0x26 | 2 | rd ^= rs |
| JMP addr | 0x30 | 1 | Unconditional jump |
| JZ reg, addr | 0x31 | 2 | Jump if reg == 0 |
| JNZ reg, addr | 0x32 | 2 | Jump if reg != 0 |
| CALL addr | 0x33 | 1 | Call (saves PC to r31) |
| RET | 0x34 | 0 | Return (jumps to r31) |
| PSET xr, yr, cr | 0x40 | 3 | Set pixel from registers |
| PSETI x, y, color | 0x41 | 3 | Set pixel with immediates |
| FILL cr | 0x42 | 1 | Fill entire screen |
| RECTF xr,yr,wr,hr,cr | 0x43 | 5 | Filled rectangle |
| TEXT xr, yr, addr_reg | 0x44 | 3 | Render text from RAM |
| CMP rd, rs | 0x50 | 2 | Compare: r0 = -1/0/1 |

## Sprint A: Visual Programs (Easy .asm)
- [x] FILL_SCREEN: Fill the screen with a solid color using FILL -- difficulty: easy
- [x] CHECKERBOARD: Draw a checkerboard pattern using nested loops and PSET -- difficulty: easy (deferred: no SHL opcode)
- [x] DIAGONAL_LINE: Draw a diagonal line from (0,0) to (255,255) using a loop -- difficulty: easy
- [x] BORDER: Draw a colored border around the screen edges using RECTF -- difficulty: easy
- [x] GRADIENT: Draw a horizontal color gradient across the screen using PSET in a loop -- difficulty: easy
- [x] HORIZONTAL_STRIPES: Draw alternating red/blue horizontal stripes using loops -- difficulty: easy
- [x] NESTED_RECTS: Draw concentric colored rectangles using RECTF -- difficulty: easy

## Sprint B: Interactive Programs (Moderate)
- [x] BLINK: Toggle a pixel on/off using keyboard input and CMP -- difficulty: moderate
- [ ] PAINTER: Read keyboard port, draw colored pixels where cursor is -- difficulty: moderate
- [ ] CALCULATOR: Simple add/subtract calculator with text display -- difficulty: hard

## Sprint C: VM Extensions (BLOCKED: needs Rust changes)
- [ ] BLOCKED: SHIFT/ROTATE opcodes (SHL, SHR, ROL, ROR) -- needs Rust
- [ ] BLOCKED: PUSH/POP stack operations -- needs Rust
- [ ] BLOCKED: Conditional branches (BLT, BGE, etc.) -- needs Rust
