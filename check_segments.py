import struct

with open('.geometry_os/build/linux-6.14/vmlinux', 'rb') as f:
    data = f.read()

# Parse ELF header (32-bit)
e_phoff = struct.unpack_from('<I', data, 28)[0]
e_phentsize = struct.unpack_from('<H', data, 42)[0]
e_phnum = struct.unpack_from('<H', data, 44)[0]
entry = struct.unpack_from('<I', data, 24)[0]

print(f"Entry point: 0x{entry:08X}")
print(f"\nLOAD segments:")

segments = []
for i in range(e_phnum):
    off = e_phoff + i * e_phentsize
    p_type = struct.unpack_from('<I', data, off)[0]
    if p_type != 1:  # PT_LOAD
        continue
    p_offset = struct.unpack_from('<I', data, off+4)[0]
    p_vaddr = struct.unpack_from('<I', data, off+8)[0]
    p_paddr = struct.unpack_from('<I', data, off+12)[0]
    p_filesz = struct.unpack_from('<I', data, off+16)[0]
    p_memsz = struct.unpack_from('<I', data, off+20)[0]
    p_flags = struct.unpack_from('<I', data, off+24)[0]
    segments.append((p_vaddr, p_paddr, p_filesz, p_memsz))
    
    flags = ""
    if p_flags & 1: flags += "X"
    if p_flags & 2: flags += "W"
    if p_flags & 4: flags += "R"
    
    va_end = p_vaddr + p_memsz
    pa_end = p_paddr + p_memsz
    l1_start = p_vaddr >> 22
    l1_end = (va_end - 1) >> 22
    
    print(f"  VA 0x{p_vaddr:08X}-0x{va_end:08X}  PA 0x{p_paddr:08X}-0x{pa_end:08X}  filesz=0x{p_filesz:X} memsz=0x{p_memsz:X}  {flags}  L1[{l1_start}-{l1_end}]")

# Check which L1 entries are needed
all_l1 = set()
for va, pa, fsz, msz in segments:
    for l1 in range(va >> 22, ((va + msz - 1) >> 22) + 1):
        all_l1.add(l1)

print(f"\nL1 entries needed: {sorted(all_l1)}")
print(f"Range: L1[{min(all_l1)}] to L1[{max(all_l1)}]")
print(f"That's {max(all_l1) - min(all_l1) + 1} entries needed")

# Check if kernel_map is covered
km_va = 0xC0C79E90
km_l1 = km_va >> 22
print(f"\nkernel_map at VA 0x{km_va:08X} -> L1[{km_l1}]")
print(f"L1[{km_l1}] in needed set: {km_l1 in all_l1}")
