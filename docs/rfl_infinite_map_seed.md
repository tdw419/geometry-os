# Design an Infinite Map for Geometry OS using "Pixels Driving Pixels"

You are designing a feature for Geometry OS: an infinite scrolling map/canvas that a user can explore with arrow keys, where the world is procedurally generated and the ENTIRE implementation lives in Geometry OS assembly (not Rust). The design MUST use the "pixels driving pixels" primitives that already exist.

## What Already Exists (DO NOT modify Rust -- design assembly-only)

### The VM
- 64K RAM (65536 u32 cells), 32 registers (r0-r31), 256x256 screen buffer
- canvas_buffer: 4096 cells (128 rows x 32 cols), mapped to VM RAM at 0x8000-0x8FFF
- LOAD/STORE to 0x8000+ reads/writes canvas_buffer directly
- Screen buffer mapped at 0x10000-0x3FFFF (LOAD/STORE reads/writes screen pixels)

### Key Opcodes for This Design
- IKEY (0x48): Read keyboard port RAM[0xFFF] into register. Arrow keys = ASCII codes
- LOAD (0x11) / STORE (0x12): Read/write RAM (canvas at 0x8000, screen at 0x10000)
- LDI (0x10): Load immediate
- ADD (0x20), SUB (0x21), MUL (0x22), AND (0x24), SHL (0x27), SHR (0x28), SAR (0x2B)
- CMP (0x50), BLT (0x35), BGE (0x36), JZ (0x31), JNZ (0x32), JMP (0x30)
- PSET (0x40): Set screen pixel: PSET x_reg, y_reg, color_reg
- PSETI (0x41): Set pixel with immediate values
- FILL (0x42): Fill screen with color
- FRAME (0x02): Yield to renderer (animation tick)
- RAND (0x49): Pseudo-random u32
- MOV (0x51): Register copy
- ASMSELF (0x73): Assemble canvas text into bytecode at 0x1000 (self-assembly)
- RUNNEXT (0x74): Jump PC to 0x1000 to execute self-assembled code
- FORMULA (0x75): Register reactive formula on canvas cell with dependencies
- FORMULACLEAR (0x76): Clear all formulas

### Memory Map (relevant parts)
```
0x000-0x3FF    Source text (32x32 visible canvas grid)
0x1000-0x1FFF  Assembled bytecode
0x2000-0x7FFF  General purpose RAM (24K cells = world data storage)
0x8000-0x8FFF  Canvas buffer mirror (LOAD/STORE intercepted to vm.canvas_buffer)
0x9000-0xFEFF  General purpose RAM
0xFFB          Key bitmask port (bits 0-5: up/down/left/right/space/enter)
0xFFE          TICKS port (frame counter)
0xFFF          Keyboard port (ASCII key value)
0x10000+       Screen buffer (256x256 = 65536 pixels)
```

### Critical Constraints
- r0 is RESERVED for CMP results. NEVER use r0 as general register.
- Registers are u32. Use SAR by 31 for sign extraction (negative check).
- BLT/BGE check r0 after CMP. They take 2 args: BLT r0, label (NOT 3-arg form).
- Canvas buffer is 4096 cells (128 rows x 32 cols). The SCREEN is 256x256 pixels.
- World data in RAM 0x2000-0x7FFF = 24,576 cells. At 1 cell per tile, that's ~150x160 world.
- The program runs on the canvas -- it's visible, editable, self-modifying.

## What "Pixels Driving Pixels" Means Here

The thesis: the viewport-scrolling program IS visible on the pixel grid. The world data IS pixel values in RAM. The camera IS two RAM cells. Scrolling = reading world pixels and writing viewport pixels. No Rust in the loop.

Ideally:
- Arrow keys change camera_x, camera_y (two RAM cells)
- The program loops over visible area, reads world[(cam_y+row)*W+(cam_x+col)], writes to screen
- World data that hasn't been generated yet gets procedurally created
- FORMULA could make the canvas reactively linked to world data
- ASMSELF could let the program rewrite its own terrain generation rules

## Your Task

Design the complete infinite map system. For each design decision:

1. **Read the actual source code** to verify assumptions. The key files are:
   - `src/vm.rs` -- VM implementation, all opcodes, memory map
   - `src/assembler.rs` -- what assembly syntax is available
   - `docs/PIXEL_DRIVING_PIXELS.md` -- the philosophy document
   - `docs/SELF_MODIFYING_GUIDE.md` -- how ASMSELF/RUNNEXT work
   - `programs/game_of_life.asm` -- existing cellular automata example
   - `programs/canvas_grid_writer.asm` -- existing canvas write example

2. **Concrete design**: memory layout, register allocation, loop structure, rendering approach
3. **Working assembly pseudocode** for the core viewport loop
4. **Identify gaps**: what can't be done with existing opcodes? What would need new opcodes vs creative workarounds?
5. **Procedural generation strategy**: how to generate terrain on-the-fly as camera explores
6. **Self-modification opportunities**: where ASMSELF/RUNNEXT add real value vs complexity

## Constraints
- The world should feel "infinite" -- at minimum larger than the 256x256 screen
- Must run at usable frame rate (the VM can execute ~1M instructions between FRAMEs)
- The program should be demonstrable -- a user presses arrow keys and scrolls through terrain
- Prefer simple working solutions over clever architectures

## Key Questions to Resolve
- Should the viewport render to the SCREEN (256x256 via PSET) or to the CANVAS (32x128 text)?
- How to handle world boundaries? Wrap? Generate-on-demand? Both?
- Is FORMULA useful here or is a simple load/store loop better?
- How many tiles can the loop render per frame before it's too slow?
- What's the minimum viable "infinite map" -- what do we build first?
