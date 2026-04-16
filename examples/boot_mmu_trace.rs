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
    let max_instructions: u64 = 200_000u64;
    
    let mut forward_count: u64 = 0;
    let mut mmu_enabled = false;
    let mut mmu_enable_count: u64 = 0;
    let mut low_pc_after_mmu = false;
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
                    forward_count += 1;
                    
                    if forward_count <= 3 {
                        eprintln!("[diag] Forward #{} at count={}: cause={} sepc=0x{:08X} mpp={}",
                            forward_count, count, cause_code, vm.cpu.csr.sepc, mpp);
                    }
                    
                    count += 1;
                    continue;
                }
            }
            
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        
        // Check for MMU enable (satp changes from 0)
        let cur_satap = vm.cpu.csr.satp;
        if !mmu_enabled && prev_satap == 0 && cur_satap != 0 {
            mmu_enabled = true;
            mmu_enable_count = count;
            eprintln!("[diag] MMU ENABLED at count={}: satp=0x{:08X} PC=0x{:08X} priv={:?}", 
                count, cur_satap, vm.cpu.pc, vm.cpu.privilege);
        }
        prev_satap = cur_satap;
        
        // After MMU is enabled, check for low PCs
        if mmu_enabled && !low_pc_after_mmu {
            if vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor && vm.cpu.pc < 0x10000 {
                low_pc_after_mmu = true;
                eprintln!("[diag] LOW PC AFTER MMU at count={}: PC=0x{:08X} ({} instructions after MMU enable)",
                    count, vm.cpu.pc, count - mmu_enable_count);
            }
        }
        
        let step_result = vm.step();
        if matches!(step_result, geometry_os::riscv::cpu::StepResult::Ebreak) {
            break;
        }
        
        count += 1;
    }
    
    eprintln!("[diag] Summary: count={} PC=0x{:08X} priv={:?} satp=0x{:08X} forwards={}", 
        count, vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp, forward_count);
    if mmu_enabled {
        eprintln!("[diag] MMU enabled at count={}, {} instructions since", mmu_enable_count, count - mmu_enable_count);
    }
}
