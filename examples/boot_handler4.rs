fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::{StepResult, Privilege};
    use geometry_os::riscv::csr;

    let (mut vm, fw_addr, _entry, _dtb_addr) = 
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    eprintln!("Starting boot, fw_addr=0x{:X}", fw_addr);
    let max_instructions = 17_200_000u64;
    let mut count = 0u64;
    let mut first_fault_at: Option<u64> = None;
    let mut handler_step = 0u64;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested { break; }

        // Intercept M-mode traps at firmware handler
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
                count += 1;
                continue;
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
                        count += 1;
                        continue;
                    }
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                count += 1;
                continue;
            }
            // ECALL_M = SBI call
            let result = vm.bus.sbi.handle_ecall(
                vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                &mut vm.bus.uart, &mut vm.bus.clint,
            );
            if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            count += 1;
            continue;
        }

        let pc_before = vm.cpu.pc;
        let step_result = vm.step();
        
        match step_result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if first_fault_at.is_none() {
                    first_fault_at = Some(count);
                    handler_step = 0;
                    eprintln!("\n[FIRST FAULT] count={} {:?} at PC=0x{:08X} -> PC=0x{:08X}", 
                        count, step_result, pc_before, vm.cpu.pc);
                    eprintln!("  sepc=0x{:08X} scause=0x{:X} stval=0x{:08X} stvec=0x{:08X}",
                        vm.cpu.csr.sepc, vm.cpu.csr.scause, vm.cpu.csr.stval, vm.cpu.csr.stvec);
                    for r in [0,1,2,5,6,10,11,28,29,30,31] {
                        eprintln!("  x{}=0x{:08X}", r, vm.cpu.x[r]);
                    }
                } else {
                    eprintln!("[FAULT {}] count={} {:?} at PC=0x{:08X} -> PC=0x{:08X} sepc=0x{:08X}",
                        handler_step, count, step_result, pc_before, vm.cpu.pc, vm.cpu.csr.sepc);
                    break;
                }
            }
            StepResult::Ebreak => { eprintln!("[EBREAK]"); break; }
            _ => {}
        }

        if first_fault_at.is_some() {
            handler_step += 1;
            if handler_step <= 5 || handler_step % 10 == 0 {
                eprintln!("[H{}] count={} 0x{:08X} -> 0x{:08X}", handler_step, count, pc_before, vm.cpu.pc);
            }
            if vm.cpu.pc < 0x100 && vm.cpu.pc != pc_before {
                eprintln!("[H{}] LOW JUMP! 0x{:08X}", handler_step, vm.cpu.pc);
                for r in 0..32 {
                    if vm.cpu.x[r] != 0 { eprintln!("  x{}=0x{:08X}", r, vm.cpu.x[r]); }
                }
                break;
            }
            if handler_step > 150 { break; }
        }
        count += 1;
    }
    eprintln!("\nDONE at count={}", count);
    eprintln!("Console: {} bytes", vm.bus.sbi.console_output.len());
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        eprintln!("Console: {}", &s[..s.len().min(200)]);
    }
}
