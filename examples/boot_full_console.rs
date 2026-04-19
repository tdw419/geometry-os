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

    // Run until shutdown or 20M instructions
    let max = 20_000_000u64;
    let mut count = 0u64;
    while count < max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let _ = vm.step();
        count += 1;
    }

    // Dump all console output
    let out = String::from_utf8_lossy(&vm.bus.sbi.console_output);
    eprintln!(
        "=== Console output ({} bytes, {} instructions) ===",
        vm.bus.sbi.console_output.len(),
        count
    );
    eprintln!("{}", out);
    eprintln!("=== END ===");

    if vm.bus.sbi.shutdown_requested {
        eprintln!("\nKernel requested shutdown (SBI SRST).");
    }
}
