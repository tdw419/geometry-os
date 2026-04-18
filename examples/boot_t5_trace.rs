fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::cpu::Privilege;
    use geometry_os::riscv::csr;
    use geometry_os::riscv::RiscvVm;

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let fn_start: u32 = 0xC08E5D58;
    let fn_end: u32 = 0xC08E5DDC;
    let mut count: u64 = 0;
    let mut in_fn = false;
    let mut last_pc: u32 = 0;
    let max_instr = 500_000u64;

    while count < max_instr {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
            if cause_code == csr::CAUSE_ECALL_S || cause_code == csr::CAUSE_ECALL_M {
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
                    vm.cpu.csr.mstatus =
                        (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP)) | (spp << csr::MSTATUS_SPP);
                    let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE))
                        | (sie << csr::MSTATUS_SPIE);
                    vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    last_pc = vm.cpu.pc;
                    in_fn = vm.cpu.pc >= fn_start && vm.cpu.pc < fn_end;
                    count += 1;
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let cur_pc = vm.cpu.pc;
        let cur_in_fn = cur_pc >= fn_start && cur_pc < fn_end;

        if cur_in_fn && !in_fn {
            eprintln!(
                "[FN_ENTRY] count={} PC=0x{:08X} from=0x{:08X}",
                count, cur_pc, last_pc
            );
            eprintln!(
                "  a0=0x{:08X} a1=0x{:08X} a2=0x{:08X} ra=0x{:08X}",
                vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[1]
            );
            eprintln!(
                "  t3=0x{:08X} t4=0x{:08X} t5=0x{:08X} t6=0x{:08X}",
                vm.cpu.x[28], vm.cpu.x[29], vm.cpu.x[30], vm.cpu.x[31]
            );
            eprintln!("  a4=0x{:08X}", vm.cpu.x[14]);

            // Single-step through the function
            for _ in 0..500 {
                let pc_before = vm.cpu.pc;
                let t5_before = vm.cpu.x[30];
                let _ = vm.step();
                count += 1;

                if vm.cpu.x[30] != t5_before {
                    eprintln!(
                        "  t5 changed at PC=0x{:08X}: 0x{:08X} -> 0x{:08X}",
                        pc_before, t5_before, vm.cpu.x[30]
                    );
                }

                // Stop at loop entry or return
                if vm.cpu.pc == 0xC08E5D6A {
                    eprintln!(
                        "  [loop entry] t5=0x{:08X} t4=0x{:08X} a4=0x{:08X}",
                        vm.cpu.x[30], vm.cpu.x[29], vm.cpu.x[14]
                    );
                    break;
                }
                if vm.cpu.pc == 0xC08E5DDA {
                    eprintln!("  [returned]");
                    break;
                }
            }
            break;
        }

        in_fn = cur_in_fn;
        last_pc = cur_pc;
        let _ = vm.step();
        count += 1;
    }
}
