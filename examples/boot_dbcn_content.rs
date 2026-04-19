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

    // Run to first DBCN call (at ~387K instructions)
    let mut count: u64 = 0;
    while count < 500_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let pc = vm.cpu.pc;

        // Check for DBCN write ECALL
        if pc == 0xC0002526 {
            // approximate SBI call site
            let a7 = vm.cpu.x[17];
            let a6 = vm.cpu.x[16];
            if a7 == 0x4442434E && a6 == 0 {
                let num_bytes = vm.cpu.x[10] as u64;
                let addr_lo = vm.cpu.x[11] as u64;
                let addr_hi = vm.cpu.x[12] as u64;
                let addr = (addr_hi << 32) | addr_lo;
                eprintln!("DBCN_WRITE: {} bytes from PA 0x{:08X}", num_bytes, addr);
                // Read content BEFORE the ECALL is handled
                let mut s = Vec::new();
                for i in 0..num_bytes.min(200) {
                    let b = vm.bus.read_byte(addr + i).unwrap();
                    s.push(b);
                }
                eprintln!(
                    "Content: \"{}\"",
                    String::from_utf8_lossy(&s).replace('\0', "\\0")
                );
                break;
            }
        }

        let _ = vm.step();
        count += 1;
    }
}
