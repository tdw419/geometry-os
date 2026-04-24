/*
 * vfs_pixel_cat.c -- Read files from the Pixel VFS Surface
 *
 * "Pixels move pixels" -- no ecall for file reads.
 * Uses vfs_pixel.h for all VFS surface access.
 *
 * Build: ./build.sh vfs_pixel_cat.c
 */

#include "vfs_pixel.h"

void c_start(void) {
    /* 1. List all files */
    struct vfs_entry entries[16];
    int n = vfs_list(entries, 16);

    if (n < 0) {
        vfs_puts("pxcat: ERROR -- no VFS surface\n");
        sbi_shutdown();
    }

    vfs_puts("pxcat: PXFS OK, ");
    vfs_put_dec((uint32_t)n);
    vfs_puts(" file(s)\n");

    /* 2. Walk each file and print its contents */
    for (int f = 0; f < n; f++) {
        vfs_puts("pxcat: file[");
        vfs_put_dec((uint32_t)f);
        vfs_puts("] row=");
        vfs_put_dec(entries[f].start_row);
        vfs_puts(" hash=");
        vfs_put_hex(entries[f].name_hash);
        vfs_puts("\n");

        /* Read header to check validity */
        volatile uint32_t *surface = vfs_surface_ptr();
        uint32_t header = surface[entries[f].start_row * VFS_COLS];
        uint32_t byte_count = header >> 16;
        uint32_t flags = header & 0xFF;

        if (byte_count == 0 || !(flags & VFS_FLAG_VALID)) {
            vfs_puts("pxcat:  empty/invalid\n");
            continue;
        }

        /* Read and print file data */
        char buf[256];
        int bytes = vfs_read((int)entries[f].start_row, buf, sizeof(buf));
        if (bytes > 0) {
            vfs_puts("pxcat:  ");
            vfs_put_dec((uint32_t)bytes);
            vfs_puts(" bytes: ");
            for (int i = 0; i < bytes; i++) {
                sbi_console_putchar(buf[i]);
            }
            vfs_puts("\n");
        }
    }

    vfs_puts("pxcat: done\n");
    sbi_shutdown();
}
