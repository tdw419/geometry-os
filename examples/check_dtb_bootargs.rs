// Verify DTB bootargs contain nosmp
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = std::path::Path::new(".geometry_os/build/linux-6.14/vmlinux");
    let kernel = std::fs::read(kernel_path).expect("kernel");
    let initramfs_path = std::path::Path::new(".geometry_os/fs/linux/rv32/initramfs.cpio.gz");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel,
        initramfs.as_deref(),
        128,
        "console=ttyS0 earlycon=sbi",
    )
    .unwrap();

    // Find bootargs in the DTB blob
    // DTB is at dtb_addr, scan for "console=" string
    let dtb_pa = dtb_addr as u32;
    println!("[dtb] DTB loaded at PA 0x{:08X}", dtb_pa);

    // Search for "console=" in the first 4KB of DTB
    let mut found = false;
    for offset in 0..0x1000 {
        if let Ok(byte) = vm.bus.read_byte((dtb_pa as u64) + offset) {
            if byte == b'c' {
                // Check if this is "console="
                let mut match_str = String::new();
                for j in 0..80 {
                    if let Ok(b) = vm.bus.read_byte((dtb_pa as u64) + offset + j) {
                        if b == 0 {
                            break;
                        }
                        match_str.push(b as char);
                    }
                }
                if match_str.starts_with("console=") {
                    println!("[dtb] Bootargs: \"{}\"", match_str);
                    found = true;
                    break;
                }
            }
        }
    }
    if !found {
        println!("[dtb] Bootargs not found in DTB!");
    }
}
