use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    
    let (mut vm, fw_addr, _entry, _dtb) = 
        geometry_os::riscv::RiscvVm::boot_linux_setup(
            &kernel_image, None, 512, bootargs,
        ).unwrap();
    
    let fw_addr_u32 = fw_addr as u32;
    let mut illegal_count = 0;
    let mut last_illegal_pc = 0u32;
    
    for i in 0..1_000_000 {
        // Check for M-mode trap at fw_addr
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            
            if cause_code == 2 {
                // Illegal instruction
                illegal_count += 1;
                let mepc = vm.cpu.csr.mepc;
                
                if illegal_count <= 5 || mepc != last_illegal_pc {
                    // Read the instruction at mepc
                    let instr = vm.bus.read_word(mepc as u64).unwrap_or(0);
                    println!("[{}] ILLEGAL at PC={:#010x} mepc={:#010x} instr={:#010x} (count={})", 
                        i, vm.cpu.pc, mepc, instr, illegal_count);
                    
                    // Try to decode
                    let opcode = instr & 0x7F;
                    let rd = (instr >> 7) & 0x1F;
                    let funct3 = (instr >> 12) & 0x7;
                    let rs1 = (instr >> 15) & 0x1F;
                    let funct7 = (instr >> 25) & 0x7F;
                    println!("    opcode={:#04x} rd={} funct3={} rs1={} funct7={:#04x}", 
                        opcode, rd, funct3, rs1, funct7);
                    
                    // Check if it's a SYSTEM instruction (CSR access)
                    if opcode == 0x73 {
                        let csr = (instr >> 20) & 0xFFF;
                        println!("    CSR access: csr={:#05x}", csr);
                    }
                    
                    // Show register context
                    println!("    x1(ra)={:#010x} x2(sp)={:#010x} x10(a0)={:#010x} x11(a1)={:#010x}",
                        vm.cpu.x[1], vm.cpu.x[2], vm.cpu.x[10], vm.cpu.x[11]);
                    println!("    satp={:#010x} sepc={:#010x} scause={:#010x}",
                        vm.cpu.csr.satp, vm.cpu.csr.sepc, vm.cpu.csr.scause);
                }
                last_illegal_pc = mepc;
            }
        }
        
        vm.step();
        
        if illegal_count >= 10 {
            break;
        }
    }
    
    println!("\nTotal illegal instructions: {}", illegal_count);
    println!("Final PC: {:#010x}", vm.cpu.pc);
}
