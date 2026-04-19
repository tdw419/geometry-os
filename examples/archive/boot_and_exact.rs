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
    while count < 15_560_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let _ = vm.step();
        count += 1;
    }

    let mut step_count: u64 = 0;
    while step_count < 500_000 {
        let pc = vm.cpu.pc;

        // Right BEFORE the AND at 0xC040AF72
        if pc == 0xC040AF72 {
            eprintln!(
                "BEFORE AND: a0=0x{:08X} a5=0x{:08X}",
                vm.cpu.x[10], vm.cpu.x[15]
            );
            eprintln!("  Expected result: 0x{:08X}", vm.cpu.x[10] & vm.cpu.x[15]);
        }

        let _ = vm.step();
        step_count += 1;
        count += 1;

        // Right AFTER: next PC should be 0xC040AF76
        if pc == 0xC040AF72 {
            eprintln!(
                "AFTER AND: s8=0x{:08X} PC now=0x{:08X}",
                vm.cpu.x[24], vm.cpu.pc
            );
        }

        if vm.cpu.csr.scause != 0 {
            break;
        }
    }
}
