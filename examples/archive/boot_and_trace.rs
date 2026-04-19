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

        if pc == 0xC040AF6E {
            eprintln!("C.LUI entry: a5=0x{:08X}", vm.cpu.x[15]);
        }
        if pc == 0xC040AF70 {
            eprintln!("C.LUI result: a5=0x{:08X}", vm.cpu.x[15]);
        }
        if pc == 0xC040AF72 {
            eprintln!(
                "AND entry: a0=0x{:08X} a5=0x{:08X}",
                vm.cpu.x[10], vm.cpu.x[15]
            );
        }
        if pc == 0xC040AF76 {
            eprintln!("AND result: s8=0x{:08X}", vm.cpu.x[24]);
        }

        let _ = vm.step();
        count += 1;

        if vm.cpu.csr.scause != 0 && vm.cpu.csr.scause != 8 && vm.cpu.csr.scause != 9 {
            break;
        }
    }
}
