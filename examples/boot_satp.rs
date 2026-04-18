fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::cpu::StepResult;
    use geometry_os::riscv::RiscvVm;

    let (mut vm, _, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let target = 16_999_000;
    let mut last_satp = vm.cpu.csr.satp;
    for count in 0..target {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let satp_before = vm.cpu.csr.satp;
        let result = vm.step();
        if vm.cpu.csr.satp != satp_before {
            println!(
                "[SATP CHANGE] count={} PC=0x{:08X} old=0x{:08X} new=0x{:08X}",
                count, vm.cpu.pc, satp_before, vm.cpu.csr.satp
            );
            last_satp = vm.cpu.csr.satp;
        }
        match result {
            StepResult::Ebreak => {
                println!("EBREAK at count={}", count);
                break;
            }
            _ => {}
        }
    }
    println!("\nReached count={}, PC=0x{:08X}", target, vm.cpu.pc);
    println!("SATP=0x{:08X}", vm.cpu.csr.satp);
    println!("TLB entries: {}", vm.cpu.tlb.valid_count());
    println!(
        "scause=0x{:X} sepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X}",
        vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.stvec
    );

    // Now trace 100 instructions from here
    for count in target..(target + 200) {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let pc_before = vm.cpu.pc;
        let satp_before = vm.cpu.csr.satp;
        let result = vm.step();

        if vm.cpu.csr.satp != satp_before {
            println!(
                "[SATP] count={} PC=0x{:08X} 0x{:08X}->0x{:08X}",
                count, pc_before, satp_before, vm.cpu.csr.satp
            );
        }

        if pc_before == 0xC08EFF1C {
            println!(
                "[HANDLER] count={} PC=0x{:08X} result={:?}",
                count, pc_before, result
            );
            println!(
                "  scause=0x{:X} sepc=0x{:08X} stval=0x{:08X}",
                vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval
            );
        }

        match result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                println!(
                    "[FAULT {}] {:?} at PC=0x{:08X} scause=0x{:X} sepc=0x{:08X} stval=0x{:08X}",
                    count, result, pc_before, vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval
                );
            }
            StepResult::Ebreak => {
                println!("[EBREAK] count={}", count);
                break;
            }
            _ => {}
        }
    }
}
