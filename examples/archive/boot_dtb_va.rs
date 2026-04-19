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

    // Check DTB content at PA
    eprintln!("DTB addr = 0x{:X}", dtb_addr);
    let mut header = Vec::new();
    for i in 0..16 {
        let b = vm.bus.read_byte(dtb_addr + i).unwrap();
        header.push(b);
    }
    eprintln!("DTB first 16 bytes: {:02X?}", header);

    // Check if DTB magic (0xD00DFEED big-endian = ED FE 0D D0 little-endian) is there
    let magic = vm.bus.read_word(dtb_addr).unwrap();
    eprintln!(
        "DTB magic at PA: 0x{:08X} (expected 0xEDFE0DD0 or 0xD00DFEED)",
        magic
    );

    // Now run to parse_dtb and check what happens
    // parse_dtb calls early_init_dt_scan(dtb_early_va, dtb_early_pa)
    // dtb_early_va = 0x01579000 (PA)
    // With identity mapping, VA 0x01579000 should read the same content

    // Run and check if early_init_dt_scan is called
    let mut count: u64 = 0;
    while count < 17_000_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let pc = vm.cpu.pc;

        // early_init_dt_scan at 0xC041BD4A
        if pc == 0xC041BD4A {
            eprintln!(
                "[{}] early_init_dt_scan called: a0=0x{:08X} a1=0x{:08X}",
                count, vm.cpu.x[10], vm.cpu.x[11]
            );
            // a0 = dtb_early_va, a1 = dtb_early_pa
            // Try reading from VA a0
            let va = vm.cpu.x[10] as u64;
            match vm.bus.read_word(va) {
                Ok(v) => eprintln!("  DTB magic via VA 0x{:08X}: 0x{:08X}", va, v),
                Err(e) => eprintln!("  FAILED to read VA 0x{:08X}: {:?}", va, e),
            }
            break;
        }

        let _ = vm.step();
        count += 1;
    }
}
