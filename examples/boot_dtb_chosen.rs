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

    // Dump area around "chosen" node
    eprintln!("Dumping DTB area 0x840-0x875:");
    for off in (0x840..0x878).step_by(16) {
        let mut hex = String::new();
        let mut ascii = String::new();
        for i in 0..16 {
            let b = vm.bus.read_byte(dtb_addr + off + i as u64).unwrap();
            hex.push_str(&format!("{:02X} ", b));
            ascii.push(if b >= 0x20 && b < 0x7F {
                b as char
            } else {
                '.'
            });
        }
        eprintln!("{:04X}: {} {}", off, hex, ascii);
    }

    // Dump area around "bootargs" property (should be at ~0x850)
    // Look for the chosen node
    eprintln!("\nSearching for 'chosen':");
    for off in 0..0x875 {
        let mut found = true;
        for (i, &c) in b"chosen".iter().enumerate() {
            if vm.bus.read_byte(dtb_addr + off + i as u64).unwrap() != c {
                found = false;
                break;
            }
        }
        if found {
            eprintln!("  Found at offset 0x{:X}", off);
        }
    }

    // Dump a broader area to find the chosen node and its properties
    eprintln!("\nDumping DTB area 0x810-0x875:");
    for off in (0x810..0x878).step_by(16) {
        let mut hex = String::new();
        let mut ascii = String::new();
        for i in 0..16 {
            let b = vm.bus.read_byte(dtb_addr + off + i as u64).unwrap();
            hex.push_str(&format!("{:02X} ", b));
            ascii.push(if b >= 0x20 && b < 0x7F {
                b as char
            } else {
                '.'
            });
        }
        eprintln!("{:04X}: {} {}", off, hex, ascii);
    }
}
