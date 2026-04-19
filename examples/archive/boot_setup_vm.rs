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
    let mut saw_setup_vm_call = false;
    let mut saw_setup_vm_ret = false;

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

        // Watch for setup_vm call (PC approaches 0x10EA where jalr is)
        if vm.cpu.pc == 0x10EA
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor
            && !saw_setup_vm_call
        {
            saw_setup_vm_call = true;
            eprintln!(
                "[diag] setup_vm CALL at count={}: PC=0x{:08X} RA=0x{:08X}",
                count, vm.cpu.pc, vm.cpu.x[1]
            );
        }

        // Watch for setup_vm return (PC == RA from the call)
        if saw_setup_vm_call
            && !saw_setup_vm_ret
            && vm.cpu.pc == 0x10EE
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor
        {
            saw_setup_vm_ret = true;
            eprintln!(
                "[diag] setup_vm RETURN at count={}: PC=0x{:08X} a0=0x{:08X}",
                count, vm.cpu.pc, vm.cpu.x[10]
            );

            // Check early_pg_dir content
            eprintln!("[diag] Checking early_pg_dir at 0x00802000:");
            let mut nonzero = 0;
            for addr in (0x00802000u64..0x00804000).step_by(4) {
                match vm.bus.read_word(addr) {
                    Ok(v) if v != 0 => {
                        nonzero += 1;
                        if nonzero <= 10 {
                            let idx = ((addr - 0x00802000) / 4) as usize;
                            eprintln!("  [{}] 0x{:08X}: 0x{:08X}", idx, addr, v);
                        }
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("  0x{:08X}: ERR {:?}", addr, e),
                }
            }
            eprintln!("[diag] Total non-zero in early_pg_dir: {}", nonzero);

            // Check trampoline_pg_dir content
            eprintln!("[diag] Checking trampoline_pg_dir at 0x01484000:");
            let mut nonzero2 = 0;
            for addr in (0x01484000u64..0x01486000).step_by(4) {
                match vm.bus.read_word(addr) {
                    Ok(v) if v != 0 => {
                        nonzero2 += 1;
                        if nonzero2 <= 10 {
                            let idx = ((addr - 0x01484000) / 4) as usize;
                            eprintln!("  [{}] 0x{:08X}: 0x{:08X}", idx, addr, v);
                        }
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("  0x{:08X}: ERR {:?}", addr, e),
                }
            }
            eprintln!("[diag] Total non-zero in trampoline_pg_dir: {}", nonzero2);
        }

        let step_result = vm.step();
        if matches!(step_result, geometry_os::riscv::cpu::StepResult::Ebreak) {
            break;
        }
        count += 1;
    }

    if !saw_setup_vm_call {
        eprintln!("[diag] setup_vm was NEVER called! PC=0x{:08X}", vm.cpu.pc);
    }
}
