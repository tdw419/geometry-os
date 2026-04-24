/*
 * guest_vfs_write.c -- RISC-V guest creates and writes a file via pixel surface
 *
 * Demonstrates the full VFS pixel surface write path:
 *   1. List existing files to find the surface state
 *   2. Find a free row for a new file
 *   3. Create a new file with vfs_create()
 *   4. Modify an existing file with vfs_write()
 *   5. Trigger flush to host filesystem with vfs_flush()
 *
 * All file access uses the pixel surface at 0x7000_0000.
 * No ecall/syscall for file I/O -- "pixels move pixels."
 *
 * Build: ./build.sh guest_vfs_write.c
 */

#include "vfs_pixel.h"

void c_start(void) {
    /* 1. Verify the VFS surface is alive */
    struct vfs_entry entries[16];
    int n = vfs_list(entries, 16);

    if (n < 0) {
        vfs_puts("guest_vfs_write: ERROR -- no VFS surface\n");
        sbi_shutdown();
    }

    vfs_puts("guest_vfs_write: PXFS surface OK, ");
    vfs_put_dec((uint32_t)n);
    vfs_puts(" existing file(s)\n");

    /* 2. List existing files */
    for (int i = 0; i < n; i++) {
        vfs_puts("  file[");
        vfs_put_dec((uint32_t)i);
        vfs_puts("] row=");
        vfs_put_dec(entries[i].start_row);
        vfs_puts(" hash=");
        vfs_put_hex(entries[i].name_hash);
        vfs_puts("\n");
    }

    /* 3. Read the first file (if any) and print its contents */
    if (n > 0) {
        char buf[256];
        int bytes = vfs_read((int)entries[0].start_row, buf, sizeof(buf) - 1);
        if (bytes > 0) {
            buf[bytes] = '\0';
            vfs_puts("  contents: ");
            for (int i = 0; i < bytes && i < 80; i++) {
                sbi_console_putchar(buf[i]);
            }
            vfs_puts("\n");
        }
    }

    /* 4. Find a free row and create a new file */
    int free_row = vfs_find_free_row();
    vfs_puts("guest_vfs_write: free_row=");
    vfs_put_dec((uint32_t)free_row);
    vfs_puts("\n");

    if (free_row > 0) {
        const char *new_content = "Hello from RISC-V guest!\nWritten via pixel surface at row ";
        char full_content[128];
        int j = 0;
        for (const char *p = new_content; *p && j < 90; p++) {
            full_content[j++] = *p;
        }
        /* Append row number */
        full_content[j++] = '(';
        if (free_row == 0) {
            full_content[j++] = '0';
        } else {
            char tmp[8];
            int t = 0;
            int r = free_row;
            while (r > 0) { tmp[t++] = '0' + (r % 10); r /= 10; }
            while (t > 0) full_content[j++] = tmp[--t];
        }
        full_content[j++] = ')';
        full_content[j++] = '\n';

        vfs_create("guest_log.txt", free_row, full_content, j);
        vfs_puts("guest_vfs_write: created guest_log.txt at row ");
        vfs_put_dec((uint32_t)free_row);
        vfs_puts(" (");
        vfs_put_dec((uint32_t)j);
        vfs_puts(" bytes)\n");
    }

    /* 5. If there's an existing file, append a line to it */
    if (n > 0) {
        char existing[256];
        int bytes = vfs_read((int)entries[0].start_row, existing, sizeof(existing));
        if (bytes > 0 && bytes < 200) {
            /* Append a timestamp-like marker */
            existing[bytes++] = '\n';
            existing[bytes++] = '[';
            existing[bytes++] = 'g';
            existing[bytes++] = 'u';
            existing[bytes++] = 'e';
            existing[bytes++] = 's';
            existing[bytes++] = 't';
            existing[bytes++] = ']';

            vfs_write((int)entries[0].start_row, existing, bytes);
            vfs_puts("guest_vfs_write: appended to existing file (now ");
            vfs_put_dec((uint32_t)bytes);
            vfs_puts(" bytes)\n");
        }
    }

    /* 6. Flush everything to the host filesystem */
    vfs_flush();
    vfs_puts("guest_vfs_write: flushed to host\n");

    /* 7. Verify: re-read the created file */
    if (free_row > 0) {
        struct vfs_entry new_entries[16];
        int new_count = vfs_list(new_entries, 16);
        vfs_puts("guest_vfs_write: after create, ");
        vfs_put_dec((uint32_t)new_count);
        vfs_puts(" files\n");

        char verify[128];
        int vbytes = vfs_read(free_row, verify, sizeof(verify) - 1);
        if (vbytes > 0) {
            verify[vbytes] = '\0';
            vfs_puts("  verify: ");
            for (int i = 0; i < vbytes && i < 40; i++) {
                sbi_console_putchar(verify[i]);
            }
            vfs_puts("\n");
        }
    }

    vfs_puts("guest_vfs_write: done\n");
    sbi_shutdown();
}
