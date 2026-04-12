# Geometry OS Roadmap

## Overview
Clean-slate rebuild of the Geometry OS pixel-composition VM. The VM executes
bytecode assembled from text typed on a 32x32 canvas grid. The canvas IS a
text editor -- type assembly, F8 to assemble, F5 to run.

**Founding document:** `docs/CANVAS_TEXT_SURFACE.md`

## Architecture
- **VM**: 32 registers (r0-r31), 64K RAM, 256x256 screen buffer
- **Opcodes**: 0x00-0x61, simple fetch-decode-execute loop
- **Assembler**: Two-pass, labels, comments, hex/binary immediates
- **Canvas**: 32x32 text surface with 8x8 VGA pixel font rendering
- **Screen**: 256x256 framebuffer, rendered to the right of the canvas
- **Source at 0x000, bytecode at 0x1000** -- they never overlap

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
| OR rd, rs | 0x25 | 2 | rd |= rs |
| XOR rd, rs | 0x26 | 2 | rd ^= rs |
| SHL rd, rs | 0x27 | 2 | rd <<= rs (mod 32) |
| SHR rd, rs | 0x28 | 2 | rd >>= rs (logical, mod 32) |
| MOD rd, rs | 0x29 | 2 | rd %= rs |
| JMP addr | 0x30 | 1 | Unconditional jump |
| JZ reg, addr | 0x31 | 2 | Jump if reg == 0 |
| JNZ reg, addr | 0x32 | 2 | Jump if reg != 0 |
| CALL addr | 0x33 | 1 | Call (saves PC to r31) |
| RET | 0x34 | 0 | Return (jumps to r31) |
| BLT reg, addr | 0x35 | 2 | Branch if CMP < (r0==0xFFFFFFFF) |
| BGE reg, addr | 0x36 | 2 | Branch if CMP >= (r0!=0xFFFFFFFF) |
| PSET xr, yr, cr | 0x40 | 3 | Set pixel from registers |
| PSETI x, y, color | 0x41 | 3 | Set pixel with immediates |
| FILL cr | 0x42 | 1 | Fill entire screen |
| RECTF xr,yr,wr,hr,cr | 0x43 | 5 | Filled rectangle |
| TEXT xr, yr, addr_reg | 0x44 | 3 | Render text from RAM |
| CMP rd, rs | 0x50 | 2 | Compare: r0 = -1/0/1 |
| PUSH reg | 0x60 | 1 | Push reg onto stack (r30=SP) |
| POP reg | 0x61 | 1 | Pop from stack into reg (r30=SP) |

## Sprint A: Visual Programs (done)
- [x] FILL_SCREEN: Fill the screen with a solid color using FILL
- [x] DIAGONAL_LINE: Draw a diagonal line from (0,0) to (255,255) using a loop
- [x] BORDER: Draw a colored border around the screen edges using RECTF
- [x] GRADIENT: Draw a horizontal color gradient across the screen using PSET
- [x] HORIZONTAL_STRIPES: Draw alternating red/blue horizontal stripes
- [x] NESTED_RECTS: Draw concentric colored rectangles using RECTF

## Sprint B: Interactive Programs
- [x] BLINK: Toggle a pixel on/off using keyboard input and CMP
- [x] CALCULATOR: Simple add/subtract calculator with text display -- difficulty: hard

## Sprint C: VM Extensions (done)
- [x] SHL/SHR: Shift left/right opcodes (need vm.rs + assembler.rs)
- [x] PUSH/POP: Stack operations using a stack pointer register
- [x] BLT/BGE: Conditional branch opcodes (less than, greater or equal)
- [x] MOD: Modulo opcode

## Sprint D: Canvas Improvements
- [ ] Clipboard paste: Ctrl+V to paste text onto the grid
- [ ] File load: Ctrl+F8 to load .asm file contents onto the grid
- [ ] Scroll/pan: support programs larger than 32x32 characters
- [ ] Syntax highlighting: color opcodes, registers, numbers differently

## Sprint E: Polish
- [ ] Save/load: F7 to save RAM, restore on startup
- [ ] Disassembly panel: show bytecode alongside source text
- [ ] Single-step: Space to step one instruction when paused
- [ ] Breakpoints: mark addresses to pause at
