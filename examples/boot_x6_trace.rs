/// Diagnostic: trace x6 through the S-mode trap handler to find where it gets zeroed.
/// The handler runs ~64 instructions after the load page fault, then C.JALR x1,x6,0
/// jumps to x6=0. This traces every write to x6 and the instruction that loads x6.
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{StepResult, Privilege};

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    // Phase 1: Run to ~16.99M instructions with trap forwarding
    let target = 16_999_000u64;
    let mut count: u64 = 0;
    let mut first_fault_count: Option<u64> = None;
    let mut handler_steps: u64 = 0;

    // Track x6 value
    let mut x6_history: Vec<(u64, u32, u32)> = Vec::new(); // (step, x6_new_value, PC)

    while count < target + 500_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Trap forwarding (same as boot_linux)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 11 {
                // ECALL_S = SBI call
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
            } else {
                let mpp = (vm.cpu.csr.mstatus & 0x300) >> 8;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;

                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                        vm.cpu.csr.mstatus &= !(1 << 1);

                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let pc_before = vm.cpu.pc;
        let x6_before = vm.cpu.x[6];
        let step_result = vm.step();

        match step_result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if vm.cpu.privilege == Privilege::Supervisor && first_fault_count.is_none() {
                    first_fault_count = Some(count);
                    eprintln!("[FAULT {}] type={:?} PC=0x{:08X} sepc=0x{:08X} scause=0x{:X} stval=0x{:08X}",
                        count, step_result, pc_before, vm.cpu.csr.sepc, vm.cpu.csr.scause, vm.cpu.csr.stval);
                    eprintln!("[FAULT] x6=0x{:08X} sscratch=0x{:08X} sp=0x{:08X}",
                        vm.cpu.x[6], vm.cpu.csr.sscratch, vm.cpu.x[2]);
                    eprintln!("[FAULT] stvec=0x{:08X} satp=0x{:08X}", vm.cpu.csr.stvec, vm.cpu.csr.satp);
                }
            }
            StepResult::Ebreak => { eprintln!("[EBREAK] count={}", count); break; }
            _ => {}
        }

        // Track x6 changes during handler execution
        if first_fault_count.is_some() {
            handler_steps += 1;
            let x6_after = vm.cpu.x[6];
            if x6_after != x6_before {
                x6_history.push((handler_steps, x6_after, pc_before));
            }

            // Print key handler instructions
            if handler_steps <= 100 {
                // Disassemble the instruction at pc_before for context
                if handler_steps <= 80 {
                    eprintln!("[H{:03}] PC=0x{:08X} x6=0x{:08X} x5=0x{:08X} sp=0x{:08X}",
                        handler_steps, pc_before, vm.cpu.x[6], vm.cpu.x[5], vm.cpu.x[2]);
                }
            }

            // Check for the crash: jump to x6=0
            if handler_steps > 60 && vm.cpu.pc == 0 && vm.cpu.privilege == Privilege::Supervisor {
                eprintln!("[CRASH] Jumped to 0x00000000 at handler step {}!", handler_steps);
                eprintln!("[CRASH] x6=0x{:08X} x1=0x{:08X}", vm.cpu.x[6], vm.cpu.x[1]);
                break;
            }

            // Safety limit
            if handler_steps > 200 {
                eprintln!("[LIMIT] Handler ran {} steps without crashing", handler_steps);
                break;
            }
        }

        count += 1;
    }

    // Print x6 write history
    eprintln!("\n--- x6 write history during handler ---");
    for (step, val, pc) in &x6_history {
        eprintln!("  H{:03}: PC=0x{:08X} x6 -> 0x{:08X}", step, pc, val);
    }

    // Print console output
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        let preview: String = s.chars().take(500).collect();
        eprintln!("\nConsole output:\n{}", preview);
    }

    eprintln!("\nTotal instructions: {}", count);
}
