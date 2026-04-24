/*
 * hello.S -- Bare-metal RISC-V assembly hello for Geometry OS hypervisor.
 * Prints "hello from C\n" via SBI_CONSOLE_PUTCHAR and halts via SBI_SHUTDOWN.
 *
 * SBI calls: a7 = extension ID, a0..a5 = args, return in a0.
 *   a7=1 (SBI_CONSOLE_PUTCHAR), a0=char -> print one char
 *   a7=8 (SBI_SHUTDOWN)                 -> clean halt
 */
    .globl _start
_start:
    .option push
    .option norelax
    lla     s0, msg            /* s0 = cursor into string (callee-saved) */
    .option pop
loop:
    lbu     a0, 0(s0)          /* a0 = current char */
    beqz    a0, done           /* null terminator? done */
    li      a7, 1              /* SBI_CONSOLE_PUTCHAR */
    ecall
    addi    s0, s0, 1
    j       loop
done:
    li      a7, 8              /* SBI_SHUTDOWN */
    ecall
    /* Unreachable, but just in case */
1:  j       1b
    .balign 4
msg:
    .asciz "hello from C\n"
