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
    let max_instructions: u64 = 178_000u64;
    let mut state = 0;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x3000) >> 12;

            if cause_code == 11 || cause_code == 9 {
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
                    count += 1;
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        if state == 0 && vm.cpu.pc == 0x10EA {
            state = 1;
            // Snapshot trampoline
            let before: u32 = vm.bus.read_word(0x01484000).unwrap_or(0);
            eprintln!("[diag] Before setup_vm: trampoline[0] = 0x{:08X}", before);
        }

        if state == 1 && vm.cpu.pc == 0x10EE {
            state = 2;
            // Check trampoline after setup_vm
            let after: u32 = vm.bus.read_word(0x01484000).unwrap_or(0);
            eprintln!("[diag] After setup_vm: trampoline[0] = 0x{:08X}", after);

            // Also check if setup_vm wrote to 0x1480000 area at all
            let mut any_write = false;
            for addr in (0x01400000u64..0x01500000).step_by(4) {
                match vm.bus.read_word(addr) {
                    Ok(v) if v != 0 => {
                        if !any_write {
                            eprintln!("[diag] First non-zero in 0x1400000-0x1500000:");
                            any_write = true;
                        }
                        eprintln!("  0x{:08X}: 0x{:08X}", addr, v);
                    }
                    _ => {}
                }
            }
            if !any_write {
                eprintln!("[diag] NO writes to 0x1400000-0x1500000 from setup_vm!");
            }

            // Check a broader range
            let mut total_nonzero = 0;
            for addr in (0u64..0x02000000).step_by(0x10000) {
                match vm.bus.read_word(addr) {
                    Ok(v) if v != 0 => {
                        total_nonzero += 1;
                    }
                    _ => {}
                }
            }
            eprintln!(
                "[diag] Non-zero words in sampled 0x0-0x2000000: {}",
                total_nonzero
            );
            break;
        }

        let step_result = vm.step();
        if matches!(step_result, geometry_os::riscv::cpu::StepResult::Ebreak) {
            break;
        }
        count += 1;
    }
}
