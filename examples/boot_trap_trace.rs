fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";
    
    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::StepResult;
    
    let (mut vm, _) = RiscvVm::boot_linux(&kernel_image, initramfs.as_deref(), 256, 1, bootargs).unwrap();
    
    let mut trap_count = 0u32;
    let mut last_ecall_pc = 0u32;
    
    for i in 1..=2_000_000u64 {
        let pc_before = vm.cpu.pc;
        let priv_before = vm.cpu.privilege;
        let result = vm.step();
        
        match result {
            StepResult::Ecall => {
                // Log ECALLs -- these are SBI calls during boot
                if trap_count < 50 || i % 100_000 == 0 {
                    println!("{:>8}: ECALL at PC=0x{:08X} priv={:?} a7={}", 
                        i, pc_before, priv_before, vm.cpu.x[17]);
                }
                last_ecall_pc = pc_before;
            }
            _ => {}
        }
        
        // Check if we entered the trap handler (PC jumped to fw_addr)
        if vm.cpu.pc == 0xC0940000 && pc_before != 0xC0940000 {
            let mcause = vm.cpu.csr.read(geometry_os::riscv::csr::MCAUSE);
            let mepc = vm.cpu.csr.read(geometry_os::riscv::csr::MEPC);
            println!("{:>8}: TRAP! from PC=0x{:08X} priv={:?} -> mcause=0x{:X} mepc=0x{:08X}", 
                i, pc_before, priv_before, mcause, mepc);
            trap_count += 1;
            if trap_count >= 10 { break; }
        }
        
        // Check if SATP was written (paging enabled)
        let satp = vm.cpu.csr.read(geometry_os::riscv::csr::SATP);
        if satp != 0 && i % 100_000 == 0 && i > 100_000 {
            // Only print occasionally
        }
    }
    
    println!("\nFinal: {} instructions, {} traps, PC=0x{:08X}", 
        2_000_000, trap_count, vm.cpu.pc);
}
