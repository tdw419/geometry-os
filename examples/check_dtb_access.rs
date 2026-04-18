//! Check if the kernel can actually read the DTB.
//! Verify initial_boot_params, _dtb_early_va, and DTB accessibility.
//! cargo run --example check_dtb_access

use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    let initramfs_data = std::path::Path::new(ir_path)
        .exists()
        .then(|| std::fs::read(ir_path).unwrap());

    let (mut vm, fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(
            &kernel_data,
            initramfs_data.as_deref(),
            512,
            "console=ttyS0 earlycon=sbi",
        ).expect("setup failed");

    eprintln!("DTB at PA: 0x{:08X}", dtb_addr);
    let dtb_va = dtb_addr.wrapping_add(0xC0000000) as u32;
    eprintln!("DTB VA: 0x{:08X}", dtb_va);

    // Check initial_boot_params (pre-set by boot_linux_setup)
    let ibp = vm.bus.read_word(0x00C7A178).unwrap_or(0);
    eprintln!("initial_boot_params (PA 0x00C7A178): 0x{:08X} (expect 0x{:08X})", ibp, dtb_addr as u32);

    // Check what's at the DTB address
    let magic = vm.bus.read_word(dtb_addr).unwrap_or(0);
    eprintln!("DTB word[0] at PA: 0x{:08X} (expect 0xEDFE0DD0 for LE read of BE 0xD00DFEED)", magic);

    // Read more DTB header fields (all big-endian in FDT spec)
    let totalsize = vm.bus.read_word(dtb_addr + 4).unwrap_or(0);
    let off_struct = vm.bus.read_word(dtb_addr + 8).unwrap_or(0);
    let off_strings = vm.bus.read_word(dtb_addr + 12).unwrap_or(0);
    let off_memrsvmap = vm.bus.read_word(dtb_addr + 16).unwrap_or(0);
    eprintln!("DTB totalsize (BE): 0x{:08X} = {} bytes", totalsize, u32::from_be(totalsize));
    eprintln!("DTB off_struct (BE): 0x{:08X} = {}", off_struct, u32::from_be(off_struct));
    eprintln!("DTB off_strings (BE): 0x{:08X} = {}", off_strings, u32::from_be(off_strings));
    eprintln!("DTB off_memrsvmap (BE): 0x{:08X} = {}", off_memrsvmap, u32::from_be(off_memrsvmap));

    // Check version
    let version = vm.bus.read_word(dtb_addr + 20).unwrap_or(0);
    eprintln!("DTB version (BE): {}", u32::from_be(version));

    // Now check the structure block - look for the memory node
    let struct_off = u32::from_be(off_struct) as u64;
    eprintln!("\nScanning DTB structure at offset {}...", struct_off);

    // Walk the structure block
    let mut pos = dtb_addr + struct_off;
    let mut depth = 0i32;
    let mut found_memory = false;
    for _ in 0..200 {
        let token = vm.bus.read_word(pos).unwrap_or(0);
        match u32::from_be(token) {
            0x0000_0001 => { // FDT_BEGIN_NODE
                // Node name follows (null-terminated, padded to 4 bytes)
                let mut name = Vec::new();
                let mut p = pos + 4;
                loop {
                    let b = vm.bus.read_byte(p).unwrap_or(0);
                    if b == 0 { break; }
                    name.push(b);
                    p += 1;
                }
                let name_str = String::from_utf8_lossy(&name);
                eprintln!("  {}BEGIN_NODE: \"{}\"", "  ".repeat(depth as usize), name_str);
                if name_str.contains("memory") {
                    found_memory = true;
                }
                // Advance past name + padding
                p += 1; // skip null
                while p % 4 != 0 { p += 1; }
                pos = p;
                depth += 1;
            }
            0x0000_0002 => { // FDT_END_NODE
                depth -= 1;
                pos += 4;
            }
            0x0000_0003 => { // FDT_PROP
                let prop_len = u32::from_be(vm.bus.read_word(pos + 4).unwrap_or(0));
                let name_off = u32::from_be(vm.bus.read_word(pos + 8).unwrap_or(0));
                // Read property name from strings block
                let strings_base = dtb_addr + u32::from_be(off_strings) as u64 + name_off as u64;
                let mut pname = Vec::new();
                let mut sp = strings_base;
                loop {
                    let b = vm.bus.read_byte(sp).unwrap_or(0);
                    if b == 0 { break; }
                    pname.push(b);
                    sp += 1;
                }
                let pname_str = String::from_utf8_lossy(&pname);

                // Read property data (first 32 bytes)
                let mut data = Vec::new();
                for i in 0..prop_len.min(32) {
                    data.push(vm.bus.read_byte(pos + 12 + i as u64).unwrap_or(0));
                }
                eprintln!("  {}PROP: \"{}\" len={} data={:02x?}", "  ".repeat(depth as usize), pname_str, prop_len, data);

                // For "reg" property, decode as u64 pairs
                if pname_str == "reg" && prop_len >= 16 {
                    let addr = u64::from_be_bytes(data[0..8].try_into().unwrap_or([0;8]));
                    let size = u64::from_be_bytes(data[8..16].try_into().unwrap_or([0;8]));
                    eprintln!("    -> addr=0x{:016X} size=0x{:016X} ({}MB)", addr, size, size / (1024*1024));
                }

                pos += 12 + ((prop_len + 3) & !3) as u64;
            }
            0x0000_0009 => { // FDT_END
                eprintln!("  FDT_END");
                break;
            }
            other => {
                eprintln!("  UNKNOWN TOKEN 0x{:08X} at pos 0x{:08X}", other, pos);
                break;
            }
        }
    }

    if !found_memory {
        eprintln!("\n*** WARNING: No memory node found in DTB! ***");
    }

    // Now run the kernel with the full boot_linux() trap handling
    eprintln!("\n--- Running boot_linux() with 5M instructions ---");
    drop(vm); // We used boot_linux_setup, now use boot_linux for full handling

    let result = RiscvVm::boot_linux(
        &kernel_data,
        initramfs_data.as_deref(),
        512,
        5_000_000,
        "console=ttyS0 earlycon=sbi",
    );

    match result {
        Ok((mut vm, stats)) => {
            eprintln!("\nAfter 5M boot_linux():");
            eprintln!("  PC: 0x{:08X}", vm.cpu.pc);

            // Check memblock at correct offsets
            let mb = 0x00803448u64;
            let mem_cnt = vm.bus.read_word(mb + 8).unwrap_or(0);
            let mem_max = vm.bus.read_word(mb + 12).unwrap_or(0);
            let mem_regions = vm.bus.read_word(mb + 20).unwrap_or(0);
            let res_cnt = vm.bus.read_word(mb + 28).unwrap_or(0);
            eprintln!("  memblock: memory.cnt={} memory.max={} memory.regions=0x{:08X}", mem_cnt, mem_max, mem_regions);
            eprintln!("  memblock: reserved.cnt={}", res_cnt);

            if mem_cnt > 0 && mem_regions >= 0xC0000000 {
                let rpa = (mem_regions - 0xC0000000) as u64;
                for i in 0..mem_cnt.min(4) {
                    let base = vm.bus.read_word(rpa + (i*8) as u64).unwrap_or(0);
                    let size = vm.bus.read_word(rpa + (i*8+4) as u64).unwrap_or(0);
                    eprintln!("    memory[{}]: base=0x{:08X} size=0x{:08X} ({}MB)", i, base, size, size/(1024*1024));
                }
            }

            let prb = vm.bus.read_word(0x00C79EAC).unwrap_or(0);
            eprintln!("  phys_ram_base: 0x{:08X}", prb);
        }
        Err(e) => eprintln!("Boot failed: {:?}", e),
    }
}
