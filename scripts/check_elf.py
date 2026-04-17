import struct

with open(".geometry_os/build/linux-6.14/vmlinux", "rb") as f:
    assert f.read(4) == b"\x7fELF"
    f.seek(28)
    phoff = struct.unpack("<I", f.read(4))[0]
    f.seek(42)
    phentsize = struct.unpack("<H", f.read(2))[0]
    f.seek(44)
    phnum = struct.unpack("<H", f.read(2))[0]
    print(f"phoff={phoff} phentsize={phentsize} phnum={phnum}")
    for i in range(phnum):
        off = phoff + i * phentsize
        f.seek(off)
        seg = f.read(phentsize)
        p_type = struct.unpack("<I", seg[0:4])[0]
        if p_type == 1:
            p_offset = struct.unpack("<I", seg[4:8])[0]
            p_vaddr = struct.unpack("<I", seg[8:12])[0]
            p_paddr = struct.unpack("<I", seg[12:16])[0]
            p_filesz = struct.unpack("<I", seg[16:20])[0]
            p_memsz = struct.unpack("<I", seg[20:24])[0]
            print(f"  LOAD: vaddr=0x{p_vaddr:08X} paddr=0x{p_paddr:08X} filesz=0x{p_filesz:X} memsz=0x{p_memsz:X} offset=0x{p_offset:X}")
