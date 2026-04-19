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

    // Run to 400K instructions (after earlycon init)
    for _ in 0..400_000 {
        let _ = vm.step();
    }

    // Read what the first DBCN write would have written
    // From the trace: a0=0xBB (187 bytes), a1=0x01488C14
    let addr = 0x01488C14u64;
    let mut s = Vec::new();
    for i in 0..187 {
        let b = vm.bus.read_byte(addr + i).unwrap();
        s.push(b);
    }
    eprintln!("First DBCN write (187 bytes from PA 0x{:08X}):", addr);
    eprintln!("{}", String::from_utf8_lossy(&s));
}
