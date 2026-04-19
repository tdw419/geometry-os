fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;

    let (mut vm, _fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    // 1. Check DTB magic at the physical address
    let dtb_magic = vm.bus.read_word(dtb_addr).unwrap_or(0);
    let dtb_magic_be = u32::from_be(dtb_magic);
    println!(
        "DTB at PA 0x{:08X}: magic=0x{:08X} (expect 0xD00DFEED)",
        dtb_addr, dtb_magic_be
    );

    // 2. Read DTB header fields
    let totalsize = u32::from_be(vm.bus.read_word(dtb_addr + 4).unwrap_or(0));
    let off_mem_rsvmap = u32::from_be(vm.bus.read_word(dtb_addr + 16).unwrap_or(0));
    let off_dt_struct = u32::from_be(vm.bus.read_word(dtb_addr + 8).unwrap_or(0));
    let off_dt_strings = u32::from_be(vm.bus.read_word(dtb_addr + 12).unwrap_or(0));
    println!(
        "DTB header: totalsize={} off_mem_rsvmap={} off_dt_struct={} off_dt_strings={}",
        totalsize, off_mem_rsvmap, off_dt_struct, off_dt_strings
    );

    // 3. Read memory reservation map entries
    let rsvmap_base = dtb_addr + off_mem_rsvmap as u64;
    println!("\nMemory reservation map at PA 0x{:08X}:", rsvmap_base);
    for i in 0..5 {
        let hi_addr = vm.bus.read_word(rsvmap_base + i as u64 * 16).unwrap_or(0);
        let lo_addr = vm
            .bus
            .read_word(rsvmap_base + i as u64 * 16 + 4)
            .unwrap_or(0);
        let hi_size = vm
            .bus
            .read_word(rsvmap_base + i as u64 * 16 + 8)
            .unwrap_or(0);
        let lo_size = vm
            .bus
            .read_word(rsvmap_base + i as u64 * 16 + 12)
            .unwrap_or(0);
        let addr = (u64::from(hi_addr) << 32) | u64::from(lo_addr);
        let size = (u64::from(hi_size) << 32) | u64::from(lo_size);
        if addr == 0 && size == 0 {
            println!("  Entry {}: (terminator)", i);
            break;
        }
        println!(
            "  Entry {}: addr=0x{:016X} size=0x{:016X} ({}KB)",
            i,
            addr,
            size,
            size / 1024
        );
    }

    // 4. Check _dtb_early_va and _dtb_early_pa
    let dtb_early_va = vm.bus.read_word(0x00801008).unwrap_or(0);
    let dtb_early_pa = vm.bus.read_word(0x0080100C).unwrap_or(0);
    println!(
        "\n_dtb_early_va = 0x{:08X} (expect 0x{:08X})",
        dtb_early_va,
        (dtb_addr as u32).wrapping_add(0xC0000000)
    );
    println!(
        "_dtb_early_pa = 0x{:08X} (expect 0x{:08X})",
        dtb_early_pa, dtb_addr as u32
    );

    // 5. Check phys_ram_base
    let prb = vm.bus.read_word(0x00C79EAC).unwrap_or(0);
    println!("phys_ram_base = 0x{:08X}", prb);

    // 6. Check memblock struct layout
    // memblock symbol should be at VA 0xC0803448 (PA 0x00803448)
    // Try different offsets for reserved.cnt
    println!("\nMemblock struct at PA 0x00803448:");
    for off in &[0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 60] {
        let val = vm.bus.read_word(0x00803448 + *off as u64).unwrap_or(0);
        if val != 0 {
            println!("  offset {}: 0x{:08X} ({})", off, val, val);
        }
    }

    // 7. Try to find memblock by searching for memory.cnt=1
    // In the kernel, after early_init_dt_scan_memory, memory.cnt should be 1
    // and memory.regions[0] should have base=0 and size=256MB
    // The regions pointer will point to a static array
    println!("\nScanning for memblock regions array...");
    for probe_off in &[0, 8, 24, 40] {
        let regions_ptr = vm.bus.read_word(0x00803448 + probe_off + 12).unwrap_or(0); // regions is 3rd field (cnt, max, total_size, regions)
        if regions_ptr > 0 && regions_ptr < 0x02000000 {
            // Try to read from this pointer (as physical address)
            let base = vm.bus.read_word(regions_ptr as u64).unwrap_or(0);
            let size = vm.bus.read_word(regions_ptr as u64 + 4).unwrap_or(0);
            println!(
                "  offset {}+12 (regions ptr): 0x{:08X} -> [0]=base=0x{:08X} size=0x{:08X}",
                probe_off, regions_ptr, base, size
            );
        }
    }

    // 8. Also try reading memory.cnt and reserved.cnt from different offset combinations
    // Layout attempt: bottom_up(4) + current_limit(4) + memory(16) + reserved(16) = 40
    // Layout attempt: bottom_up(4) + current_limit(4) + padding(4) + memory(16) + reserved(16) = 44
    // Layout attempt with physmem: ... + physmem(16) = 56
    println!("\nTrying different memblock layouts:");
    for mem_off in &[8, 12, 16, 20, 24] {
        for res_off in &[mem_off + 16, mem_off + 20, mem_off + 24] {
            let mem_cnt = vm.bus.read_word(0x00803448 + mem_off).unwrap_or(0);
            let res_cnt = vm.bus.read_word(0x00803448 + res_off).unwrap_or(0);
            if mem_cnt == 1 && res_cnt <= 10 {
                let mem_max = vm.bus.read_word(0x00803448 + mem_off + 4).unwrap_or(0);
                let res_max = vm.bus.read_word(0x00803448 + res_off + 4).unwrap_or(0);
                println!(
                    "  memory@{} cnt={} max={}, reserved@{} cnt={} max={}",
                    mem_off, mem_cnt, mem_max, res_off, res_cnt, res_max
                );
            }
        }
    }
}
