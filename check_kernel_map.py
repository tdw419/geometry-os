import struct

with open('.geometry_os/build/linux-6.14/vmlinux', 'rb') as f:
    data = f.read()

# Parse ELF header to find program headers (32-bit ELF)
e_phoff = struct.unpack_from('<I', data, 28)[0]
e_phentsize = struct.unpack_from('<H', data, 42)[0]
e_phnum = struct.unpack_from('<H', data, 44)[0]

# kernel_map VA = 0xC0C79E90
target_va = 0xC0C79E90

for i in range(e_phnum):
    off = e_phoff + i * e_phentsize
    p_type = struct.unpack_from('<I', data, off)[0]
    p_offset = struct.unpack_from('<I', data, off+4)[0]
    p_vaddr = struct.unpack_from('<I', data, off+8)[0]
    p_filesz = struct.unpack_from('<I', data, off+16)[0]
    p_memsz = struct.unpack_from('<I', data, off+20)[0]
    
    if p_type == 1 and p_vaddr <= target_va < p_vaddr + p_filesz:
        file_off = p_offset + (target_va - p_vaddr)
        # Read 28 bytes (7 u32 fields)
        vals = struct.unpack_from('<7I', data, file_off)
        names = ['page_offset', 'virt_addr', 'virt_offset', 'phys_addr', 'size', 'va_pa_offset', 'va_kernel_pa_offset']
        print(f'kernel_map in ELF (segment {i}, file_off=0x{file_off:X}):')
        for j, (name, val) in enumerate(zip(names, vals)):
            print(f'  offset {j*4}: {name:25s} = 0x{val:08X}')
        break
else:
    print('kernel_map not found in any PT_LOAD segment')
