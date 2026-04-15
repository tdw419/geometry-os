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
    let mut last_pc = 0u32;
    let mut same_count = 0u64;
    
    for i in 0..500_000 {
        let prev_pc = vm.cpu.pc;
        
        // Check for trap at fw_addr
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus >> 11) & 0x3;
            println!("[{}] TRAP at fw: cause={} mepc={:#010x} mpp={} sepc={:#010x}",
                i, cause_code, vm.cpu.csr.mepc, mpp, vm.cpu.csr.sepc);
        }
        
        vm.step();
        
        // Detect when executing in low address area (< 0x100)
        if vm.cpu.pc < 0x100 && vm.cpu.pc != last_pc {
            if same_count > 0 {
                println!("[{}] Left loop at {:#010x} after {} steps", i, last_pc, same_count);
            }
            println!("[{}] Low addr execution: PC={:#010x} prev_pc={:#010x}",
                i, vm.cpu.pc, prev_pc);
            last_pc = vm.cpu.pc;
            same_count = 0;
        } else if vm.cpu.pc == last_pc {
            same_count += 1;
            if same_count == 100 {
                println!("[{}] Stuck at {:#010x}", i, last_pc);
            }
            if same_count > 5000 {
                println!("[{}] Terminating: stuck at {:#010x} for {} steps", i, last_pc, same_count);
                break;
            }
        } else {
            if same_count > 100 {
                println!("[{}] Left loop at {:#010x} after {} steps", i, last_pc, same_count);
            }
            last_pc = vm.cpu.pc;
            same_count = 0;
        }
    }
    
    println!("\nFinal: PC={:#010x} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    println!("RA={:#010x} SP={:#010x}", vm.cpu.x[1], vm.cpu.x[2]);
}
