// Check kernel_map values in memory
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi loglevel=8",
    )
    .unwrap();

    // kernel_map is at VA 0xC0C7A098, PA = 0x00C7A098
    let km_va: u32 = 0xC0C7A098;
    let km_pa = (km_va - 0xC0000000) as u64;

    eprintln!("kernel_map at PA 0x{:08X}:", km_pa);
    for off in (0..28).step_by(4) {
        let v = vm.bus.read_word(km_pa + off).unwrap_or(0xDEAD);
        eprintln!("  +{}: 0x{:08X}", off, v);
    }

    // struct kernel_mapping {
    //   unsigned long page_offset;    // +0
    //   unsigned long virt_addr;      // +4
    //   unsigned long virt_offset;    // +8
    //   uintptr_t phys_addr;          // +12
    //   uintptr_t size;               // +16
    //   unsigned long va_pa_offset;   // +20
    //   unsigned long va_kernel_pa_offset; // +24
    // }

    let page_offset = vm.bus.read_word(km_pa).unwrap();
    let virt_addr = vm.bus.read_word(km_pa + 4).unwrap();
    let virt_offset = vm.bus.read_word(km_pa + 8).unwrap();
    let phys_addr = vm.bus.read_word(km_pa + 12).unwrap();
    let size = vm.bus.read_word(km_pa + 16).unwrap();
    let va_pa_offset = vm.bus.read_word(km_pa + 20).unwrap();
    let va_kernel_pa_offset = vm.bus.read_word(km_pa + 24).unwrap();

    eprintln!("\nkernel_map fields:");
    eprintln!("  page_offset = 0x{:08X}", page_offset);
    eprintln!("  virt_addr = 0x{:08X}", virt_addr);
    eprintln!("  virt_offset = 0x{:08X}", virt_offset);
    eprintln!("  phys_addr = 0x{:08X}", phys_addr);
    eprintln!("  size = 0x{:08X}", size);
    eprintln!("  va_pa_offset = 0x{:08X}", va_pa_offset);
    eprintln!("  va_kernel_pa_offset = 0x{:08X}", va_kernel_pa_offset);

    // Now run to 15.7M and check again
    let mut count: u64 = 0;
    while count < 15_600_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let _ = vm.step();
        count += 1;
    }

    eprintln!("\nAfter 15.6M instructions:");
    let va_pa_offset2 = vm.bus.read_word(km_pa + 20).unwrap();
    let phys_addr2 = vm.bus.read_word(km_pa + 12).unwrap();
    eprintln!("  va_pa_offset = 0x{:08X}", va_pa_offset2);
    eprintln!("  phys_addr = 0x{:08X}", phys_addr2);
}
