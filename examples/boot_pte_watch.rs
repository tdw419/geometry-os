fn main() {
    // Diagnostic: watch writes to the PTE at PA 0xC1002C08
    // Boot to 16.5M, then step one at a time watching for the PTE change
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::StepResult;
    use geometry_os::riscv::csr;

    // Boot to 16.5M
    let (mut vm, _) = RiscvVm::boot_linux(&kernel_image, initramfs.as_deref(), 256, 16_500_000, bootargs).unwrap();
    println!("At 16.5M: PC=0x{:08X}", vm.cpu.pc);

    let satp = vm.cpu.csr.read(csr::SATP);
    let ppn = satp & 0x3FFFFF;
    let pt_base = (ppn as u64) << 12;
    let pte_pa = pt_base + 770u64 * 4;
    let mut last_pte = vm.bus.read_word(pte_pa).unwrap_or(0);
    println!("Initial L1[770] at PA=0x{:X}: 0x{:08X}", pte_pa, last_pte);

    // Step one at a time, checking PTE every 1000 steps
    let mut pte_change_count = 0;
    for count in 16_500_000..17_200_000 {
        let step_result = vm.step();

        if count % 1000 == 0 {
            let pte = vm.bus.read_word(pte_pa).unwrap_or(0);
            if pte != last_pte {
                pte_change_count += 1;
                println!("\n[PTE CHANGE #{}] at count={}", pte_change_count, count);
                println!("  L1[770]: 0x{:08X} -> 0x{:08X}", last_pte, pte);
                println!("  PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
                println!("  sepc=0x{:08X} scause=0x{:08X} stval=0x{:08X}",
                    vm.cpu.csr.sepc, vm.cpu.csr.scause, vm.cpu.csr.stval);
                // Print registers that might be relevant
                println!("  x1(ra)=0x{:08X} x5(t0)=0x{:08X} x6(t1)=0x{:08X}",
                    vm.cpu.x[1], vm.cpu.x[5], vm.cpu.x[6]);
                println!("  x7(t2)=0x{:08X} x14(a4)=0x{:08X} x29(t4)=0x{:08X} x30(t5)=0x{:08X}",
                    vm.cpu.x[7], vm.cpu.x[14], vm.cpu.x[29], vm.cpu.x[30]);
                
                // Disassemble the instruction at PC-4 (the store that caused the change)
                if let Some(ref last_step) = vm.cpu.last_step {
                    println!("  Last step: pc=0x{:08X} op={:?}", last_step.pc, last_step.op);
                }
                
                last_pte = pte;
                if pte_change_count >= 5 {
                    break;
                }
            }
        }

        match step_result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                println!("\n[FAULT] count={} PC=0x{:08X} scause=0x{:08X} stval=0x{:08X}",
                    count, vm.cpu.pc, vm.cpu.csr.scause, vm.cpu.csr.stval);
                break;
            }
            StepResult::Ebreak => break,
            _ => {}
        }
    }
}
