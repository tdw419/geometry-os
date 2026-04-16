use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    
    let (mut vm, fw_addr, _entry, _dtb_addr) = 
        geometry_os::riscv::RiscvVm::boot_linux_setup(
            &kernel_image, initramfs.as_deref(), 512, bootargs,
        ).unwrap();
    
    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instructions: u64 = 178_000u64;
    
    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested { break; }
        
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x3000) >> 12;
            
            if cause_code == 11 || cause_code == 9 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            } else if mpp != 3 {
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                    let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                    vm.cpu.csr.mstatus &= !(1 << 1);
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    count += 1;
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        
        // Catch SATAP change
        if vm.cpu.csr.satp != 0 && count < 177270 {
            let satp = vm.cpu.csr.satp;
            let pt_root = ((satp & 0x3FFFFF) as u64) << 12;
            eprintln!("[diag] SATAP=0x{:08X} at count={}, PC=0x{:08X}", satp, count, vm.cpu.pc);
            eprintln!("[diag] PT root at physical 0x{:08X}", pt_root);
            
            // Dump all non-zero entries in the page table
            for i in 0..1024 {
                let pte_addr = pt_root + (i as u64) * 4;
                match vm.bus.read_word(pte_addr) {
                    Ok(pte) if pte != 0 => {
                        let r = (pte >> 1) & 1;
                        let w = (pte >> 2) & 1;
                        let x = (pte >> 3) & 1;
                        let ppn = pte >> 10;
                        let is_leaf = (r | w | x) != 0;
                        if is_leaf {
                            eprintln!("  L1[{:3}] = 0x{:08X} MEGAPAGE VA=0x{:06X}000-0x{:06X}FFF PA=0x{:06X}000",
                                i, pte, i, i, ppn);
                        } else {
                            eprintln!("  L1[{:3}] = 0x{:08X} -> L2 at PA=0x{:08X}", i, pte, (ppn as u64) << 12);
                        }
                    }
                    Ok(_) => {},
                    Err(e) => eprintln!("  L1[{:3}] READ ERROR: {:?}", i, e),
                }
            }
            break;
        }
        
        let step_result = vm.step();
        if matches!(step_result, geometry_os::riscv::cpu::StepResult::Ebreak) { break; }
        count += 1;
    }
}
