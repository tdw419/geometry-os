use geometry_os::riscv::cpu::StepResult;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    // Run until PC enters panic() at 0xC000252E
    let (mut vm, _br) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        30_000_000,
        "console=ttyS0 earlycon=sbi loglevel=8",
    )
    .unwrap();

    // Check if we're already in the panic loop
    let panic_start = 0xC000252Eu32;
    let pc = vm.cpu.pc;
    println!("Current PC: 0x{:08X}", pc);

    // Step a few and check if we enter panic
    let mut hit_panic = false;
    let mut pre_panic_pc = 0u32;
    for i in 0..100000 {
        let prev_pc = vm.cpu.pc;
        vm.step();
        if vm.cpu.pc == panic_start {
            println!(
                "Hit panic() at instruction {}! Called from PC=0x{:08X}",
                i, prev_pc
            );
            println!("  a0 (arg0/format string ptr) = 0x{:08X}", vm.cpu.x[10]);
            println!("  a1 (arg1) = 0x{:08X}", vm.cpu.x[11]);
            println!("  ra (return addr) = 0x{:08X}", vm.cpu.x[1]);
            println!("  sp = 0x{:08X}", vm.cpu.x[2]);
            hit_panic = true;
            break;
        }
    }

    if !hit_panic {
        println!("Did not hit panic entry in 100K instructions after 30M");
    }

    println!("SBI console: {} bytes", vm.bus.sbi.console_output.len());
    println!("UART tx_buf: {} bytes", vm.bus.uart.tx_buf.len());
}
