# Design + Build: GlyphLang Tetris Game (Phase 113)

## Context

Geometry OS is a pixel-art virtual machine with 167 opcodes, 32 registers, 64K RAM, 256x256 framebuffer. It has a GlyphLang compiler (`src/glyph_backend.rs`, 978 lines) that translates stack-based `.glyph` programs into Geometry OS assembly, then bytecode.

The GlyphLang compiler was recently expanded (Phase 112) with:
- RECTF (`[`), DRAWTEXT (`{`), FILL (`|`), FRAME (`!`), IKEY (`^`), LS (`$`), EXEC (`&`)
- String literals (`"string"`), hex numbers (`0xNN`)
- Labels (`:name`), unconditional jumps (`~name`), conditional jumps (`(label` for JZ, `)label` for JNZ)
- Registers a-z (store), A-Z (load)
- Arithmetic: `+ - * /`, Comparison: `= > <`, Conditional: `?`, Loop: `L`

The ONLY program using these new features is `programs/glyph_shell.glyph` (73 lines, a menu shell).

## Your Task

Build a playable Tetris game ENTIRELY in GlyphLang as `programs/tetris.glyph`.

Requirements:
1. **Falling pieces** -- at minimum 3 tetromino shapes (I, O, T) that fall one row per frame tick
2. **Player control** -- W=rotate, A=left, D=right, S=drop (using IKEY opcode `^`)
3. **Collision detection** -- pieces stop at the bottom or on top of other pieces
4. **Line clearing** -- completed rows disappear
5. **Score display** -- show current score using DRAWTEXT `{`
6. **Game over** -- detect when pieces stack to the top
7. **Playfield** -- visible grid on screen, pieces rendered as colored blocks using RECTF `[`

## Key Source Files (read these to verify assumptions)

- `src/glyph_backend.rs` (978 lines) -- the GlyphLang compiler. Read this to understand:
  - What tokens are available and how they compile
  - How the stack machine maps to GeoOS registers (r1-r26 = a-z, r27-r29 = scratch, r30 = SP)
  - How labels/jumps work (`:name` -> `name:`, `~name` -> `JMP name`, `(name` -> POP r27 + JZ r27,name, `)name` -> POP r27 + JNZ r27,name)
  - How RECTF compiles: pops 5 args in reverse order (color, h, w, y, x from stack)
  - How DRAWTEXT compiles: pops 5 args (bg, color, addr, y, x)
  - How IKEY compiles: reads key into r27, pushes r27

- `programs/glyph_shell.glyph` (73 lines) -- reference program using new features. Study for patterns.
- `programs/tetris.asm` (if it exists) -- there may already be an assembly Tetris. Read it for game logic inspiration.

## Constraints

- ONLY write `programs/tetris.glyph`. Do NOT modify any Rust source files.
- The program must compile via `geometry_os::glyph_backend::compile_glyph()` and assemble cleanly.
- Playfield should be centered on the 256x256 screen.
- Use FRAME (`!`) in the game loop to yield between ticks.
- Memory management is up to you -- use registers (a-z) for game state, the stack for transient values.

## Architecture Guidance

Tetris in a stack-based language is non-trivial. Consider this approach:
- **Playfield**: 10 columns x 20 rows. Store as a flat array in RAM using LOAD/STORE. But GlyphLang doesn't have LOAD/STORE yet, so use registers for key state (current piece x/y/shape, score).
- **Piece rendering**: Each frame, clear screen, draw grid border, draw placed blocks, draw current piece. All via RECTF.
- **Game loop**: FRAME + IKEY + process input + update state + redraw.
- **Simplification**: Start with just the I-piece (vertical bar) if full tetrominoes are too complex for the stack model.

## Verification

After writing the program:
1. Verify it compiles: `cargo run --bin asm_bin` on the compiled assembly
2. Write a test in `tests/capability_tests.rs` that compiles tetris.glyph, loads it in a VM, runs for 100K cycles, and verifies the screen has been drawn to (non-zero pixels in the playfield area)
3. Run all tests: `cargo test --test capability_tests`
4. Run full suite: `cargo test --lib`

## What NOT to Do

- Do NOT modify `src/glyph_backend.rs` -- the compiler is complete for this task
- Do NOT modify any Rust source files
- Do NOT create new .asm files -- the deliverable is `programs/tetris.glyph`
- Do NOT add new opcodes to GlyphLang
