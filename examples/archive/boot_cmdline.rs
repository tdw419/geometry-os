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

        // Check boot_command_line right after each parse_early_param call
        if pc == 0xC04006DA {
            let bcl_pa: u64 = 0x0800018;
            let mut s = Vec::new();
            for i in 0..256 {
                let b = vm.bus.read_byte(bcl_pa + i).unwrap();
                if b == 0 {
                    break;
                }
                s.push(b);
            }
            eprintln!(
                "[{}] parse_early_param: boot_command_line = \"{}\"",
                count,
                String::from_utf8_lossy(&s)
            );
        }

        let _ = vm.step();
        count += 1;

        if vm.cpu.csr.scause != 0 && vm.cpu.csr.scause != 8 && vm.cpu.csr.scause != 9 {
            break;
        }
    }
}
