import struct

with open('.geometry_os/build/linux-6.14/vmlinux', 'rb') as f:
    data = f.read()

e_phoff = struct.unpack_from('<I', data, 28)[0]
e_phentsize = struct.unpack_from('<H', data, 42)[0]
e_phnum = struct.unpack_from('<H', data, 44)[0]

print(f'phoff={e_phoff} phentsize={e_phentsize} phnum={e_phnum}')

for i in range(e_phnum):
    off = e_phoff + i * e_phentsize
    seg = data[off:off+e_phentsize]
    p_type = struct.unpack_from('<I', seg, 0)[0]
    if p_type != 1:
        continue
    p_offset = struct.unpack_from('<I', seg, 4)[0]
    p_vaddr = struct.unpack_from('<I', seg, 8)[0]
    p_paddr = struct.unpack_from('<I', seg, 12)[0]
    p_filesz = struct.unpack_from('<I', seg, 16)[0]
    p_memsz = struct.unpack_from('<I', seg, 20)[0]
    p_flags = struct.unpack_from('<I', seg, 24)[0]
    print(f'Seg{i}: vaddr=0x{p_vaddr:08X} paddr=0x{p_paddr:08X} filesz=0x{p_filesz:X} memsz=0x{p_memsz:X} flags=0x{p_flags:X}')
    
    # Check if 0xC0003018 falls in this segment
    if p_vaddr <= 0xC0003018 < p_vaddr + p_memsz:
        file_off = p_offset + (0xC0003018 - p_vaddr)
        print(f'  VA 0xC0003018 -> PA 0x{p_paddr + (0xC0003018 - p_vaddr):08X}')
        print(f'  File offset: 0x{file_off:X}')
        if file_off + 4 <= len(data):
            word = struct.unpack_from('<I', data, file_off)[0]
            print(f'  Word at file offset: 0x{word:08X}')

# Also check what the loader sees at PA 0x3018
print()
print('Checking what the kernel should have at PA 0x3018 (mapped from VA 0xC0003018):')
# The kernel loads segments at p_paddr. For the first segment with vaddr starting near 0xC0000000,
# p_paddr should be near 0x00000000
for i in range(e_phnum):
    off = e_phoff + i * e_phentsize
    seg = data[off:off+e_phentsize]
    p_type = struct.unpack_from('<I', seg, 0)[0]
    if p_type != 1:
        continue
    p_offset = struct.unpack_from('<I', seg, 4)[0]
    p_vaddr = struct.unpack_from('<I', seg, 8)[0]
    p_paddr = struct.unpack_from('<I', seg, 12)[0]
    p_filesz = struct.unpack_from('<I', seg, 16)[0]
    p_memsz = struct.unpack_from('<I', seg, 20)[0]
    
    # Check if PA 0x3018 falls in this segment
    if p_paddr <= 0x3018 < p_paddr + p_filesz:
        file_off = p_offset + (0x3018 - p_paddr)
        word = struct.unpack_from('<I', data, file_off)[0]
        print(f'  PA 0x3018 is in segment {i} (paddr=0x{p_paddr:08X}-0x{p_paddr+p_filesz:08X})')
        print(f'  File offset: 0x{file_off:X}, word: 0x{word:08X}')
