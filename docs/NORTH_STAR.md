# Geometry OS North Star

## The One Rule

**Every change must move Geometry OS closer to being a real operating system.**

A real OS has: memory protection, a filesystem, proper scheduling, IPC, device abstraction, a shell, and programs that use all of it. If a change doesn't advance one of those, it's not the priority.

## Priority Hierarchy

1. **Kernel features** (syscalls, memory protection, scheduling, IPC) -- this IS the OS
2. **Programs that prove kernel features work** -- a syscall without a program using it is vaporware
3. **Filesystem and persistence** -- programs need to store and load data
4. **Shell and user interface** -- the user needs to interact with the OS
5. **Standard library** -- raises the floor for all programs
6. **Assembler/VM quality-of-life** -- only when it unblocks something above

## DO

- Build the kernel boundary first, then memory protection, then filesystem, in that order
- Write a test program for every new syscall or opcode before moving on
- Keep existing programs working -- add a compatibility mode if kernel mode breaks them
- Make every commit leave `cargo test` green
- Document syscall conventions as you build them

## DON'T

- Add opcodes without a program that needs them
- Polish the visual debugger instead of building kernel features
- Spend a session on refactoring when there's kernel work to do
- Build speculative features ("wouldn't it be cool if...") -- build what the roadmap says
- Skip tests. Every new VM behavior gets a test.

## The Drift Risk

This project has 22 phases of VM construction behind it. The VM is fun to tinker with. New opcodes are easy to add. Visual polish is tempting. All of that is drift now. The goal is an OS, not a better VM. The VM serves the OS.

## The Test

After any change, ask: "Is Geometry OS more like Linux/Windows/macOS than it was before?" If no, it's probably not the right work right now.
