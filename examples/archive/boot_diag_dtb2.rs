// Diagnostic: Check DTB integrity
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs_data = fs::read(initramfs_path).unwrap();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, _fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, Some(&initramfs_data), 128, bootargs).unwrap();

    // Read full DTB (2169 bytes)
    let mut dtb_bytes = Vec::new();
    for i in 0..2200u64 {
        if let Ok(b) = vm.bus.read_byte(dtb_addr + i) {
            dtb_bytes.push(b);
        }
    }

    // Parse FDT header (big-endian)
    let magic = u32::from_be_bytes([dtb_bytes[0], dtb_bytes[1], dtb_bytes[2], dtb_bytes[3]]);
    let totalsize = u32::from_be_bytes([dtb_bytes[4], dtb_bytes[5], dtb_bytes[6], dtb_bytes[7]]);
    let off_struct = u32::from_be_bytes([dtb_bytes[8], dtb_bytes[9], dtb_bytes[10], dtb_bytes[11]]);
    let off_strings =
        u32::from_be_bytes([dtb_bytes[12], dtb_bytes[13], dtb_bytes[14], dtb_bytes[15]]);

    eprintln!("DTB at PA 0x{:08X}", dtb_addr);
    eprintln!("  magic:     0x{:08X} (expect 0xD00DFEED)", magic);
    eprintln!("  totalsize:  {} bytes", totalsize);
    eprintln!("  off_struct: 0x{:04X}", off_struct);
    eprintln!("  off_strings: 0x{:04X}", off_strings);

    // Count non-zero bytes in different regions
    let mut nonzero_header = 0;
    for i in 0..40usize {
        if dtb_bytes[i] != 0 {
            nonzero_header += 1;
        }
    }
    let mut nonzero_struct = 0;
    for i in off_struct as usize..std::cmp::min(off_struct as usize + 200, dtb_bytes.len()) {
        if dtb_bytes[i] != 0 {
            nonzero_struct += 1;
        }
    }
    let mut nonzero_strings = 0;
    let strings_start = off_strings as usize;
    let strings_end = std::cmp::min(strings_start + 600, dtb_bytes.len());
    for i in strings_start..strings_end {
        if dtb_bytes[i] != 0 {
            nonzero_strings += 1;
        }
    }

    eprintln!(
        "  Non-zero bytes: header={} struct={} strings={}",
        nonzero_header, nonzero_struct, nonzero_strings
    );

    // Show strings area
    eprintln!(
        "\n  Strings area (0x{:04X}-0x{:04X}):",
        strings_start, strings_end
    );
    let mut current_str = String::new();
    for i in strings_start..strings_end {
        if dtb_bytes[i] == 0 {
            if !current_str.is_empty() {
                eprintln!("    '{}'", current_str);
                current_str.clear();
            }
        } else if dtb_bytes[i] >= 0x20 && dtb_bytes[i] < 0x7f {
            current_str.push(dtb_bytes[i] as char);
        } else {
            current_str.push_str(&format!("\\x{:02X}", dtb_bytes[i]));
        }
    }
    if !current_str.is_empty() {
        eprintln!("    '{}'", current_str);
    }

    // Check chosen node - look for bootargs and initrd properties
    // In FDT, properties are stored in the structure block
    eprintln!("\n  Structure block (0x{:04X}..):", off_struct);
    let mut pos = off_struct as usize;
    while pos < dtb_bytes.len() {
        let token = u32::from_be_bytes([
            dtb_bytes[pos],
            dtb_bytes[pos + 1],
            dtb_bytes[pos + 2],
            dtb_bytes[pos + 3],
        ]);
        match token {
            0x00000001 => {
                eprintln!("    [0x{:04X}] FDT_BEGIN_NODE", pos);
                pos += 4;
            }
            0x00000002 => {
                eprintln!("    [0x{:04X}] FDT_END_NODE", pos);
                pos += 4;
            }
            0x00000003 => {
                let name_off = u32::from_be_bytes([
                    dtb_bytes[pos + 4],
                    dtb_bytes[pos + 5],
                    dtb_bytes[pos + 6],
                    dtb_bytes[pos + 7],
                ]);
                // Read property name from strings area
                let mut name = String::new();
                let noff = strings_start + name_off as usize;
                let len = u32::from_be_bytes([
                    dtb_bytes[pos + 4],
                    dtb_bytes[pos + 5],
                    dtb_bytes[pos + 6],
                    dtb_bytes[pos + 7],
                ]);
                let name_off = u32::from_be_bytes([
                    dtb_bytes[pos + 8],
                    dtb_bytes[pos + 9],
                    dtb_bytes[pos + 10],
                    dtb_bytes[pos + 11],
                ]);
                eprintln!("    [0x{:04X}] FDT_PROP '{}' len={}", pos, name, len);
                if name.contains("initrd") || name.contains("bootargs") {
                    let val_start = pos + 12;
                    let val_end = val_start + len as usize;
                    if len > 0 && len < 200 {
                        let val: Vec<u8> = dtb_bytes[val_start..val_end].to_vec();
                        // Check if it's a string
                        if val.iter().all(|&b| b >= 0x20 && b < 0x7f || b == 0) {
                            eprintln!("      value: '{}'", String::from_utf8_lossy(&val));
                        } else {
                            // It's a u64 (initrd address)
                            if len == 8 {
                                let addr = u64::from_be_bytes([
                                    val[0], val[1], val[2], val[3], val[4], val[5], val[6], val[7],
                                ]);
                                eprintln!("      value: 0x{:016X}", addr);
                            } else {
                                eprintln!("      value: {:02X?}", val);
                            }
                        }
                    }
                }
                pos += 12 + len as usize;
                // Align to 4 bytes
                pos = (pos + 3) & !3;
            }
            0x00000009 => {
                eprintln!("    [0x{:04X}] FDT_END", pos);
                break;
            }
            _ => {
                pos += 4;
            }
        }
        if pos > dtb_bytes.len() - 4 {
            break;
        }
    }
}
