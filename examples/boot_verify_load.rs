fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::loader::load_elf;
    use geometry_os::riscv::bus::Bus;

    // Create a VM just like boot_linux_setup does
    let mut vm = RiscvVm::new_with_base(0, 256 * 1024 * 1024);
    
    // Load the kernel
    let load_info = load_elf(&mut vm.bus, &kernel_image).unwrap();
    println!("Entry: 0x{:08X}", load_info.entry);
    println!("Highest addr: 0x{:08X}", load_info.highest_addr);
    
    // Check a few key addresses
    // Segment 1: VirtAddr=0xC0000000, PhysAddr=0x00000000, FileSiz=0x21110C
    // The first instruction of the kernel (virtual 0xC0000000) should be at physical 0
    let phys_0 = vm.bus.read_word(0).unwrap_or(0);
    println!("\nPhysical 0x00000000: 0x{:08X}", phys_0);
    
    // Handler at virtual 0xC0210F14 should be at physical 0x00210F14
    let handler_phys = 0x00210F14u64;
    let val = vm.bus.read_word(handler_phys).unwrap_or(0);
    println!("Physical 0x{:08X}: 0x{:08X}", handler_phys, val);
    
    // Check nearby
    for offset in [0x210F00, 0x210F04, 0x210F08, 0x210F0C, 0x210F10, 0x210F14, 0x210F18, 0x210F1C] {
        let v = vm.bus.read_word(offset as u64).unwrap_or(0);
        if v != 0 {
            println!("  Physical 0x{:08X}: 0x{:08X} (NON-ZERO)", offset, v);
        } else {
            println!("  Physical 0x{:08X}: 0x00000000", offset);
        }
    }
    
    // Check the raw ELF file at offset for segment 1
    // Segment 1: offset=0x1000, VirtAddr=0xC0000000, PhysAddr=0x00000000, FileSiz=0x21110C
    // File offset for virtual 0xC0210F14 = 0x1000 + (0xC0210F14 - 0xC0000000) = 0x1000 + 0x210F14 = 0x211F14
    let elf_offset = 0x211F14;
    if elf_offset + 4 <= kernel_image.len() {
        let b = &kernel_image[elf_offset..elf_offset+4];
        let val = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        println!("\nELF file at offset 0x{:X}: 0x{:08X}", elf_offset, val);
    }
    
    // Check a few random spots in the first segment
    println!("\nRandom spots in segment 1 (phys 0 - 0x21110C):");
    for phys in [0x0, 0x1000, 0x10000, 0x100000, 0x200000] {
        let v = vm.bus.read_word(phys).unwrap_or(0);
        println!("  Physical 0x{:08X}: 0x{:08X} {}", phys, v, if v != 0 { "OK" } else { "ZERO!" });
    }
    
    // Check segment 2: VirtAddr=0xC0400000, PhysAddr=0x00400000
    println!("\nSegment 2 (phys 0x400000):");
    for phys in [0x400000, 0x400004, 0x400008] {
        let v = vm.bus.read_word(phys).unwrap_or(0);
        println!("  Physical 0x{:08X}: 0x{:08X} {}", phys, v, if v != 0 { "OK" } else { "ZERO!" });
    }
}
