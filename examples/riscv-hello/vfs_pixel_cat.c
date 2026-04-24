/*
 * vfs_pixel_cat.c -- Read files from the Pixel VFS Surface
 *
 * "Pixels move pixels" -- no ecall for file reads.
 * File data lives as RGBA pixels at 0x7000_0000.
 * We load words directly from the surface.
 *
 * SBI calls used only for UART output and shutdown:
 *   a7=1 (SBI_CONSOLE_PUTCHAR), a0=char
 *   a7=8 (SBI_SHUTDOWN)
 */

#include <stdint.h>

#define VFS_SURFACE_BASE  0x70000000u
#define VFS_COLS          256

static volatile uint32_t *const surface = (volatile uint32_t *)VFS_SURFACE_BASE;
static volatile uint32_t *const uart_thr = (volatile uint32_t *)0x10000000u;

/* SBI console putchar */
static inline long sbi_console_putchar(int ch) {
    register long a0 __asm__("a0") = ch;
    register long a7 __asm__("a7") = 1;
    __asm__ volatile("ecall" : "+r"(a0) : "r"(a7) : "memory");
    return a0;
}

/* SBI shutdown */
static __attribute__((noreturn)) void sbi_shutdown(void) {
    register long a7 __asm__("a7") = 8;
    __asm__ volatile("ecall" : : "r"(a7) : "memory");
    __builtin_unreachable();
}

static void puts(const char *s) {
    while (*s) sbi_console_putchar(*s++);
}

static void put_hex(uint32_t val) {
    const char *hex = "0123456789ABCDEF";
    puts("0x");
    for (int i = 28; i >= 0; i -= 4) {
        sbi_console_putchar(hex[(val >> i) & 0xF]);
    }
}

void c_start(void) {
    /* 1. Verify PXFS magic */
    uint32_t magic = surface[0];
    if (magic != 0x50584653) {
        puts("pxcat: bad magic: ");
        put_hex(magic);
        puts(" (expected 0x50584653)\n");
        sbi_shutdown();
    }

    /* 2. Get file count */
    uint32_t file_count = surface[1];
    puts("pxcat: PXFS OK, ");
    put_hex(file_count);
    puts(" file(s)\n");

    /* 3. Walk directory index (row 0, pixels 2..2+count) */
    for (uint32_t f = 0; f < file_count && f < 254; f++) {
        uint32_t idx = surface[2 + f];
        uint32_t start_row = idx >> 16;
        uint32_t name_hash = idx & 0xFFFF;

        puts("pxcat: file[");
        put_hex(f);
        puts("] row=");
        put_hex(start_row);
        puts(" hash=");
        put_hex(name_hash);
        puts("\n");

        /* 4. Read header pixel at start_row, col 0 */
        uint32_t header = surface[start_row * VFS_COLS];
        uint32_t byte_count = header >> 16;
        uint32_t flags = header & 0xFF;

        if (byte_count == 0 || !(flags & 1)) {
            puts("pxcat:  empty/invalid\n");
            continue;
        }

        puts("pxcat:  ");
        put_hex(byte_count);
        puts(" bytes: ");

        /* 5. Read data pixels starting at start_row, col 1 */
        for (uint32_t i = 0; i < byte_count; i++) {
            uint32_t pixel_offset = i / 4;
            uint32_t byte_in_pixel = i % 4;
            uint32_t pixel = surface[start_row * VFS_COLS + 1 + pixel_offset];
            char ch = (pixel >> (byte_in_pixel * 8)) & 0xFF;
            sbi_console_putchar(ch);
        }
        puts("\n");
    }

    puts("pxcat: done\n");
    sbi_shutdown();
}
