/*
 * crt0.S -- Minimal C runtime startup for bare-metal RISC-V programs
 * targeting the Geometry OS hypervisor.
 *
 * Sets up the stack pointer (pointing into the end of the 1MB RAM region
 * at 0x80000000 + 1MB = 0x80100000), zeroes BSS, then calls _start.
 *
 * Link this BEFORE your C source in the build command:
 *   gcc crt0.S hello.c -o hello.elf ...
 *   gcc crt0.S cat.c -o cat.elf ...
 */

    .section .text.entry
    .globl _start
_start:
    /* Set up stack pointer at top of 1MB RAM (0x80100000) */
    lui     sp, 0x80100

    /* Zero BSS section */
    la      t0, __bss_start
    la      t1, __bss_end
1:
    bge     t0, t1, 2f
    sw      zero, 0(t0)
    addi    t0, t0, 4
    j       1b
2:
    /* Call the C entry point */
    jal     ra, c_start

    /* If c_start returns, shut down */
    li      a7, 8              /* SBI_SHUTDOWN */
    ecall
