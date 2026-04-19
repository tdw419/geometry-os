use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi loglevel=8",
    )
    .unwrap();

    eprintln!(
        "Setup: fw_addr=0x{:X} entry=0x{:X} dtb_addr=0x{:X}",
        fw_addr, entry, dtb_addr
    );

    // Run a bit to let the kernel copy values
    for _ in 0..500_000 {
        let _ = vm.step();
    }

    // Read _dtb_early_va and _dtb_early_pa
    // These are at VA 0xC0801008 and 0xC080100C
    // PA = VA - 0xC0000000
    let va_pa = 0x0801008;
    let pa_pa = 0x080100C;

    let dtb_va = vm.bus.read_word(va_pa).unwrap();
    let dtb_pa = vm.bus.read_word(pa_pa).unwrap();

    eprintln!("_dtb_early_va = 0x{:08X}", dtb_va);
    eprintln!("_dtb_early_pa = 0x{:08X}", dtb_pa);
    eprintln!(
        "Expected dtb_early_va = 0x{:08X} (dtb_addr + va_pa_offset)",
        dtb_addr as u32 + 0xC0000000
    );
    eprintln!("Expected dtb_early_pa = 0x{:08X}", dtb_addr as u32);

    // Also check boot_command_line after parse_dtb
    // boot_command_line is typically the first thing filled
    // Let's check after a few million more instructions
    for _ in 0..10_000_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let _ = vm.step();
    }

    // Check boot_command_line
    // boot_command_line is a global array, find its address
    let bcl_addr = 0x08041900; // approximate, from nm
                               // Actually let me find it properly
                               // It's usually a static array in init/main.c
}
