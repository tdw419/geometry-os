// check_rsvmap.rs - Verify the DTB memory reservation map is correct
use geometry_os::riscv::dtb::DtbConfig;

fn main() {
    let config = DtbConfig {
        ram_base: 0,
        ram_size: 256 * 1024 * 1024,
        initrd_start: Some(0x014BC000),
        initrd_end: Some(0x01579000),
        bootargs: "console=ttyS0".to_string(),
        reserved_regions: vec![
            (0u64, 0x0157A000), // kernel + initrd + DTB
        ],
        ..Default::default()
    };

    let dtb = geometry_os::riscv::dtb::generate_dtb(&config);

    // Parse the FDT header
    let magic = u32::from_be_bytes([dtb[0], dtb[1], dtb[2], dtb[3]]);
    let totalsize = u32::from_be_bytes([dtb[4], dtb[5], dtb[6], dtb[7]]);
    let off_mem_rsvmap = u32::from_be_bytes([dtb[16], dtb[17], dtb[18], dtb[19]]);

    eprintln!("DTB: magic=0x{:08X} totalsize={}", magic, totalsize);
    eprintln!(
        "  off_mem_rsvmap={} (0x{:X})",
        off_mem_rsvmap, off_mem_rsvmap
    );

    // Parse memory reservation map
    eprintln!("\nMemory Reservation Map (mem_rsvmap):");
    let mut pos = off_mem_rsvmap as usize;
    let mut entry_count = 0;
    loop {
        if pos + 16 > dtb.len() {
            eprintln!("  ERROR: ran past end of DTB at offset {}", pos);
            break;
        }
        let addr = u64::from_be_bytes(dtb[pos..pos + 8].try_into().unwrap());
        let size = u64::from_be_bytes(dtb[pos + 8..pos + 16].try_into().unwrap());
        if addr == 0 && size == 0 {
            eprintln!("  [terminator]");
            break;
        }
        eprintln!(
            "  entry {}: addr=0x{:016X} size=0x{:016X} ({} bytes)",
            entry_count, addr, size, size
        );
        pos += 16;
        entry_count += 1;
    }

    // Also check what boot.rs actually passes as reserved_regions
    eprintln!("\n--- What boot.rs passes ---");
    eprintln!("  kernel_phys_end from ELF LoadInfo.highest_addr");
    eprintln!("  reserved_regions = [(0, kernel_phys_end)]");
    eprintln!("  This goes into mem_rsvmap via add_mem_reserve()");
}
