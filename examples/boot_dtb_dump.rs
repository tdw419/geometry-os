//! Dump DTB content from physical memory after boot setup.
//! Run: cargo run --example boot_dtb_dump

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    let initramfs_data = std::path::Path::new(ir_path)
        .exists()
        .then(|| std::fs::read(ir_path).unwrap());

    let (mut vm, _fw, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_data,
        initramfs_data.as_deref(),
        512,
        "console=ttyS0 earlycon=sbi loglevel=7",
    )
    .expect("boot setup failed");

    // Dump DTB from physical memory
    eprintln!("[dtb_dump] DTB at PA 0x{:08X}, size:", dtb_addr);

    // Read DTB header (40 bytes)
    let mut hdr = Vec::new();
    for i in 0..10 {
        let val = vm.bus.read_word(dtb_addr + (i as u64) * 4).unwrap_or(0);
        hdr.push(val);
    }
    eprintln!("  magic:       0x{:08X} (expect 0xD00DFEED)", hdr[0]);
    eprintln!("  totalsize:   0x{:08X} ({} bytes)", hdr[1], hdr[1]);
    eprintln!("  off_dt_struct: 0x{:08X}", hdr[2]);
    eprintln!("  off_dt_strings: 0x{:08X}", hdr[3]);
    eprintln!("  off_mem_rsvmap: 0x{:08X}", hdr[4]);
    eprintln!("  version:     0x{:08X}", hdr[5]);
    eprintln!("  last_comp_version: 0x{:08X}", hdr[6]);

    // Scan DTB structure for memory node
    let struct_start = dtb_addr + hdr[2] as u64;
    let strings_start = dtb_addr + hdr[3] as u64;
    let mut pos = struct_start;
    let mut depth = 0i32;
    let mut in_memory = false;
    let mut found_memory = false;
    let mut found_cpus = false;
    let mut found_soc = false;

    while pos < dtb_addr + hdr[1] as u64 {
        let token = vm.bus.read_word(pos).unwrap_or(0);
        pos += 4;

        match token {
            0x00000001 => {
                // FDT_BEGIN_NODE
                let name_start = pos;
                let mut name = String::new();
                loop {
                    let b = vm.bus.read_byte(pos).unwrap_or(0);
                    pos += 1;
                    if b == 0 {
                        break;
                    }
                    name.push(b as char);
                }
                pos = (pos + 3) & !3; // align

                if name.is_empty() {
                    eprintln!("  BEGIN_NODE (root)");
                } else {
                    eprintln!("  {}BEGIN_NODE \"{}\"", "  ".repeat(depth as usize), name);
                    if name == "memory@0" {
                        in_memory = true;
                        found_memory = true;
                    }
                }
                depth += 1;
            }
            0x00000002 => {
                // FDT_END_NODE
                depth -= 1;
                if in_memory && depth <= 1 {
                    in_memory = false;
                }
            }
            0x00000003 => {
                // FDT_PROP
                let prop_len = vm.bus.read_word(pos).unwrap_or(0);
                let name_off = vm.bus.read_word(pos + 4).unwrap_or(0);
                pos += 8;

                // Read property name from strings block
                let mut prop_name = String::new();
                let mut str_pos = strings_start + name_off as u64;
                loop {
                    let b = vm.bus.read_byte(str_pos).unwrap_or(0);
                    str_pos += 1;
                    if b == 0 {
                        break;
                    }
                    prop_name.push(b as char);
                }

                // Read property value
                let mut val_bytes = Vec::new();
                for _ in 0..prop_len {
                    val_bytes.push(vm.bus.read_byte(pos).unwrap_or(0));
                    pos += 1;
                }
                pos = (pos + 3) & !3; // align

                // Format value
                let val_str = if prop_len <= 32
                    && val_bytes.iter().all(|&b| b.is_ascii_graphic() || b == 0)
                {
                    let s: String = val_bytes
                        .iter()
                        .take_while(|&&b| b != 0)
                        .map(|&b| b as char)
                        .collect();
                    format!("\"{}\"", s)
                } else {
                    let hex: Vec<String> = val_bytes
                        .chunks(4)
                        .map(|chunk| {
                            if chunk.len() == 4 {
                                format!(
                                    "{:02X}{:02X}{:02X}{:02X}",
                                    chunk[0], chunk[1], chunk[2], chunk[3]
                                )
                            } else {
                                format!("{:02X?}", chunk)
                            }
                        })
                        .collect();
                    format!("[{}]", hex.join(" "))
                };

                eprintln!(
                    "  {}PROP \"{}\" = {}",
                    "  ".repeat(depth as usize),
                    prop_name,
                    val_str
                );

                // Check key properties
                if prop_name == "device_type" && val_bytes.starts_with(b"memory") {
                    found_memory = true;
                }
                if in_memory && prop_name == "reg" {
                    if val_bytes.len() == 16 {
                        let addr = u64::from_be_bytes(val_bytes[0..8].try_into().unwrap());
                        let size = u64::from_be_bytes(val_bytes[8..16].try_into().unwrap());
                        eprintln!(
                            "  --> Memory region: base=0x{:08X} size=0x{:08X} ({}MB)",
                            addr,
                            size,
                            size / (1024 * 1024)
                        );
                    }
                }
                if prop_name == "compatible" {
                    let s: String = val_bytes
                        .iter()
                        .take_while(|&&b| b != 0)
                        .map(|&b| b as char)
                        .collect();
                    if s.contains("cpu") {
                        found_cpus = true;
                    }
                    if s.contains("riscv,cpu-intc") || s.contains("clint") || s.contains("uart") {
                        found_soc = true;
                    }
                }
                if prop_name == "timebase-frequency" && val_bytes.len() == 4 {
                    let freq = u32::from_be_bytes(val_bytes.try_into().unwrap());
                    eprintln!(
                        "  --> Timebase frequency: {} Hz ({} MHz)",
                        freq,
                        freq / 1_000_000
                    );
                }
            }
            0x00000009 => {
                // FDT_END
                break;
            }
            _ => {
                eprintln!("  UNKNOWN token 0x{:08X} at offset 0x{:X}", token, pos - 4);
                break;
            }
        }
    }

    eprintln!(
        "\n[dtb_dump] Summary: memory={}, cpus={}, soc={}",
        found_memory, found_cpus, found_soc
    );
}
