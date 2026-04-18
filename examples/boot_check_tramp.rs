use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) = geometry_os::riscv::RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        512,
        bootargs,
    )
    .unwrap();

    // Check L1[0] RIGHT NOW (after setup)
    let l1_0 = vm.bus.read_word(0x01484000).unwrap_or(0);
    let l1_768 = vm.bus.read_word(0x01484C00).unwrap_or(0);
    eprintln!(
        "[diag] After boot_linux_setup: trampoline L1[0]=0x{:08X} L1[768]=0x{:08X}",
        l1_0, l1_768
    );

    // Also check the raw RAM
    eprintln!("[diag] RAM at 0x1484000-0x148400F:");
    for i in 0..4 {
        let addr = 0x01484000u64 + (i as u64) * 4;
        let v = vm.bus.read_word(addr).unwrap_or(0);
        eprintln!("  [{}] 0x{:08X}: 0x{:08X}", i, addr, v);
    }
}
