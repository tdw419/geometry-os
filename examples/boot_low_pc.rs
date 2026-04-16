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
    let max_instructions: u64 = 180_000u64;
    
    let mut forward_count: u64 = 0;
    let mut last_smode_pc: u32 = 0;
    let mut trap_count: u64 = 0;
    
    // We want to know: what instruction does the kernel execute RIGHT BEFORE
    // it ends up at address 0x4? The kernel should never be at 0x4.
    // Let's track: when does the PC first become a low address (< 0x1000)?
    
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
                    
                    trap_count += 1;
                    if trap_count <= 3 {
                        eprintln!("[diag] Trap #{} at count={}: cause={} sepc=0x{:08X} mpp={} stvec=0x{:08X}",
                            trap_count, count, cause_code, vm.cpu.csr.sepc, mpp, stvec);
                        eprintln!("[diag]   Last S-mode PC before trap: 0x{:08X}", last_smode_pc);
                    }
                    
                    count += 1;
                    continue;
                }
            }
            
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        
        // Track last S-mode PC
        if vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor {
            // Check if PC is suspiciously low (kernel should be at 0xC0xxxxxx)
            let old_satp = vm.cpu.csr.satp; if old_satp == 0 && vm.cpu.pc < 0x10000 && vm.cpu.pc != 0 && count > 100 {
                eprintln!("[diag] LOW PC at count={}: PC=0x{:08X} (expected > 0xC0000000)", count, vm.cpu.pc);
                // Disassemble the last few instructions
                for i in 1..=5 {
                    let addr = last_smode_pc.wrapping_sub((i as u32) * 4);
                    let instr = vm.bus.read_word(addr as u64).unwrap_or(0);
                    eprintln!("  [{}] 0x{:08X}: 0x{:08X}", i, addr, instr);
                }
                eprintln!("  Current: 0x{:08X}: 0x{:08X}", vm.cpu.pc, vm.bus.read_word(vm.cpu.pc as u64).unwrap_or(0));
                // Only report first occurrence
                break;
            }
            last_smode_pc = vm.cpu.pc;
        }
        
        let step_result = vm.step();
        if matches!(step_result, geometry_os::riscv::cpu::StepResult::Ebreak) {
            break;
        }
        
        count += 1;
    }
    
    eprintln!("[diag] Summary: count={} PC=0x{:08X} priv={:?} forwards={}", 
        count, vm.cpu.pc, vm.cpu.privilege, forward_count);
}
