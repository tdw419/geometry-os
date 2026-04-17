use geometry_os::riscv::{RiscvVm, cpu::StepResult};

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _, _, _) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    ).unwrap();
    
    // Step and check interrupt state periodically
    let mut count = 0u64;
    let mut printed = false;
    
    while count < 5_000_000 {
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        
        let result = vm.step();
        
        // Print interrupt state at key points
        if !printed && count > 100_000 {
            let mie = vm.cpu.csr.mie;
            let mip = vm.cpu.csr.mip;
            let mstatus = vm.cpu.csr.mstatus;
            let sie_bit = (mstatus >> 1) & 1; // SIE
            let mie_bit = (mstatus >> 3) & 1; // MIE
            
            eprintln!("At count={}:", count);
            eprintln!("  MIE=0x{:08X} (MTIE={} STIE={} SEIE={})", mie,
                (mie >> 7) & 1, (mie >> 5) & 1, (mie >> 9) & 1);
            eprintln!("  MIP=0x{:08X} (MTIP={} STIP={} SEIP={})", mip,
                (mip >> 7) & 1, (mip >> 5) & 1, (mip >> 9) & 1);
            eprintln!("  mstatus=0x{:08X} (MIE={} SIE={})", mstatus, mie_bit, sie_bit);
            eprintln!("  SATP=0x{:08X}", vm.cpu.csr.satp);
            eprintln!("  stvec=0x{:08X}", vm.cpu.csr.stvec);
            eprintln!("  mtvec=0x{:08X}", vm.cpu.csr.mtvec);
            eprintln!("  medeleg=0x{:08X}", vm.cpu.csr.medeleg);
            eprintln!("  mideleg=0x{:08X}", vm.cpu.csr.mideleg);
            eprintln!("  mtime={} mtimecmp={}", vm.bus.clint.mtime, vm.bus.clint.mtimecmp);
            eprintln!("  timer_pending={}", vm.bus.clint.timer_pending());
            eprintln!("  PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
            printed = true;
        }
        
        count += 1;
    }
    
    // Now check if we manually set STIP, what happens
    eprintln!("\n=== Manually setting STIP ===");
    vm.cpu.csr.mip |= 1 << 5; // STIP
    for _ in 0..1000 {
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        vm.cpu.csr.mip |= 1 << 5; // Force STIP since sync_mip clears it
        let result = vm.step();
    }
    eprintln!("After 1K with forced STIP: PC=0x{:08X} UART={}", vm.cpu.pc, vm.bus.uart.tx_buf.len());
    let uart_str: String = vm.bus.uart.tx_buf.iter().chain(vm.bus.sbi.console_output.iter()).map(|&b| b as char).collect();
    eprintln!("Output: {:?}", &uart_str[..uart_str.len().min(200)]);
}
