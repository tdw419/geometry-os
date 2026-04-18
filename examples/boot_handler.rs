fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::cpu::StepResult;
    use geometry_os::riscv::RiscvVm;

    let (mut vm, _, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    // Run to just before the crash
    let target = 16_993_000;
    for count in 0..target {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let _ = vm.step();
    }

    // Trace the handler execution after the first fault
    let mut in_handler = false;
    let mut handler_count = 0;
    for count in target..(target + 500) {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let pc_before = vm.cpu.pc;
        let result = vm.step();

        if vm.cpu.pc == 0xC08EFF1C && !in_handler {
            in_handler = true;
            println!(
                "\n=== Handler entered at count={}, first fault was load at 0x{:08X} ===",
                count, pc_before
            );
            println!(
                "  sepc=0x{:08X} scause=0x{:X} stval=0x{:08X}",
                vm.cpu.csr.sepc, vm.cpu.csr.scause, vm.cpu.csr.stval
            );
            // Print key registers
            for r in [1, 2, 5, 6, 10, 11, 28, 29, 30, 31] {
                println!("  x{}=0x{:08X}", r, vm.cpu.x[r]);
            }
        }

        if in_handler {
            handler_count += 1;
            // Only print every 10th step + JAL/JR instructions + fault
            let is_jalr = matches!(result, StepResult::Ok)
                && vm.cpu.pc != pc_before + 2
                && vm.cpu.pc != pc_before + 4;

            if handler_count <= 5
                || handler_count % 10 == 0
                || is_jalr
                || matches!(
                    result,
                    StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault
                )
            {
                println!(
                    "[H{}] count={} PC=0x{:08X} -> 0x{:08X} {:?} x1=0x{:X} x6=0x{:X} sp=0x{:X}",
                    handler_count,
                    count,
                    pc_before,
                    vm.cpu.pc,
                    result,
                    vm.cpu.x[1],
                    vm.cpu.x[6],
                    vm.cpu.x[2]
                );
            }

            if matches!(
                result,
                StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault
            ) {
                println!(
                    "  scause=0x{:X} sepc=0x{:08X} stval=0x{:08X}",
                    vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval
                );
                // Print all regs at fault
                for r in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 28, 29, 30, 31] {
                    println!("  x{}=0x{:08X}", r, vm.cpu.x[r]);
                }
                break;
            }

            if handler_count > 200 {
                println!("[H] handler ran 200+ steps without fault, breaking");
                break;
            }
        }
    }
}
