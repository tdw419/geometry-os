#!/usr/bin/env python3
"""Debug LOAD segments."""
import struct, sys

path = sys.argv[1] if len(sys.argv) > 1 else ".geometry_os/build/linux-6.14/vmlinux"

with open(path, "rb") as f:
    f.seek(0)
    magic = f.read(4)
    assert magic == b'\x7fELF', "Not ELF"
    ei_class = struct.unpack('B', f.read(1))[0]
    print(f"ELF class: {ei_class}")
    
    f.seek(4)
    ei_data = struct.unpack('B', f.read(1))[0]
    print(f"Data: {ei_data} (1=LE, 2=BE)")
    
    f.seek(28)
    e_phoff = struct.unpack('<Q', f.read(8))[0]
    e_phentsize = struct.unpack('<H', f.read(2))[0]
    e_phnum = struct.unpack('<H', f.read(2))[0]

    print(f"phoff=0x{e_phoff} phentsize={e_phentsize} phnum={e_phnum}")

    for i in range(e_phnum):
        f.seek(e_phoff + i * e_phentsize)
        ph = f.read(e_phentsize)
        print(f"\nSegment {i}: raw len={len(ph)}")
        p_type = struct.unpack('<I', ph[0:4])[0]
        p_flags = struct.unpack('<I', ph[4:8])[0]
        p_offset = struct.unpack('<Q', ph[8:16])[0]
        p_vaddr = struct.unpack('<Q', ph[16:24])[0]
        p_paddr = struct.unpack('<Q', ph[24:32])[0]
        p_filesz = struct.unpack('<Q', ph[32:40])[0]
        p_memsz = struct.unpack('<Q', ph[40:48))[0]
        p_align = struct.unpack('<Q', ph[48:56])[0]
        print(f"  type=0x{p_type:X} flags=0x{p_flags:X}")
        print(f"  offset=0x{p_offset:X} vaddr=0x{p_vaddr:08X} paddr=0x{p_paddr:08X}")
        print(f"  filesz=0x{p_filesz:X} memsz=0x{p_memsz:X} align=0x{p_align:X}")
        print(f"  vaddr_end=0x{p_vaddr + p_filesz:08X}")
