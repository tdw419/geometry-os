fn main() {
    // Count ECALL instructions during boot
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::{StepResult, Privilege};
    use geometry_os::riscv::decode::Operation;

    // Boot to 17M
    let (mut vm, _) = RiscvVm::boot_linux(&kernel_image, initramfs.as_deref(), 256, 17_000_000, bootargs).unwrap();

    // Continue stepping and count ECALLs
    let mut ecall_count: u64 = 0;
    let mut ecall_sbi_count: u64 = 0;
    let mut ecall_details: Vec<(u64, u32, u32, u32)> = Vec::new(); // (count, pc, a7, a0)

    for count in 17_000_000..17_200_000 {
        let step_result = vm.step();

        if let Some(ref last) = vm.cpu.last_step {
            if matches!(last.op, Operation::Ecall) {
                ecall_count += 1;
                let a7 = vm.cpu.x[17];
                let a0 = vm.cpu.x[10];
                ecall_details.push((count, last.pc, a7, a0));
                if ecall_details.len() <= 10 {
                    println!("[{}] ECALL at PC=0x{:08X} priv={:?} a7=0x{:X} a0=0x{:X}",
                        count, last.pc, vm.cpu.privilege, a7, a0);
                }
                // Check if SBI handled it
                if !vm.bus.sbi.console_output.is_empty() {
                    ecall_sbi_count += 1;
                }
            }
        }

        match step_result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                println!("\n[FAULT] count={} PC=0x{:08X}", count, vm.cpu.pc);
                break;
            }
            StepResult::Ebreak => break,
            _ => {}
        }
    }

    println!("\nTotal ECALLs: {}", ecall_count);
    println!("SBI console output: {} bytes", vm.bus.sbi.console_output.len());
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        let preview: String = s.chars().take(200).collect();
        println!("Console: {}", preview);
    }
}
