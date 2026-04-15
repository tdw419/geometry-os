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

    let memmove_entry = 0xC08E5C58u32;
    let table_addr = 0xC1400AE8u64;

    // Run to 11.7M then watch for __memmove calls and t4 value
    for i in 0..11_700_000u64 {
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

    // Watch for __memmove entry and t4 changes
    let mut count = 11_700_000u64;
    let mut memmove_calls = 0u64;
    let mut found_bad = false;

    while count < 11_770_000 && !found_bad {
        if vm.bus.sbi.shutdown_requested { break; }
        let pc_before = vm.cpu.pc;

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mc = vm.cpu.csr.mcause & !(1u32<<31);
            if mc == csr::CAUSE_ECALL_S {
                let r = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint);
                if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                count += 1; continue;
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

        let regs_before = vm.cpu.x;
        let result = vm.step();

        // Detect __memmove entry
        if pc_before == memmove_entry {
            memmove_calls += 1;
            let dst = vm.cpu.x[10]; // a0 - BEFORE prologue modifies t4
            let src = vm.cpu.x[11]; // a1
            let len = vm.cpu.x[12]; // a2
            // After this step, t4 = a0 + a2
            // Let's check t4 AFTER the step (the prologue executes add t4, a0, a2)
            let expected_t4 = dst.wrapping_add(len);
            let actual_t4 = vm.cpu.x[29]; // t4 = x29
            let near_table = (actual_t4 as u64) >= 0xC1400A00 && (actual_t4 as u64) <= 0xC1400C00;
            
            eprintln!("[MEMMOVE #{}] count={} dst=0x{:X} src=0x{:X} len=0x{:X} t4=0x{:X} (expected 0x{:X}){}",
                memmove_calls, count, dst, src, len, actual_t4, expected_t4,
                if near_table { " *** T4 NEAR TABLE!" } else { "" });
            
            if near_table {
                // This __memmove call has t4 pointing at the table!
                // Check if dst is near the table
                let dst_near_table = (dst as u64) >= 0xC1400A00 && (dst as u64) <= 0xC1400C00;
                eprintln!("  dst near table: {}", dst_near_table);
                eprintln!("  x1(ra)=0x{:X} x5(t0)=0x{:X}", vm.cpu.x[1], vm.cpu.x[5]);
                found_bad = true;
            }
        }
        
        if matches!(result, StepResult::Ebreak) { break; }
        count += 1;
    }
    
    if !found_bad {
        eprintln!("No bad __memmove calls found in {} calls, checking table...", memmove_calls);
        let val = vm.bus.read_word(table_addr).unwrap_or(0);
        eprintln!("Table at 0xC1400AE8 = 0x{:08X}", val);
    }
    eprintln!("DONE count={}", count);
}
