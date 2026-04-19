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

    // Dump first 200 bytes of DTB for manual inspection
    eprintln!("DTB at PA 0x{:X}, dumping structure:", dtb_addr);
    for off in (0..256).step_by(16) {
        let mut hex = String::new();
        let mut ascii = String::new();
        for i in 0..16 {
            let b = vm.bus.read_byte(dtb_addr + off + i).unwrap();
            hex.push_str(&format!("{:02X} ", b));
            ascii.push(if b >= 0x20 && b < 0x7F {
                b as char
            } else {
                '.'
            });
        }
        eprintln!("{:04X}: {} {}", off, hex, ascii);
    }

    // Search for "bootargs" string in DTB
    let dtb_size = vm.bus.read_word(dtb_addr + 4).unwrap(); // totalsize
    eprintln!("\nDTB total size: 0x{:X} ({}) bytes", dtb_size, dtb_size);

    // Search for "bootargs" in the DTB
    let dtb_size = u32::from_be(dtb_size as u32) as u64;
    eprintln!("DTB total size (BE): 0x{:X}", dtb_size);
    for off in 0..dtb_size {
        let mut found = true;
        for (i, &c) in b"bootargs".iter().enumerate() {
            if vm.bus.read_byte(dtb_addr + off + i as u64).unwrap() != c {
                found = false;
                break;
            }
        }
        if found {
            eprintln!("Found 'bootargs' at DTB offset 0x{:X}", off);
            // Read the value (after the null-terminated key, aligned to 4 bytes)
            let val_start = (off + 9 + 3) & !3; // skip "bootargs\0" + align
            let mut val = Vec::new();
            for i in 0..80 {
                let b = vm.bus.read_byte(dtb_addr + val_start + i as u64).unwrap();
                if b == 0 {
                    break;
                }
                val.push(b);
            }
            eprintln!("  Value: \"{}\"", String::from_utf8_lossy(&val));
            break;
        }
    }
}
