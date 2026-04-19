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

    // Read full DTB (2200 bytes)
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
    let off_strings = u32::from_be_bytes([dtb_bytes[12], dtb_bytes[13], dtb_bytes[14], dtb_bytes[15]]);

    eprintln!("DTB at PA 0x{:08X}", dtb_addr);
    eprintln!("  magic:      0x{:08X} (expect 0xD00DFEED)", magic);
    eprintln!("  totalsize:  {} bytes", totalsize);
    eprintln!("  off_struct: 0x{:04X}", off_struct);
    eprintln!("  off_strings: 0x{:04X}", off_strings);

    let strings_start = off_strings as usize;
    let strings_end = std::cmp::min(strings_start + 600, dtb_bytes.len());

    // Parse structure block
    let mut pos = off_struct as usize;
    let mut depth = 0;
    let mut found_chosen = false;
    let mut found_initrd = false;
    let mut found_bootargs = false;

    while pos + 4 <= dtb_bytes.len() {
        let token = u32::from_be_bytes([
            dtb_bytes[pos], dtb_bytes[pos + 1], dtb_bytes[pos + 2], dtb_bytes[pos + 3],
        ]);
        match token {
            0x00000001 => {
                // FDT_BEGIN_NODE
                pos += 4;
                // Skip node name (null-terminated string)
                while pos < dtb_bytes.len() && dtb_bytes[pos] != 0 {
                    pos += 1;
                }
                pos += 1; // skip null
                pos = (pos + 3) & !3; // align to 4

                // Get node name for display
                let name_start = pos - 1;
                // Walk back to find start of name
                let mut ns = name_start;
                while ns > 0 && dtb_bytes[ns - 1] != 0 && dtb_bytes[ns - 1] >= 0x20 {
                    ns -= 1;
                }
                let name: String = (ns..name_start)
                    .filter_map(|i| {
                        if dtb_bytes[i] >= 0x20 && dtb_bytes[i] < 0x7f {
                            Some(dtb_bytes[i] as char)
                        } else {
                            None
                        }
                    })
                    .collect();

                let indent = "  ".repeat(depth);
                eprintln!("{}[0x{:04X}] BEGIN_NODE '{}'", indent, pos - 4, name);
                if name == "chosen" {
                    found_chosen = true;
                }
                depth += 1;
            }
            0x00000002 => {
                depth -= 1;
                let indent = "  ".repeat(depth);
                eprintln!("{}[0x{:04X}] END_NODE", indent, pos);
                pos += 4;
            }
            0x00000003 => {
                // FDT_PROP: token(4) + len(4) + nameoff(4) + data(len)
                let prop_len = u32::from_be_bytes([
                    dtb_bytes[pos + 4], dtb_bytes[pos + 5], dtb_bytes[pos + 6], dtb_bytes[pos + 7],
                ]) as usize;
                let name_off = u32::from_be_bytes([
                    dtb_bytes[pos + 8], dtb_bytes[pos + 9], dtb_bytes[pos + 10], dtb_bytes[pos + 11],
                ]) as usize;

                // Read property name from strings area
                let noff = strings_start + name_off;
                let mut prop_name = String::new();
                for i in noff..std::cmp::min(noff + 50, dtb_bytes.len()) {
                    if dtb_bytes[i] == 0 {
                        break;
                    }
                    prop_name.push(dtb_bytes[i] as char);
                }

                let indent = "  ".repeat(depth);

                // Read property value
                let val_start = pos + 12;
                let val_end = val_start + prop_len;
                if prop_name == "linux,initrd-start" && prop_len == 8 {
                    let addr = u64::from_be_bytes([
                        dtb_bytes[val_start], dtb_bytes[val_start + 1],
                        dtb_bytes[val_start + 2], dtb_bytes[val_start + 3],
                        dtb_bytes[val_start + 4], dtb_bytes[val_start + 5],
                        dtb_bytes[val_start + 6], dtb_bytes[val_start + 7],
                    ]);
                    eprintln!("{}[0x{:04X}] PROP '{}' = 0x{:016X}", indent, pos, prop_name, addr);
                    found_initrd = true;
                } else if prop_name == "linux,initrd-end" && prop_len == 8 {
                    let addr = u64::from_be_bytes([
                        dtb_bytes[val_start], dtb_bytes[val_start + 1],
                        dtb_bytes[val_start + 2], dtb_bytes[val_start + 3],
                        dtb_bytes[val_start + 4], dtb_bytes[val_start + 5],
                        dtb_bytes[val_start + 6], dtb_bytes[val_start + 7],
                    ]);
                    eprintln!("{}[0x{:04X}] PROP '{}' = 0x{:016X}", indent, pos, prop_name, addr);
                } else if prop_name == "bootargs" {
                    let val: Vec<u8> = dtb_bytes[val_start..val_end].to_vec();
                    let s = String::from_utf8_lossy(&val);
                    eprintln!("{}[0x{:04X}] PROP '{}' = '{}'", indent, pos, prop_name, s.trim_end_matches('\0'));
                    found_bootargs = true;
                } else {
                    eprintln!("{}[0x{:04X}] PROP '{}' len={}", indent, pos, prop_name, prop_len);
                }

                pos = val_end;
                pos = (pos + 3) & !3; // align to 4
            }
            0x00000009 => {
                eprintln!("[0x{:04X}] FDT_END", pos);
                break;
            }
            _ => {
                pos += 4;
            }
        }
    }

    eprintln!("\nSummary: chosen={} initrd={} bootargs={}", found_chosen, found_initrd, found_bootargs);
}
