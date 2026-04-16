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
    
    // Watch for the first SATP write (MMU enable)
    let mut first_satap_write = true;
    let mut trampoline_dumped = false;
    let mut final_dumped = false;
    let mut prev_satap: u32 = 0;
    
    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        
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
        
        // Watch for SATP changes
        let cur_satap = vm.cpu.csr.satp;
        if cur_satap != prev_satap {
            eprintln!("[diag] SATAP changed at count={}: 0x{:08X} -> 0x{:08X} PC=0x{:08X}",
                count, prev_satap, cur_satap, vm.cpu.pc);
            
            if first_satap_write {
                first_satap_write = false;
                // Dump the trampoline page table
                let pt_root = ((cur_satap & 0x3FFFFF) as u64) << 12;
                eprintln!("[diag] Trampoline PT at 0x{:08X}:", pt_root);
                dump_pt(&mut vm, pt_root, "trampoline");
                trampoline_dumped = true;
            } else if !final_dumped {
                final_dumped = true;
                let pt_root = ((cur_satap & 0x3FFFFF) as u64) << 12;
                eprintln!("[diag] Final PT at 0x{:08X}:", pt_root);
                dump_pt(&mut vm, pt_root, "final");
            }
            
            prev_satap = cur_satap;
        }
        
        let step_result = vm.step();
        if matches!(step_result, geometry_os::riscv::cpu::StepResult::Ebreak) {
            break;
        }
        
        count += 1;
    }
}

fn dump_pt(vm: &mut geometry_os::riscv::RiscvVm, pt_root: u64, label: &str) {
    for i in 0..1024 {
        let addr = pt_root + (i as u64) * 4;
        if let Ok(pte) = vm.bus.read_word(addr) {
            if pte != 0 {
                let v = pte & 1;
                let r = (pte >> 1) & 1;
                let w = (pte >> 2) & 1;
                let x = (pte >> 3) & 1;
                let ppn = pte >> 10;
                let is_leaf = (r | w | x) != 0;
                if is_leaf {
                    let pa = (ppn as u64) << 12;
                    let va_start = (i as u64) << 22;
                    let va_end = va_start + (1 << 22);
                    eprintln!("  {} L1[{:3}] MEGAPAGE VA=0x{:08X}-0x{:08X} PA=0x{:08X} R={}W={}X={}",
                        label, i, va_start, va_end, pa, r, w, x);
                } else {
                    eprintln!("  {} L1[{:3}] -> L2 at PA=0x{:08X}", label, i, (ppn as u64) << 12);
                    // Dump L2 entries
                    let l2_base = (ppn as u64) << 12;
                    for j in 0..1024 {
                        if let Ok(l2pte) = vm.bus.read_word(l2_base + (j as u64) * 4) {
                            if l2pte != 0 {
                                let l2v = l2pte & 1;
                                let l2r = (l2pte >> 1) & 1;
                                let l2w = (l2pte >> 2) & 1;
                                let l2x = (l2pte >> 3) & 1;
                                let l2ppn = l2pte >> 10;
                                let l2pa = (l2ppn as u64) << 12;
                                let va = ((i as u64) << 22) | ((j as u64) << 12);
                                eprintln!("    L2[{:3}] VA=0x{:08X} PA=0x{:08X} R={}W={}X={}",
                                    j, va, l2pa, l2r, l2w, l2x);
                            }
                        }
                    }
                }
            }
        }
    }
}
