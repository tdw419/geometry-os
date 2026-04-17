#!/usr/bin/env python3
import struct

data = open('.geometry_os/build/linux-6.14/vmlinux', 'rb').read()
is64 = data[4] == 2
if is64:
    e_phoff = struct.unpack_from('<Q', data, 32)[0]
    e_phentsize = struct.unpack_from('<H', data, 54)[0]
    e_phnum = struct.unpack_from('<H', data, 56)[0]
    highest = 0
    for i in range(e_phnum):
        off = e_phoff + i * e_phentsize
        p_type = struct.unpack_from('<I', data, off)[0]
        if p_type == 1:
            p_paddr = struct.unpack_from('<Q', data, off + 0x20)[0]
            p_memsz = struct.unpack_from('<Q', data, off + 0x28)[0]
            end = p_paddr + p_memsz
            if end > highest:
                highest = end
    print(f'ELF64: highest PA = 0x{highest:X} ({highest/1024/1024:.1f} MB)')
