use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    
    let (mut vm, fw_addr, _entry, _dtb) = 
        geometry_os::riscv::RiscvVm::boot_linux_setup(
            &kernel_image, initramfs.as_deref(), 512, bootargs,
        ).expect("boot setup failed");
    
    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instructions: u64 = 300_000;
    
    let mut trampoline_patched = false;
    let mut last_satp: u32 = 0;
    let mut satp_change_count: u64 = 0;
    let mut pc_log: Vec<(u64, u32)> = Vec::new(); // (count, PC)
    let mut tracing = false;
    
    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        
        // Inject identity mappings at PC=0x10EE (after setup_vm)
        if !trampoline_patched
            && vm.cpu.pc == 0x10EE
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor
            && vm.cpu.csr.satp == 0
        {
            let identity_pte: u32 = 0x0000_00EF;
            let l1_entries: &[u64] = &[0, 2, 4, 5, 6, 8, 10];
            let trampoline_phys = 0x0148_4000u64;
            let early_pg_dir_phys = 0x0080_2000u64;
            
            for &l1_idx in l1_entries {
                let pte = identity_pte | ((l1_idx as u32) << 20);
                let addr_offset = (l1_idx * 4) as u64;
                vm.bus.write_word(trampoline_phys + addr_offset, pte).ok();
                vm.bus.write_word(early_pg_dir_phys + addr_offset, pte).ok();
            }
            trampoline_patched = true;
            eprintln!("[diag] Injected identity mappings at count={}", count);
        }
        
        // Track SATP changes
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp && cur_satp != 0 {
            satp_change_count += 1;
            eprintln!("[diag] SATP #{} changed to 0x{:08X} at count={}, PC=0x{:08X}",
                satp_change_count, cur_satp, count, vm.cpu.pc);
            
            // After 2nd SATP change, start tracing
            if satp_change_count >= 2 && !tracing {
                tracing = true;
                eprintln!("[diag] Starting PC trace after 2nd SATP change...");
            }
        }
        last_satp = cur_satp;
        
        // Forward traps (OpenSBI emulation)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            
            if cause_code != 11 { // Not ECALL_M
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
                
                if cause_code == 2 { // Illegal instruction
                    let mepc = vm.cpu.csr.mepc;
                    eprintln!("[diag] ILLEGAL at count={}: mepc=0x{:08X} stvec=0x{:08X} satp=0x{:08X} mpp={}",
                        count, mepc, vm.cpu.csr.stvec, vm.cpu.csr.satp, mpp);
                    
                    // Dump page table at current SATP
                    let satp_ppn = vm.cpu.csr.satp & 0x003F_FFFF;
                    let pt_base = (satp_ppn as u64) << 12;
                    eprintln!("[diag] Page table at PA 0x{:08X}:", pt_base);
                    // Dump L1 entries around identity range and 0xC0 range
                    for l1_idx in &[0, 1, 2, 768, 769, 770, 771, 772] {
                        let pte = vm.bus.read_word(pt_base + (*l1_idx as u64) * 4).unwrap_or(0);
                        eprintln!("[diag]   L1[{}] = 0x{:08X}", l1_idx, pte);
                    }
                    
                    // Forward to S-mode
                    if mpp != 3 {
                        let stvec = vm.cpu.csr.stvec & !0x3u32;
                        if stvec != 0 {
                            vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                            vm.cpu.csr.scause = mcause;
                            vm.cpu.csr.stval = vm.cpu.csr.mtval;
                            vm.cpu.pc = stvec;
                            vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                            vm.cpu.tlb.flush_all();
                            count += 1;
                            continue;
                        }
                    }
                } else if mpp != 3 && cause_code == 11 {
                    // ECALL_S = SBI call
                    let result = vm.bus.sbi.handle_ecall(
                        vm.cpu.x[17], vm.cpu.x[16],
                        vm.cpu.x[10], vm.cpu.x[11],
                        vm.cpu.x[12], vm.cpu.x[13],
                        vm.cpu.x[14], vm.cpu.x[15],
                        &mut vm.bus.uart, &mut vm.bus.clint,
                    );
                    if let Some((a0, a1)) = result {
                        vm.cpu.x[10] = a0;
                        vm.cpu.x[11] = a1;
                    }
                } else if mpp != 3 {
                    // Other exceptions from S/U-mode: forward to S-mode
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
                
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                count += 1;
                continue;
            }
            
            // ECALL_M: SBI call
            let result = vm.bus.sbi.handle_ecall(
                vm.cpu.x[17], vm.cpu.x[16],
                vm.cpu.x[10], vm.cpu.x[11],
                vm.cpu.x[12], vm.cpu.x[13],
                vm.cpu.x[14], vm.cpu.x[15],
                &mut vm.bus.uart, &mut vm.bus.clint,
            );
            if let Some((a0, a1)) = result {
                vm.cpu.x[10] = a0;
                vm.cpu.x[11] = a1;
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            count += 1;
            continue;
        }
        
        // Collect PC trace
        if tracing && pc_log.len() < 500 {
            pc_log.push((count, vm.cpu.pc));
        }
        
        vm.step();
        count += 1;
    }
    
    // Print PC trace
    eprintln!("\n[diag] PC trace (last 500 steps before max):");
    for (c, pc) in &pc_log {
        eprintln!("  count={}: PC=0x{:08X}", c, pc);
    }
    
    // Final state
    eprintln!("\n[diag] Final: PC=0x{:08X} priv={:?} satp=0x{:08X} instructions={}",
        vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp, count);
}
