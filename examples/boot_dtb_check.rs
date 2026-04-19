use geometry_os::riscv::RiscvVm;
fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    let initramfs_data = std::path::Path::new(ir_path)
        .exists()
        .then(|| std::fs::read(ir_path).unwrap());

    let (mut vm, fw_addr, entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_data,
        initramfs_data.as_deref(),
        512,
        "console=ttyS0 earlycon=sbi loglevel=8 nosmp",
    ).expect("setup");

    // Check DTB in memory
    let dtb_magic = vm.bus.read_word(dtb_addr).expect("read");
    eprintln!("DTB magic at PA 0x{:08X}: 0x{:08X} (expect 0xD00DFEED)", dtb_addr, dtb_magic);
    
    // Dump first 256 bytes of DTB as hex
    eprintln!("DTB first 128 bytes:");
    for i in 0..128 {
        let b = vm.bus.read_byte(dtb_addr + i as u64).unwrap_or(0);
        if (i % 16) == 0 { eprint!("\n  {:04X}: ", i); }
        eprint!("{:02X} ", b);
    }
    eprintln!();

    // Search for "console" string in DTB
    eprintln!("Searching for bootargs in DTB...");
    let mut found = false;
    for i in 0..4096 {
        let b = vm.bus.read_byte(dtb_addr + i as u64).unwrap_or(0);
        if b == b'c' {
            let mut s = Vec::new();
            for j in 0..60 {
                let cb = vm.bus.read_byte(dtb_addr + i as u64 + j as u64).unwrap_or(0);
                s.push(cb);
                if cb == 0 { break; }
            }
            let str_s = String::from_utf8_lossy(&s);
            if str_s.contains("console") || str_s.contains("nosmp") || str_s.contains("earlycon") {
                eprintln!("  Found at offset {}: {:?}", i, str_s);
                found = true;
            }
        }
    }
    if !found {
        eprintln!("  No bootargs string found in DTB!");
    }

    // Check DTB size (total size field at offset 4)
    let dtb_totalsize = vm.bus.read_word(dtb_addr + 4).expect("read");
    eprintln!("DTB totalsize field: {}", dtb_totalsize);
}
