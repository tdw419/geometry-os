# Geometry OS AI Guide

## Project Overview

Geometry OS is a pixel-art virtual machine evolving into a real operating system. It's written in Rust with a minifb GUI, a custom assembler, and 36 programs. 185 tests (183 passing, 2 ignored), all green.

The goal: build toward modern OS standards (kernel boundary, memory protection, filesystem, scheduler, IPC, device drivers, shell).

## Architecture

```
src/
  main.rs       -- GUI (minifb), input handling, rendering (1024x768 host window)
  vm.rs         -- VM core: 60 opcodes, 32 registers, 64K RAM, 256x256 screen, multi-process
  assembler.rs  -- Two-pass assembler: labels, #define, .org, .db, .asciz
  preprocessor.rs -- Abstraction layer: VAR/SET/GET/INC/DEC macros, shared tokenizer
  font.rs       -- 8x8 VGA bitmap font for text rendering
  lib.rs        -- Re-exports
tests/
  program_tests.rs -- 140 tests: assembly, execution, opcode behavior, multi-process, IPC
programs/
  *.asm         -- 36 programs: games, demos, self-hosting assembler, window manager
docs/
  NORTH_STAR.md -- Read this FIRST. Defines what work matters.
  CANVAS_TEXT_SURFACE.md -- How the text editor/assembler pipeline works
  KEYSTROKE_TO_PIXELS.md -- Foundational keystroke handling
roadmap.yaml    -- Single source of truth for project state and deliverables
ROADMAP.md      -- Human-readable roadmap with priority checkboxes
```

## Key Conventions

1. **Every new opcode gets a test** in tests/program_tests.rs
2. **Every new program gets assembled** by test_all_programs_assemble
3. **Every commit leaves `cargo test` green** -- no exceptions
4. **roadmap.yaml is truth** -- update it when deliverables land
5. **Programs prove the need** -- no speculative opcodes
6. **The OPCODES list in preprocessor.rs must sync with assembler.rs**
7. **Temp registers r27-r29 are reserved** for preprocessor macros
8. **Opcode 0x52+** is where new OS opcodes go (SYSCALL, RETK, etc.)

## VM State

- 32 registers (r0-r31), r30 = stack pointer, r31 = call return
- 65536 u32 words of RAM
- 256x256 u32 screen buffer
- Multi-process: up to 8 spawned processes with own registers, shared RAM
- Hardware ports: 0xFFF (keyboard), 0xFFE (ticks), 0xFFB (multi-key bitmask), 0xFFC (network), 0xFFA (last PID)
- Window Bounds Protocol: RAM[0xF00..0xF03] for spatial coordination between processes

## Test Command

```bash
cargo test                    # run all tests (should be ~115)
cargo test test_name          # run specific test
cargo build 2>&1 | grep -E "^error|^warning\["  # check for errors/warnings
```

## How to Add a New Opcode

1. Add the opcode handler in `vm.rs` match block
2. Add the disassembler entry in `vm.rs` disassemble_at match
3. Add the assembler dispatch in `assembler.rs`
4. Add to the OPCODES list in `preprocessor.rs` (for syntax highlighting)
5. Add to opcode reference in `main.rs` (Hermes context)
6. Write tests in `tests/program_tests.rs`
7. Update `roadmap.yaml` deliverable status
8. Write a program that uses it

## Current Phase: 30 (Boot Sequence & Init)

Building the boot sequence and init process. Read docs/NORTH_STAR.md before starting.
