#!/usr/bin/env python3
# Read the vmlinux binary and find panic-related strings
import struct
data = open('.geometry_os/build/linux-6.14/vmlinux', 'rb').read()
searches = [
    b'not syncing', b'BUG: ', b'Unable to handle', b'Kernel panic',
    b'SMP: ', b'FATAL: ', b'No init found', b'Try passing',
    b'run_init_process', b'earlycon', b'initrd', b'page table',
    b'memblock', b'start_kernel', b'bad pmd', b'bad pte',
    b'ptdump', b'setup_vm', b'phys_addr', b'early_init',
    b'FDT', b'device tree', b'Invalid DTB',
]
for s in searches:
    idx = 0
    while True:
        idx = data.find(s, idx)
        if idx < 0:
            break
        end = data.find(b'\x00', idx)
        if end < 0:
            end = idx + 80
        full = data[idx:end].decode('ascii', errors='replace')
        print(f'  0x{idx:08X} (VA=0x{idx+0xC0000000:08X}): {full[:120]}')
        idx = end
