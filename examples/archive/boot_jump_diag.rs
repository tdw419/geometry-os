use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_image = fs::read(kernel_path).expect("kernel");

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _entry, _dtb) =
        geometry_os::riscv::RiscvVm::boot_linux_setup(&kernel_image, None, 512, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;

    // Track last 20 PCs before we enter low addresses
    let mut pc_history: Vec<(u64, u32, geometry_os::riscv::cpu::Privilege)> = Vec::new();

    for i in 0..300_000 {
        let pc = vm.cpu.pc;
        let priv_level = vm.cpu.privilege;

        // Check for trap at fw_addr
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            println!(
                "[{}] M-TRAP: cause={} mepc={:#010x}",
                i, cause_code, vm.cpu.csr.mepc
            );
        }

        // Keep last 30 PCs
        pc_history.push((i, pc, priv_level));
        if pc_history.len() > 30 {
            pc_history.remove(0);
        }

        vm.step();

        // Detect jump to low address (< 0x100) after being in kernel space
        if vm.cpu.pc < 0x100 && pc >= 0x100 {
            println!("\n=== JUMP TO FIRMWARE at instruction {} ===", i);
            println!("From PC={:#010x} to PC={:#010x}", pc, vm.cpu.pc);
            println!("Previous 30 PCs:");
            for (count, p, pr) in &pc_history {
                let marker = if *p < 0x100 { " [LOW]" } else { "" };
                println!("  [{}] {:#010x} priv={:?}{}", count, p, pr, marker);
            }
            println!("\nRegister state at jump:");
            println!(
                "  RA={:#010x} SP={:#010x} GP={:#010x} TP={:#010x}",
                vm.cpu.x[1], vm.cpu.x[2], vm.cpu.x[3], vm.cpu.x[4]
            );
            println!(
                "  T0={:#010x} T1={:#010x} T2={:#010x}",
                vm.cpu.x[5], vm.cpu.x[6], vm.cpu.x[7]
            );
            println!(
                "  A0={:#010x} A1={:#010x} A2={:#010x} A3={:#010x}",
                vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[13]
            );
            println!(
                "  A4={:#010x} A5={:#010x} A6={:#010x} A7={:#010x}",
                vm.cpu.x[14], vm.cpu.x[15], vm.cpu.x[16], vm.cpu.x[17]
            );
            println!(
                "  satp={:#010x} stvec={:#010x}",
                vm.cpu.csr.satp, vm.cpu.csr.stvec
            );
            println!(
                "  sepc={:#010x} scause={:#010x} stval={:#010x}",
                vm.cpu.csr.sepc, vm.cpu.csr.scause, vm.cpu.csr.stval
            );
            println!(
                "  mepc={:#010x} mcause={:#010x}",
                vm.cpu.csr.mepc, vm.cpu.csr.mcause
            );

            // Read instruction at the jump-from address
            if let Ok(instr) = vm.bus.read_word(pc as u64) {
                println!("\n  Instruction at from-PC {:#010x}: {:#010x}", pc, instr);
            }

            break;
        }
    }
}
