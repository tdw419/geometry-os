use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::{StepResult, Privilege};
    use geometry_os::riscv::mmu::AccessType;

    let (mut vm, fw_addr, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let mut count: u64 = 0;
    let mut trampoline_patched = false;

    while count < 177_460 {
        if vm.bus.sbi.shutdown_requested { break; }

        if !trampoline_patched
            && vm.cpu.pc == 0x10EE
            && vm.cpu.privilege == Privilege::Supervisor
            && vm.cpu.csr.satp == 0
        {
            vm.bus.write_word(0x0148_4000u64, 0x0000_00EFu32).ok();
            vm.bus.write_word(0x0080_2000u64, 0x0000_00EFu32).ok();
            trampoline_patched = true;
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus >> 0xC) & 3;
                if cause_code == 9 {
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
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                        vm.cpu.csr.mstatus &= !(1 << 1);
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

        vm.step();
        count += 1;
    }

    eprintln!("=== Pre-fault state ===");
    eprintln!("PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("GP (x3) = 0x{:08X}", vm.cpu.x[3]);
    eprintln!("satp=0x{:08X}", vm.cpu.csr.satp);
    eprintln!("mstatus=0x{:08X}", vm.cpu.csr.mstatus);

    let fault_va = 0xC14809D0u32;
    let sum = (vm.cpu.csr.mstatus >> 18) & 1 != 0;
    let mxr = (vm.cpu.csr.mstatus >> 19) & 1 != 0;
    let satp = vm.cpu.csr.satp;

    eprintln!("\n=== Manual MMU translate for VA 0x{:08X} (Load, S-mode) ===", fault_va);
    let result = geometry_os::riscv::mmu::translate(fault_va, AccessType::Load, Privilege::Supervisor, sum, mxr, satp, &mut vm.bus, &mut vm.cpu.tlb);
    match result {
        geometry_os::riscv::mmu::TranslateResult::Ok(pa) => eprintln!("  -> OK, PA=0x{:08X}", pa),
        geometry_os::riscv::mmu::TranslateResult::LoadFault => eprintln!("  -> LOAD FAULT"),
        geometry_os::riscv::mmu::TranslateResult::FetchFault => eprintln!("  -> FETCH FAULT"),
        geometry_os::riscv::mmu::TranslateResult::StoreFault => eprintln!("  -> STORE FAULT"),
    }

    // Also try fetch
    eprintln!("\n=== Manual MMU translate for VA 0x{:08X} (Fetch, S-mode) ===", fault_va);
    let result2 = geometry_os::riscv::mmu::translate(fault_va, AccessType::Fetch, Privilege::Supervisor, sum, mxr, satp, &mut vm.bus, &mut vm.cpu.tlb);
    match result2 {
        geometry_os::riscv::mmu::TranslateResult::Ok(pa) => eprintln!("  -> OK, PA=0x{:08X}", pa),
        geometry_os::riscv::mmu::TranslateResult::LoadFault => eprintln!("  -> LOAD FAULT"),
        geometry_os::riscv::mmu::TranslateResult::FetchFault => eprintln!("  -> FETCH FAULT"),
        geometry_os::riscv::mmu::TranslateResult::StoreFault => eprintln!("  -> STORE FAULT"),
    }

    // Execute the faulting instruction
    eprintln!("\n=== Executing faulting instruction ===");
    let step_result = vm.step();
    eprintln!("Step result: {:?}", step_result);
    eprintln!("PC after: 0x{:08X}, priv: {:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("scause=0x{:08X} sepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X}",
        vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.stvec);

    // MMU log
    let log_len = vm.bus.mmu_log.len();
    eprintln!("\n=== Last 5 MMU events (of {}) ===", log_len);
    for entry in vm.bus.mmu_log.iter().rev().take(5) {
        eprintln!("  {:?}", entry);
    }
}
