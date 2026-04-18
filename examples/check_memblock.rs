fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;

    let (mut vm, _fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    // memblock is at VA 0xC0803448 (PA 0x00803448)
    // Dump first 48 bytes of the struct
    let mb_pa = 0x00803448u64;
    println!("memblock struct at PA 0x{:08X}:", mb_pa);
    for i in 0..12 {
        let off = i * 4;
        let val = vm.bus.read_word(mb_pa + off).unwrap_or(0);
        // Try to interpret as VA pointer
        let va_str = if val >= 0xC0000000 {
            format!(" (VA -> PA 0x{:08X})", val - 0xC0000000)
        } else {
            String::new()
        };
        println!(
            "  +{:2} (0x{:02X}): 0x{:08X} ({}){}",
            off, off, val, val, va_str
        );
    }

    // Known symbols:
    // memblock at 0xC0803448
    // memblock_memory_init_regions at 0xC080348C (offset 0x44=68)
    // memblock_reserved_init_regions at 0xC0803A8C (offset 0x144=324)

    // So the struct layout must be:
    // offset 0: bottom_up (4) = 1 or 0
    // offset 4: current_limit (4) = 0xFFFFFFFF
    // offset 8: memory.memblock_type.cnt (4) = 0
    // offset 12: memory.memblock_type.max (4) = 128
    // offset 16: memory.memblock_type.total_size (4)
    // offset 20: memory.memblock_type.regions (4) = ptr to 0xC080348C
    // offset 24: memory.memblock_type.name (4) = ptr to "memory"
    // offset 28: reserved.memblock_type.cnt (4)
    // offset 32: reserved.memblock_type.max (4) = 128
    // offset 36: reserved.memblock_type.total_size (4)
    // offset 40: reserved.memblock_type.regions (4) = ptr to 0xC0803A8C
    // offset 44: reserved.memblock_type.name (4) = ptr to "reserved"

    println!("\nInterpreting as 20-byte memblock_type sub-structs:");
    println!("  bottom_up = {}", vm.bus.read_word(mb_pa).unwrap_or(0));
    println!(
        "  current_limit = 0x{:08X}",
        vm.bus.read_word(mb_pa + 4).unwrap_or(0)
    );
    println!(
        "  memory.cnt = {}",
        vm.bus.read_word(mb_pa + 8).unwrap_or(0)
    );
    println!(
        "  memory.max = {}",
        vm.bus.read_word(mb_pa + 12).unwrap_or(0)
    );
    println!(
        "  memory.total_size = {}",
        vm.bus.read_word(mb_pa + 16).unwrap_or(0)
    );
    let mem_reg = vm.bus.read_word(mb_pa + 20).unwrap_or(0);
    println!("  memory.regions = 0x{:08X} (expect 0xC080348C)", mem_reg);
    let mem_name = vm.bus.read_word(mb_pa + 24).unwrap_or(0);
    println!("  memory.name = 0x{:08X}", mem_name);
    // Try to read the name string
    if mem_name >= 0xC0000000 {
        let name_pa = (mem_name - 0xC0000000) as u64;
        let mut chars = Vec::new();
        for j in 0..20 {
            let b = vm.bus.read_byte(name_pa + j as u64).unwrap_or(0);
            if b == 0 {
                break;
            }
            if b >= 0x20 && b < 0x7f {
                chars.push(b as char);
            }
        }
        println!("    name = \"{}\"", chars.iter().collect::<String>());
    }
    println!(
        "  reserved.cnt = {}",
        vm.bus.read_word(mb_pa + 28).unwrap_or(0)
    );
    println!(
        "  reserved.max = {}",
        vm.bus.read_word(mb_pa + 32).unwrap_or(0)
    );
    println!(
        "  reserved.total_size = {}",
        vm.bus.read_word(mb_pa + 36).unwrap_or(0)
    );
    let res_reg = vm.bus.read_word(mb_pa + 40).unwrap_or(0);
    println!("  reserved.regions = 0x{:08X} (expect 0xC0803A8C)", res_reg);
    let res_name = vm.bus.read_word(mb_pa + 44).unwrap_or(0);
    println!("  reserved.name = 0x{:08X}", res_name);
    if res_name >= 0xC0000000 {
        let name_pa = (res_name - 0xC0000000) as u64;
        let mut chars = Vec::new();
        for j in 0..20 {
            let b = vm.bus.read_byte(name_pa + j as u64).unwrap_or(0);
            if b == 0 {
                break;
            }
            if b >= 0x20 && b < 0x7f {
                chars.push(b as char);
            }
        }
        println!("    name = \"{}\"", chars.iter().collect::<String>());
    }
}
