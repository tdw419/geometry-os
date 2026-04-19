//! Full DTB header diagnostic.
use geometry_os::riscv::RiscvVm;
fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    let (mut vm, _fw, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_data, None, 512, "console=ttyS0 earlycon=sbi nosmp")
            .expect("setup");

    // Read full 28-byte FDT header (big-endian)
    let mut hdr = [0u8; 28];
    for i in 0..28 {
        hdr[i] = vm.bus.read_byte(dtb_addr + i as u64).unwrap_or(0);
    }
    let r32 = |b: &[u8], off: usize| -> u32 {
        ((b[off] as u32) << 24)
            | ((b[off + 1] as u32) << 16)
            | ((b[off + 2] as u32) << 8)
            | (b[off + 3] as u32)
    };
    println!("DTB header at PA 0x{:08X}:", dtb_addr);
    println!(
        "  magic:             0x{:08X} (expect 0xD00DFEED)",
        r32(&hdr, 0)
    );
    println!("  totalsize:         {}", r32(&hdr, 4));
    println!("  off_dt_struct:     {}", r32(&hdr, 8));
    println!("  off_dt_strings:    {}", r32(&hdr, 12));
    println!("  off_mem_rsvmap:    0x{:08X}", r32(&hdr, 16));
    println!("  version:           {}", r32(&hdr, 20));
    println!("  last_comp_version: {}", r32(&hdr, 24));

    // Read memory reservation map (at off_mem_rsvmap)
    let mem_rsv_off = r32(&hdr, 16) as u64;
    println!("\nMemory reservation map at offset {}:", mem_rsv_off);
    for i in 0..8 {
        let addr = vm
            .bus
            .read_word(dtb_addr + mem_rsv_off + (i * 16) as u64)
            .unwrap_or(0);
        let size = vm
            .bus
            .read_word(dtb_addr + mem_rsv_off + (i * 16 + 4) as u64)
            .unwrap_or(0);
        if addr == 0 && size == 0 {
            break;
        }
        println!("  reservation: addr=0x{:08X} size=0x{:08X}", addr, size);
    }

    // Check initial_boot_params
    let ibp_va = 0xC0C7A380u32;
    let ibp_pa = ibp_va - 0xC0000000;
    let ibp_val = vm.bus.read_word(ibp_pa as u64).unwrap_or(0);
    println!("\ninitial_boot_params:");
    println!("  VA: 0x{:08X}, PA: 0x{:08X}", ibp_va, ibp_pa);
    println!(
        "  Value: 0x{:08X} (expect DTB PA 0x{:08X})",
        ibp_val, dtb_addr as u32
    );

    // Check if DTB is readable through the boot page table
    // Try reading DTB through the MMU (VA = PA since identity mapped)
    let dtb_via_va = vm.bus.read_word(0x01579000).unwrap_or(0);
    println!(
        "\nDTB readable via identity map (VA=PA 0x01579000): 0x{:08X}",
        dtb_via_va
    );

    // Check kernel_map values
    let km_pa = 0x00C7A098u64;
    println!("\nkernel_map at PA 0x{:08X}:", km_pa);
    println!(
        "  page_offset:    0x{:08X}",
        vm.bus.read_word(km_pa).unwrap_or(0)
    );
    println!(
        "  virt_addr:      0x{:08X}",
        vm.bus.read_word(km_pa + 4).unwrap_or(0)
    );
    println!(
        "  virt_offset:    0x{:08X}",
        vm.bus.read_word(km_pa + 8).unwrap_or(0)
    );
    println!(
        "  phys_addr:      0x{:08X}",
        vm.bus.read_word(km_pa + 12).unwrap_or(0)
    );
    println!(
        "  size:           0x{:08X}",
        vm.bus.read_word(km_pa + 16).unwrap_or(0)
    );
    println!(
        "  va_pa_offset:   0x{:08X}",
        vm.bus.read_word(km_pa + 20).unwrap_or(0)
    );
    println!(
        "  va_kernel_pa_offset: 0x{:08X}",
        vm.bus.read_word(km_pa + 24).unwrap_or(0)
    );
}
