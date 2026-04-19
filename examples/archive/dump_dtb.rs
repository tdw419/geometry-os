fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;

    let (mut vm, _fw, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    // Dump DTB structure to verify memory node
    let totalsize = u32::from_be(vm.bus.read_word(dtb_addr + 4).unwrap_or(0));
    let off_struct = u32::from_be(vm.bus.read_word(dtb_addr + 8).unwrap_or(0));
    let off_strings = u32::from_be(vm.bus.read_word(dtb_addr + 12).unwrap_or(0));

    // Read strings block to find "memory" and "device_type" and "reg"
    let strings_start = dtb_addr + off_strings as u64;
    let strings_end = dtb_addr + totalsize as u64;
    let mut strings = Vec::new();
    for i in 0..((strings_end - strings_start) as usize) {
        let b = vm.bus.read_byte(strings_start + i as u64).unwrap_or(0);
        strings.push(b);
    }
    let s = String::from_utf8_lossy(&strings);
    println!("Strings block ({} bytes):", strings.len());
    // Print strings with offsets
    let mut pos = 0;
    while pos < strings.len() {
        let end = strings[pos..]
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(strings.len() - pos);
        let str_val = String::from_utf8_lossy(&strings[pos..pos + end]);
        if !str_val.is_empty() && str_val.len() < 80 {
            println!("  offset {}: \"{}\"", pos, str_val);
        }
        pos += end + 1;
    }

    // Walk structure block
    println!("\nStructure block:");
    let struct_start = dtb_addr + off_struct as u64;
    let mut pos = 0u64;
    let mut depth = 0;
    let mut in_memory_node = false;
    let mut node_count = 0;
    while pos < off_strings as u64 {
        let token = u32::from_be(vm.bus.read_word(struct_start + pos).unwrap_or(0));
        pos += 4;
        match token {
            0x00000001 => {
                // FDT_BEGIN_NODE
                let mut name = Vec::new();
                loop {
                    let b = vm.bus.read_byte(struct_start + pos).unwrap_or(0);
                    pos += 1;
                    if b == 0 {
                        break;
                    }
                    name.push(b);
                }
                // Align to 4 bytes
                while pos % 4 != 0 {
                    pos += 1;
                }
                let name_str = String::from_utf8_lossy(&name);
                if name_str.contains("memory") {
                    in_memory_node = true;
                }
                println!("{}BEGIN_NODE \"{}\"", "  ".repeat(depth), name_str);
                depth += 1;
                node_count += 1;
            }
            0x00000002 => {
                // FDT_END_NODE
                depth -= 1;
                if depth <= 1 {
                    in_memory_node = false;
                }
                println!("{}END_NODE", "  ".repeat(depth));
            }
            0x00000003 => {
                // FDT_PROP
                let prop_len = u32::from_be(vm.bus.read_word(struct_start + pos).unwrap_or(0));
                let name_off = u32::from_be(vm.bus.read_word(struct_start + pos + 4).unwrap_or(0));
                pos += 8;

                // Read property name from strings
                let mut name = Vec::new();
                let mut p = name_off as usize;
                while p < strings.len() {
                    let b = strings[p];
                    p += 1;
                    if b == 0 {
                        break;
                    }
                    name.push(b);
                }
                let name_str = String::from_utf8_lossy(&name);

                // Read property value
                let mut val_bytes = Vec::new();
                for i in 0..prop_len as usize {
                    val_bytes.push(vm.bus.read_byte(struct_start + pos + i as u64).unwrap_or(0));
                }

                let val_str = match name_str.as_ref() {
                    "reg" => {
                        if val_bytes.len() == 16 {
                            let base =
                                u64::from_be_bytes(val_bytes[0..8].try_into().unwrap_or([0; 8]));
                            let size =
                                u64::from_be_bytes(val_bytes[8..16].try_into().unwrap_or([0; 8]));
                            format!(
                                "base=0x{:X} size=0x{:X} ({}MB)",
                                base,
                                size,
                                size / (1024 * 1024)
                            )
                        } else {
                            format!("{:?}", val_bytes)
                        }
                    }
                    "bootargs" | "stdout-path" | "compatible" | "model" | "device_type"
                    | "status" | "isa" | "mmu-type" => {
                        String::from_utf8_lossy(&val_bytes).to_string()
                    }
                    _ => {
                        // Show as hex for small values, string for longer
                        if prop_len <= 8 {
                            let hex: Vec<String> =
                                val_bytes.iter().map(|b| format!("{:02X}", b)).collect();
                            format!("0x{}", hex.join(""))
                        } else {
                            format!("{} bytes", prop_len)
                        }
                    }
                };

                println!("{}PROP \"{}\" = {}", "  ".repeat(depth), name_str, val_str);

                pos += prop_len as u64;
                while pos % 4 != 0 {
                    pos += 1;
                }
            }
            0x00000009 => {
                // FDT_END
                println!("END");
                break;
            }
            _ => {
                println!("{}UNKNOWN TOKEN 0x{:08X}", "  ".repeat(depth), token);
                break;
            }
        }
        if node_count > 50 {
            break;
        }
    }
}
