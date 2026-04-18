use geometry_os::riscv::{cpu, csr, mmu, RiscvVm};

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    )
    .unwrap();

    // Use the boot_linux loop logic but with tracing
    let fw_addr_u32 = _fw_addr as u32;
    let mut count: u64 = 0;
    let max: u64 = 180_000;
    let trace_from: u64 = 177_300; // Just before first SATP switch

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // SATP change hook (same as boot_linux)
        let cur_satp = vm.cpu.csr.satp;
        // (skipping SATP hook for trace - it's done in boot_linux_setup)

        // M-mode trap handler forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;

            if cause_code != csr::CAUSE_ECALL_M {
                if cause_code == csr::CAUSE_ECALL_S {
                    // SBI call
                    let result = vm.bus.sbi.handle_ecall(
                        vm.cpu.x[17],
                        vm.cpu.x[16],
                        vm.cpu.x[10],
                        vm.cpu.x[11],
                        vm.cpu.x[12],
                        vm.cpu.x[13],
                        vm.cpu.x[14],
                        vm.cpu.x[15],
                        &mut vm.bus.uart,
                        &mut vm.bus.clint,
                    );
                    if let Some((a0, a1)) = result {
                        vm.cpu.x[10] = a0;
                        vm.cpu.x[11] = a1;
                    }
                } else if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP))
                            | (spp << csr::MSTATUS_SPP);
                        let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE))
                            | (sie << csr::MSTATUS_SPIE);
                        vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        if count >= trace_from {
                            eprintln!(
                                "[{}] FORWARD to stvec=0x{:08X} scause=0x{:08X} sepc=0x{:08X}",
                                count, stvec, mcause, vm.cpu.csr.mepc
                            );
                        }
                        count += 1;
                        continue;
                    }
                }
            }

            if cause_code == csr::CAUSE_ECALL_M {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17],
                    vm.cpu.x[16],
                    vm.cpu.x[10],
                    vm.cpu.x[11],
                    vm.cpu.x[12],
                    vm.cpu.x[13],
                    vm.cpu.x[14],
                    vm.cpu.x[15],
                    &mut vm.bus.uart,
                    &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
            }

            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        // Trace
        if count >= trace_from {
            let pc = vm.cpu.pc;
            let priv_name = match vm.cpu.privilege {
                cpu::Privilege::Machine => "M",
                cpu::Privilege::Supervisor => "S",
                cpu::Privilege::User => "U",
                _ => "?",
            };
            // Try to get instruction from physical address
            let satp = vm.cpu.csr.satp;
            let sum = (vm.cpu.csr.mstatus >> 19) & 1 != 0;
            let mxr = (vm.cpu.csr.mstatus >> 19) & 1 != 0;
            let inst_str = match mmu::translate(
                pc,
                mmu::AccessType::Fetch,
                vm.cpu.privilege,
                sum,
                mxr,
                satp,
                &mut vm.bus,
                &mut vm.cpu.tlb,
            ) {
                mmu::TranslateResult::Ok(pa) => {
                    match vm.bus.read_half(pa) {
                        Ok(hw) => {
                            if hw & 0x3 != 0x3 {
                                format!("0x{:04X}", hw) // compressed
                            } else {
                                match vm.bus.read_word(pa) {
                                    Ok(w) => format!("0x{:08X}", w),
                                    Err(_) => "???".into(),
                                }
                            }
                        }
                        Err(_) => "???".into(),
                    }
                }
                _ => "FAULT".into(),
            };

            // Log key state changes
            let ra = vm.cpu.x[1];
            let sp = vm.cpu.x[2];
            let satp_val = vm.cpu.csr.satp;

            if count <= 177_350 || count % 100 == 0 || ra >= 0x3FFF0000 {
                eprintln!(
                    "[{}] PC=0x{:08X} {} inst={} ra=0x{:08X} sp=0x{:08X} SATP=0x{:08X}",
                    count, pc, priv_name, inst_str, ra, sp, satp_val
                );
            }
        }

        let step_result = vm.step();
        count += 1;

        if let geometry_os::riscv::cpu::StepResult::FetchFault = step_result {
            if count >= trace_from {
                eprintln!(
                    "[{}] *** FETCH FAULT *** PC=0x{:08X} sepc=0x{:08X} stval=0x{:08X}",
                    count, vm.cpu.pc, vm.cpu.csr.sepc, vm.cpu.csr.stval
                );
                // Dump all regs
                for i in 0..32 {
                    eprintln!("  x{}=0x{:08X}", i, vm.cpu.x[i]);
                }
                break;
            }
        }
    }
}
