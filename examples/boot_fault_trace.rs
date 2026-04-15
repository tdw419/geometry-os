use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    
    let (mut vm, _fw_addr, _entry, _dtb) = 
        geometry_os::riscv::RiscvVm::boot_linux_setup(
            &kernel_image, None, 128, bootargs,
        ).unwrap();
    
    // Run to the first load fault
    for i in 0..178_000 {
        let prev_pc = vm.cpu.pc;
        let result = vm.step();
        
        match result {
            geometry_os::riscv::cpu::StepResult::LoadFault => {
                println!("[{}] LOAD_FAULT: prev_pc=0x{:08X} -> PC=0x{:08X} sepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X} x15=0x{:08X}",
                    i, prev_pc, vm.cpu.pc, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.stvec, vm.cpu.x[15]);
                
                // Trace the next 200 instructions to see what the trap handler does
                for j in 0..200 {
                    let p2 = vm.cpu.pc;
                    let r2 = vm.step();
                    if matches!(r2, geometry_os::riscv::cpu::StepResult::LoadFault 
                                | geometry_os::riscv::cpu::StepResult::FetchFault
                                | geometry_os::riscv::cpu::StepResult::StoreFault) {
                        println!("  [{}] {} at PC=0x{:08X} sepc=0x{:08X} stval=0x{:08X}",
                            i+j, match r2 {
                                geometry_os::riscv::cpu::StepResult::FetchFault => "FETCH_FAULT",
                                geometry_os::riscv::cpu::StepResult::LoadFault => "LOAD_FAULT",
                                geometry_os::riscv::cpu::StepResult::StoreFault => "STORE_FAULT",
                                _ => "?",
                            }, vm.cpu.pc, vm.cpu.csr.sepc, vm.cpu.csr.stval);
                    }
                    if p2 == vm.cpu.pc && j > 0 {
                        // PC didn't advance - possible spin
                        println!("  [{}] SPIN at PC=0x{:08X} (same as prev)", i+j, vm.cpu.pc);
                        break;
                    }
                }
                
                // Check if SRET was executed (PC jumped back to user code)
                println!("  After 200 steps: PC=0x{:08X} sepc=0x{:08X} sstatus=0x{:08X}", 
                    vm.cpu.pc, vm.cpu.csr.sepc, vm.cpu.csr.mstatus);
                break;
            }
            _ => {}
        }
    }
}
