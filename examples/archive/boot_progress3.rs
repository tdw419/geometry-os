use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_image = fs::read(kernel_path).expect("kernel");

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, _fw_addr, _entry, _dtb) =
        geometry_os::riscv::RiscvVm::boot_linux_setup(&kernel_image, None, 128, bootargs).unwrap();

    // Run and track unique PCs, last 100 register states, SBI calls
    let max = 5_000_000;
    let mut last_ecall_pc = 0u32;
    let mut ecall_count = 0;
    let mut pc_history: Vec<u32> = Vec::new();
    let mut last_unique_pc = 0u32;
    let mut same_pc_count = 0u64;

    for i in 0..max {
        let prev_pc = vm.cpu.pc;
        let result = vm.step();

        // Track SBI ECALLs
        if vm.cpu.pc as u64 == _fw_addr
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                ecall_count += 1;
                let a7 = vm.cpu.x[17];
                let a6 = vm.cpu.x[16];
                let a0 = vm.cpu.x[10];
                println!(
                    "[{}] ECALL_M: a7={:#x} a6={:#x} a0={:#x} (from PC={:#x})",
                    i, a7, a6, a0, prev_pc
                );
                last_ecall_pc = prev_pc;
            }
        }

        // Detect loops
        if vm.cpu.pc == last_unique_pc {
            same_pc_count += 1;
        } else {
            if same_pc_count > 100 {
                println!(
                    "[{}] Stuck at PC={:#x} for {} steps, prev_pc={:#x}",
                    i, last_unique_pc, same_pc_count, prev_pc
                );
            }
            last_unique_pc = vm.cpu.pc;
            same_pc_count = 0;
        }

        // Track final PCs
        if i > max - 200 {
            pc_history.push(vm.cpu.pc);
        }

        match result {
            geometry_os::riscv::cpu::StepResult::Ebreak => {
                println!("[{}] EBREAK at PC={:#x}", i, vm.cpu.pc);
                break;
            }
            _ => {}
        }

        // Check for stuck in exception handler
        if same_pc_count > 10000 {
            println!("[{}] Terminated: stuck at PC={:#x}", i, vm.cpu.pc);
            println!(
                "  mcause={:#x} mepc={:#x} privilege={:?}",
                vm.cpu.csr.mcause, vm.cpu.csr.mepc, vm.cpu.privilege
            );
            println!(
                "  sepc={:#x} scause={:#x} stval={:#x}",
                vm.cpu.csr.sepc, vm.cpu.csr.scause, vm.cpu.csr.stval
            );
            break;
        }
    }

    println!("\n=== Summary ===");
    println!("Instructions: {}", max);
    println!("SBI ECALLs: {}", ecall_count);
    println!("Final PC: {:#x}", vm.cpu.pc);
    println!("Privilege: {:?}", vm.cpu.privilege);
    println!("satp: {:#x}", vm.cpu.csr.satp);
    println!(
        "mcause: {:#x}, mepc: {:#x}",
        vm.cpu.csr.mcause, vm.cpu.csr.mepc
    );
    println!(
        "scause: {:#x}, sepc: {:#x}, stval: {:#x}",
        vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval
    );
    println!("SBI console: {} bytes", vm.bus.sbi.console_output.len());

    // Print last unique PCs from history
    let unique_pcs: Vec<u32> = pc_history.iter().copied().fold(Vec::new(), |mut acc, pc| {
        if acc.last() != Some(&pc) {
            acc.push(pc);
        }
        acc
    });
    println!("\nLast {} unique PCs:", unique_pcs.len().min(20));
    for pc in unique_pcs.iter().rev().take(20) {
        println!("  {:#x}", pc);
    }
}
