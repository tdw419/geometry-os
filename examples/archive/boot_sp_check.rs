fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 panic=1";

    use geometry_os::riscv::RiscvVm;

    // Check stack pointer at various points
    for max_instr in [100, 1000, 10000, 100000, 500000] {
        let (mut vm, _) = RiscvVm::boot_linux(
            &kernel_image,
            initramfs.as_deref(),
            256,
            max_instr,
            bootargs,
        )
        .unwrap();
        println!(
            "After {} instr: PC=0x{:08X} SP=0x{:08X} RA=0x{:08X} TP=0x{:08X}",
            max_instr, vm.cpu.pc, vm.cpu.x[2], vm.cpu.x[1], vm.cpu.x[4]
        );
    }

    // Also check what _start expects for the stack
    // _start at 0xC0000000 just does: li s4, -13; j _start_kernel
    // _start_kernel at 0xC00010D0 is the real entry
    // Let's see what happens in the first few hundred instructions
}
