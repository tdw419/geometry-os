use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    
    let (mut vm, _fw_addr, _entry, _dtb) = 
        geometry_os::riscv::RiscvVm::boot_linux_setup(
            &kernel_image, None, 128, bootargs,
        ).unwrap();
    
    let mut last_satp = 0u32;
    let mut satp_changes = 0;
    
    // Run up to 300K instructions, watch for satp changes and faults
    for i in 0..300_000 {
        let prev_satp = vm.cpu.csr.satp;
        let prev_pc = vm.cpu.pc;
        
        let result = vm.step();
        
        // Detect satp change
        if vm.cpu.csr.satp != prev_satp && vm.cpu.csr.satp != last_satp {
            last_satp = vm.cpu.csr.satp;
            satp_changes += 1;
            println!("[{}] SATP changed: 0x{:08X} (priv={:?}, PC=0x{:08X})", 
                i, vm.cpu.csr.satp, vm.cpu.privilege, vm.cpu.pc);
        }
        
        match result {
            geometry_os::riscv::cpu::StepResult::Ok => {}
            geometry_os::riscv::cpu::StepResult::Ecall => {}
            geometry_os::riscv::cpu::StepResult::Ebreak => {
                println!("[{}] EBREAK at PC=0x{:08X}", i, vm.cpu.pc);
                break;
            }
            fault => {
                let fault_name = match fault {
                    geometry_os::riscv::cpu::StepResult::FetchFault => "FETCH_FAULT",
                    geometry_os::riscv::cpu::StepResult::LoadFault => "LOAD_FAULT",
                    geometry_os::riscv::cpu::StepResult::StoreFault => "STORE_FAULT",
                    _ => "?",
                };
                println!("[{}] {} at PC=0x{:08X} (prev_pc=0x{:08X})", 
                    i, fault_name, vm.cpu.pc, prev_pc);
                println!("    satp=0x{:08X} priv={:?}", vm.cpu.csr.satp, vm.cpu.privilege);
                println!("    sepc=0x{:08X} scause=0x{:08X} stval=0x{:08X} stvec=0x{:08X}",
                    vm.cpu.csr.sepc, vm.cpu.csr.scause, vm.cpu.csr.stval, vm.cpu.csr.stvec);
                
                // Decode the faulting instruction
                if let Ok(word) = vm.bus.read_word(vm.cpu.pc as u64) {
                    println!("    instr at PC: 0x{:08X}", word);
                }
                
                if i > 200_000 || satp_changes > 3 {
                    break;
                }
            }
        }
    }
    
    println!("\nFinal: PC=0x{:08X} satp=0x{:08X} satp_changes={}", 
        vm.cpu.pc, vm.cpu.csr.satp, satp_changes);
}
