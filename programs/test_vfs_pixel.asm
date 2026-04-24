# programs/test_vfs_pixel.asm -- Test for Pixel VFS Surface
#
# Reads from MMIO 0x7000_0000 and prints magic/file count to UART.

.section .text
.globl _start

_start:
    # 1. Load VFS Surface Magic (0x7000_0000)
    li t0, 0x70000000
    lw a0, 0(t0)        # a0 = PXFS magic
    
    # 2. Load File Count (0x7000_0004)
    lw a1, 4(t0)        # a1 = file_count
    
    # 3. Print Magic to UART (0x1000_0000)
    # For now, just store them in registers and loop.
    # In a real test, we'd output to UART.
    li t1, 0x10000000
    sw a0, 0(t1)        # Output magic to UART (if UART supports word writes)
    
    # Check if a0 == 0x50584653 ('PXFS')
    li t2, 0x50584653
    beq a0, t2, success
    
failure:
    li a7, 93           # exit(1)
    li a0, 1
    ecall

success:
    # If successful, check if first file exists (if count > 0)
    beqz a1, done
    
    # Load first file index pixel (0x7000_0008)
    lw a2, 8(t0)        # a2 = [start_row(16) | name_hash(16)]
    
    # Get start_row (upper 16 bits)
    srli a3, a2, 16     # a3 = start_row
    
    # Load File Header Pixel
    slli a3, a3, 8      # start_row * 256
    slli a3, a3, 2      # * 4 bytes
    add a3, a3, t0      # a3 = address of file header
    lw a4, 0(a3)        # a4 = [byte_count(16) | name_hash_8(8) | flags(8)]

done:
    li a7, 93           # exit(0)
    li a0, 0
    ecall
