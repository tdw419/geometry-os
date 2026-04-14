# Recursive Feedback Loop — Conversation Export

Generated: 2026-04-14T16:50:58.387693

## USER (Iteration 0)

# Task: Fix Linux RV32 Boot in Geometry OS RISC-V Interpreter

## Current State
The RISC-V interpreter can load a Linux RV32 kernel (vmlinux ELF) and execute ~256K instructions before getting stuck. The kernel boots in M-mode, does BSS clearing, sets up initial page tables, then takes a load page fault (mcause=0xD) at PC=0xC00BB2C0 while still in Machine mode. 

The current M-mode trap handler (installed at boot time in `src/riscv/mod.rs` boot_linux()) just skips the faulting instruction (mepc += 4) and returns via mret. This is wrong -- it causes the kernel to use uninitialized values and jump to garbage address 0x00006604, where it takes repeated instruction page faults (mcause=0xC) forever.

## What Needs to Happen
The trap handler needs to be smarter. When a page fault happens in M-mode, the handler should forward it to S-mode (set sepc=mepc, scause=mcause, stval=mtval, then jump to stvec via mret) so the kernel's own page fault handler can set up the missing mapping and retry.

There are two approaches:

**Approach A: Rust-level trap forwarding**
In `boot_linux()` in `src/riscv/mod.rs`, instead of injecting a machine-code trap handler, intercept traps at the Rust level. In the step loop (around line 335), detect when PC lands at the trap handler address and check mcause. If it's a page fault (12/13/15), manually set sepc/scause/stval and redirect PC to stvec. Otherwise skip the instruction.

**Approach B: Better machine-code handler**
Write a proper assembly trap handler that checks mcause and forwards page faults to S-mode. The handler is written into guest RAM at fw_addr.

## Key Files
- `src/riscv/mod.rs` -- boot_linux() function, installs trap handler and runs step loop
- `src/riscv/cpu.rs` -- CPU step(), trap handling, privilege transitions  
- `src/riscv/csr.rs` -- CSR read/write, trap_target_priv(), trap_vector()
- `src/riscv/mmu.rs` -- SV32 page table translation, translate()
- `examples/boot_linux_test.rs` -- Test harness that boots Linux and prints UART output

## How to Test
After making changes:
1. `cargo test` -- must pass all ~1011 tests
2. `cargo run --example boot_linux_test` -- should show UART output containing "Linux version"
3. The kernel should progress past 256K instructions without getting stuck

## Verification
Run: `cargo run --example boot_trap_trace` (if it exists) or create a small example that runs 5M instructions and checks:
- mcause should NOT be stuck on 0xC (instruction page fault)
- UART canvas should contain text
- PC should not be stuck in the trap handler at 0xC0940000

## Rules
- Do NOT break existing tests (1011 tests must pass)
- Do NOT modify the MMU translate() logic -- page table walking works correctly
- The CSR delegation (medeleg=0xB309, mideleg=0x222) is correct
- Focus on the trap handler in boot_linux() -- that's where the bug is
- Prefer Approach A (Rust-level interception) -- it's more maintainable than hand-encoded RISC-V machine code

---
