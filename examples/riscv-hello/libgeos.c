/*
 * libgeos.c -- shared bare-metal primitives for Geometry OS guest programs
 *
 * SBI console I/O and utility functions. Linked as libgeos.a.
 * Framebuffer inline helpers are in libgeos.h (zero function-call overhead).
 *
 * Build: riscv64-linux-gnu-gcc -c -march=rv32imac_zicsr -mabi=ilp32 -O2 libgeos.c
 *        riscv64-linux-gnu-ar rcs libgeos.a libgeos.o
 */

#include "libgeos.h"

/* ---- SBI helpers ---- */

long sbi_console_putchar(int ch) {
    register long a0 __asm__("a0") = ch;
    register long a7 __asm__("a7") = 1;
    __asm__ volatile("ecall" : "+r"(a0) : "r"(a7) : "memory", "a1");
    return a0;
}

__attribute__((noreturn)) void sbi_shutdown(void) {
    register long a7 __asm__("a7") = 8;
    __asm__ volatile("ecall" : : "r"(a7) : "memory", "a0", "a1");
    __builtin_unreachable();
}

long sbi_console_getchar(void) {
    register long a0 __asm__("a0") = 0;
    register long a7 __asm__("a7") = 2;
    __asm__ volatile("ecall" : "+r"(a0) : "r"(a7) : "memory", "a1");
    return a0;
}

/* Read one character from SBI console, blocking until available. */
char geos_getchar(void) {
    long ch;
    while ((ch = sbi_console_getchar()) < 0) {
        /* spin until a character is available */
    }
    return (char)ch;
}

/* ---- Console output ---- */

void geos_puts(const char *s) {
    while (*s) sbi_console_putchar(*s++);
}

void geos_put_dec(uint32_t val) {
    if (val == 0) {
        sbi_console_putchar('0');
        return;
    }
    char buf[12];
    int i = 0;
    while (val > 0) {
        buf[i++] = '0' + (val % 10);
        val /= 10;
    }
    while (i > 0) sbi_console_putchar(buf[--i]);
}

void geos_put_hex(uint32_t val) {
    static const char hex[] = "0123456789ABCDEF";
    sbi_console_putchar('0');
    sbi_console_putchar('x');
    for (int i = 28; i >= 0; i -= 4) {
        sbi_console_putchar(hex[(val >> i) & 0xF]);
    }
}
