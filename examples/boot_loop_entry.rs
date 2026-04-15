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
    let mut count: u64 = 0;
    let max_instr = 20_000_000u64;
    
    // Watch for any entry into the __memmove backward word-copy loop
    // The loop is at 0xC08E5D6A. We detect entry by checking if the PREVIOUS
    // PC was NOT 0xC08E5D76 (the bne at the end of the loop).
    let loop_start: u32 = 0xC08E5D6A;
    let loop_end: u32 = 0xC08E5D76;
    let mut last_pc: u32 = 0;
    let mut in_loop = false;
    let mut entry_count = 0;

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
                    last_pc = vm.cpu.pc;
                    count += 1;
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        
        let cur_pc = vm.cpu.pc;
        
        // Detect fresh entry into the word-copy backward loop
        if cur_pc == loop_start && last_pc != loop_end {
            entry_count += 1;
            eprintln!("[LOOP_ENTRY#{}] count={} from_pc=0x{:08X}", entry_count, count, last_pc);
            eprintln!("  a0=0x{:08X} a1=0x{:08X} a2=0x{:08X} ra=0x{:08X}",
                vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[1]);
            eprintln!("  t3(x28)=0x{:08X} t4(x29)=0x{:08X} t5(x30)=0x{:08X} t6(x31)=0x{:08X}",
                vm.cpu.x[28], vm.cpu.x[29], vm.cpu.x[30], vm.cpu.x[31]);
            eprintln!("  a4(x14)=0x{:08X}", vm.cpu.x[14]);
            if entry_count >= 10 {
                break;
            }
        }
        
        in_loop = cur_pc >= loop_start && cur_pc <= loop_end;
        last_pc = cur_pc;
        
        let _ = vm.step();
        count += 1;
    }
    
    eprintln!("[DONE] count={} entries={}", count, entry_count);
}
