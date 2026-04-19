fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::cpu::{Privilege, StepResult};
    use geometry_os::riscv::csr;
    use geometry_os::riscv::RiscvVm;

    let (mut vm, fw_addr, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let table_addr = 0xC1400AE8u64;

    // Run to 11.7M
    for i in 0..11_700_000u64 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mc = vm.cpu.csr.mcause & !(1u32 << 31);
            if mc == csr::CAUSE_ECALL_S {
                let r = vm.bus.sbi.handle_ecall(
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
                if let Some((a0, a1)) = r {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else if mc != csr::CAUSE_ECALL_M {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = vm.cpu.csr.mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP))
                            | (spp << csr::MSTATUS_SPP);
                        let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE))
                            | (sie << csr::MSTATUS_SPIE);
                        vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                    } else {
                        vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                    }
                } else {
                    vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                }
            } else {
                let r = vm.bus.sbi.handle_ecall(
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
                if let Some((a0, a1)) = r {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
            continue;
        }
        let _ = vm.step();
    }

    eprintln!(
        "At 11.7M: table=0x{:08X}",
        vm.bus.read_word(table_addr).unwrap_or(0)
    );

    // Now scan with 1K steps
    let mut count = 11_700_000u64;
    while count < 12_000_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let target = count + 1_000;
        for _ in count..target {
            if vm.bus.sbi.shutdown_requested {
                break;
            }
            if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
                let mc = vm.cpu.csr.mcause & !(1u32 << 31);
                if mc == csr::CAUSE_ECALL_S {
                    let r = vm.bus.sbi.handle_ecall(
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
                    if let Some((a0, a1)) = r {
                        vm.cpu.x[10] = a0;
                        vm.cpu.x[11] = a1;
                    }
                    vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                } else if mc != csr::CAUSE_ECALL_M {
                    let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                    if mpp != 3 {
                        let stvec = vm.cpu.csr.stvec & !0x3u32;
                        if stvec != 0 {
                            vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                            vm.cpu.csr.scause = vm.cpu.csr.mcause;
                            vm.cpu.csr.stval = vm.cpu.csr.mtval;
                            let spp = if mpp == 1 { 1u32 } else { 0u32 };
                            vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP))
                                | (spp << csr::MSTATUS_SPP);
                            let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                            vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE))
                                | (sie << csr::MSTATUS_SPIE);
                            vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                            vm.cpu.pc = stvec;
                            vm.cpu.privilege = Privilege::Supervisor;
                            vm.cpu.tlb.flush_all();
                        } else {
                            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                        }
                    } else {
                        vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                    }
                } else {
                    let r = vm.bus.sbi.handle_ecall(
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
                    if let Some((a0, a1)) = r {
                        vm.cpu.x[10] = a0;
                        vm.cpu.x[11] = a1;
                    }
                    vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                }
                continue;
            }
            let _ = vm.step();
        }
        count = target;
        let val = vm.bus.read_word(table_addr).unwrap_or(0);
        if val == 0 {
            eprintln!("ZEROED at count={}", count);
            break;
        }
    }
    eprintln!("PC=0x{:08X}", vm.cpu.pc);
}
