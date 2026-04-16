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
    let mut done = false;
    
    while count < max_instructions && !done {
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
        
        if !done && vm.cpu.pc == 0x10EE && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor {
            done = true;
            eprintln!("=== PAGE TABLE DUMP AFTER setup_vm (count={}) ===", count);
            
            // Dump early_pg_dir at 0x802000
            eprintln!("early_pg_dir at 0x802000:");
            for i in 0..512 {
                let addr = 0x802000u64 + (i as u64) * 4;
                match vm.bus.read_word(addr) {
                    Ok(v) if v != 0 => {
                        let r = (v >> 1) & 1; let w = (v >> 2) & 1; let x = (v >> 3) & 1;
                        let ppn = v >> 10;
                        let leaf = (r|w|x) != 0;
                        if leaf {
                            eprintln!("  L1[{:3}] = 0x{:08X} MEGAPAGE PPN=0x{:06X} R={}W={}X={}", i, v, ppn, r, w, x);
                        } else {
                            eprintln!("  L1[{:3}] = 0x{:08X} -> L2@0x{:08X}", i, v, (ppn as u64)<<12);
                        }
                    }
                    _ => {}
                }
            }
            
            // Dump trampoline_pg_dir at 0x1484000
            eprintln!("trampoline_pg_dir at 0x1484000:");
            for i in 0..16 {
                let addr = 0x1484000u64 + (i as u64) * 4;
                match vm.bus.read_word(addr) {
                    Ok(v) if v != 0 => {
                        let r = (v >> 1) & 1; let w = (v >> 2) & 1; let x = (v >> 3) & 1;
                        let ppn = v >> 10;
                        eprintln!("  L1[{:3}] = 0x{:08X} PPN=0x{:06X} R={}W={}X={}", i, v, ppn, r, w, x);
                    }
                    _ => {}
                }
            }
        }
        
        let step_result = vm.step();
        if matches!(step_result, geometry_os::riscv::cpu::StepResult::Ebreak) { break; }
        count += 1;
    }
}
