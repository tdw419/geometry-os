import struct
with open('.geometry_os/build/linux-6.14/vmlinux', 'rb') as f:
    magic = f.read(4)
    assert magic == b'\x7fELF'
    ei_class = struct.unpack('B', f.read(1))[0]
    f.seek(0)
    hdr = f.read(52 if ei_class == 1 else 64)
    if ei_class == 1:
        e_phoff = struct.unpack_from('<I', hdr, 28)[0]
        e_phentsize = struct.unpack_from('<H', hdr, 42)[0]
        e_phnum = struct.unpack_from('<H', hdr, 44)[0]
        entry = struct.unpack_from('<I', hdr, 24)[0]
    else:
        e_phoff = struct.unpack_from('<Q', hdr, 32)[0]
        e_phentsize = struct.unpack_from('<H', hdr, 54)[0]
        e_phnum = struct.unpack_from('<H', hdr, 56)[0]
        entry = struct.unpack_from('<Q', hdr, 24)[0]
    print(f'ELF class: {"32" if ei_class==1 else "64"}-bit')
    print(f'Entry: 0x{entry:08X}')
    f.seek(e_phoff)
    for i in range(e_phnum):
        phdr = f.read(e_phentsize)
        if ei_class == 1:
            p_type = struct.unpack_from('<I', phdr, 0)[0]
            p_offset = struct.unpack_from('<I', phdr, 4)[0]
            p_vaddr = struct.unpack_from('<I', phdr, 8)[0]
            p_paddr = struct.unpack_from('<I', phdr, 12)[0]
            p_filesz = struct.unpack_from('<I', phdr, 16)[0]
            p_memsz = struct.unpack_from('<I', phdr, 20)[0]
        else:
            p_type = struct.unpack_from('<I', phdr, 0)[0]
            p_offset = struct.unpack_from('<Q', phdr, 8)[0]
            p_vaddr = struct.unpack_from('<Q', phdr, 16)[0]
            p_paddr = struct.unpack_from('<Q', phdr, 24)[0]
            p_filesz = struct.unpack_from('<Q', phdr, 32)[0]
            p_memsz = struct.unpack_from('<Q', phdr, 40)[0]
        if p_type == 1:
            print(f'  LOAD: vaddr=0x{p_vaddr:08X} paddr=0x{p_paddr:08X} filesz=0x{p_filesz:06X} memsz=0x{p_memsz:06X} end=0x{p_paddr+p_memsz:08X}')
