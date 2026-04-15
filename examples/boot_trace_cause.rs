fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::{StepResult, Privilege};

    let (mut vm, _, _, _) = RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    // Run to just before the crash (~16,999,000) 
    // then trace carefully through the fault
    let target = 16_999_000;
    for count in 0..target {
        if vm.bus.sbi.shutdown_requested { break; }
        let result = vm.step();
        match result {
            StepResult::Ebreak => { println!("EBREAK at count={}", count); break; }
            _ => {}
        }
    }
    println!("Reached count={}, PC=0x{:08X}, priv={:?}", target, vm.cpu.pc, vm.cpu.privilege);

    // Now trace the next 1000 instructions with full register dumps
    for count in target..(target + 1000) {
        if vm.bus.sbi.shutdown_requested { break; }
        let pc_before = vm.cpu.pc;
        let result = vm.step();
        match result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                println!("[FAULT {}] type={:?} PC=0x{:08X} priv={:?}", 
                    count, result, pc_before, vm.cpu.privilege);
                if vm.cpu.privilege == Privilege::Supervisor {
                    println!("  scause=0x{:X} sepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X}",
                        vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.stvec);
                    println!("  satp=0x{:08X} mstatus=0x{:08X}", vm.cpu.csr.satp, vm.cpu.csr.mstatus);
                }
            }
            StepResult::Ebreak => { println!("[EBREAK] count={}", count); break; }
            _ => {}
        }
    }

    // Check what's at the faulting address 0xC08EFF1C
    println!("\n--- Fault handler page check ---");
    println!("PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    
    // Check SATP and try to translate 0xC08EFF1C manually
    let satp = vm.cpu.csr.satp;
    println!("SATP=0x{:08X}", satp);
    
    // Print UART console output
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        let preview: String = s.chars().take(500).collect();
        println!("\nConsole output:\n{}", preview);
    } else {
        println!("\nNo console output");
    }
}
