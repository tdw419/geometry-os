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

        if pc == 0xC04006DA {
            eprintln!("[{}] parse_early_param called", count);
        }
        if pc == 0xC04149AE {
            eprintln!("[{}] param_setup_earlycon called", count);
        }
        if pc == 0xC0414750 {
            eprintln!("[{}] setup_earlycon called", count);
        }
        if pc == 0xC0414C9E {
            eprintln!("[{}] early_sbi_setup called", count);
        }
        if pc == 0xC04146C0 {
            eprintln!("[{}] earlycon_init called", count);
        }
        if pc == 0xC041BD4A {
            eprintln!("[{}] early_init_dt_scan called", count);
        }
        if pc == 0xC0403C4C {
            eprintln!("[{}] init_IRQ entered", count);
        }
        if pc == 0xC000252E {
            eprintln!("[{}] PANIC", count);
        }

        let _ = vm.step();
        count += 1;

        if vm.cpu.csr.scause != 0 && vm.cpu.csr.scause != 8 && vm.cpu.csr.scause != 9 {
            break;
        }
    }
}
