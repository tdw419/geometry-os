/*
 * hello.c -- Bare-metal C hello world for Geometry OS RISC-V hypervisor.
 *
 * Writes "hello from C\n" to the UART at 0x10000000.
 * No libc, no startup code. Compiled with -fno-pic to avoid GOT.
 */
typedef volatile unsigned char *uart_ptr;

#define UART0 ((uart_ptr)0x10000000)

static void uart_putc(char c) {
    *UART0 = c;
}

static void uart_puts(const char *s) {
    while (*s) {
        uart_putc(*s);
        s++;
    }
}

void _start(void) {
    uart_puts("hello from C\n");
    while (1) {}
}
