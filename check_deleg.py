#!/usr/bin/env python3
m = 0xA109
print(f'medeleg = 0x{m:04X} = {m:016b}')
causes = [
    "misaligned_fetch", "fetch_access_fault", "illegal_instruction", "breakpoint",
    "misaligned_load", "load_access_fault", "misaligned_store", "store_access_fault",
    "ecall_u", "ecall_s", "reserved", "ecall_m",
    "instruction_page_fault", "load_page_fault", "reserved", "store_page_fault"
]
for i in range(16):
    if (m >> i) & 1:
        name = causes[i] if i < len(causes) else f"cause_{i}"
        print(f"  bit {i:2d} ({name}): DELEGATED to S-mode")

m = 0x222
print(f'\nmideleg = 0x{m:04X} = {m:016b}')
interrupts = [
    "usoft", "ssoft", "reserved", "mssoft",
    "utimer", "stimer", "reserved", "mtimer",
    "uext", "sext", "reserved", "mext"
]
for i in range(12):
    if (m >> i) & 1:
        name = interrupts[i] if i < len(interrupts) else f"int_{i}"
        print(f"  bit {i:2d} ({name}): DELEGATED to S-mode")
