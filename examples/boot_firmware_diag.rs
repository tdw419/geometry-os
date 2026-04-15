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
    
    // Track all M-mode traps
    let mut trap_log: Vec<(u64, u32, u32, u32, u32)> = Vec::new(); // (count, cause, mepc, pc, mpp)
    let mut low_addr_entries: Vec<(u64, u32, u32)> = Vec::new(); // (count, pc, ra)
    let mut ecall_count = 0u64;
    
    for i in 0..300_000 {
        let prev_pc = vm.cpu.pc;
        let prev_priv = vm.cpu.privilege;
        
        // Check for trap at fw_addr
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus >> 11) & 0x3;
            trap_log.push((i, cause_code, vm.cpu.csr.mepc, vm.cpu.pc, mpp));
            
            if cause_code == 9 || cause_code == 11 {
                ecall_count += 1;
                println!("[{}] ECALL: cause={} a7={:#x} a6={:#x} mepc={:#010x} mpp={}",
                    i, cause_code, vm.cpu.x[17], vm.cpu.x[16], vm.cpu.csr.mepc, mpp);
            }
        }
        
        vm.step();
        
        // Track low address execution
        if vm.cpu.pc < 0x100 && vm.cpu.pc != prev_pc {
            if low_addr_entries.len() < 30 {
                low_addr_entries.push((i, vm.cpu.pc, vm.cpu.x[1]));
            }
        }
        
        // Stop if we've been in the low address loop for too long
        if vm.cpu.pc < 0x100 && i > 180_000 && i > 177_000 {
            if vm.cpu.pc == 0x00 {
                // Check how many times we've been to 0x00
                let zero_visits = low_addr_entries.iter().filter(|(_, pc, _)| *pc == 0x00).count();
                if zero_visits > 10 {
                    println!("[{}] Firmware loop detected, breaking", i);
                    break;
                }
            }
        }
    }
    
    println!("\n=== Summary ===");
    println!("Total M-mode traps: {}", trap_log.len());
    println!("ECALL count: {}", ecall_count);
    
    // Unique trap causes
    let mut cause_counts: std::collections::HashMap<u32, u64> = std::collections::HashMap::new();
    for (_, cause, _, _, _) in &trap_log {
        *cause_counts.entry(*cause).or_insert(0) += 1;
    }
    println!("\nTrap causes:");
    for (cause, count) in &cause_counts {
        println!("  cause {}: {} times", cause, count);
    }
    
    println!("\nLow address entries (first 30):");
    for (count, pc, ra) in &low_addr_entries {
        println!("  [{}] PC={:#010x} RA={:#010x}", count, pc, ra);
    }
    
    println!("\nFinal: PC={:#010x} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    println!("RA={:#010x} SP={:#010x}", vm.cpu.x[1], vm.cpu.x[2]);
    println!("satp={:#010x} stvec={:#010x}", vm.cpu.csr.satp, vm.cpu.csr.stvec);
    println!("sepc={:#010x} scause={:#010x}", vm.cpu.csr.sepc, vm.cpu.csr.scause);
    
    // Show what instructions are at the low addresses
    println!("\nFirmware region contents:");
    for addr in [0u64, 2, 4, 6, 8, 0xc, 0x10, 0x12, 0x14, 0x16, 0x18, 0x1a] {
        if let Ok(word) = vm.bus.read_word(addr) {
            println!("  {:#06x}: {:#010x} ({})", addr, word, 
                if word == 0 { "zero" } else { "" });
        }
    }
}
