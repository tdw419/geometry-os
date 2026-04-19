use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    )
    .unwrap();

    eprintln!("DTB at PA 0x{:08X}", dtb_addr);

    // Parse FDT header
    let magic = u32::from_be_bytes([
        vm.bus.read_byte(dtb_addr).unwrap(),
        vm.bus.read_byte(dtb_addr + 1).unwrap(),
        vm.bus.read_byte(dtb_addr + 2).unwrap(),
        vm.bus.read_byte(dtb_addr + 3).unwrap(),
    ]);
    let totalsize = u32::from_be_bytes([
        vm.bus.read_byte(dtb_addr + 4).unwrap(),
        vm.bus.read_byte(dtb_addr + 5).unwrap(),
        vm.bus.read_byte(dtb_addr + 6).unwrap(),
        vm.bus.read_byte(dtb_addr + 7).unwrap(),
    ]);
    let off_dt_struct = u32::from_be_bytes([
        vm.bus.read_byte(dtb_addr + 8).unwrap(),
        vm.bus.read_byte(dtb_addr + 9).unwrap(),
        vm.bus.read_byte(dtb_addr + 10).unwrap(),
        vm.bus.read_byte(dtb_addr + 11).unwrap(),
    ]);

    eprintln!(
        "magic=0x{:08X} totalsize={} struct_offset=0x{:X}",
        magic, totalsize, off_dt_struct
    );

    // Find the memory node by scanning for "memory" string
    let mut memory_reg_found = false;
    let struct_start = dtb_addr + off_dt_struct as u64;
    let mut pos = struct_start;
    let end = struct_start + totalsize as u64;

    while pos < end {
        let token = u32::from_be_bytes([
            vm.bus.read_byte(pos).unwrap(),
            vm.bus.read_byte(pos + 1).unwrap(),
            vm.bus.read_byte(pos + 2).unwrap(),
            vm.bus.read_byte(pos + 3).unwrap(),
        ]);

        if token == 0x00000001 {
            // FDT_BEGIN_NODE
            pos += 4;
            // Read node name
            let mut name = String::new();
            loop {
                let b = vm.bus.read_byte(pos).unwrap();
                pos += 1;
                if b == 0 {
                    break;
                }
                if b >= 0x20 && b < 0x7f {
                    name.push(b as char);
                }
            }
            // Align to 4 bytes
            pos = (pos + 3) & !3;
            eprintln!("Node: \"{}\" at offset 0x{:X}", name, pos - struct_start);

            if name.contains("memory") {
                // Scan properties for "reg"
                loop {
                    let prop_token = u32::from_be_bytes([
                        vm.bus.read_byte(pos).unwrap(),
                        vm.bus.read_byte(pos + 1).unwrap(),
                        vm.bus.read_byte(pos + 2).unwrap(),
                        vm.bus.read_byte(pos + 3).unwrap(),
                    ]);

                    if prop_token == 0x00000003 {
                        // FDT_PROP
                        pos += 4;
                        let prop_len = u32::from_be_bytes([
                            vm.bus.read_byte(pos).unwrap(),
                            vm.bus.read_byte(pos + 1).unwrap(),
                            vm.bus.read_byte(pos + 2).unwrap(),
                            vm.bus.read_byte(pos + 3).unwrap(),
                        ]);
                        let name_off = u32::from_be_bytes([
                            vm.bus.read_byte(pos + 4).unwrap(),
                            vm.bus.read_byte(pos + 5).unwrap(),
                            vm.bus.read_byte(pos + 6).unwrap(),
                            vm.bus.read_byte(pos + 7).unwrap(),
                        ]);
                        pos += 8;

                        // Read property name from strings block
                        let strings_off = u32::from_be_bytes([
                            vm.bus.read_byte(dtb_addr + 12).unwrap(),
                            vm.bus.read_byte(dtb_addr + 13).unwrap(),
                            vm.bus.read_byte(dtb_addr + 14).unwrap(),
                            vm.bus.read_byte(dtb_addr + 15).unwrap(),
                        ]);
                        let mut prop_name = String::new();
                        let mut name_pos = dtb_addr + strings_off as u64 + name_off as u64;
                        loop {
                            let b = vm.bus.read_byte(name_pos).unwrap();
                            name_pos += 1;
                            if b == 0 {
                                break;
                            }
                            if b >= 0x20 && b < 0x7f {
                                prop_name.push(b as char);
                            }
                        }

                        if prop_name == "reg" {
                            // Read reg value (address, size pairs, each 8 bytes for 64-bit)
                            let mut addr_parts = [0u8; 8];
                            let mut size_parts = [0u8; 8];
                            for i in 0..8u64 {
                                addr_parts[i as usize] = vm.bus.read_byte(pos + i).unwrap();
                                size_parts[i as usize] = vm.bus.read_byte(pos + 8 + i).unwrap();
                            }
                            let addr = u64::from_be_bytes(addr_parts);
                            let size = u64::from_be_bytes(size_parts);
                            eprintln!(
                                "  reg: addr=0x{:08X} size=0x{:08X} ({}MB)",
                                addr,
                                size,
                                size / (1024 * 1024)
                            );
                            memory_reg_found = true;
                        } else {
                            eprintln!("  prop \"{}\" len={}", prop_name, prop_len);
                        }

                        pos += (prop_len as u64 + 3) & !3;
                    } else if prop_token == 0x00000002 {
                        // FDT_END_NODE
                        pos += 4;
                        break;
                    } else if prop_token == 0x00000009 {
                        // FDT_END
                        pos += 4;
                        break;
                    } else {
                        pos += 4;
                    }
                }
            }
        } else if token == 0x00000002 {
            // FDT_END_NODE
            pos += 4;
        } else if token == 0x00000009 {
            // FDT_END
            break;
        } else {
            pos += 4;
        }
    }

    if !memory_reg_found {
        eprintln!("WARNING: memory/reg property not found!");
    }
}
