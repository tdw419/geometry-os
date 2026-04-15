fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 panic=1";

    use geometry_os::riscv::RiscvVm;

    // Boot until we first hit __memmove (PC=0xC08E5D6A)
    let (mut vm, _) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        260000,  // Just before __memmove
        bootargs,
    )
    .unwrap();

    // Run step by step until we reach __memmove
    let mut count = 260000;
    let target_pc: u32 = 0xC08E5D6A;
    while count < 300000 {
        use geometry_os::riscv::cpu::StepResult;
        match vm.step() {
            StepResult::Ok => {}
            StepResult::Ebreak => break,
            _ => {}
        }
        count += 1;
        if vm.cpu.pc == target_pc {
            // Found __memmove entry point - print register state
            println!("Entered __memmove at count={}", count);
            println!("PC=0x{:08X} (should be __memmove+0x112)", vm.cpu.pc);
            println!("a0=0x{:08X} (dst)", vm.cpu.x[10]);
            println!("a1=0x{:08X} (src)", vm.cpu.x[11]);
            println!("a2=0x{:08X} (n/len)", vm.cpu.x[12]);
            println!("a3=0x{:08X} (t1)", vm.cpu.x[13]);
            println!("a4=0x{:08X} (src_end? actual src ptr in loop)", vm.cpu.x[14]);
            println!("t0=0x{:08X}", vm.cpu.x[5]);
            println!("t1=0x{:08X}", vm.cpu.x[6]);
            println!("t2=0x{:08X}", vm.cpu.x[7]);
            println!("t3=0x{:08X}", vm.cpu.x[28]);
            println!("t4=0x{:08X} (dst ptr in loop)", vm.cpu.x[29]);
            println!("t5=0x{:08X} (end ptr in loop)", vm.cpu.x[30]);
            println!("t6=0x{:08X}", vm.cpu.x[31]);
            println!("sp=0x{:08X}", vm.cpu.x[2]);
            println!("ra=0x{:08X}", vm.cpu.x[1]);
            println!("s9=0x{:08X} (buffer in vsnprintf)", vm.cpu.x[25]);
            println!("s11=0x{:08X}", vm.cpu.x[27]);
            
            // Calculate memmove size
            let dst = vm.cpu.x[29]; // t4
            let end = vm.cpu.x[30]; // t5
            if dst > end {
                println!("\n*** BACKWARD COPY: dst=0x{:08X} end=0x{:08X} size=0x{:08X} ({}) ***",
                    dst, end, dst - end, dst - end);
            } else {
                println!("\n*** FORWARD COPY: dst=0x{:08X} end=0x{:08X} ***", dst, end);
            }
            break;
        }
    }
}
