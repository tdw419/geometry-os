/// Trace instructions between count 177400 and 178000 to see what handle_exception does.
fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::{StepResult, Privilege};
    use geometry_os::riscv::csr;

    let (mut vm, fw_addr, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let mut count: u64 = 0;
    let mut trampoline_patched = false;
    let mut tracing = false;
    let mut trace_count = 0u64;
    let max_trace = 600u64;

    while count < 200_000 {
        if vm.bus.sbi.shutdown_requested { break; }

        if !trampoline_patched
            && vm.cpu.pc == 0x10EE
            && vm.cpu.privilege == Privilege::Supervisor
            && vm.cpu.csr.satp == 0
        {
            let identity_pte: u32 = 0x0000_00EF;
            vm.bus.write_word(0x0148_4000u64, identity_pte).ok();
            vm.bus.write_word(0x0080_2000u64, identity_pte).ok();
            trampoline_patched = true;
            eprintln!("[{}] TRAMPOLINE PATCHED", count);
        }

        // Start tracing just before the fault
        if !tracing && count >= 177_450 {
            tracing = true;
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code != csr::CAUSE_ECALL_M {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if cause_code == csr::CAUSE_ECALL_S {
                    let r = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                        vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                        &mut vm.bus.uart, &mut vm.bus.clint);
                    if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                    vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                } else if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = vm.cpu.csr.mcause;
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
                    } else { vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4); }
                } else { vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4); }
            } else {
                let r = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint);
                if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
            count += 1;
            continue;
        }

        let pc_before = vm.cpu.pc;
        let result = vm.step();

        if tracing && trace_count < max_trace {
            let op_name = vm.cpu.last_step.as_ref()
                .map(|l| format!("{:?}", l.op))
                .unwrap_or_default();
            let word = vm.cpu.last_step.as_ref()
                .map(|l| l.word)
                .unwrap_or(0);
            match result {
                StepResult::Ok | StepResult::Ecall => {
                    eprintln!("[{}] 0x{:08X} {:08X} {:30} a0={:08X} ra={:08X} sp={:08X}",
                        count, pc_before, word, op_name,
                        vm.cpu.x[10], vm.cpu.x[1], vm.cpu.x[2]);
                }
                _ => {
                    eprintln!("[{}] 0x{:08X} {:08X} {:30} *** FAULT *** sepc={:08X} stval={:08X}",
                        count, pc_before, word, op_name,
                        vm.cpu.csr.sepc, vm.cpu.csr.stval);
                }
            }
            trace_count += 1;
        }

        if tracing && trace_count >= max_trace {
            eprintln!("... trace limit reached, stopping");
            break;
        }

        count += 1;
    }
}
