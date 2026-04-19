fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 panic=1";

    use geometry_os::riscv::cpu::StepResult;
    use geometry_os::riscv::RiscvVm;

    // Boot until MMU is enabled
    let (mut vm, _) =
        RiscvVm::boot_linux(&kernel_image, initramfs.as_deref(), 256, 200000, bootargs).unwrap();

    let mut count = 200000u64;
    let mut memmove_count = 0u32;
    let mut prev_pc = vm.cpu.pc;

    // Track first 10 calls to __memmove (entry at 0xC08E5D58)
    let memmove_entry: u32 = 0xC08E5D58;

    while count < 500000 && memmove_count < 10 {
        match vm.step() {
            StepResult::Ok => {}
            StepResult::Ebreak => break,
            r => {
                // S-mode fault, just continue
            }
        }
        count += 1;

        if vm.cpu.pc == memmove_entry && prev_pc != memmove_entry {
            memmove_count += 1;
            println!("__memmove call #{} at count={}:", memmove_count, count);
            println!("  a0=0x{:08X} (dst)", vm.cpu.x[10]);
            println!("  a1=0x{:08X} (src)", vm.cpu.x[11]);
            println!("  a2=0x{:08X} (n={})", vm.cpu.x[12], vm.cpu.x[12]);
            println!("  ra=0x{:08X}", vm.cpu.x[1]);
        }

        prev_pc = vm.cpu.pc;
    }

    if memmove_count == 0 {
        println!("No __memmove calls found in 500K instructions");
        println!("Final PC: 0x{:08X}", vm.cpu.pc);
    }
}
