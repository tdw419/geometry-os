fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 panic=1";

    use geometry_os::riscv::cpu::StepResult;
    use geometry_os::riscv::RiscvVm;

    let (mut vm, _) =
        RiscvVm::boot_linux(&kernel_image, initramfs.as_deref(), 256, 200000, bootargs).unwrap();

    let mut count = 200000u64;
    let loop_pc: u32 = 0xC08E5D6A;
    let mut found = false;
    let mut prev_pc = vm.cpu.pc;

    // Watch for ANY jump to 0xC08E5D6A
    while count < 350000 {
        match vm.step() {
            StepResult::Ok => {}
            StepResult::Ebreak => break,
            _ => {}
        }
        count += 1;

        if vm.cpu.pc == loop_pc && prev_pc != loop_pc && !found {
            found = true;
            println!(
                "First entry to 0xC08E5D6A at count={} (prev=0x{:08X}):",
                count, prev_pc
            );
            for i in 0..32 {
                println!("  x{}=0x{:08X}", i, vm.cpu.x[i]);
            }
            break;
        }

        prev_pc = vm.cpu.pc;
    }

    if !found {
        println!("0xC08E5D6A not reached. Final PC: 0x{:08X}", vm.cpu.pc);
    }
}
