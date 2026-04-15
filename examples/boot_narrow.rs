fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::StepResult;

    let (mut vm, _, _, _) = RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    // Run to just before the crash
    let target = 16_998_900;
    for count in 0..target {
        if vm.bus.sbi.shutdown_requested { break; }
        let _ = vm.step();
    }
    println!("At count={}, PC=0x{:08X}", target, vm.cpu.pc);

    // Trace 1500 instructions from here with PC tracking
    let mut last_pc = vm.cpu.pc;
    let mut same_count = 0u64;
    for count in target..(target + 1500) {
        if vm.bus.sbi.shutdown_requested { break; }
        let pc_before = vm.cpu.pc;
        let result = vm.step();

        if vm.cpu.pc == last_pc {
            same_count += 1;
        } else {
            if same_count > 10 {
                println!("  [spin {} for {} steps]", last_pc, same_count);
            }
            last_pc = vm.cpu.pc;
            same_count = 0;
        }

        match result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                println!("[FAULT {}] {:?} at PC=0x{:08X} -> now PC=0x{:08X} scause=0x{:X} sepc=0x{:08X} stval=0x{:08X}",
                    count, result, pc_before, vm.cpu.pc, vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval);
                if same_count == 0 && (count - target) < 200 {
                    // Print first 10 registers for first few faults
                    let regs: Vec<String> = vm.cpu.x.iter().take(10).enumerate()
                        .map(|(i, v)| format!("x{}=0x{:X}", i, v)).collect();
                    println!("  regs: {}", regs.join(" "));
                }
            }
            StepResult::Ebreak => { println!("[EBREAK] count={}", count); break; }
            _ => {}
        }
        if same_count > 100 {
            println!("[SPIN LOOP] stuck at PC=0x{:08X} for {} steps", last_pc, same_count);
            break;
        }
    }
    
    // Check if we can translate the handler address through the TLB
    println!("\n--- TLB lookup for 0xC08EFF1C ---");
    let vpn = 0xC08EFF1C >> 12;
    let asid: u16 = ((vm.cpu.csr.satp >> 16) & 0xFFFF) as u16;
    match vm.cpu.tlb.lookup(vpn, asid) {
        Some((ppn, flags)) => println!("TLB HIT: ppn=0x{:X} flags=0x{:X}", ppn, flags),
        None => println!("TLB MISS: not cached"),
    }
    
    println!("\nConsole: {} bytes", vm.bus.sbi.console_output.len());
}
