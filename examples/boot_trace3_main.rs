use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    // Run for 500K then inspect  
    let (mut vm, _) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        500_000,
        "console=ttyS0 loglevel=8",
    ).unwrap();

    // Single step a bunch of times and print each instruction
    let mut last_satp = vm.cpu.csr.satp;
    for i in 0..200 {
        let pc_before = vm.cpu.pc;
        let priv_before = format!("{:?}", vm.cpu.privilege);
        
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        
        let result = vm.step();
        
        let inst = vm.bus.read_word(pc_before as u64).unwrap_or(0);
        let inst_half = vm.bus.read_half(pc_before as u64).unwrap_or(0);
        
        if vm.cpu.csr.satp != last_satp {
            eprintln!("  [{}] SATP changed: 0x{:08X} -> 0x{:08X}", i, last_satp, vm.cpu.csr.satp);
            last_satp = vm.cpu.csr.satp;
        }
        
        println!("[{:3}] PC=0x{:08X} -> 0x{:08X} {} inst=0x{:08X}(0x{:04X}) a0=0x{:08X} a1=0x{:08X} scause=0x{:08X} result={:?}",
            i, pc_before, vm.cpu.pc, priv_before, inst, inst_half, vm.cpu.x[10], vm.cpu.x[11],
            vm.cpu.csr.scause, result);
            
        if result == geometry_os::riscv::cpu::StepResult::Ebreak { break; }
    }
}
