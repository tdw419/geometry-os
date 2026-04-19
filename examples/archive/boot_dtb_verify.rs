//! Verify DTB is properly loaded and readable by the kernel.
//! Run: cargo run --example boot_dtb_verify

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    let initramfs_data = std::path::Path::new(ir_path)
        .exists()
        .then(|| std::fs::read(ir_path).unwrap());

    let (mut vm, _fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_data,
        initramfs_data.as_deref(),
        128,
        "console=ttyS0 earlycon=sbi loglevel=8",
    )
    .expect("boot setup failed");

    let dtb_va = dtb_addr.wrapping_add(0xC0000000) & 0xFFFFFFFF;

    // Check DTB magic at PA
    let magic = vm.bus.read_word(dtb_addr).unwrap_or(0);
    eprintln!(
        "[dtb] DTB at PA 0x{:08X}, VA 0x{:08X}",
        dtb_addr as u32, dtb_va
    );
    eprintln!("[dtb] Magic at PA: 0x{:08X} (expected 0xD00DFEED)", magic);

    // Check _dtb_early_va / _dtb_early_pa
    let dtb_early_va = vm.bus.read_word(0x00801008).unwrap_or(0);
    let dtb_early_pa = vm.bus.read_word(0x0080100C).unwrap_or(0);
    eprintln!(
        "[dtb] _dtb_early_va (PA 0x00801008) = 0x{:08X}",
        dtb_early_va
    );
    eprintln!(
        "[dtb] _dtb_early_pa (PA 0x0080100C) = 0x{:08X}",
        dtb_early_pa
    );

    // Check DTB magic at _dtb_early_va (using VA mapping)
    // Since we're pre-boot, the boot page table should map VA -> PA
    // VA 0xC1579000 should map to PA 0x01579000
    let dtb_early_va_magic = vm.bus.read_word(dtb_early_va as u64).unwrap_or(0);
    eprintln!(
        "[dtb] Magic at _dtb_early_va (0x{:08X}): 0x{:08X}",
        dtb_early_va, dtb_early_va_magic
    );

    // Check DTB magic at _dtb_early_pa
    let dtb_early_pa_magic = vm.bus.read_word(dtb_early_pa as u64).unwrap_or(0);
    eprintln!(
        "[dtb] Magic at _dtb_early_pa (0x{:08X}): 0x{:08X}",
        dtb_early_pa, dtb_early_pa_magic
    );

    // Dump first 64 bytes of DTB header
    eprintln!("[dtb] DTB header (first 64 bytes):");
    for i in 0..16 {
        let word = vm.bus.read_word(dtb_addr + (i as u64) * 4).unwrap_or(0);
        eprintln!("[dtb]   +0x{:02X}: 0x{:08X}", i * 4, word);
    }

    // Check if DTB has correct totalsize
    let totalsize = vm.bus.read_word(dtb_addr + 4).unwrap_or(0);
    let off_dt_struct = vm.bus.read_word(dtb_addr + 8).unwrap_or(0);
    let off_dt_strings = vm.bus.read_word(dtb_addr + 12).unwrap_or(0);
    eprintln!(
        "[dtb] totalsize = 0x{:08X} ({} bytes)",
        totalsize, totalsize
    );
    eprintln!("[dtb] off_dt_struct = 0x{:08X}", off_dt_struct);
    eprintln!("[dtb] off_dt_strings = 0x{:08X}", off_dt_strings);

    // Search for "timebase-frequency" string in the DTB
    eprintln!("[dtb] Searching for 'timebase-frequency' in DTB...");
    let search = b"timebase-frequency";
    let mut found = false;
    for offset in 0..totalsize.saturating_sub(search.len() as u32) {
        let mut match_all = true;
        for (j, &byte) in search.iter().enumerate() {
            let b = vm
                .bus
                .read_byte(dtb_addr + offset as u64 + j as u64)
                .unwrap_or(0);
            if b != byte {
                match_all = false;
                break;
            }
        }
        if match_all {
            eprintln!("[dtb]   Found at DTB offset 0x{:04X}", offset);
            // Read the 4-byte value that follows (u32 big-endian)
            let val_bytes: Vec<u8> = (0..4)
                .map(|j| {
                    vm.bus
                        .read_byte(dtb_addr + offset as u64 + search.len() as u64 + j as u64)
                        .unwrap_or(0)
                })
                .collect();
            // Actually the value is in the struct block, not right after the string name
            // The string is in the strings block, the value is a prop in the struct block
            found = true;
        }
    }
    if !found {
        eprintln!("[dtb]   NOT FOUND!");
    }

    // Check the /cpus node - search for "cpus" string
    eprintln!("[dtb] Searching for 'cpus' in DTB strings block...");
    let search_cpus = b"cpus";
    for offset in 0..totalsize.saturating_sub(search_cpus.len() as u32) {
        let mut match_all = true;
        for (j, &byte) in search_cpus.iter().enumerate() {
            let b = vm
                .bus
                .read_byte(dtb_addr + offset as u64 + j as u64)
                .unwrap_or(0);
            if b != byte {
                match_all = false;
                break;
            }
        }
        if match_all {
            eprintln!("[dtb]   Found 'cpus' at DTB offset 0x{:04X}", offset);
        }
    }
}
