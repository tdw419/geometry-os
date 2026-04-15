fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::StepResult;

    let (mut vm, _) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        200000,
        bootargs,
    )
    .unwrap();

    let mut count = 200000u64;
    let loop_pc: u32 = 0xC08E5D6A;
    let mut found = false;
    let mut prev_pc = vm.cpu.pc;

    while count < 350000 {
        match vm.step() {
            StepResult::Ok => {}
            StepResult::Ebreak => break,
            _ => {}
        }
        count += 1;

        if vm.cpu.pc == loop_pc && prev_pc != loop_pc && !found {
            found = true;
            println!("First entry to 0xC08E5D6A loop at count={}:", count);
            println!("  prev_pc=0x{:08X}", prev_pc);
            println!("  ra=0x{:08X}", vm.cpu.x[1]);
            println!("  a0=0x{:08X} (dst)", vm.cpu.x[10]);
            println!("  a1=0x{:08X} (src)", vm.cpu.x[11]);
            println!("  a2=0x{:08X} (n)", vm.cpu.x[12]);
            println!("  t3=0x{:08X}", vm.cpu.x[28]);
            println!("  t4=0x{:08X}", vm.cpu.x[29]);
            println!("  t5=0x{:08X}", vm.cpu.x[30]);
            println!("  t6=0x{:08X}", vm.cpu.x[31]);
            println!("  sp=0x{:08X}", vm.cpu.x[2]);
        }

        prev_pc = vm.cpu.pc;
    }

    if !found {
        println!("Loop at 0x{:08X} not found. Final PC: 0x{:08X}", loop_pc, vm.cpu.pc);
    }
}
