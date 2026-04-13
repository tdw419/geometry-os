# Memory Protection (Phase 24)

## Overview

Geometry OS implements per-process memory protection through page tables. Each spawned process gets its own virtual address space, isolated from other processes and the kernel.

## Architecture

### Pages

- **Page size:** 1024 u32 words (4096 bytes)
- **Total pages:** 64 (64K RAM / 1024 words per page)
- **Pages per process:** 4 (4096 words = 16KB per process)
- **Unmapped sentinel:** 0xFFFFFFFF

### Page Tables

Each process has a page directory -- an array of 64 entries, one per virtual page. Each entry is either:
- A physical page number (0-63) -- the virtual page maps to this physical page
- `0xFFFFFFFF` (PAGE_UNMAPPED) -- the virtual page is not accessible

### Address Translation

```
virtual_address -> virtual_page = vaddr / 1024, offset = vaddr % 1024
page_directory[virtual_page] -> physical_page
physical_address = physical_page * 1024 + offset
```

If the page directory entry is PAGE_UNMAPPED or the virtual page exceeds the directory, the access triggers a **SEGFAULT**.

The kernel (main process) has no page directory, meaning all addresses are identity-mapped.

## Process Memory Layout

Each spawned process gets this virtual address space:

```
Virtual Page 0 (0x0000-0x03FF): Code segment
Virtual Page 1 (0x0400-0x07FF): Heap/data segment
Virtual Page 2 (0x0800-0x0BFF): Stack segment
Virtual Page 3 (0x0C00-0x0FFF): Shared region (identity-mapped)
```

### Code Segment (Page 0)

When a process is spawned via `SPAWN addr_reg`, the VM copies 4096 words from the parent's `addr_reg` into the child's private physical pages. The child starts executing at virtual address 0.

### Heap/Data Segment (Page 1)

General-purpose read/write memory for the process. Programs use LOAD/STORE with addresses in the 0x0400-0x07FF range.

### Stack Segment (Page 2)

Used by PUSH/POP opcodes via r30 (stack pointer). The process should initialize r30 to point into this region.

### Shared Region (Page 3, identity-mapped)

Virtual page 3 maps to physical page 3, which contains:
- **0x0F00-0x0FFF:** Window Bounds Protocol (win_x, win_y, win_w, win_h)

This is shared across all processes so they can coordinate window positioning without IPC.

## Hardware/Kernel Region (Page 63, identity-mapped)

Virtual page 63 (0xFC00-0xFFFF) is identity-mapped for all processes:
- **0xFF00-0xFFA9:** Hardware ports (keyboard, ticks, etc.)
- **0xFE00-0xFEFF:** Syscall dispatch table

**Important:** In User mode, direct STORE to addresses >= 0xFF00 is blocked (triggers SEGFAULT). User processes must use SYSCALL to access hardware. LOAD from these addresses is allowed (read-only access to hardware state).

## SEGFAULT Behavior

When a process accesses an unmapped virtual page:
1. The process is immediately halted
2. `process.segfaulted` flag is set to true
3. `RAM[0xFF9]` is set to the PID of the segfaulted process
4. The process's physical pages remain allocated until KILL is called

### Common SEGFAULT causes:
- LOAD/STORE to virtual address in unmapped page (page 4-62)
- Instruction fetch from unmapped page (e.g., jumping to unmapped code)
- STORE to hardware port region (0xFF00+) in User mode
- Stack overflow/underflow into unmapped pages

## Physical Page Allocation

The VM maintains a bitmap (`allocated_pages: u64`) of physical page usage:
- Bits 0-1: Reserved for kernel/main process
- Bits 2+: Available for child processes

`SPAWN` allocates 4 contiguous physical pages (starting search from page 2).
`KILL` frees the physical pages by clearing their bits in the bitmap.

## Syscall Convention

| Register | Purpose |
|----------|---------|
| r0 | Syscall number / return value |
| r1-r5 | Arguments |

User processes invoke `SYSCALL N` to request kernel services. The kernel handler address is looked up from `RAM[0xFE00 + N]`.

## Example: Safe Child Process

```asm
; Parent: spawn a child at address 0x200
LDI r1, 0x200
SPAWN r1      ; creates child with isolated address space
HALT

.org 0x200
; Child code (copied to child's virtual page 0)
LDI r30, 0x0A00   ; initialize stack pointer (page 2)
LDI r0, 0x0400    ; heap address (page 1)
LDI r2, 42
STORE r0, r2      ; write to own heap -- OK

; This would segfault:
; LDI r0, 0x1000
; STORE r0, r2      ; page 4 is unmapped!

HALT
```
