//! Verify DTB memory reservation map entries.
//! Run: cargo run --example boot_dtb_check

use std::fs;
use geometry_os::riscv::{dtb::DtbConfig, loader};

fn main() {
    let kernel_image = fs::read(".geometry_os/build/linux-6.14/vmlinux").expect("kernel not found");

    let mut bus = geometry_os::riscv::bus::Bus::new(0, 256 * 1024 * 1024);
    let load_info = loader::load_elf(&mut bus, &kernel_image).expect("load elf failed");
    let kernel_phys_end = ((load_info.highest_addr + 0xFFF) & !0xFFF) as u64;

    eprintln!("Kernel highest_addr = 0x{:X}", load_info.highest_addr);
    eprintln!("Kernel phys end (aligned) = 0x{:X} ({} KB)", kernel_phys_end, kernel_phys_end / 1024);

    let reserved_regions = vec![(0u64, kernel_phys_end)];
    let dtb_config = DtbConfig {
        ram_base: 0,
        ram_size: 256 * 1024 * 1024,
        reserved_regions,
        ..Default::default()
    };
    let dtb = geometry_os::riscv::dtb::generate_dtb(&dtb_config);

    eprintln!("DTB size: {} bytes", dtb.len());

    // Parse DTB header
    let magic = u32::from_be_bytes([dtb[0], dtb[1], dtb[2], dtb[3]]);
    let off_mem_rsvmap = u32::from_be_bytes([dtb[16], dtb[17], dtb[18], dtb[19]]);

    eprintln!("DTB magic=0x{:08X} off_mem_rsvmap={}", magic, off_mem_rsvmap);

    // Parse memory reservation map
    if off_mem_rsvmap > 0 {
        let mut pos = off_mem_rsvmap as usize;
        let mut entry_idx = 0;
        loop {
            let addr = u64::from_be_bytes([
                dtb[pos], dtb[pos + 1], dtb[pos + 2], dtb[pos + 3],
                dtb[pos + 4], dtb[pos + 5], dtb[pos + 6], dtb[pos + 7],
            ]);
            let size = u64::from_be_bytes([
                dtb[pos + 8], dtb[pos + 9], dtb[pos + 10], dtb[pos + 11],
                dtb[pos + 12], dtb[pos + 13], dtb[pos + 14], dtb[pos + 15],
            ]);
            if addr == 0 && size == 0 {
                eprintln!("  [{}] TERMINATOR (0, 0)", entry_idx);
                break;
            }
            eprintln!(
                "  [{}] address=0x{:08X} size=0x{:08X} ({} KB)",
                entry_idx, addr, size, size / 1024
            );
            pos += 16;
            entry_idx += 1;
        }
    } else {
        eprintln!("  off_mem_rsvmap = 0 (NO memory reservations!)");
    }
}
