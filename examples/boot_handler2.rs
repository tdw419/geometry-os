fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::StepResult;

    let (mut vm, _, _, _) = RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    // Run to 300 before the expected fault
    let target = 16_993_000 - 300;
    for count in 0..target {
        if vm.bus.sbi.shutdown_requested { break; }
        let _ = vm.step();
    }

    // Trace carefully, looking for first fault then handler trace
    let mut handler_entered = false;
    let mut handler_count = 0u64;
    let mut fault_count = 0u64;
    for count in target..(target + 1000) {
        if vm.bus.sbi.shutdown_requested { break; }
        let pc_before = vm.cpu.pc;
        let result = vm.step();
        
        match result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                fault_count += 1;
                eprintln!("[FAULT {}] {:?} at PC=0x{:08X} -> PC=0x{:08X} scause=0x{:X} sepc=0x{:08X} stval=0x{:08X}",
                    fault_count, result, pc_before, vm.cpu.pc, vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval);
                if fault_count == 1 {
                    // First fault - dump regs
                    eprintln!("  First fault regs:");
                    for r in [0,1,2,5,6,10,11,28,29,30,31] {
                        eprintln!("    x{}=0x{:08X}", r, vm.cpu.x[r]);
                    }
                    handler_entered = true;
                    handler_count = 0;
                }
                if fault_count >= 3 {
                    // Too many faults, stop
                    break;
                }
            }
            _ => {}
        }
        
        if handler_entered && !matches!(result, StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault) {
            handler_count += 1;
            if handler_count <= 3 {
                eprintln!("[H{}] count={} PC=0x{:08X} -> 0x{:08X}", handler_count, count, pc_before, vm.cpu.pc);
            }
            // Check for jump to zero
            if vm.cpu.pc == 0 || vm.cpu.pc < 0x1000 {
                eprintln!("[H{}] JUMP TO LOW ADDR! PC=0x{:08X} x1=0x{:X} x6=0x{:X} sp=0x{:X}",
                    handler_count, vm.cpu.pc, vm.cpu.x[1], vm.cpu.x[6], vm.cpu.x[2]);
                // Print regs
                for r in 0..32 {
                    if vm.cpu.x[r] != 0 {
                        eprintln!("  x{}=0x{:08X}", r, vm.cpu.x[r]);
                    }
                }
            }
            if handler_count > 100 {
                break;
            }
        }
    }
}
