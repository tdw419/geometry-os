import struct

with open('.geometry_os/build/linux-6.14/vmlinux', 'rb') as f:
    data = f.read()

# Read 4 bytes at offset 0x10B8
word = struct.unpack_from('<I', data, 0x10B8)[0]
print(f'Instruction at PA 0x10B8: 0x{word:08X}')

# Check a range around it
for off in range(0x1080, 0x10C0, 4):
    w = struct.unpack_from('<I', data, off)[0]
    print(f'  PA 0x{off:04X}: 0x{w:08X}')

# Also check the kernel's _start at VA 0xC0000000 (PA 0x0)
print('\nKernel entry point area:')
for off in range(0, 0x40, 4):
    w = struct.unpack_from('<I', data, off)[0]
    print(f'  PA 0x{off:04X}: 0x{w:08X}')
