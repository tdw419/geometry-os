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

    let table_addr = 0xC1400AE8u64;
    
    // Run to 11.765M then watch for table change with register dump BEFORE step
    for i in 0..11_765_000u64 {
        if vm.bus.sbi.shutdown_requested { break; }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mc = vm.cpu.csr.mcause & !(1u32<<31);
            if mc == csr::CAUSE_ECALL_S {
                let r = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint);
                if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else if mc != csr::CAUSE_ECALL_M {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc; vm.cpu.csr.scause = vm.cpu.csr.mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP)) | (spp << csr::MSTATUS_SPP);
                        let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE)) | (sie << csr::MSTATUS_SPIE);
                        vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                        vm.cpu.pc = stvec; vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                    } else { vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4); }
                } else { vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4); }
            } else {
                let r = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint);
                if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
            continue;
        }
        let _ = vm.step();
    }
    
    // Now check: what is at the dst address (0xC1CCA520) in memory?
    // Is it actually being used for __memmove, or is the actual write going elsewhere?
    // The __memmove at 0xC08E5D6A uses registers for dst/src/len.
    // Let me snapshot the registers at the exact step where the table changes.
    
    let mut last_table_val = vm.bus.read_word(table_addr).unwrap_or(0);
    let mut steps = 0u64;
    
    loop {
        if vm.bus.sbi.shutdown_requested { break; }
        let pc_before = vm.cpu.pc;
        
        // Snapshot registers BEFORE the step
        let regs_snapshot = vm.cpu.x;
        let table_val_before = vm.bus.read_word(table_addr).unwrap_or(0);
        
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mc = vm.cpu.csr.mcause & !(1u32<<31);
            if mc == csr::CAUSE_ECALL_S {
                let r = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint);
                if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                continue;
            } else if mc != csr::CAUSE_ECALL_M {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc; vm.cpu.csr.scause = vm.cpu.csr.mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP)) | (spp << csr::MSTATUS_SPP);
                        let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE)) | (sie << csr::MSTATUS_SPIE);
                        vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                        vm.cpu.pc = stvec; vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                    } else { vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4); }
                } else { vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4); }
                continue;
            } else {
                let r = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint);
                if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                continue;
            }
        }

        let result = vm.step();
        
        let table_val_after = vm.bus.read_word(table_addr).unwrap_or(0);
        if table_val_after != table_val_before {
            eprintln!("[TABLE CHANGE at step {}] PC=0x{:08X} 0x{:08X}->0x{:08X}", 
                steps, pc_before, table_val_before, table_val_after);
            eprintln!("  Instruction: {:?}", result);
            if let Some(ref last) = vm.cpu.last_step {
                eprintln!("  inst=0x{:08X} op={:?}", last.word, last.op);
            }
            // Print ALL registers before the step
            eprintln!("  Registers BEFORE step:");
            for r in 0..32 {
                if regs_snapshot[r] != 0 {
                    eprintln!("    x{}=0x{:08X}", r, regs_snapshot[r]);
                }
            }
            // Check which register contains the table address
            eprintln!("\n  Registers containing table-related addresses:");
            for r in 0..32 {
                let v = regs_snapshot[r];
                // Check if register points near the table or near dst
                if (v as u64) >= 0xC1400A00 && (v as u64) <= 0xC1400C00 {
                    eprintln!("    x{}=0x{:08X} <-- NEAR TABLE 0xC1400AE8", r, v);
                }
                if (v as u64) >= 0xC1CCA400 && (v as u64) <= 0xC1CCA600 {
                    eprintln!("    x{}=0x{:08X} <-- NEAR DST 0xC1CCA520", r, v);
                }
            }
            
            // Also check: is the write going to the table address directly?
            // Check the physical address of the table via TLB
            let table_vpn = (0xC1400AE8u32 >> 12);
            let asid: u16 = ((vm.cpu.csr.satp >> 16) & 0xFFFF) as u16;
            if let Some((ppn, flags)) = vm.cpu.tlb.lookup(table_vpn, asid) {
                let pa = ((ppn as u64) << 12) | 0xAE8;
                eprintln!("\n  TLB: table 0xC1400AE8 -> PA 0x{:X} flags=0x{:X}", pa, flags);
                // Check what's at the physical address
                eprintln!("  Physical read at PA 0x{:X}: 0x{:08X}", pa, vm.bus.read_word(pa).unwrap_or(0));
            }
            
            break;
        }
        
        if matches!(result, StepResult::Ebreak) { break; }
        steps += 1;
        if steps > 10000 { break; }
    }
    eprintln!("DONE");
}
