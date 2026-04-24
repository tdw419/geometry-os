/*
 * test_surface.c -- Minimal test to verify VFS pixel surface reads work.
 * Reads surface[0], [1], [2] and prints them via UART.
 */
#include "vfs_pixel.h"

void c_start(void) {
    volatile uint32_t *surface = (volatile uint32_t *)0x70000000u;

    /* Read and print first 6 surface pixels */
    vfs_puts("test: magic=");
    vfs_put_hex(surface[0]);
    vfs_puts("\n");

    vfs_puts("test: count=");
    vfs_put_dec(surface[1]);
    vfs_puts("\n");

    vfs_puts("test: entry[0]=");
    vfs_put_hex(surface[2]);
    vfs_puts("\n");

    vfs_puts("test: entry[1]=");
    vfs_put_hex(surface[3]);
    vfs_puts("\n");

    vfs_puts("test: entry[2]=");
    vfs_put_hex(surface[4]);
    vfs_puts("\n");

    vfs_puts("test: entry[3]=");
    vfs_put_hex(surface[5]);
    vfs_puts("\n");

    /* Now test via vfs_list */
    struct vfs_entry entries[16];
    int n = vfs_list(entries, 16);
    vfs_puts("test: vfs_list returned ");
    vfs_put_dec((uint32_t)n);
    vfs_puts("\n");

    for (int i = 0; i < n; i++) {
        vfs_puts("test: vfs_entry[");
        vfs_put_dec((uint32_t)i);
        vfs_puts("] row=");
        vfs_put_dec(entries[i].start_row);
        vfs_puts(" hash=");
        vfs_put_hex(entries[i].name_hash);
        vfs_puts("\n");
    }

    vfs_puts("test: done\n");
    sbi_shutdown();
}
