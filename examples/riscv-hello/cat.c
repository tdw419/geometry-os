/*
 * cat.c -- DEPRECATED: Read a file from Geometry OS VFS via SBI GEO_VFS_READ.
 *
 * THIS EXAMPLE IS DEPRECATED. GEO_VFS_READ now returns NOT_SUPPORTED.
 * Use vfs_pixel_cat.c instead, which reads file data directly from
 * the Pixel VFS Surface at 0x7000_0000 (no ecall needed).
 *
 * See docs/design/pixel-vfs-surface.md for the replacement API.
 *
 * Original SBI calls used:
 *   a7=1 (SBI_CONSOLE_PUTCHAR), a0=char    -> print one char
 *   a7=0x47454F00 (SBI_EXT_GEOMETRY), a6=0 (GEO_VFS_READ)
 *     a0=name_ptr, a1=name_len, a2=buf_ptr, a3=buf_len
 *     -> returns bytes_read in a0
 *   a7=8 (SBI_SHUTDOWN) -> clean halt
 */

#include <stdint.h>

/* SBI console putchar: print one character */
static inline long sbi_console_putchar(int ch) {
    register long a0 __asm__("a0") = ch;
    register long a7 __asm__("a7") = 1;
    __asm__ volatile("ecall" : "+r"(a0) : "r"(a7) : "memory", "a1");
    return a0;
}

/* SBI shutdown: halt the machine */
static __attribute__((noreturn)) void sbi_shutdown(void) {
    register long a7 __asm__("a7") = 8;
    __asm__ volatile("ecall" : : "r"(a7) : "memory", "a0", "a1");
    __builtin_unreachable();
}

/* Geometry OS VFS read hypercall */
static inline long geo_vfs_read(const char *name, uint32_t name_len,
                                 void *buf, uint32_t buf_len) {
    register long a0 __asm__("a0") = (long)(uintptr_t)name;
    register long a1 __asm__("a1") = (long)name_len;
    register long a2 __asm__("a2") = (long)(uintptr_t)buf;
    register long a3 __asm__("a3") = (long)buf_len;
    register long a6 __asm__("a6") = 0; /* GEO_FN_VFS_READ */
    register long a7 __asm__("a7") = 0x47454F00; /* SBI_EXT_GEOMETRY */
    __asm__ volatile("ecall"
        : "+r"(a0)
        : "r"(a1), "r"(a2), "r"(a3), "r"(a6), "r"(a7)
        : "memory");
    return a0;
}

static void sbi_puts(const char *s) {
    while (*s) {
        sbi_console_putchar(*s++);
    }
}

static void sbi_put_hex(uint32_t val) {
    const char *hex = "0123456789ABCDEF";
    sbi_puts("0x");
    for (int i = 28; i >= 0; i -= 4) {
        sbi_console_putchar(hex[(val >> i) & 0xF]);
    }
}

void c_start(void) {
    /* Buffer for file contents */
    static char buf[512];

    sbi_puts("cat: reading test.txt...\n");

    long n = geo_vfs_read("test.txt", 8, buf, sizeof(buf) - 1);

    if (n < 0) {
        sbi_puts("cat: error reading file, code=");
        sbi_put_hex((uint32_t)n);
        sbi_puts("\n");
    } else {
        sbi_puts("cat: read ");
        sbi_put_hex((uint32_t)n);
        sbi_puts(" bytes:\n");
        buf[n] = '\0';
        sbi_puts(buf);
    }

    sbi_puts("cat: done.\n");
    sbi_shutdown();
}
