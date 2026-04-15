fn main() {
    eprintln!("START");
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::StepResult;

    let (mut vm, _, _, _) = RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    eprintln!("SETUP DONE, starting loop to 16_992_700...");

    let target = 16_992_700;
    let mut last_print = 0u64;
    for i in 0..target {
        if vm.bus.sbi.shutdown_requested { break; }
        let _ = vm.step();
        if i - last_print > 5_000_000 {
            eprintln!("  progress: {}/{}", i, target);
            last_print = i;
        }
    }
    eprintln!("Reached {}, PC=0x{:08X}", target, vm.cpu.pc);
    
    let mut fault_count = 0u64;
    for count in target..(target + 2000) {
        if vm.bus.sbi.shutdown_requested { break; }
        let pc_before = vm.cpu.pc;
        let result = vm.step();
        
        match result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                fault_count += 1;
                eprintln!("[FAULT {}] {:?} at PC=0x{:08X} -> PC=0x{:08X} sepc=0x{:08X} stval=0x{:08X}",
                    fault_count, result, pc_before, vm.cpu.pc, vm.cpu.csr.sepc, vm.cpu.csr.stval);
                if fault_count == 1 {
                    for r in [0,1,2,5,6,10,11,28,29,30,31] {
                        eprintln!("  x{}=0x{:08X}", r, vm.cpu.x[r]);
                    }
                }
                if fault_count >= 5 { break; }
            }
            StepResult::Ebreak => { eprintln!("EBREAK at {}", count); break; }
            _ => {}
        }
    }
    eprintln!("DONE. faults={}", fault_count);
}
