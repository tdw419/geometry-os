#!/usr/bin/env python3
"""Read a string from vmlinux at a given virtual address."""
import struct, sys

va = int(sys.argv[1], 0) if len(sys.argv) > 1 else 0xC0C1B830
path = sys.argv[2] if len(sys.argv) > 2 else ".geometry_os/build/linux-6.14/vmlinux"

with open(path, "rb") as f:
    f.seek(0)
    magic = f.read(4)
    assert magic == b'\x7fELF', "Not ELF"
    ei_class = struct.unpack('B', f.read(1))[0]
    f.seek(28)
    e_phoff = struct.unpack('<Q', f.read(8))[0]
    e_phentsize = struct.unpack('<H', f.read(2))[0]
    e_phnum = struct.unpack('<H', f.read(2))[0]

    print(f"Looking for VA 0x{va:08X}")
    for i in range(e_phnum):
        f.seek(e_phoff + i * e_phentsize)
        ph = f.read(e_phentsize)
        p_type = struct.unpack('<I', ph[0:4])[0]
        p_flags = struct.unpack('<I', ph[4:8])[0]
        p_offset = struct.unpack('<Q', ph[8:16])[0]
        p_vaddr = struct.unpack('<Q', ph[16:24])[0]
        p_paddr = struct.unpack('<Q', ph[24:32])[0]
        p_filesz = struct.unpack('<Q', ph[32:40])[0]
        p_memsz = struct.unpack('<Q', ph[40:48])[0]

        if p_type != 1:
            continue

        seg_end = p_vaddr + max(p_filesz, p_memsz)
        print(f"  LOAD[{i}]: VA 0x{p_vaddr:08X}-0x{seg_end:08X} (filesz=0x{p_filesz:X} memsz=0x{p_memsz:X}) flags=0x{p_flags:X}")

        if p_vaddr <= va < seg_end:
            file_off = p_offset + (va - p_vaddr)
            f.seek(file_off)
            data = f.read(200)
            s = data.split(b'\x00')[0].decode('ascii', errors='replace')
            print(f"  -> file 0x{file_off:08X}: \"{s}\"")
            break
    else:
        print(f"  VA 0x{va:08X} not found in any LOAD segment")
