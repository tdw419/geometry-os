/*
 * hello.S -- Bare-metal RISC-V assembly hello world for Geometry OS hypervisor.
 * Writes "hello from C\n" to UART at 0x10000000.
 * Uses lla (load local address) with .option norelax to avoid GOT.
 */
    .globl _start
_start:
    .option push
    .option norelax
    lla     a0, msg            /* a0 = address of string */
    .option pop
loop:
    lbu     a1, 0(a0)          /* load char */
    beqz    a1, halt           /* null? done */
    li      a2, 0x10000000     /* UART base */
    sb      a1, 0(a2)          /* write to UART THR */
    addi    a0, a0, 1          /* next char */
    j       loop
halt:
    wfi
    j       halt
    .balign 4
msg:
    .asciz "hello from C\n"
