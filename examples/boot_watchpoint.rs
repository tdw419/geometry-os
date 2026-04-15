fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::Privilege;
    use geometry_os::riscv::csr;

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    
    // Set write watchpoint on L1[770] = 0x1002C08
    vm.bus.write_watch_addr = Some(0x1002C08);
    
    let max_instr = 20_000_000u64;
    let mut count: u64 = 0;

    while count < max_instr {
        if vm.bus.sbi.shutdown_requested { break; }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
            if cause_code == csr::CAUSE_ECALL_S || cause_code == csr::CAUSE_ECALL_M {
                let result = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15], &mut vm.bus.uart, &mut vm.bus.clint);
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            } else if mpp != 3 {
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc; vm.cpu.csr.scause = mcause; vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP)) | (spp << csr::MSTATUS_SPP);
                    let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE)) | (sie << csr::MSTATUS_SPIE);
                    vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                    vm.cpu.pc = stvec; vm.cpu.privilege = Privilege::Supervisor; vm.cpu.tlb.flush_all();
                    count += 1; continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        
        // Before stepping, save PC for watchpoint
        let pc_before = vm.cpu.pc;
        
        let _ = vm.step();
        count += 1;
        
        // Check if watchpoint was hit
        if vm.bus.write_watch_hit {
            eprintln!("[WATCH] count={} PC_before=0x{:08X} val=0x{:08X}",
                count, pc_before, vm.bus.write_watch_val);
            eprintln!("  a0=0x{:08X} a1=0x{:08X} a2=0x{:08X} a3=0x{:08X}",
                vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[13]);
            eprintln!("  sp=0x{:08X} ra=0x{:08X} t0=0x{:08X} t1=0x{:08X}",
                vm.cpu.x[2], vm.cpu.x[1], vm.cpu.x[5], vm.cpu.x[6]);
            eprintln!("  priv={:?}", vm.cpu.privilege);
            
            // Read the PTE now
            let pte = vm.bus.read_word(0x1002C08).unwrap_or(0);
            eprintln!("  L1[770] after write: 0x{:08X} V={}", pte, pte & 1);
            
            // Reset watchpoint to catch the next write
            vm.bus.write_watch_hit = false;
            
            // Only report first hit
            break;
        }
    }
    
    if !vm.bus.write_watch_hit {
        eprintln!("Watchpoint not hit in {} instructions", count);
        let pte = vm.bus.read_word(0x1002C08).unwrap_or(0);
        eprintln!("L1[770] = 0x{:08X} V={}", pte, pte & 1);
    }
}
