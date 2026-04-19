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
    while count < 20_000_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let pc = vm.cpu.pc;

        // setup_earlycon entry
        if pc == 0xC0414750 {
            eprintln!(
                "[{}] setup_earlycon called! a0=0x{:08X} (buf ptr)",
                count, vm.cpu.x[10]
            );
            // Read the string
            let pa = vm.cpu.x[10] as u64;
            let mut s = Vec::new();
            for i in 0..40 {
                match vm.bus.read_byte(pa + i) {
                    Ok(b) if b > 0 && b < 128 => s.push(b as u8),
                    _ => break,
                }
            }
            eprintln!("[{}] buf = \"{}\"", count, String::from_utf8_lossy(&s));
        }

        // param_setup_earlycon
        if pc == 0xC04149AE {
            eprintln!(
                "[{}] param_setup_earlycon called! a0=0x{:08X}",
                count, vm.cpu.x[10]
            );
            let pa = vm.cpu.x[10] as u64;
            let mut s = Vec::new();
            for i in 0..40 {
                match vm.bus.read_byte(pa + i) {
                    Ok(b) if b > 0 && b < 128 => s.push(b as u8),
                    _ => break,
                }
            }
            eprintln!("[{}] val = \"{}\"", count, String::from_utf8_lossy(&s));
        }

        // earlycon_init
        if pc == 0xC04146C0 {
            eprintln!("[{}] earlycon_init called", count);
        }

        // early_sbi_setup
        if pc == 0xC0414C9E {
            eprintln!("[{}] early_sbi_setup called!", count);
        }

        let _ = vm.step();
        count += 1;

        if vm.cpu.csr.scause != 0 && vm.cpu.csr.scause != 8 && vm.cpu.csr.scause != 9 {
            break;
        }
    }
}
