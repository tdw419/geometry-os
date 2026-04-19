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
    while count < 17_000_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let pc = vm.cpu.pc;

        // early_init_dt_scan_chosen
        if pc == 0xC041B798 {
            eprintln!(
                "[{}] early_init_dt_scan_chosen called, a0=0x{:08X}",
                count, vm.cpu.x[10]
            );
        }

        // early_init_dt_scan (the main entry)
        if pc == 0xC041BD4A {
            eprintln!("[{}] early_init_dt_scan called", count);
        }

        // parse_early_param
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
                "[{}] parse_early_param: boot_command_line=\"{}\"",
                count,
                String::from_utf8_lossy(&s)
            );
        }

        let _ = vm.step();
        count += 1;
    }
}
