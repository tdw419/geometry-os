// Diagnostic: Check DTB content
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs_data = fs::read(initramfs_path).unwrap();

    // Use boot_linux_setup to get the same config
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, _fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, Some(&initramfs_data), 128, bootargs).unwrap();

    // Read the DTB and dump its content
    let mut dtb_bytes = Vec::new();
    for i in 0..4096 {
        if let Ok(b) = vm.bus.read_byte(dtb_addr + i) {
            dtb_bytes.push(b);
            if b == 0 && i > 100 {
                // Check if we're past the last structure
                let last_nonzero = dtb_bytes.iter().rposition(|&x| x != 0).unwrap_or(0);
                if i as usize > last_nonzero + 10 {
                    break;
                }
            }
        }
    }
    
    // Simple FDT parser to find key properties
    eprintln!("DTB at PA 0x{:08X}, {} bytes", dtb_addr, dtb_bytes.len());
    
    // Look for key strings in the DTB
    let dtb_str = String::from_utf8_lossy(&dtb_bytes);
    
    // Find initrd-start and initrd-end
    if let Some(pos) = dtb_str.find("linux,initrd-start") {
        eprintln!("Found 'linux,initrd-start' at offset {}", pos);
    }
    if let Some(pos) = dtb_str.find("linux,initrd-end") {
        eprintln!("Found 'linux,initrd-end' at offset {}", pos);
    }
    if let Some(pos) = dtb_str.find("bootargs") {
        eprintln!("Found 'bootargs' at offset {}", pos);
    }
    if let Some(pos) = dtb_str.find("memory@") {
        eprintln!("Found 'memory@' at offset {}", pos);
    }
    
    // Read first few bytes of initramfs to verify it's loaded
    let initrd_start_offset = 0; // We need to find where initramfs was loaded
    // Check the DTB's initrd-start value
    // For now, let's just dump the DTB hex
    eprintln!("\nDTB first 256 bytes (hex):");
    for i in (0..256).step_by(16) {
        let hex: Vec<String> = dtb_bytes[i..std::cmp::min(i+16, dtb_bytes.len())]
            .iter().map(|b| format!("{:02X}", b)).collect();
        let ascii: String = dtb_bytes[i..std::cmp::min(i+16, dtb_bytes.len())]
            .iter().map(|&b| if b >= 0x20 && b < 0x7f { b as char } else { '.' }).collect();
        eprintln!("  {:04X}: {} {}", i, hex.join(" "), ascii);
    }
}
