fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::{StepResult, Privilege};
    use geometry_os::riscv::csr;

    let (mut vm, fw_addr, _, _) = 
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let max_instructions = 17_200_000u64;
    let mut count = 0u64;
    let mut first_fault_at: Option<u64> = None;
    let mut handler_step = 0u64;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested { break; }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == csr::CAUSE_ECALL_S {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                count += 1; continue;
            } else if cause_code != csr::CAUSE_ECALL_M {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP)) | (spp << csr::MSTATUS_SPP);
                        let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE)) | (sie << csr::MSTATUS_SPIE);
                        vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1; continue;
                    }
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                count += 1; continue;
            }
            let result = vm.bus.sbi.handle_ecall(
                vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                &mut vm.bus.uart, &mut vm.bus.clint,
            );
            if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            count += 1; continue;
        }

        let pc_before = vm.cpu.pc;
        let regs_before = vm.cpu.x;
        let step_result = vm.step();
        
        if first_fault_at.is_none() && matches!(step_result, StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault) {
            first_fault_at = Some(count);
            handler_step = 0;
            eprintln!("[FAULT] count={} {:?} PC=0x{:08X} -> 0x{:08X} sepc=0x{:08X}", 
                count, step_result, pc_before, vm.cpu.pc, vm.cpu.csr.sepc);
        }
        
        if first_fault_at.is_some() {
            handler_step += 1;
            // Log ALL handler steps from H55 onward
            if handler_step >= 55 {
                let diff_regs: Vec<String> = (0..32).filter(|&r| vm.cpu.x[r] != regs_before[r])
                    .map(|r| format!("x{}={:X}->0x{:X}", r, regs_before[r], vm.cpu.x[r])).collect();
                eprintln!("[H{}] 0x{:08X} -> 0x{:08X} changed: {}",
                    handler_step, pc_before, vm.cpu.pc, if diff_regs.is_empty() { "none".to_string() } else { diff_regs.join(", ") });
            }
            if vm.cpu.pc < 0x100 && vm.cpu.pc != pc_before {
                eprintln!("\n[H{}] JUMP TO 0x{:08X} from 0x{:08X}", handler_step, vm.cpu.pc, pc_before);
                // Disassemble the instruction at pc_before
                if let Some(ref last) = vm.cpu.last_step {
                    eprintln!("  instruction word: 0x{:08X} op={:?}", last.word, last.op);
                }
                break;
            }
            if handler_step > 100 { break; }
        }
        count += 1;
    }
}
