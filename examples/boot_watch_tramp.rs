use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    
    let (mut vm, fw_addr, _entry, _dtb) = RiscvVm::boot_linux_setup(
        &kernel_image, initramfs.as_deref(), 256, "loglevel=0 quiet"
    ).unwrap();
    
    // Set write watchpoints on trampoline_pg_dir entries
    // L1[0xFF] at PA 0x01484000 + 0xFF*4 = 0x014840FC
    vm.bus.write_watch_addr = Some(0x014840FC);
    vm.bus.write_watch_hit = false;
    
    let fw_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max = 200_000u64;
    let mut trap_handler_pc = false;
    
    while count < max {
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        
        let r = vm.step();
        
        // Check if watchpoint was hit
        if vm.bus.write_watch_hit && !trap_handler_pc {
            eprintln!("[WP] WRITE to PA 0x{:08X} val=0x{:08X} at count={} PC=0x{:08X}", 
                0x014840FC, vm.bus.write_watch_val, count, vm.bus.write_watch_pc);
            trap_handler_pc = true;
            // Read all trampoline_pg_dir entries now
            for i in 0..1024u32 {
                let pte = vm.bus.read_word(0x01484000 + (i as u64) * 4).unwrap_or(0);
                if pte != 0 {
                    eprintln!("  tramp L1[0x{:03X}] = 0x{:08X}", i, pte);
                }
            }
        }
        
        // Also watch L1[0x300]
        if count % 50_000 == 0 {
            let l1_300 = vm.bus.read_word(0x01484000 + 0x300 * 4).unwrap_or(0);
            let l1_ff = vm.bus.read_word(0x01484000 + 0xFF * 4).unwrap_or(0);
            if l1_300 != 0 || l1_ff != 0 {
                eprintln!("[check] count={} L1[0x300]=0x{:08X} L1[0xFF]=0x{:08X}", 
                    count, l1_300, l1_ff);
            }
        }
        
        if vm.cpu.pc == fw_u32 {
            let mcause = vm.cpu.csr.mcause & !(1u32 << 31);
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        
        count += 1;
    }
    
    // Final check
    eprintln!("\nFinal trampoline_pg_dir:");
    for i in 0..1024u32 {
        let pte = vm.bus.read_word(0x01484000 + (i as u64) * 4).unwrap_or(0);
        if pte != 0 {
            eprintln!("  L1[0x{:03X}] = 0x{:08X}", i, pte);
        }
    }
    eprintln!("\nWatchpoint hit: {}", vm.bus.write_watch_hit);
    eprintln!("SATP: 0x{:08X}", vm.cpu.csr.satp);
}
