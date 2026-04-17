use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(
            &kernel_image,
            initramfs.as_deref(),
            256,
            "console=ttyS0 loglevel=8",
        ).unwrap();

    eprintln!("DTB at PA 0x{:08X}", dtb_addr);
    
    // Dump first 300 bytes of DTB
    let mut buf = Vec::new();
    for i in 0..300 {
        if let Ok(b) = vm.bus.read_byte(dtb_addr + i) {
            buf.push(b);
        }
    }
    
    // FDT header
    let magic = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let totalsize = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
    let off_dt_struct = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let off_dt_strings = u32::from_be_bytes([buf[12], buf[13], buf[14], buf[15]]);
    let off_mem_rsvmap = u32::from_be_bytes([buf[16], buf[17], buf[18], buf[19]]);
    let version = u32::from_be_bytes([buf[20], buf[21], buf[22], buf[23]]);
    
    eprintln!("magic=0x{:08X} (expect 0xD00DFEED) totalsize={} version={}", magic, totalsize, version);
    eprintln!("off_dt_struct={} off_dt_strings={} off_mem_rsvmap={}", off_dt_struct, off_dt_strings, off_mem_rsvmap);
    
    // Dump strings block
    eprintln!("\n--- Strings block ---");
    let str_start = off_dt_strings as usize;
    let mut pos = str_start;
    while pos < buf.len() {
        let mut s = String::new();
        while pos < buf.len() && buf[pos] != 0 {
            s.push(buf[pos] as char);
            pos += 1;
        }
        if s.is_empty() { break; }
        eprintln!("  [{}] \"{}\"", pos - str_start, s);
        pos += 1; // skip null
    }
    
    // Dump structure block tokens
    eprintln!("\n--- Structure block (first 600 bytes) ---");
    let struct_start = off_dt_struct as usize;
    let struct_end = off_dt_strings as usize;
    let mut pos = struct_start;
    let mut indent = 0;
    while pos < struct_end && pos < struct_start + 600 {
        let token = u32::from_be_bytes([buf[pos], buf[pos+1], buf[pos+2], buf[pos+3]]);
        match token {
            0x1 => { // FDT_BEGIN_NODE
                pos += 4;
                let mut name = String::new();
                while pos < buf.len() && buf[pos] != 0 {
                    name.push(buf[pos] as char);
                    pos += 1;
                }
                pos += 1; // skip null, align to 4
                pos = (pos + 3) & !3;
                eprintln!("{}BEGIN_NODE \"{}\"", "  ".repeat(indent), name);
                indent += 1;
            }
            0x2 => { // FDT_END_NODE
                indent = indent.saturating_sub(1);
                eprintln!("{}END_NODE", "  ".repeat(indent));
                pos += 4;
            }
            0x3 => { // FDT_PROP
                pos += 4;
                let len = u32::from_be_bytes([buf[pos], buf[pos+1], buf[pos+2], buf[pos+3]]);
                pos += 4;
                let nameoff = u32::from_be_bytes([buf[pos], buf[pos+1], buf[pos+2], buf[pos+3]]);
                pos += 4;
                // Read property value
                let val_start = pos;
                let mut val_str = String::new();
                for _ in 0..len.min(64) {
                    if buf[pos] >= 0x20 && buf[pos] < 0x7f {
                        val_str.push(buf[pos] as char);
                    } else {
                        val_str.push('.');
                    }
                    pos += 1;
                }
                // Also show as hex for reg properties
                if len == 16 { // likely a reg (address, size) pair
                    let addr = u64::from_be_bytes([buf[val_start], buf[val_start+1], buf[val_start+2], buf[val_start+3],
                        buf[val_start+4], buf[val_start+5], buf[val_start+6], buf[val_start+7]]);
                    let size = u64::from_be_bytes([buf[val_start+8], buf[val_start+9], buf[val_start+10], buf[val_start+11],
                        buf[val_start+12], buf[val_start+13], buf[val_start+14], buf[val_start+15]]);
                    eprintln!("{}PROP \"{}\" = reg(0x{:X}, 0x{:X}) [{}MB]", "  ".repeat(indent), nameoff, addr, size, size / (1024*1024));
                } else {
                    eprintln!("{}PROP \"{}\" (len={}) = \"{}\"", "  ".repeat(indent), nameoff, len, val_str);
                }
                pos = (pos + 3) & !3;
            }
            0x9 => {
                eprintln!("FDT_END");
                break;
            }
            _ => {
                eprintln!("UNKNOWN TOKEN 0x{:08X} at offset {}", token, pos);
                break;
            }
        }
    }
    
    // Also dump mem_rsvmap
    eprintln!("\n--- Memory reservation map ---");
    let rsv_start = off_mem_rsvmap as usize;
    for entry in 0..4 {
        let off = rsv_start + entry * 16;
        if off + 16 > buf.len() { break; }
        let addr = u64::from_be_bytes([buf[off], buf[off+1], buf[off+2], buf[off+3],
            buf[off+4], buf[off+5], buf[off+6], buf[off+7]]);
        let size = u64::from_be_bytes([buf[off+8], buf[off+9], buf[off+10], buf[off+11],
            buf[off+12], buf[off+13], buf[off+14], buf[off+15]]);
        if addr == 0 && size == 0 {
            eprintln!("  [entry {}] TERMINATOR (0, 0)", entry);
            break;
        }
        eprintln!("  [entry {}] addr=0x{:X} size=0x{:X} ({}KB)", entry, addr, size, size / 1024);
    }
}
