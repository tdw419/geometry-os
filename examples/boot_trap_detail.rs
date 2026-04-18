use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) = geometry_os::riscv::RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        512,
        bootargs,
    )
    .unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instructions: u64 = 200_000u64;

    let mut illegal_count: u64 = 0;
    let mut sbi_call_count: u64 = 0;
    let mut forward_count: u64 = 0;
    let mut first_trap = true;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            eprintln!("[diag] SBI shutdown at count={}", count);
            break;
        }

        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x3000) >> 12;

            if cause_code == 11 || cause_code == 9 {
                sbi_call_count += 1;
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
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            } else if mpp != 3 {
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                    let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                    vm.cpu.csr.mstatus &= !(1 << 1);
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    forward_count += 1;

                    if cause_code == 2 && first_trap {
                        first_trap = false;
                        illegal_count += 1;
                        eprintln!("[diag] FIRST ILLEGAL at count={}", count);
                        eprintln!(
                            "[diag] sepc=0x{:08X} mpp={} stvec=0x{:08X}",
                            vm.cpu.csr.sepc, mpp, stvec
                        );

                        // Read instruction at sepc using virtual->physical translation
                        // First, try direct physical read
                        let phys_instr = vm.bus.read_word(vm.cpu.csr.sepc as u64).unwrap_or(0);
                        eprintln!("[diag] Physical read at sepc: 0x{:08X}", phys_instr);

                        // Dump memory 0x0-0x20
                        eprintln!("[diag] Memory 0x0000-0x0020:");
                        for addr in (0..0x20).step_by(4) {
                            let val = vm.bus.read_word(addr as u64).unwrap_or(0);
                            eprintln!("  0x{:04X}: 0x{:08X}", addr, val);
                        }

                        // Dump memory 0x6600-0x6620
                        eprintln!("[diag] Memory 0x6600-0x6620:");
                        for addr in (0x6600..0x6620).step_by(4) {
                            let val = vm.bus.read_word(addr as u64).unwrap_or(0);
                            eprintln!("  0x{:04X}: 0x{:08X}", addr, val);
                        }

                        // Check what the kernel's stvec handler is
                        eprintln!("[diag] Memory at stvec (0x{:08X}):", stvec);
                        for i in 0..8 {
                            let addr = stvec as u64 + (i * 4);
                            let val = vm.bus.read_word(addr).unwrap_or(0);
                            eprintln!("  0x{:08X}: 0x{:08X}", addr, val);
                        }

                        eprintln!("[diag] Registers: a0=0x{:08X} a1=0x{:08X} a2=0x{:08X} a3=0x{:08X} a4=0x{:08X} a5=0x{:08X} a6=0x{:08X} a7=0x{:08X}",
                            vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[13],
                            vm.cpu.x[14], vm.cpu.x[15], vm.cpu.x[16], vm.cpu.x[17]);
                        eprintln!(
                            "[diag] SP=0x{:08X} RA=0x{:08X} GP=0x{:08X} TP=0x{:08X}",
                            vm.cpu.x[2], vm.cpu.x[1], vm.cpu.x[3], vm.cpu.x[4]
                        );
                        eprintln!(
                            "[diag] T0=0x{:08X} T1=0x{:08X} T2=0x{:08X} S0=0x{:08X} S1=0x{:08X}",
                            vm.cpu.x[5], vm.cpu.x[6], vm.cpu.x[7], vm.cpu.x[8], vm.cpu.x[9]
                        );
                    }

                    count += 1;
                    continue;
                }
            }

            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let step_result = vm.step();
        if matches!(step_result, geometry_os::riscv::cpu::StepResult::Ebreak) {
            eprintln!("[diag] EBREAK at count={}", count);
            break;
        }

        count += 1;
    }

    eprintln!(
        "[diag] SBI calls={}, forwards={}, illegal={}",
        sbi_call_count, forward_count, illegal_count
    );
    if first_trap {
        eprintln!(
            "[diag] No illegal trap in {} instructions. PC=0x{:08X} priv={:?}",
            count, vm.cpu.pc, vm.cpu.privilege
        );
    }
}
