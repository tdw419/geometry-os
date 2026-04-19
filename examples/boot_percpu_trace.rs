// Trace the percpu crash: find exactly what address gets computed and how
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

    // Run to 15.5M (just before crash at ~15.7M)
    let mut count: u64 = 0;
    let fast_target = 15_500_000;

    while count < fast_target {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let _ = vm.step();
        count += 1;
    }
    eprintln!("Fast-forward to {} instructions, PC=0x{:08X}", count, vm.cpu.pc);

    // Single-step and watch for the crash area or exceptions
    let watch_limit = 500_000;
    let mut step_count: u64 = 0;
    let mut high_access_count = 0;

    while step_count < watch_limit {
        let pc = vm.cpu.pc;
        let instr = vm.bus.read_word(pc.into()).unwrap_or(0);
        let _ = vm.step();
        step_count += 1;
        count += 1;

        // Watch for the crash area
        if pc >= 0xC0228270 && pc <= 0xC02282A0 {
            let x = &vm.cpu.x;
            eprintln!(
                "@ {}M PC=0x{:08X} instr=0x{:08X}",
                count as f64 / 1e6, pc, instr
            );
            eprintln!(
                "  a0=0x{:08X} a1=0x{:08X} a2=0x{:08X} a3=0x{:08X}",
                x[10], x[11], x[12], x[13]
            );
            eprintln!(
                "  t0=0x{:08X} t1=0x{:08X} t2=0x{:08X} t3=0x{:08X}",
                x[5], x[6], x[7], x[28]
            );
            eprintln!(
                "  s0=0x{:08X} s1=0x{:08X} s2=0x{:08X} s3=0x{:08X}",
                x[8], x[9], x[18], x[19]
            );
            high_access_count += 1;
            if high_access_count > 20 {
                break;
            }
        }

        // Break on exception
        let scause = vm.cpu.csr.scause;
        if scause != 0 {
            let stval = vm.cpu.csr.stval;
            eprintln!(
                "\nEXCEPTION at count={} PC=0x{:08X}: scause=0x{:08X} stval=0x{:08X}",
                count, pc, scause, stval
            );
            let x = &vm.cpu.x;
            eprintln!(
                "  a0=0x{:08X} a1=0x{:08X} a2=0x{:08X} a3=0x{:08X}",
                x[10], x[11], x[12], x[13]
            );
            eprintln!(
                "  t0=0x{:08X} t1=0x{:08X} t2=0x{:08X} t3=0x{:08X}",
                x[5], x[6], x[7], x[28]
            );
            eprintln!(
                "  s0=0x{:08X} s1=0x{:08X} s2=0x{:08X} s3=0x{:08X}",
                x[8], x[9], x[18], x[19]
            );
            break;
        }
    }

    eprintln!(
        "\nFinal: {} total instructions, {} single-steps",
        count, step_count
    );
}
