use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (vm, info) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        5_000_000,
        "console=ttyS0 loglevel=8",
    ).unwrap();

    eprintln!(
        "[result] Boot completed: {} instructions, PC=0x{:08X}",
        info.instructions, vm.cpu.pc
    );
}
