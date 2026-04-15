fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::Privilege;
    use geometry_os::riscv::csr;

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instr = 500_000u64;

    // Track SBI calls and UART output
    let mut sbi_count = 0u64;
    let mut ecall_m_count = 0u64;
    let mut forward_count = 0u64;
    let mut first_fault = true;
    let mut satp_after_setup = 0u32;
    let mut setup_done = false;

    while count < max_instr {
        if vm.bus.sbi.shutdown_requested { break; }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
            if cause_code == csr::CAUSE_ECALL_S || cause_code == csr::CAUSE_ECALL_M {
                sbi_count += 1;
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            } else if mpp != 3 {
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
                    // NOTE: NOT flushing TLB here (was the bug)
                    forward_count += 1;
                    count += 1; continue;
                }
            }
            if cause_code == csr::CAUSE_ECALL_M {
                ecall_m_count += 1;
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let step_result = vm.step();

        // Log first S-mode fault
        if first_fault && matches!(step_result, geometry_os::riscv::cpu::StepResult::FetchFault
            | geometry_os::riscv::cpu::StepResult::LoadFault
            | geometry_os::riscv::cpu::StepResult::StoreFault)
            && vm.cpu.privilege == Privilege::Supervisor {
            first_fault = false;
            let fault_type = match step_result {
                geometry_os::riscv::cpu::StepResult::FetchFault => "fetch",
                geometry_os::riscv::cpu::StepResult::LoadFault => "load",
                geometry_os::riscv::cpu::StepResult::StoreFault => "store",
                _ => unreachable!(),
            };
            eprintln!("[FIRST_FAULT] {} at count={}: PC=0x{:08X} scause=0x{:08X} sepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X}",
                fault_type, count, vm.cpu.pc, vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.stvec);
            eprintln!("  SATP=0x{:08X} SSTATUS=0x{:08X} mstatus=0x{:08X}", vm.cpu.csr.satp, vm.cpu.csr.mstatus, vm.cpu.csr.mstatus);

            // Dump the page table root
            let satp_val = vm.cpu.csr.satp;
            let root_ppn = (satp_val & 0x003FFFFF) as u64;
            let root_addr = root_ppn << 12;
            eprintln!("  Root PT at phys 0x{:08X}", root_addr);

            // Read first 16 L1 entries
            eprintln!("  L1 entries [0..16]:");
            for i in 0..16 {
                let pte_addr = root_addr + (i as u64) * 4;
                let pte = vm.bus.read_word(pte_addr).unwrap_or(0);
                if pte != 0 {
                    let v = pte & 1;
                    let r = (pte >> 1) & 1;
                    let w = (pte >> 2) & 1;
                    let x = (pte >> 3) & 1;
                    let ppn = (pte >> 10) & 0x3FFFFF;
                    eprintln!("    L1[{}] at 0x{:08X} = 0x{:08X} V={} R={} W={} X={} PPN=0x{:06X}",
                        i, pte_addr, pte, v, r, w, x, ppn);
                }
            }

            // Check L1[770] specifically
            let l1_770_addr = root_addr + 770 * 4;
            let l1_770 = vm.bus.read_word(l1_770_addr).unwrap_or(0);
            eprintln!("  L1[770] at 0x{:08X} = 0x{:08X} V={}", l1_770_addr, l1_770, l1_770 & 1);
        }

        // Check if SATP changes (indicates setup_vm completed)
        if !setup_done && vm.cpu.csr.satp != 0 {
            satp_after_setup = vm.cpu.csr.satp;
            setup_done = true;
            eprintln!("[SETUP_VM] SATP changed to 0x{:08X} at count={}", vm.cpu.csr.satp, count);

            // Dump first 16 L1 entries after setup
            let root_ppn = (vm.cpu.csr.satp & 0x003FFFFF) as u64;
            let root_addr = root_ppn << 12;
            eprintln!("  Root PT at phys 0x{:08X}", root_addr);
            for i in 0..16 {
                let pte_addr = root_addr + (i as u64) * 4;
                let pte = vm.bus.read_word(pte_addr).unwrap_or(0);
                if pte != 0 {
                    let v = pte & 1;
                    let ppn = (pte >> 10) & 0x3FFFFF;
                    eprintln!("    L1[{}] at 0x{:08X} = 0x{:08X} V={} PPN=0x{:06X}",
                        i, pte_addr, pte, v, ppn);
                }
            }
        }

        count += 1;
    }

    eprintln!("[boot] Done at count={}: SBI={} ECALL_M={} forwards={}", count, sbi_count, ecall_m_count, forward_count);
    eprintln!("  PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("  SATP=0x{:08X} SBI_chars={}", vm.cpu.csr.satp, vm.bus.sbi.console_output.len());
    eprintln!("  UART_tx={}", vm.bus.uart.tx_buf.len());
}
