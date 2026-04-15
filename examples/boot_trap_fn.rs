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
    let max_instr = 1_000_000u64; // Just first 1M instructions
    
    let fn_start: u32 = 0xC08E5D58;
    let fn_end: u32 = 0xC08E5DDC;
    let mut in_fn = false;

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
                    // DON'T set PC yet - first check if we'd enter __memmove
                    let new_pc = stvec;
                    let new_in_fn = new_pc >= fn_start && new_pc < fn_end;
                    if new_in_fn && !in_fn {
                        eprintln!("[TRAP_TO_FN] count={} stvec=0x{:08X} mcause=0x{:08X} mpp={}",
                            count, stvec, mcause, mpp);
                        eprintln!("  a0=0x{:08X} a1=0x{:08X} a2=0x{:08X}",
                            vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12]);
                    }
                    vm.cpu.pc = stvec; vm.cpu.privilege = Privilege::Supervisor; vm.cpu.tlb.flush_all();
                    in_fn = new_in_fn;
                    count += 1; continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        
        let _ = vm.step();
        count += 1;
    }
    
    eprintln!("[DONE] count={}", count);
}
