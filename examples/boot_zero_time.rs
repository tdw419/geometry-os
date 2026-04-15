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

    // Binary search: find when table at 0xC1400AE8 goes from nonzero to zero
    let table_addr = 0xC1400AE8u64;
    let max_instructions = 17_200_000u64;
    let mut count = 0u64;
    
    // First, find a range where it's still nonzero
    // We know it's nonzero at 10M and zero at 15M
    let mut lo = 10_000_000u64;
    let mut hi = 15_000_000u64;
    
    // We need to run to each checkpoint efficiently
    // Use a two-phase approach: run to lo, check, then binary search
    
    // Phase 1: run to 10M and verify nonzero
    eprintln!("Phase 1: running to 10M...");
    while count < lo {
        if vm.bus.sbi.shutdown_requested { break; }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == csr::CAUSE_ECALL_S {
                let r = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint);
                if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
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
            let r = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                &mut vm.bus.uart, &mut vm.bus.clint);
            if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            count += 1; continue;
        }
        let result = vm.step();
        if matches!(result, StepResult::Ebreak) { break; }
        count += 1;
    }
    
    let val = vm.bus.read_word(table_addr).unwrap_or(0);
    eprintln!("At {}: table[0xC1400AE8] = 0x{:08X}", count, val);
    
    // Binary search between 10M and 15M
    // This is too slow for step-by-step. Instead, let's just scan with wider steps
    // Check every 100K steps from 10M to 15M
    eprintln!("Scanning 10M-15M in 100K steps...");
    let mut prev_nonzero = val != 0;
    while count < 15_000_000 {
        if vm.bus.sbi.shutdown_requested { break; }
        
        let target = count + 100_000;
        for _ in count..target.min(max_instructions) {
            if vm.bus.sbi.shutdown_requested { break; }
            if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
                // Simplified: just skip
                let mcause = vm.cpu.csr.mcause;
                let cause_code = mcause & !(1u32 << 31);
                if cause_code == csr::CAUSE_ECALL_S {
                    let r = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                        vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                        &mut vm.bus.uart, &mut vm.bus.clint);
                    if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
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
                let r = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint);
                if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                count += 1; continue;
            }
            let result = vm.step();
            if matches!(result, StepResult::Ebreak) { break; }
            count += 1;
        }
        
        let val = vm.bus.read_word(table_addr).unwrap_or(0);
        let is_nonzero = val != 0;
        if is_nonzero != prev_nonzero {
            eprintln!("CHANGE at count={}: 0x{:08X} (was {})", count, val, if prev_nonzero { "nonzero" } else { "zero" });
            prev_nonzero = is_nonzero;
        }
    }
    eprintln!("DONE count={}", count);
}
