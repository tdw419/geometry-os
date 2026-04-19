use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi loglevel=8",
    )
    .unwrap();

    // DTB header: totalsize at offset 4, struct_off at offset 8
    let totalsize = u32::from_be(vm.bus.read_word(dtb_addr + 4).unwrap() as u32);
    let struct_off = u32::from_be(vm.bus.read_word(dtb_addr + 8).unwrap() as u32);
    let strings_off = u32::from_be(vm.bus.read_word(dtb_addr + 12).unwrap() as u32);
    let struct_size = strings_off - struct_off;

    eprintln!(
        "DTB: totalsize=0x{:X} struct_off=0x{:X} strings_off=0x{:X} struct_size=0x{:X}",
        totalsize, struct_off, strings_off, struct_size
    );

    // Dump the struct block from chosen node onwards
    // chosen is at struct offset 0x588
    eprintln!("\nStruct block around chosen node:");
    for off in (0x580..std::cmp::min(0x700, struct_size as u64)).step_by(16) {
        let mut hex = String::new();
        for i in (0..16).step_by(4) {
            let w = vm
                .bus
                .read_word(dtb_addr + struct_off as u64 + off + i as u64)
                .unwrap();
            hex.push_str(&format!("{:08X} ", u32::from_be(w as u32)));
        }
        eprintln!("  {:04X}: {}", off, hex);
    }

    // Check if bootargs property exists in the chosen node
    // Look for FDT_PROP (0x00000003) followed by a length and name offset
    // The name "bootargs" is at strings offset 0x86C - strings_off
    let bootargs_stroff = 0x86C - strings_off;
    eprintln!(
        "bootargs string offset in strings block: 0x{:X}",
        bootargs_stroff
    );

    // Search struct block for a property with nameoff = bootargs_stroff
    eprintln!("\nSearching for bootargs property in struct block...");
    for off in (0..struct_size as u64).step_by(4) {
        let token = u32::from_be(
            vm.bus
                .read_word(dtb_addr + struct_off as u64 + off)
                .unwrap() as u32,
        );
        if token == 3 {
            // FDT_PROP
            let len = u32::from_be(
                vm.bus
                    .read_word(dtb_addr + struct_off as u64 + off + 4)
                    .unwrap() as u32,
            );
            let nameoff = u32::from_be(
                vm.bus
                    .read_word(dtb_addr + struct_off as u64 + off + 8)
                    .unwrap() as u32,
            );
            if nameoff == bootargs_stroff {
                eprintln!(
                    "  Found bootargs property at struct offset 0x{:X}, len={}",
                    off, len
                );
                // Read value
                let val_start = dtb_addr + struct_off as u64 + off + 12;
                let mut val = Vec::new();
                for i in 0..len {
                    let b = vm.bus.read_byte(val_start + i as u64).unwrap();
                    if b > 0 {
                        val.push(b);
                    }
                }
                eprintln!("  Value: \"{}\"", String::from_utf8_lossy(&val));
            }
        }
    }
}
