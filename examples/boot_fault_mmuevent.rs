use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::{StepResult, Privilege};

    let (mut vm, fw_addr, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let mut count: u64 = 0;
    let mut trampoline_patched = false;

    // Run to count 177460 (just before the fault in boot_5m)
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
            eprintln!("[{}] Trampoline patched", count);
        }

        // Handle M-mode traps (same as boot_linux)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                // ECALL_M = SBI call
                let r = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint);
                if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else {
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
            count += 1;
            continue;
        }

        vm.step();
        count += 1;
    }

    // Clear MMU log to see only the fault-related events
    vm.bus.mmu_log.clear();

    // Now step through the next 10 instructions, logging MMU events
    eprintln!("\n=== Stepping through fault region ===");
    for _ in 0..10 {
        let pc_before = vm.cpu.pc;
        let result = vm.step();
        let fault_str = match result {
            StepResult::Ok => "OK",
            StepResult::LoadFault => "LOAD_FAULT",
            StepResult::FetchFault => "FETCH_FAULT",
            StepResult::StoreFault => "STORE_FAULT",
            StepResult::Ebreak => "EBREAK",
            StepResult::Ecall => "ECALL",
        };
        eprintln!("[{}] PC=0x{:08X} -> 0x{:08X} priv={:?} result={} GP=0x{:08X}",
            count, pc_before, vm.cpu.pc, vm.cpu.privilege, fault_str, vm.cpu.x[3]);

        // Print MMU events from this step
        for event in vm.bus.mmu_log.iter() {
            eprintln!("  MMU: {:?}", event);
        }
        vm.bus.mmu_log.clear();

        if !matches!(result, StepResult::Ok | StepResult::Ecall) {
            break;
        }
        count += 1;
    }
}
