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
    let memmove_entry: u32 = 0xC08E5C58;
    let mut memmove_count = 0u32;
    let mut prev_pc = vm.cpu.pc;
    let mut in_memmove = false;

    while count < 350000 {
        match vm.step() {
            StepResult::Ok => {}
            StepResult::Ebreak => break,
            _ => {}
        }
        count += 1;

        // Track __memmove entry
        if vm.cpu.pc == memmove_entry && prev_pc != memmove_entry {
            memmove_count += 1;
            in_memmove = true;
            if memmove_count <= 5 {
                println!("__memmove entry #{} at count={}:", memmove_count, count);
                println!("  a0=0x{:08X} (dst)", vm.cpu.x[10]);
                println!("  a1=0x{:08X} (src)", vm.cpu.x[11]);
                println!("  a2=0x{:08X} (n={})", vm.cpu.x[12], vm.cpu.x[12]);
            }
        }

        // Track backward copy loop entry
        if vm.cpu.pc == loop_pc && prev_pc != loop_pc && in_memmove {
            println!(
                "  backward loop at count={}: t3=0x{:08X} t4=0x{:08X} t5=0x{:08X} t6=0x{:08X}",
                count, vm.cpu.x[28], vm.cpu.x[29], vm.cpu.x[30], vm.cpu.x[31]
            );
        }

        // Track __memmove return
        if vm.cpu.pc == 0xC08E5DDA && in_memmove {
            in_memmove = false;
            if memmove_count <= 5 {
                println!("  __memmove returned at count={}", count);
            }
        }

        prev_pc = vm.cpu.pc;
    }
}
