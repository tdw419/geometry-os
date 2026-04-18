/// Diagnostic: Parse the generated DTB to verify memory layout
use geometry_os::riscv::dtb::DtbConfig;
use geometry_os::riscv::loader;
use geometry_os::riscv::RiscvVm;

fn parse_dtb(dtb: &[u8]) {
    if dtb.len() < 28 || &dtb[0..4] != b"\xd0\x0d\xfe\xed" {
        eprintln!("[DTB] Invalid DTB magic");
        return;
    }
    let totalsize = u32::from_be_bytes(dtb[4..8].try_into().unwrap()) as usize;
    let off_dt_struct = u32::from_be_bytes(dtb[8..12].try_into().unwrap()) as usize;
    let off_dt_strings = u32::from_be_bytes(dtb[12..16].try_into().unwrap()) as usize;
    let off_mem_rsvmap = u32::from_be_bytes(dtb[16..20].try_into().unwrap()) as usize;
    let version = u32::from_be_bytes(dtb[20..24].try_into().unwrap());

    eprintln!(
        "[DTB] totalsize={} struct_off={} strings_off={} mem_rsvmap_off={} version={}",
        totalsize, off_dt_struct, off_dt_strings, off_mem_rsvmap, version
    );

    // Parse memory reservation map
    eprintln!("[DTB] Memory reservation map:");
    let mut pos = off_mem_rsvmap;
    loop {
        if pos + 16 > dtb.len() {
            break;
        }
        let addr = u64::from_be_bytes(dtb[pos..pos + 8].try_into().unwrap());
        let size = u64::from_be_bytes(dtb[pos + 8..pos + 16].try_into().unwrap());
        pos += 16;
        if addr == 0 && size == 0 {
            break;
        }
        eprintln!(
            "  reserved: PA 0x{:08X} - 0x{:08X} ({} bytes)",
            addr,
            addr + size,
            size
        );
    }

    // Parse structure block to find memory node
    eprintln!("[DTB] Looking for memory node in structure block...");
    pos = off_dt_struct;
    let strings = &dtb[off_dt_strings..];
    let mut depth = 0i32;
    while pos < off_dt_strings {
        let token = u32::from_be_bytes(dtb[pos..pos + 4].try_into().unwrap());
        pos += 4;
        match token {
            0x00000001 => {
                // FDT_BEGIN_NODE
                let name_start = pos;
                while pos < dtb.len() && dtb[pos] != 0 {
                    pos += 1;
                }
                let name = std::str::from_utf8(&dtb[name_start..pos]).unwrap_or("?");
                pos += 1; // null terminator
                pos = (pos + 3) & !3; // align to 4 bytes
                eprintln!("  BEGIN_NODE (depth={}): '{}'", depth, name);
                depth += 1;
            }
            0x00000002 => {
                // FDT_END_NODE
                depth -= 1;
            }
            0x00000003 => {
                // FDT_PROP
                let prop_len = u32::from_be_bytes(dtb[pos..pos + 4].try_into().unwrap()) as usize;
                let name_off =
                    u32::from_be_bytes(dtb[pos + 4..pos + 8].try_into().unwrap()) as usize;
                pos += 8;
                let prop_data = &dtb[pos..pos + prop_len];
                pos += prop_len;
                pos = (pos + 3) & !3; // align to 4 bytes
                                      // Get property name
                let name_end = strings[name_off..]
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(0);
                let name =
                    std::str::from_utf8(&strings[name_off..name_off + name_end]).unwrap_or("?");
                if name == "reg" || name == "device_type" || name == "compatible" {
                    if prop_len == 16 && name == "reg" {
                        let a = u64::from_be_bytes(prop_data[0..8].try_into().unwrap());
                        let s = u64::from_be_bytes(prop_data[8..16].try_into().unwrap());
                        eprintln!(
                            "    PROP '{}': base=0x{:08X} size=0x{:08X} ({}MB)",
                            name,
                            a,
                            s,
                            s / (1024 * 1024)
                        );
                    } else {
                        let preview: String = prop_data
                            .iter()
                            .take(40)
                            .map(|&b| {
                                if b >= 0x20 && b < 0x7f {
                                    b as char
                                } else {
                                    '.'
                                }
                            })
                            .collect();
                        eprintln!(
                            "    PROP '{}': len={} data={:?}...",
                            name, prop_len, preview
                        );
                    }
                }
            }
            0x00000009 => break, // FDT_END
            _ => break,
        }
    }
}

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    // Use boot_linux_setup to get the DTB address
    let (mut vm, _fw, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    )
    .unwrap();

    // Read the DTB from the VM's memory (use totalsize from header)
    let mut dtb_blob = Vec::new();
    // First read the header to get totalsize
    for i in 0..8 {
        if let Ok(b) = vm.bus.read_byte(dtb_addr + i as u64) {
            dtb_blob.push(b);
        }
    }
    let totalsize = u32::from_be_bytes(dtb_blob[4..8].try_into().unwrap()) as usize;
    // Read the rest
    for i in 8..totalsize {
        if let Ok(b) = vm.bus.read_byte(dtb_addr + i as u64) {
            dtb_blob.push(b);
        } else {
            break;
        }
    }
    eprintln!(
        "[DIAG] DTB at PA 0x{:08X}, size {} bytes",
        dtb_addr,
        dtb_blob.len()
    );
    parse_dtb(&dtb_blob);

    // Also generate a fresh DTB and parse it
    eprintln!("\n[DIAG] Fresh DTB from DtbConfig:");
    let config = DtbConfig {
        ram_base: 0,
        ram_size: 256 * 1024 * 1024,
        bootargs: "console=ttyS0 loglevel=8".to_string(),
        ..Default::default()
    };
    let fresh_dtb = geometry_os::riscv::dtb::generate_dtb(&config);
    parse_dtb(&fresh_dtb);
}
