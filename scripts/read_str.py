#!/usr/bin/env python3
import sys

path = '.geometry_os/build/linux-6.14/vmlinux'
va = int(sys.argv[1], 0) if len(sys.argv) > 1 else 0xC0C1B830

# LOAD segments from readelf output (offset, vaddr, filesz)
segs = [
    (0x001000, 0xC0000000, 0x21110C),
    (0x400000, 0xC0400000, 0x027572),
    (0x428000, 0xC0800000, 0x00B7B4),
    (0x434000, 0xC080C000, 0x00360),
    (0x435000, 0xC0C00000, 0x7B4A8),
    (0x4B1000, 0xC1000000, 0x002EC),
    (0x4B2000, 0xC1400000, 0x81200),
]

for off, vaddr, filesz in segs:
    end = vaddr + filesz
    if vaddr <= va < end:
        file_off = off + (va - vaddr)
        with open(path, 'rb') as f:
            f.seek(file_off)
            data = f.read(200)
            s = data.split(b'\x00')[0].decode('ascii', errors='replace')
            print(f"VA 0x{va:08X} -> file 0x{file_off:X}: \"{s}\"")
            sys.exit(0)

print(f"VA 0x{va:08X} not found")
