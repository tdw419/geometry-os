/*
 * hello.c -- Bare-metal C hello world for Geometry OS RISC-V hypervisor.
 *
 * Uses SBI (Supervisor Binary Interface) instead of raw UART MMIO:
 *   a7=1 (SBI_CONSOLE_PUTCHAR), a0=char  -> print one character
 *   a7=8 (SBI_SHUTDOWN)                  -> clean halt
 *
 * The Geometry OS SBI dispatcher (src/riscv/sbi.rs) intercepts the ecall,
 * routes the byte through the UART, and UartBridge renders it on the canvas.
 */

static inline void sbi_console_putchar(int ch) {
    register int a0 asm("a0") = ch;
    register int a7 asm("a7") = 1;
    asm volatile("ecall" : "+r"(a0) : "r"(a7) : "memory");
}

static __attribute__((noreturn)) void sbi_shutdown(void) {
    register int a7 asm("a7") = 8;
    asm volatile("ecall" : : "r"(a7) : "memory");
    __builtin_unreachable();
}

static void sbi_puts(const char *s) {
    while (*s) {
        sbi_console_putchar(*s++);
    }
}

void c_start(void) {
    sbi_puts("hello from C\n");
    sbi_shutdown();
}
