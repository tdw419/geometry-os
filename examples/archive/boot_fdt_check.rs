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

    let mut count: u64 = 0;
    while count < 500_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let pc = vm.cpu.pc;

        // fdt_check_header at 0xC01FB112
        if pc == 0xC01FB112 {
            eprintln!("[{}] fdt_check_header(0x{:08X})", count, vm.cpu.x[10]);
            // Check what's at that address
            let addr = vm.cpu.x[10] as u64;
            match vm.bus.read_word(addr) {
                Ok(v) => eprintln!(
                    "  Magic at 0x{:08X}: 0x{:08X} (BE: 0x{:08X})",
                    addr,
                    v,
                    u32::from_be(v as u32)
                ),
                Err(e) => eprintln!("  FAILED to read: {:?}", e),
            }
        }

        let _ = vm.step();
        count += 1;

        // Check return value (a0 after return from fdt_check_header)
        // The return address is in early_init_dt_verify at 0xC041BCA4 (approximately)
        // Actually, let me check after each step if we just returned from fdt_check_header
    }
}
