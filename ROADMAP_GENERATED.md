# Pixel Driving Pixels

Self-modifying canvas programs in Geometry OS. Pixels write pixels.

**Progress:** 3/6 phases complete, 0 in progress

**Deliverables:** 0/18 complete

**Tasks:** 0/40 complete

## Scope Summary

| Phase | Status | Deliverables | LOC Target | Tests |
|-------|--------|-------------|-----------|-------|
| phase-45 RAM-Mapped Canvas Buffer | COMPLETE | 0/5 | 370 | 10 |
| phase-46 RAM-Mapped Screen Buffer | COMPLETE | 0/3 | 220 | 8 |
| phase-47 Self-Assembly Opcode (ASMSELF) | COMPLETE | 0/3 | 340 | 8 |
| phase-48 Self-Execution Opcode (RUNNEXT) | PLANNED | 0/2 | 140 | 5 |
| phase-49 Self-Modifying Programs: Demos and Patterns | PLANNED | 0/2 | 400 | - |
| phase-50 Reactive Canvas: Live Cell Formulas | FUTURE | 0/3 | 800 | 10 |

## Dependencies

| From | To | Type | Reason |
|------|----|------|--------|
| phase-45 | phase-46 | hard | Screen mapping follows the same interception pattern as canvas mapping |
| phase-45 | phase-47 | hard | ASMSELF needs the canvas buffer to be addressable so programs can write to it first |
| phase-46 | phase-47 | soft | Screen mapping not required for ASMSELF, but useful for demos |
| phase-47 | phase-48 | hard | RUNNEXT is meaningless without ASMSELF to produce the bytecode |
| phase-45 | phase-49 | hard | Canvas RAM mapping needed for all demos |
| phase-46 | phase-49 | soft | Screen mapping needed for Game of Life demo |
| phase-47 | phase-49 | hard | ASMSELF needed for self-writing and code evolution demos |
| phase-48 | phase-49 | hard | RUNNEXT needed for self-writing and code evolution demos |
| phase-49 | phase-50 | soft | Reactive canvas builds on proven self-modifying patterns |

## [x] phase-45: RAM-Mapped Canvas Buffer (COMPLETE)

**Goal:** Make the canvas grid addressable from VM RAM via STORE/LOAD

The canvas buffer (128 rows x 32 cols = 4096 cells) currently lives in a separate Vec<u32> outside VM RAM. Map it into the VM address space at 0x8000-0x8FFF so that existing STORE and LOAD opcodes can read and write grid cells directly. No new opcodes needed -- just intercept the address range in the VM's memory access path.


### Deliverables

- [ ] **Canvas memory region constant and address mapping** -- Define CANVAS_RAM_BASE = 0x8000, CANVAS_RAM_SIZE = 4096 (128*32). Document the mapping: address 0x8000 + row*32 + col corresponds to canvas_buffer[row * 32 + col]. Add to memory map docs.

  - [ ] `p45.d1.t1` Define CANVAS_RAM_BASE and CANVAS_RAM_SIZE constants
    > Add `pub const CANVAS_RAM_BASE: usize = 0x8000;` and `pub const CANVAS_RAM_SIZE: usize = 4096;` to vm.rs (or main.rs if canvas_buffer ownership stays there). These are the address range [0x8000, 0x8FFF] that maps to the canvas grid.
    - Constants defined and visible to both vm.rs and main.rs
    _Files: src/vm.rs_
  - [ ] `p45.d1.t2` Update CANVAS_TEXT_SURFACE.md memory map with 0x8000 range (depends: p45.d1.t1)
    > Add a row to the memory map table in CANVAS_TEXT_SURFACE.md: 0x8000-0x8FFF | 4096 | Canvas grid (RAM-mapped mirror of canvas_buffer)
    - Memory map shows 0x8000-0x8FFF as canvas region
    _Files: docs/CANVAS_TEXT_SURFACE.md_
  - [ ] CANVAS_RAM_BASE constant defined in vm.rs or main.rs
    _Validation: grep CANVAS_RAM_BASE src/vm.rs src/main.rs_
  - [ ] Memory map documentation updated in CANVAS_TEXT_SURFACE.md
    _Validation: grep 0x8000 docs/CANVAS_TEXT_SURFACE.md_
  _~20 LOC_
- [ ] **Intercept LOAD for canvas address range** -- In the LOAD opcode handler (0x11 in vm.rs), when the translated physical address falls in [CANVAS_RAM_BASE, CANVAS_RAM_BASE + 4095], read from canvas_buffer instead of self.ram. The VM needs a reference or copy of the canvas buffer. Easiest approach: the canvas_buffer is passed to the VM (or VM holds a reference) so LOAD can index into it.

  - [ ] `p45.d2.t1` Add canvas_buffer reference to VM struct (depends: p45.d1.t1)
    > The VM struct needs access to the canvas buffer for both LOAD and STORE interception. Add a field like `pub canvas_buffer: Vec<u32>` to the VM struct (a copy that gets synced back to main.rs canvas_buffer each frame) OR pass it as a mutable reference through the execute method. The copy approach is simpler and avoids lifetime issues.
    - VM struct has access to canvas buffer data
    - cargo build succeeds
    _Files: src/vm.rs, src/main.rs_
  - [ ] `p45.d2.t2` Intercept LOAD opcode for canvas range (depends: p45.d2.t1)
    > In the LOAD handler (opcode 0x11), after page translation produces a physical address, check if it falls in [CANVAS_RAM_BASE, CANVAS_RAM_BASE + CANVAS_RAM_SIZE). If so, read from the canvas buffer at (addr - CANVAS_RAM_BASE) instead of self.ram[addr]. The canvas buffer index maps directly: canvas_buffer[addr - 0x8000].
    - LOAD from canvas addr returns the glyph value stored there
    - LOAD from normal RAM addr is unchanged
    _Files: src/vm.rs_
  - [ ] `p45.d2.t3` Sync canvas_buffer to VM before execution (depends: p45.d2.t1)
    > Before each frame's VM execution, copy the current canvas_buffer contents into the VM's canvas mirror (or set up the reference). This ensures the VM sees the latest grid state including human-typed text.
    - VM canvas mirror matches main.rs canvas_buffer at start of each frame
    _Files: src/main.rs_
  - [ ] LOAD from 0x8000+row*32+col returns canvas_buffer value
    _Validation: Write test program: STORE to canvas addr, LOAD back, verify_
  - [ ] LOAD from addresses outside 0x8000-0x8FFF still works normally
    _Validation: Existing tests pass without modification_
  _~80 LOC_
- [ ] **Intercept STORE for canvas address range** -- In the STORE opcode handler (0x12 in vm.rs), when the translated physical address falls in [CANVAS_RAM_BASE, CANVAS_RAM_BASE + 4095], write to canvas_buffer instead of self.ram. After the store, mark the canvas as dirty so the renderer picks up the change on the next frame.

  - [ ] `p45.d3.t1` Intercept STORE opcode for canvas range (depends: p45.d2.t1)
    > In the STORE handler (opcode 0x12), after page translation, check if the address is in the canvas range. If so, write to the canvas buffer at (addr - CANVAS_RAM_BASE) instead of self.ram[addr]. Bypass the user-mode protection for this range (canvas is not I/O).
    - STORE to canvas addr writes to canvas buffer
    - User-mode programs can write to canvas (no segfault)
    _Files: src/vm.rs_
  - [ ] `p45.d3.t2` Sync VM canvas mutations back to main canvas_buffer (depends: p45.d3.t1)
    > After each frame's VM execution, copy any changed canvas cells from the VM's mirror back to main.rs's canvas_buffer. This ensures the renderer displays the VM's writes. A simple full-copy each frame is fine (4096 u32 values = 16KB).
    - Changes made by VM via STORE appear on the visible canvas grid
    _Files: src/main.rs_
  - [ ] `p45.d3.t3` Handle User-mode access to canvas region (depends: p45.d3.t1)
    > The STORE handler currently blocks User-mode writes to addr >= 0xFF00. The canvas range (0x8000) is below this threshold, so User-mode should work by default. But verify and add a comment clarifying that canvas writes are permitted in User mode. If any page translation or protection logic would block it, add an explicit exception.
    - User-mode programs can STORE to canvas range without segfault
    _Files: src/vm.rs_
  - [ ] STORE to 0x8000+row*32+col writes value to canvas_buffer
    _Validation: Write test: STORE 0x8000 with 'H', see 'H' appear on grid_
  - [ ] Stored values appear as glyphs on the canvas grid
    _Validation: Visual test: program writes ASCII chars, grid shows them_
  - [ ] STORE to addresses outside canvas range still works
    _Validation: Existing tests pass_
  _~60 LOC_
- [ ] **Test suite for RAM-mapped canvas** -- Write tests that verify STORE/LOAD to canvas addresses work correctly. Test read-after-write, boundary conditions, interaction with normal RAM, and multi-process canvas access.

  - [ ] `p45.d4.t1` Test: LOAD reads canvas buffer values (depends: p45.d2.t2, p45.d3.t1)
    > Write a test that pre-fills canvas_buffer cells with known values, runs a program that LOADs from 0x8000+offset, and checks the register contains the expected value.
    - Test asserts register value matches canvas cell content
    _Files: src/vm.rs_
  - [ ] `p45.d4.t2` Test: STORE writes appear in canvas buffer (depends: p45.d3.t1)
    > Write a test that runs a program storing values to canvas addresses, then checks the canvas buffer contains those values.
    - Test asserts canvas_buffer has stored values after execution
    _Files: src/vm.rs_
  - [ ] `p45.d4.t3` Test: boundary conditions (first/last cell, row boundaries) (depends: p45.d3.t1)
    > Test STORE/LOAD at 0x8000 (first cell), 0x8FFF (last cell), and at row boundaries (e.g. end of row 0, start of row 1). Verify no off-by-one errors.
    - All boundary addresses read/write correctly
    _Files: src/vm.rs_
  - [ ] `p45.d4.t4` Test: canvas access does not corrupt normal RAM (depends: p45.d3.t1)
    > Write a test that stores to both normal RAM and canvas addresses, then verifies the normal RAM values are unchanged and the canvas values are correct. Ensures the two memory spaces don't overlap.
    - RAM values unchanged after canvas writes
    - Canvas values unchanged after RAM writes
    _Files: src/vm.rs_
  - [ ] `p45.d4.t5` Test: page translation works with canvas addresses (depends: p45.d2.t2, p45.d3.t1)
    > Verify that LOAD/STORE to canvas addresses still go through the page translation mechanism. A process with a page table that maps 0x8000 to a different physical address should see the translated result. Or if canvas is identity-mapped, verify that works.
    - Canvas LOAD/STORE respects page translation
    _Files: src/vm.rs_
  - [ ] At least 5 tests covering canvas LOAD/STORE behavior
    _Validation: cargo test passes with new tests_
  - [ ] All existing tests still pass
    _Validation: cargo test --no-fail-fast 2>&1 | tail -5_
  _~150 LOC_
- [ ] **Demo program: canvas grid writer** -- Write an assembly program that writes ASCII characters to the canvas grid using STORE. The program fills the grid with a visible pattern -- for example, writing "HELLO WORLD" across the top row, or filling the grid with sequential ASCII values. The human sees the text appear on the grid while the program runs.

  - [ ] `p45.d5.t1` Write canvas_grid_writer.asm demo (depends: p45.d3.t1)
    > Create programs/canvas_grid_writer.asm. The program uses LDI to load ASCII values and STORE to write them to 0x8000+ addresses. Writes "PIXELS DRIVE PIXELS" across the first visible row. Uses a loop with an index register incrementing through the string.
    - Program assembles without errors
    - Running the program shows text on the canvas grid
    _Files: programs/canvas_grid_writer.asm_
  - [ ] `p45.d5.t2` Write canvas_counter.asm demo (depends: p45.d3.t1)
    > Create programs/canvas_counter.asm. A loop that increments a counter and writes the digit (as ASCII) to a specific canvas cell each iteration. The human sees a digit ticking up on the grid in real time.
    - Counter digit visibly changes on the grid each frame
    _Files: programs/canvas_counter.asm_
  - [ ] Program writes visible text to canvas grid via STORE
    _Validation: Load program, F8 assemble, F5 run, see text on grid_
  - [ ] Demo program added to programs/ directory
    _Validation: ls programs/canvas_*.asm_
  _~60 LOC_

### Technical Notes

The VM's RAM is 0x10000 (65536 cells). The canvas buffer is 4096 cells. Mapping at 0x8000 leaves plenty of headroom (0x9000-0xFFFF still available). The screen buffer (256x256 = 65536 pixels) is too large for a contiguous RAM mapping -- that's addressed in phase 46.
Canvas buffer sync strategy: copy main's canvas_buffer into VM before execution, copy VM's canvas writes back after execution. 4096 * 4 bytes = 16KB per frame, negligible cost.
The page translation layer (translate_va_or_fault) must be considered. For kernel-mode processes (the default for canvas-assembled programs), virtual address == physical address. For user-mode child processes, the page table may remap things. The canvas range should work through the normal translation path.


### Risks

- Page translation might block canvas access for user-mode processes
- Canvas buffer ownership between main.rs and vm.rs needs careful handling
- STORE handler's user-mode protection (addr >= 0xFF00 check) must not block canvas writes

## [x] phase-46: RAM-Mapped Screen Buffer (COMPLETE)

**Goal:** Make the 256x256 screen buffer addressable from VM RAM

The screen buffer (256x256 = 65536 pixels) is currently only accessible via PIXEL (write) and PEEK (read) opcodes. Map it into the VM address space at 0x9000-0x13FFF (a 64K region) so that normal LOAD/STORE can read and write screen pixels. This unifies all three memory spaces (RAM, canvas, screen) under one addressing scheme.


### Deliverables

- [ ] **Screen memory region mapping** -- Define SCREEN_RAM_BASE = 0x10000. The screen is 256x256 = 65536 cells, so it spans 0x9000-0x18FFF. However, VM RAM is only 0x10000 total. Options: (a) expand RAM_SIZE to 0x20000, (b) use a sparse/aliased mapping where only low-res access works, (c) map screen at a higher address with extended RAM. Simplest: expand RAM to 0x20000 (128K) and map screen at 0x10000.

  - [ ] `p46.d1.t1` Determine screen mapping strategy and expand RAM if needed (depends: p45.d3.t1)
    > Evaluate options for mapping the 64K screen buffer. The simplest approach: expand RAM_SIZE from 0x10000 to 0x20000 (128K) and map the screen buffer at 0x10000. This keeps everything in one flat address space. Alternative: use a windowed mapping at 0x9000 where only a 4K window is visible at a time (controlled by a register). Recommend the flat mapping for simplicity.
    - Decision documented with address range and RAM size
    _Files: src/vm.rs_
  - [ ] `p46.d1.t2` Implement screen buffer LOAD interception (depends: p46.d1.t1)
    > In the LOAD handler, check if the translated address falls in the screen buffer range. If so, read from self.screen[addr - SCREEN_RAM_BASE] instead of self.ram[addr]. The screen buffer already exists on the VM struct as `pub screen: Vec<u32>`.
    - LOAD from screen addr returns pixel color value
    _Files: src/vm.rs_
  - [ ] `p46.d1.t3` Implement screen buffer STORE interception (depends: p46.d1.t1)
    > In the STORE handler, check if the translated address falls in the screen buffer range. If so, write to self.screen[addr - SCREEN_RAM_BASE]. The renderer will pick up the change on the next frame automatically since it reads from self.screen.
    - STORE to screen addr changes the visible pixel
    _Files: src/vm.rs_
  - [x] Screen buffer is LOAD/STORE accessible at a defined address range
    _Validation: LOAD from screen addr returns same value as PEEK_
  - [x] Existing PIXEL and PEEK opcodes still work
    _Validation: cargo test passes_
  _~100 LOC_
- [ ] **Tests for screen buffer mapping** -- Verify that LOAD/STORE to screen addresses correctly read and write pixels. Cross-validate against PEEK and PIXEL opcodes.

  - [ ] `p46.d2.t1` Test: LOAD from screen matches PEEK (depends: p46.d1.t2)
    > Write a test that draws a pixel with PIXEL opcode, then reads it with both PEEK and LOAD (via screen-mapped address). Verify both return the same color value.
    - PEEK and LOAD return identical values
    _Files: src/vm.rs_
  - [ ] `p46.d2.t2` Test: STORE to screen matches PIXEL (depends: p46.d1.t3)
    > Write a test that writes a pixel via both PIXEL opcode and STORE to screen-mapped address. Read back with PEEK and verify both wrote the same value.
    - Both methods produce identical pixel values on screen
    _Files: src/vm.rs_
  - [x] LOAD from screen address matches PEEK result
    _Validation: Test program: PEEK and LOAD same pixel, compare registers_
  - [x] STORE to screen address matches PIXEL result
    _Validation: Test program: STORE and PIXEL write same location, compare_
  _~80 LOC_
- [ ] **Unified memory map documentation** -- Update all memory map documentation to show the complete unified address space: RAM (0x0000-0x7FFF), canvas (0x8000-0x8FFF), screen (0x10000+). Add a new doc section showing the full map.

  - [ ] `p46.d3.t1` Write UNIFIED_MEMORY_MAP section in docs (depends: p46.d1.t3)
    > Add a section to CANVAS_TEXT_SURFACE.md (or create UNIFIED_MEMORY_MAP.md) showing the complete address space: 0x0000-0x0FFF: bytecode/data, 0x1000-0x1FFF: canvas bytecode, 0x8000-0x8FFF: canvas grid (mirror), 0x10000-0x1FFFF: screen buffer. Explain the design: one address space, three backing stores, LOAD/STORE as the universal access method.
    - Document shows all regions with address ranges and purposes
    _Files: docs/CANVAS_TEXT_SURFACE.md_
  - [x] All three regions documented in one place
    _Validation: grep 'canvas\|screen\|RAM' docs/CANVAS_TEXT_SURFACE.md shows unified map_
  _~40 LOC_

### Technical Notes

The screen buffer (self.screen) is already a field on the VM struct, unlike canvas_buffer which lives in main.rs. This makes interception simpler -- no sync step needed.
RAM_SIZE expansion from 0x10000 to 0x20000 adds 256KB of memory (64K u32 cells). At current RAM usage this is fine. The screen mapping at 0x10000 means screen pixels are at screen[y * 256 + x], accessed as RAM[0x10000 + y * 256 + x].
Alternative: don't expand RAM, instead use a separate mapping that redirects LOAD/STORE at 0x9000-0xFFFF to the screen buffer. But this creates an address collision with I/O ports (0xFFB-0xFFF). Expanding RAM is cleaner.


### Risks

- RAM_SIZE expansion may affect fuzzer or existing test assumptions about address space
- Screen buffer is 256x256=64K which exactly fills the expansion -- no room for growth
- Page translation for screen addresses may need special handling

## [x] phase-47: Self-Assembly Opcode (ASMSELF) (COMPLETE)

**Goal:** Add an opcode that lets a running program assemble canvas text into bytecode

Add the ASMSELF opcode (or RECOMPILE) that reads the current canvas text, runs it through the preprocessor and assembler, and stores the resulting bytecode at 0x1000. This lets a program write new assembly onto the canvas grid (using STORE to the canvas range from phase 45) and then compile it without human intervention. Combined, a program can generate its own replacement.


### Deliverables

- [ ] **ASMSELF opcode implementation** -- New opcode (suggest 0x52 or next available). When executed: 1. Read the canvas buffer as a text string (same logic as F8 handler) 2. Run through preprocessor::preprocess() 3. Run through assembler::assemble() 4. If success: write bytecode to 0x1000, set a flag 5. If failure: set an error register/port with the error info The VM needs access to the preprocessor and assembler modules.

  - [ ] `p47.d1.t1` Add ASMSELF opcode constant and handler skeleton (depends: p45.d3.t1)
    > Reserve the next available opcode number for ASMSELF. Add a stub handler in the VM's execute loop that reads the canvas buffer, converts to text string, and logs "ASMSELF called" for now.
    - Opcode constant defined in vm.rs
    - Handler appears in execute match arm
    _Files: src/vm.rs_
  - [ ] `p47.d1.t2` Implement canvas-to-text conversion in VM context (depends: p47.d1.t1)
    > Extract the canvas-to-text conversion logic from the F8 handler in main.rs into a reusable function. This function takes a &[u32] (canvas buffer slice) and returns a String. The F8 handler and the ASMSELF opcode both call this function. Place it in a shared module (e.g., preprocessor.rs or a new canvas.rs).
    - Function exists and is callable from both vm.rs and main.rs
    - F8 handler refactored to use the shared function
    _Files: src/vm.rs, src/main.rs_
  - [ ] `p47.d1.t3` Wire preprocessor and assembler into ASMSELF handler (depends: p47.d1.t2)
    > In the ASMSELF handler, after getting the text string from the canvas: call preprocessor::preprocess(), then assembler::assemble(). On success, write bytecode bytes to self.ram starting at CANVAS_BYTECODE_ADDR (0x1000). On failure, write the error string to a memory-mapped error port or a designated RAM region. The VM will need to import/use the preprocessor and assembler modules.
    - ASMSELF produces valid bytecode at 0x1000
    - Invalid assembly writes error info without crashing
    _Files: src/vm.rs_
  - [ ] `p47.d1.t4` Add ASMSELF to disassembler (depends: p47.d1.t1)
    > Add the ASMSELF opcode to the disassemble() method in vm.rs so it appears correctly in trace output and disassembly views.
    - Disassembler shows ASMSELF with correct operand count
    _Files: src/vm.rs_
  - [ ] `p47.d1.t5` Add ASMSELF to assembler mnemonic list (depends: p47.d1.t1)
    > Add "ASMSELF" to the OPCODES list in preprocessor.rs and the assembler in assembler.rs. It takes no operands (just the opcode byte). Update the opcode count in docs and meta.
    - Can type ASMSELF in assembly source and it assembles
    - Opcode count incremented in documentation
    _Files: src/assembler.rs, src/preprocessor.rs_
  - [x] ASMSELF assembles canvas text into bytecode at 0x1000
    _Validation: Program writes text to canvas, calls ASMSELF, then LOADs from 0x1000 to verify bytecode_
  - [x] Assembly errors are reported without crashing the VM
    _Validation: Write invalid text to canvas, call ASMSELF, VM continues running_
  _~200 LOC_
- [ ] **Assembly status port** -- Define a memory-mapped port (e.g., 0xFFE or 0xFFA) where the ASMSELF opcode writes its result: success (bytecode length) or failure (0xFFFFFFFF). Programs poll this port after calling ASMSELF to check if assembly succeeded.

  - [ ] `p47.d2.t1` Define ASM_STATUS port and write logic (depends: p47.d1.t3)
    > Use existing RAM[0xFFD] (ASM result port) which already exists for this purpose (bytecode word count, or 0xFFFFFFFF on error). Ensure ASMSELF writes to this port identically to how F8 assembly does.
    - RAM[0xFFD] contains result after ASMSELF
    _Files: src/vm.rs_
  - [x] Port shows bytecode length on success
    _Validation: LOAD from status port after ASMSELF returns positive number_
  - [x] Port shows 0xFFFFFFFF on failure
    _Validation: LOAD from status port after bad ASMSELF returns 0xFFFFFFFF_
  _~20 LOC_
- [ ] **Test suite for ASMSELF** -- Test that ASMSELF correctly assembles canvas text, handles errors, and the resulting bytecode is executable.

  - [ ] `p47.d3.t1` Test: ASMSELF assembles valid canvas text (depends: p47.d1.t3)
    > Pre-fill canvas buffer with "LDI r0, 42\nHALT\n". Execute ASMSELF. Verify RAM[0xFFD] contains a positive byte count. Verify RAM at 0x1000 contains expected bytecode for LDI r0, 42.
    - Bytecode at 0x1000 matches hand-assembled LDI r0, 42; HALT
    _Files: src/vm.rs_
  - [ ] `p47.d3.t2` Test: ASMSELF handles invalid assembly gracefully (depends: p47.d1.t3)
    > Pre-fill canvas with garbage text. Execute ASMSELF. Verify RAM[0xFFD] contains 0xFFFFFFFF. Verify VM did not crash and continues executing.
    - Error port set, VM still running
    _Files: src/vm.rs_
  - [ ] `p47.d3.t3` Test: program writes code to canvas then assembles it (depends: p47.d1.t3)
    > Full integration test: a program uses STORE to write "LDI r0, 99\nHALT\n" to the canvas address range, calls ASMSELF, then jumps to 0x1000 (or uses RUNNEXT from phase 48). Verify r0 ends up as 99.
    - Self-written program executes correctly after ASMSELF
    _Files: src/vm.rs_
  - [x] ASMSELF assembles and the result runs correctly
    _Validation: Test program: write simple ASM to canvas, ASMSELF, jump to 0x1000, verify behavior_
  _~120 LOC_

### Technical Notes

The assembler and preprocessor are currently called from main.rs. The VM (vm.rs) will need to import them. Since vm.rs is a separate module, this means adding `use crate::assembler;` and `use crate::preprocessor;` to vm.rs.
The canvas-to-text conversion currently lives in the F8 handler in main.rs. It reads 4096 cells, converts each u32 to a char, collapses newlines. This logic needs to be extracted into a shared function. The function should be in a neutral module (preprocessor.rs is a good candidate since it already handles text processing).
ASMSELF takes no operands (1-byte instruction). The assembled bytecode always goes to 0x1000 (CANVAS_BYTECODE_ADDR), same as F8. This means calling ASMSELF overwrites whatever bytecode is currently running. The program should use RUNNEXT (phase 48) to jump to the new bytecode.


### Risks

- ASMSELF during execution replaces the running bytecode -- program must jump to new code carefully
- Preprocessor/assembler errors in a running VM context need careful error handling
- Self-assembly is inherently dangerous (infinite loops, corrupting own code)

## [ ] phase-48: Self-Execution Opcode (RUNNEXT) (PLANNED)

**Goal:** Add an opcode that starts executing the newly assembled bytecode

Add the RUNNEXT opcode that sets PC to 0x1000 (the canvas bytecode region) and continues execution. Combined with ASMSELF, a program can write new code onto the canvas, compile it, and run it -- all from within the VM. This closes the loop: pixels write pixels, pixels assemble pixels, pixels execute pixels.


### Deliverables

- [ ] **RUNNEXT opcode implementation** -- New opcode (next available after ASMSELF). When executed: 1. Set PC = CANVAS_BYTECODE_ADDR (0x1000) 2. Reset halted flag 3. Clear any error state 4. Execution continues from the new bytecode on the next fetch cycle
This is essentially JMP 0x1000 but with awareness that the bytecode at 0x1000 was just assembled from canvas text. Could be implemented as a simple PC set, or as JMP with an implicit operand.

  - [ ] `p48.d1.t1` Implement RUNNEXT opcode handler (depends: p47.d1.t1)
    > Add RUNNEXT opcode in vm.rs execute match. Handler sets self.pc = CANVAS_BYTECODE_ADDR (0x1000). No operands needed (1-byte instruction). Register file is preserved. The VM continues fetching from the new PC on the next cycle.
    - PC set to 0x1000 after RUNNEXT
    - Execution continues from new bytecode
    _Files: src/vm.rs_
  - [ ] `p48.d1.t2` Add RUNNEXT to disassembler and assembler (depends: p48.d1.t1)
    > Add RUNNEXT to the mnemonic list in assembler.rs, the OPCODES list in preprocessor.rs, and the disassemble() method in vm.rs. No operands.
    - RUNNEXT appears in trace output correctly
    - Can type RUNNEXT in assembly source
    _Files: src/vm.rs, src/assembler.rs, src/preprocessor.rs_
  - [ ] RUNNEXT starts executing bytecode at 0x1000
    _Validation: Program writes code, ASMSELF, RUNNEXT, verify new code runs_
  - [ ] Register state preserved across RUNNEXT
    _Validation: r0-r26 retain their values after RUNNEXT_
  _~40 LOC_
- [ ] **Test suite for RUNNEXT** -- Test the full write-compile-execute cycle. A program writes new code, assembles it, runs it, and the new code's effects are visible.

  - [ ] `p48.d2.t1` Test: RUNNEXT executes newly assembled code (depends: p47.d1.t3, p48.d1.t1)
    > Write a test program that: (1) stores "LDI r0, 77\nHALT" to canvas addresses, (2) calls ASMSELF, (3) checks RAM[0xFFD] for success, (4) calls RUNNEXT, (5) verify r0 == 77 after execution.
    - r0 == 77 after RUNNEXT
    _Files: src/vm.rs_
  - [ ] `p48.d2.t2` Test: registers preserved across RUNNEXT (depends: p48.d1.t1)
    > Set r5 = 12345. Write code to canvas that reads r5 and adds 1. ASMSELF, RUNNEXT. Verify r5 is still 12345 in the new program's context, and that the new program can read it.
    - Register values survive the transition
    _Files: src/vm.rs_
  - [ ] `p48.d2.t3` Test: chained self-modification (depends: p48.d1.t1)
    > Program A writes Program B to canvas. ASMSELF. RUNNEXT. Program B writes Program C to canvas. ASMSELF. RUNNEXT. Program C HALTs. Verify all three ran in sequence. This is the generational self-modification test.
    - Three generations of code execute in sequence
    _Files: src/vm.rs_
  - [ ] Full write-compile-execute cycle works end-to-end
    _Validation: Test program writes LDI r0, 77 to canvas, ASMSELF, RUNNEXT, verify r0=77_
  _~100 LOC_

### Technical Notes

RUNNEXT is intentionally simple: it just sets PC = 0x1000. The complexity is in ASMSELF (phase 47). RUNNEXT could alternatively be implemented as a JMP to a label at 0x1000, but having a dedicated opcode makes the intent clear and enables future extensions (e.g., RUNNEXT with a timeout, RUNNEXT in a sandboxed context).
Register preservation: RUNNEXT does NOT reset registers. The new program inherits all register state. This is by design -- it allows data passing between program generations. If a clean slate is needed, the new program can zero registers itself.
Stack preservation: the return stack is NOT reset. This means the new program can RET back to the caller if the caller used CALL before RUNNEXT. This is a feature, not a bug -- it enables coroutines.


### Risks

- Infinite self-modification loops (program rewrites itself forever)
- Assembler errors in a running context could leave the VM in a bad state

## [ ] phase-49: Self-Modifying Programs: Demos and Patterns (PLANNED)

**Goal:** Build demonstration programs that showcase the pixel-driving-pixels capability

With phases 45-48 complete, write programs that demonstrate the full self-modifying capability: programs that write their own code, programs whose state IS the display, programs that evolve over time. These demos prove that the pixel-driving-pixels problem is solved.


### Deliverables

- [ ] **Demo: Self-writing program** -- A program that writes another program onto the canvas grid using STORE to canvas addresses, calls ASMSELF to compile it, and RUNNEXT to execute it. The generated program is different from the original -- it's a true successor. The human watches text appear on the grid, then sees the new program run.

  - [ ] `p49.d1.t1` Write programs/self_writer.asm (depends: p48.d1.t1)
    > A program that uses STORE to canvas addresses (0x8000+) to write "LDI r0, 42\nLDI r1, 1\nADD r0, r1\nHALT\n" onto the grid. The text becomes visible as typed glyphs. Then calls ASMSELF and RUNNEXT. The successor runs and r0 = 43.
    - Text appears on grid before assembly
    - Successor program executes correctly
    _Files: programs/self_writer.asm_
  - [ ] `p49.d1.t2` Write programs/evolving_counter.asm (depends: p45.d3.t1)
    > A program that counts frames (via TICKS port 0xFFE) and writes the count as ASCII digits directly onto the canvas grid. The grid becomes a live dashboard. The count digits are the program's visible state -- no separate output. The digit changes each frame. This demonstrates that the grid IS the display.
    - Digits visibly increment on the canvas grid
    _Files: programs/evolving_counter.asm_
  - [ ] `p49.d1.t3` Write programs/game_of_life.asm (depends: p46.d1.t3)
    > Conway's Game of Life implemented entirely in Geometry OS assembly. Uses PEEK to read the screen, POKE (or STORE to screen-mapped RAM) to write the next generation. The screen IS the cellular automaton. No Rust code involved in the logic -- pure pixel-driven-pixels. Initialize with a glider or blinker pattern.
    - Cells evolve according to Conway's rules
    - Gliders move, blinkers blink
    _Files: programs/game_of_life.asm_
  - [ ] `p49.d1.t4` Write programs/code_evolution.asm (depends: p48.d1.t1)
    > The crown jewel demo. A program that writes increasingly complex versions of itself to the canvas grid. Generation 0 just halts. Generation 1 writes generation 2 which adds a counter. Generation 2 writes generation 3 which adds a screen effect. Each generation writes its successor, compiles, and runs it. The human watches the code evolve on the grid in real time.
    - At least 3 generations of code evolution
    - Each generation visibly different from the last
    _Files: programs/code_evolution.asm_
  - [ ] Program generates a visually different successor and runs it
    _Validation: Load demo, F5, watch grid change, see new program execute_
  _~300 LOC_
- [ ] **Documentation: pixel-driving-pixels patterns** -- Write a guide for building self-modifying programs. Document the patterns: canvas STORE for writing code, ASMSELF for compiling, RUNNEXT for executing, register passing between generations, and common pitfalls (infinite loops, corrupting your own code).

  - [ ] `p49.d2.t1` Write docs/SELF_MODIFYING_GUIDE.md (depends: p48.d1.t1)
    > Create a guide covering: (1) Canvas STORE pattern -- how to write text to canvas cells, (2) ASMSELF + RUNNEXT pattern -- compile and execute, (3) Register passing -- sharing state between generations, (4) Self-reading -- using LOAD from canvas to inspect your own source, (5) Pitfalls -- infinite loops, corruption, error handling. Include code snippets for each pattern.
    - Guide covers all 5 topics with working code examples
    _Files: docs/SELF_MODIFYING_GUIDE.md_
  - [ ] Guide document exists with at least 3 documented patterns
    _Validation: ls docs/SELF_MODIFYING_GUIDE.md_
  _~100 LOC_

### Technical Notes

Demo programs should be small enough to fit on the canvas grid (32 columns, 128 rows). Complex programs may need to use the .org directive for layout. The code_evolution demo is the most ambitious -- it may need careful tuning to keep each generation's code within grid size limits.
The game_of_life.asm demo is the purest expression of pixels-driving-pixels. It needs the screen buffer mapping from phase 46 to work optimally, but could also work with just PEEK and PIXEL opcodes.


### Risks

- Demo programs may be too complex to write in raw assembly within grid size limits
- Code evolution demo may be too ambitious for initial implementation

## [?] phase-50: Reactive Canvas: Live Cell Formulas (FUTURE)

**Goal:** Make canvas cells react to changes in other cells automatically

Extend the canvas with an optional formula layer. A cell can have a formula instead of a static value. When a referenced cell changes, the formula cell recalculates. This is the spreadsheet model applied to the pixel grid. It makes the canvas reactive without explicit STORE/LOAD loops.
This is a future phase -- it depends on phases 45-48 being stable and is a natural evolution of the pixel-driving-pixels concept. Not required for the initial self-modifying capability.


### Deliverables

- [ ] **Formula cell type and evaluation engine** -- Add a parallel buffer (formula_buffer, same size as canvas_buffer) where each cell can optionally hold a formula instead of a value. Formulas reference other cells by address. When a STORE writes to a cell, the engine checks if any formula depends on that cell and recalculates.

- [ ] **Formula syntax in preprocessor** -- Extend the preprocessor to recognize formula syntax in canvas text. A line like `= r0 + r1` means "this cell displays the value of r0 + r1". The preprocessor generates the reactive update hooks.

- [ ] **Demo: live register dashboard** -- A program where the canvas grid shows live register values. As the program runs, the grid cells update to show r0, r1, r2 etc. as changing digits. The display IS the debug view. No separate inspector.


### Technical Notes

This phase explores the spreadsheet model. Each cell can be: - A literal value (current behavior) - A formula that references other cells or registers - A formula that references screen pixels (PEEK-equivalent)
The dependency graph needs cycle detection to prevent infinite recalculation. Simple approach: single-pass topological sort of formula dependencies, recalculate in order after any STORE to the canvas.
This is marked "future" because it's a significant new feature. The core pixel-driving-pixels capability (phases 45-48) does not require this.


## Global Risks

- Self-modifying code is inherently hard to debug -- need good error reporting
- Assembly inside a running VM may be slow for large programs -- may need optimization
- The concept of a program rewriting itself challenges test design -- how do you unit test a program that changes?
- RAM size expansion (phase 46) affects the fuzzer which generates random addresses

## Conventions

- All new opcodes added to assembler.rs, preprocessor.rs OPCODES list, and vm.rs disassembler
- Opcode numbers assigned sequentially from next available
- Canvas and screen mappings use LOAD/STORE interception, not new opcodes
- ASMSELF and RUNNEXT take no operands (1-byte instructions)
- Error reporting via RAM[0xFFD] (existing ASM result port)
